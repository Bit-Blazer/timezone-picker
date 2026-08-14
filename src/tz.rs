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

/// The user's default "convert to" timezone when the OCR/UIA text doesn't
/// explicitly contain a "X to Y" instruction.
///
/// TODO: load this from a small config file (e.g. %APPDATA%\timezone-picker\config.toml)
/// instead of hardcoding. Left as a constant for the first working version.
pub fn default_target_tz() -> Tz {
    chrono_tz::Asia::Kolkata
}
