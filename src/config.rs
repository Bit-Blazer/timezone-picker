use chrono_tz::Tz;
use once_cell::sync::Lazy;
use std::env;
use std::fs;
use std::path::PathBuf;

pub static CONFIG: Lazy<Config> = Lazy::new(Config::load);

pub struct Config {
    pub target_tzs: Vec<Tz>,
    pub hotkey: String,
}

impl Config {
    pub fn load() -> Self {
        // Defaults: IST, UTC, PST, EST
        let mut target_tzs = vec![
            chrono_tz::Asia::Kolkata,
            chrono_tz::UTC,
            chrono_tz::America::Los_Angeles,
            chrono_tz::America::New_York,
        ];

        let mut hotkey = "Ctrl+Alt+Z".to_string();

        if let Ok(appdata) = env::var("APPDATA") {
            let mut path = PathBuf::from(appdata);
            path.push("timezone-picker");
            path.push("config.toml");

            if let Ok(content) = fs::read_to_string(&path) {
                let parsed = Self::parse_config(&content, target_tzs.clone());
                target_tzs = parsed.target_tzs;
                hotkey = parsed.hotkey;
            } else {
                // Try to create default config file
                fs::create_dir_all(path.parent().unwrap()).ok();
                let default_content = format!(
                    "# Timezone Picker Configuration\n\n# Target timezones for conversions (IANA format, comma separated)\n# Examples: America/New_York, Europe/London, Asia/Kolkata\ntarget_tzs = \"{}\"\n\n# Global shortcut to trigger the app\nhotkey = \"Ctrl+Alt+Z\"\n",
                    target_tzs
                        .iter()
                        .map(|t| t.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                fs::write(&path, default_content).ok();
            }
        }

        Self { target_tzs, hotkey }
    }

    pub fn parse_config(content: &str, mut target_tzs: Vec<Tz>) -> Self {
        let mut hotkey = "Ctrl+Alt+Z".to_string();
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("target_tzs") || line.starts_with("target_tz") {
                if let Some(val) = extract_value(line) {
                    let mut tzs = Vec::new();
                    for part in val.split(',') {
                        if let Ok(tz) = part.trim().parse::<Tz>() {
                            tzs.push(tz);
                        }
                    }
                    if !tzs.is_empty() {
                        target_tzs = tzs;
                    }
                }
            } else if line.starts_with("hotkey")
                && let Some(val) = extract_value(line)
            {
                hotkey = val;
            }
        }
        Self { target_tzs, hotkey }
    }
}

fn extract_value(line: &str) -> Option<String> {
    // looking for `key = "value"`
    let parts: Vec<&str> = line.splitn(2, '=').collect();
    if parts.len() == 2 {
        let val = parts[1].trim();
        if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
            return Some(val[1..val.len() - 1].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::UTC;

    #[test]
    fn test_parse_valid_config() {
        let content = "target_tzs = \"America/Los_Angeles, Asia/Kolkata\"\nhotkey = \"Ctrl+Alt+X\"";
        let config = Config::parse_config(content, vec![UTC]);
        assert_eq!(
            config.target_tzs,
            vec![chrono_tz::America::Los_Angeles, chrono_tz::Asia::Kolkata]
        );
        assert_eq!(config.hotkey, "Ctrl+Alt+X");
    }

    #[test]
    fn test_parse_missing_fields_uses_defaults() {
        let content = "target_tz = \"Europe/London\"";
        let config = Config::parse_config(content, vec![UTC]);
        assert_eq!(config.target_tzs, vec![chrono_tz::Europe::London]);
        assert_eq!(config.hotkey, "Ctrl+Alt+Z"); // Default
    }

    #[test]
    fn test_parse_corrupted_data_reverts_to_defaults() {
        let content = "target_tz = Invalid_No_Quotes\nhotkey = also_missing_quotes";
        let config = Config::parse_config(content, vec![UTC]);
        assert_eq!(config.target_tzs, vec![UTC]); // Default fallthrough
        assert_eq!(config.hotkey, "Ctrl+Alt+Z"); // Because extract_value returns None when quotes are missing
    }
}
