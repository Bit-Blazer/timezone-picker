use chrono_tz::Tz;
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Common timezone abbreviations -> IANA identifiers.
///
/// This is inherently lossy/ambiguous (e.g. "CST" is US Central, China
/// Standard, or Cuba Standard depending on context; "IST" is India,
/// Israel, or Ireland). We resolve to the single most common meaning.
/// If you personally need a different resolution, edit this table --
/// that's the intended way to "configure" it for now.
pub static TZ_ABBREVIATIONS: Lazy<HashMap<&'static str, Tz>> = Lazy::new(|| {
    use chrono_tz::Tz::*;
    HashMap::from([
        // North America
        ("PST", America__Los_Angeles),
        ("PDT", America__Los_Angeles),
        ("MST", America__Denver),
        ("MDT", America__Denver),
        ("CST", America__Chicago),
        ("CDT", America__Chicago),
        ("EST", America__New_York),
        ("EDT", America__New_York),
        ("AKST", America__Anchorage),
        ("AKDT", America__Anchorage),
        ("HST", Pacific__Honolulu),
        // UTC / Europe
        ("UTC", UTC),
        ("GMT", UTC),
        ("BST", Europe__London),
        ("CET", Europe__Paris),
        ("CEST", Europe__Paris),
        ("EET", Europe__Helsinki),
        ("EEST", Europe__Helsinki),
        // Asia / Oceania
        ("IST", Asia__Kolkata), // most common meaning; India Standard Time
        ("JST", Asia__Tokyo),
        ("KST", Asia__Seoul),
        ("SGT", Asia__Singapore),
        ("HKT", Asia__Hong_Kong),
        ("PHT", Asia__Manila),
        ("ICT", Asia__Bangkok),
        ("AEST", Australia__Sydney),
        ("AEDT", Australia__Sydney),
        ("ACST", Australia__Adelaide),
        ("ACDT", Australia__Adelaide),
        ("AWST", Australia__Perth),
        ("NZST", Pacific__Auckland),
        ("NZDT", Pacific__Auckland),
    ])
});

pub fn resolve_abbreviation(abbr: &str) -> Option<Tz> {
    TZ_ABBREVIATIONS.get(abbr.to_uppercase().as_str()).copied()
}

/// Detects the machine's local IANA timezone (e.g. "Asia/Kolkata"). Used as
/// the assumed *source* timezone when selected text is a bare datetime with
/// no timezone info at all -- e.g. "2026-08-14 17:35:00" typed in Notepad,
/// a log line, or most app timestamps that are implicitly "local time."
/// Without this fallback, extract_request() has nothing to resolve
/// source_tz to and gives up on an otherwise perfectly parseable datetime.
pub fn local_tz() -> Option<Tz> {
    let name = iana_time_zone::get_timezone().ok()?;
    name.parse::<Tz>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_abbreviation() {
        assert_eq!(
            resolve_abbreviation("PST"),
            Some(chrono_tz::America::Los_Angeles)
        );
        assert_eq!(
            resolve_abbreviation("pst"),
            Some(chrono_tz::America::Los_Angeles)
        );
        assert_eq!(
            resolve_abbreviation("pSt"),
            Some(chrono_tz::America::Los_Angeles)
        );

        // Invalid should return None
        assert_eq!(resolve_abbreviation("INVALID"), None);
        assert_eq!(resolve_abbreviation(""), None);
    }
}
