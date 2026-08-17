use std::env;
use winreg::RegKey;
use winreg::enums::*;

const APP_NAME: &str = "TimezonePicker";

pub fn register_if_needed() {
    if let Ok(exe_path) = env::current_exe()
        && let Some(exe_str) = exe_path.to_str()
    {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_READ | KEY_WRITE,
        ) {
            // Check if it already exists and points to the correct path
            let current_val: Result<String, _> = run_key.get_value(APP_NAME);
            let needs_update = match current_val {
                Ok(val) => val != exe_str,
                Err(_) => true,
            };

            if needs_update {
                let _ = run_key.set_value(APP_NAME, &exe_str);
            }
        }
    }
}

pub fn unregister() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run_key) = hkcu.open_subkey_with_flags(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
        KEY_READ | KEY_WRITE,
    ) {
        let _ = run_key.delete_value(APP_NAME);
    }
}
