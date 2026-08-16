use crate::tz::resolve_abbreviation;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use chrono_tz::Tz;
use once_cell::sync::Lazy;
use regex::Regex;
use windows::Win32::Globalization::GetUserDefaultLocaleName;

pub struct ParsedRequest {
    pub datetime: NaiveDateTime,
    pub source_tz: Tz,
    /// Explicit target tzs if the text contained "X to Y"; otherwise None
    /// and the caller should fall back to the configured default.
    pub target_tzs: Option<Vec<Tz>>,
}

// Matches things like "PST to IST", "pst->ist", "PST -> IST"
static INSTRUCTION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b([A-Z]{2,5})\s*(?:to|->|→)\s*([A-Z]{2,5})\b").unwrap());

// Candidate date+time substrings. Kept intentionally permissive; we lean on
// chrono's format parser (tried with several patterns) to do the real work,
// and just use this regex to find *where* to start looking, plus to grab
// a trailing timezone abbreviation if present.
// e.g. "Aug 15, 2026 3:30 PM PST" or "15th of August 15:30"
static TEXT_DATETIME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?ix)
        (?:(?P<day_first>\d{1,2})(?:st|nd|rd|th)?\s+(?:of\s+)?)?
        (?P<month>Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*\.?\s+
        (?:(?P<day_last>\d{1,2})(?:st|nd|rd|th)?,?)?\s*
        (?P<year>\d{4})?,?\s*
        (?:at\s+)?
        (?P<hour>\d{1,2}):(?P<minute>\d{2})(?::(?P<second>\d{2}))?\s*
        (?P<ampm>AM|PM)?\s*
        (?P<tzabbr>[A-Z]{2,5})?
        ",
    )
    .unwrap()
});

// e.g. "08/15/2026 3:30 PM PST" or "15-08-2026 15:30"
static NUMERIC_DATETIME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?ix)
        (?P<p1>\d{1,2})[-/](?P<p2>\d{1,2})[-/](?P<year>\d{4}|\d{2})\s*
        (?:at\s+)?
        (?P<hour>\d{1,2}):(?P<minute>\d{2})(?::(?P<second>\d{2}))?\s*
        (?P<ampm>AM|PM)?\s*
        (?P<tzabbr>[A-Z]{2,5})?
        ",
    )
    .unwrap()
});

// e.g. "2026-08-15T15:30:00Z"
static ISO_DATETIME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?ix)
        (?P<year>\d{4})-(?P<month>\d{2})-(?P<day>\d{2})[T\s]
        (?P<hour>\d{2}):(?P<minute>\d{2})(?::(?P<second>\d{2}))?
        (?P<tzabbr>Z|[+-]\d{2}:\d{2})?
        ",
    )
    .unwrap()
});

pub fn is_us_locale() -> bool {
    unsafe {
        let mut name = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH
        let len = GetUserDefaultLocaleName(&mut name);
        if len > 0 {
            let s = String::from_utf16_lossy(&name[..len as usize - 1]);
            return s.contains("US");
        }
        true // Default to US
    }
}

/// Try to find a datetime + source timezone + (optional) explicit target
/// timezone somewhere within an arbitrary blob of text (e.g. a whole
/// sentence or UI element string, not just the exact selected substring).
pub fn extract_request(text: &str, assumed_year: i32) -> Option<ParsedRequest> {
    let explicit_target = INSTRUCTION_RE.captures(text).and_then(|c| {
        let to = resolve_abbreviation(&c[2])?;
        Some(to)
    });

    let parsed_dt = parse_iso(text)
        .or_else(|| parse_text(text, assumed_year))
        .or_else(|| parse_numeric(text, assumed_year))
        .or_else(|| parse_natural(text));

    let (datetime, source_tz) = match parsed_dt {
        Some((dt, tzabbr)) => {
            let src = tzabbr.and_then(|m| resolve_abbreviation(&m)).or_else(|| {
                INSTRUCTION_RE
                    .captures(text)
                    .and_then(|c| resolve_abbreviation(&c[1]))
            })?;
            (dt, src)
        }
        None => {
            if explicit_target.is_some() {
                // No time provided, but an instruction was found (e.g., "PST to IST").
                // Default to the current local time.
                let caps = INSTRUCTION_RE.captures(text).unwrap();
                let src = resolve_abbreviation(&caps[1])?;
                (chrono::Local::now().naive_local(), src)
            } else {
                return None;
            }
        }
    };

    let target_tzs = explicit_target
        .map(|t| vec![t])
        .or_else(|| Some(crate::config::CONFIG.target_tzs.clone()));

    Some(ParsedRequest {
        datetime,
        source_tz,
        target_tzs,
    })
}

fn parse_natural(text: &str) -> Option<(NaiveDateTime, Option<String>)> {
    let mut clean_text = text.trim();
    let mut tzabbr = None;

    // Extract trailing timezone abbreviation to help chrono-english
    let re = regex::Regex::new(r"(?i)\s+([A-Z]{2,5})$").unwrap();
    if let Some(caps) = re.captures(clean_text)
        && let Some(m) = caps.get(1)
    {
        tzabbr = Some(m.as_str().to_string());
        clean_text = clean_text[..caps.get(0).unwrap().start()].trim();
    }

    let dialect = if is_us_locale() {
        chrono_english::Dialect::Us
    } else {
        chrono_english::Dialect::Uk
    };
    let now = chrono::Local::now();
    let safe_text = clean_text.replace(" at ", " ");

    // chrono_english fails if we pass it empty strings or junk, so we just absorb the error.
    if let Ok(dt) = chrono_english::parse_date_string(&safe_text, now, dialect) {
        return Some((dt.naive_local(), tzabbr));
    }
    None
}

fn parse_iso(text: &str) -> Option<(NaiveDateTime, Option<String>)> {
    let caps = ISO_DATETIME_RE.captures(text)?;
    let year: i32 = caps["year"].parse().ok()?;
    let month: u32 = caps["month"].parse().ok()?;
    let day: u32 = caps["day"].parse().ok()?;
    let hour: u32 = caps["hour"].parse().ok()?;
    let minute: u32 = caps["minute"].parse().ok()?;
    let second: u32 = caps
        .name("second")
        .map_or(0, |m| m.as_str().parse().unwrap_or(0));

    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(hour, minute, second)?;

    let tzabbr = caps.name("tzabbr").map(|m| {
        if m.as_str().eq_ignore_ascii_case("z") {
            "UTC".to_string()
        } else {
            m.as_str().to_string()
        }
    });

    Some((NaiveDateTime::new(date, time), tzabbr))
}

fn parse_text(text: &str, assumed_year: i32) -> Option<(NaiveDateTime, Option<String>)> {
    let caps = TEXT_DATETIME_RE.captures(text)?;
    let month = month_from_name(&caps["month"])?;

    let day_str = caps.name("day_first").or(caps.name("day_last"))?;
    let day: u32 = day_str.as_str().parse().ok()?;

    let year: i32 = caps
        .name("year")
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(assumed_year);

    let mut hour: u32 = caps["hour"].parse().ok()?;
    let minute: u32 = caps["minute"].parse().ok()?;
    let second: u32 = caps
        .name("second")
        .map_or(0, |m| m.as_str().parse().unwrap_or(0));

    if let Some(ampm) = caps.name("ampm") {
        let is_pm = ampm.as_str().eq_ignore_ascii_case("pm");
        hour = to_24h(hour, is_pm);
    }

    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(hour, minute, second)?;

    let tzabbr = caps.name("tzabbr").map(|m| m.as_str().to_string());

    Some((NaiveDateTime::new(date, time), tzabbr))
}

fn parse_numeric(text: &str, _assumed_year: i32) -> Option<(NaiveDateTime, Option<String>)> {
    let caps = NUMERIC_DATETIME_RE.captures(text)?;
    let p1: u32 = caps["p1"].parse().ok()?;
    let p2: u32 = caps["p2"].parse().ok()?;

    let mut year: i32 = caps["year"].parse().ok()?;
    if year < 100 {
        year += 2000;
    }

    let mut month = p1;
    let mut day = p2;

    // Disambiguate US vs EU
    if p1 > 12 {
        // Definitely EU (DD/MM/YYYY)
        day = p1;
        month = p2;
    } else if p2 > 12 {
        // Definitely US (MM/DD/YYYY)
        month = p1;
        day = p2;
    } else {
        // Ambiguous. Check locale.
        if !is_us_locale() {
            day = p1;
            month = p2;
        }
    }

    let mut hour: u32 = caps["hour"].parse().ok()?;
    let minute: u32 = caps["minute"].parse().ok()?;
    let second: u32 = caps
        .name("second")
        .map_or(0, |m| m.as_str().parse().unwrap_or(0));

    if let Some(ampm) = caps.name("ampm") {
        let is_pm = ampm.as_str().eq_ignore_ascii_case("pm");
        hour = to_24h(hour, is_pm);
    }

    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(hour, minute, second)?;

    let tzabbr = caps.name("tzabbr").map(|m| m.as_str().to_string());

    Some((NaiveDateTime::new(date, time), tzabbr))
}

fn to_24h(hour: u32, is_pm: bool) -> u32 {
    match (hour, is_pm) {
        (12, false) => 0, // 12 AM -> 0
        (12, true) => 12, // 12 PM -> 12
        (h, true) => h + 12,
        (h, false) => h,
    }
}

fn month_from_name(s: &str) -> Option<u32> {
    let s = s.to_lowercase();
    Some(match &s[..3.min(s.len())] {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_example() {
        let r = extract_request("Aug 15, 2026 3:30 PM PST", 2026).unwrap();
        assert_eq!(r.datetime.to_string(), "2026-08-15 15:30:00");
        assert_eq!(r.source_tz, chrono_tz::America::Los_Angeles);
    }

    #[test]
    fn picks_up_explicit_instruction() {
        let r = extract_request("Aug 15, 2026 3:30 PM PST to IST", 2026).unwrap();
        assert_eq!(r.target_tzs, Some(vec![chrono_tz::Asia::Kolkata]));
    }

    #[test]
    fn no_year_defaults_to_assumed() {
        let r = extract_request("Aug 15, 3:30 PM PST", 2026).unwrap();
        assert_eq!(r.datetime.date().to_string(), "2026-08-15");
    }

    #[test]
    fn parses_eu_text_format() {
        let r = extract_request("15th of August 2026 at 15:30 CET", 2026).unwrap();
        assert_eq!(r.datetime.to_string(), "2026-08-15 15:30:00");
        assert_eq!(r.source_tz, chrono_tz::Europe::Paris); // Assuming CET maps to Paris
    }

    #[test]
    fn parses_numeric_format_unambiguous() {
        // Definitely US since 15 > 12
        let r = extract_request("08/15/2026 3:30 PM PST", 2026).unwrap();
        assert_eq!(r.datetime.to_string(), "2026-08-15 15:30:00");

        // Definitely EU since 15 > 12
        let r2 = extract_request("15/08/2026 15:30 CET", 2026).unwrap();
        assert_eq!(r2.datetime.to_string(), "2026-08-15 15:30:00");
    }

    #[test]
    fn parses_iso_8601() {
        let r = extract_request("2026-08-15T15:30:00Z", 2026).unwrap();
        assert_eq!(r.datetime.to_string(), "2026-08-15 15:30:00");
        assert_eq!(r.source_tz, chrono_tz::UTC);
    }

    #[test]
    fn parses_short_numeric_year() {
        let r = extract_request("08/15/26 15:30 EST", 2026).unwrap();
        assert_eq!(r.datetime.to_string(), "2026-08-15 15:30:00");
    }

    #[test]
    fn parses_just_instruction_as_current_time() {
        let r = extract_request("PST to IST", 2026).unwrap();
        assert_eq!(r.source_tz, chrono_tz::America::Los_Angeles);
        assert_eq!(r.target_tzs, Some(vec![chrono_tz::Asia::Kolkata]));
        // We can't strictly assert the datetime because it uses Local::now()
        // but we know it parsed successfully!
    }

    #[test]
    fn parses_natural_language() {
        let r = extract_request("tomorrow at 3pm PST", 2026).unwrap();
        assert_eq!(r.source_tz, chrono_tz::America::Los_Angeles);
        // We know hour will be 15
        assert_eq!(r.datetime.time().to_string(), "15:00:00");
    }
}
