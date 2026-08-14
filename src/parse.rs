use crate::tz::resolve_abbreviation;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use chrono_tz::Tz;
use once_cell::sync::Lazy;
use regex::Regex;

pub struct ParsedRequest {
    pub datetime: NaiveDateTime,
    pub source_tz: Tz,
    /// Explicit target tz if the text contained "X to Y"; otherwise None
    /// and the caller should fall back to the configured default.
    pub target_tz: Option<Tz>,
}

// Matches things like "PST to IST", "pst->ist", "PST -> IST"
static INSTRUCTION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b([A-Z]{2,5})\s*(?:to|->|→)\s*([A-Z]{2,5})\b").unwrap()
});

// Candidate date+time substrings. Kept intentionally permissive; we lean on
// chrono's format parser (tried with several patterns) to do the real work,
// and just use this regex to find *where* to start looking, plus to grab
// a trailing timezone abbreviation if present.
static DATETIME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?ix)
        (?P<month>Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*\.?\s+
        (?P<day>\d{1,2}),?\s+
        (?P<year>\d{4})?,?\s*
        (?P<hour>\d{1,2}):(?P<minute>\d{2})\s*
        (?P<ampm>AM|PM)?\s*
        (?P<tzabbr>[A-Z]{2,5})?
        ",
    )
    .unwrap()
});

/// Try to find a datetime + source timezone + (optional) explicit target
/// timezone somewhere within an arbitrary blob of text (e.g. a whole
/// sentence or UI element string, not just the exact selected substring).
pub fn extract_request(text: &str, assumed_year: i32) -> Option<ParsedRequest> {
    let explicit_target = INSTRUCTION_RE.captures(text).and_then(|c| {
        let to = resolve_abbreviation(&c[2])?;
        Some(to)
    });

    let caps = DATETIME_RE.captures(text)?;

    let month = month_from_name(&caps["month"])?;
    let day: u32 = caps["day"].parse().ok()?;
    let year: i32 = caps
        .name("year")
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(assumed_year);

    let mut hour: u32 = caps["hour"].parse().ok()?;
    let minute: u32 = caps["minute"].parse().ok()?;

    if let Some(ampm) = caps.name("ampm") {
        let is_pm = ampm.as_str().eq_ignore_ascii_case("pm");
        hour = to_24h(hour, is_pm);
    }

    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
    let datetime = NaiveDateTime::new(date, time);

    // Source tz: prefer an abbreviation trailing the datetime itself; else
    // fall back to the "from" side of an "X to Y" instruction if present.
    let source_tz = caps
        .name("tzabbr")
        .and_then(|m| resolve_abbreviation(m.as_str()))
        .or_else(|| {
            INSTRUCTION_RE
                .captures(text)
                .and_then(|c| resolve_abbreviation(&c[1]))
        })?;

    Some(ParsedRequest {
        datetime,
        source_tz,
        target_tz: explicit_target,
    })
}

fn to_24h(hour: u32, is_pm: bool) -> u32 {
    match (hour, is_pm) {
        (12, false) => 0,  // 12 AM -> 0
        (12, true) => 12,  // 12 PM -> 12
        (h, true) => h + 12,
        (h, false) => h,
    }
}

fn month_from_name(s: &str) -> Option<u32> {
    let s = s.to_lowercase();
    Some(match &s[..3.min(s.len())] {
        "jan" => 1, "feb" => 2, "mar" => 3, "apr" => 4,
        "may" => 5, "jun" => 6, "jul" => 7, "aug" => 8,
        "sep" => 9, "oct" => 10, "nov" => 11, "dec" => 12,
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
        assert_eq!(r.target_tz, Some(chrono_tz::Asia::Kolkata));
    }

    #[test]
    fn no_year_defaults_to_assumed() {
        let r = extract_request("Aug 15, 3:30 PM PST", 2026).unwrap();
        assert_eq!(r.datetime.date().to_string(), "2026-08-15");
    }
}
