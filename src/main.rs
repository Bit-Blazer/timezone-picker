// Prevents a console window from flashing on launch (GUI subsystem).
#![windows_subsystem = "windows"]

mod autostart;
mod clipboard;
mod config;
mod ocr;
mod overlay;
mod parse;
mod popup;
mod tz;
mod uia;

use std::env;
use std::process::Command;
use tray_item::{IconSource, TrayItem};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, RegisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

const HOTKEY_ID: i32 = 1;

fn main() {
    autostart::register_if_needed();

    // Trigger config load
    let cfg = &config::CONFIG;

    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();

        let class_name = w!("TZPickerHidden");
        UnregisterClassW(w!("TZPickerHidden"), Some(hinstance.into())).ok();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("TZPicker"),
            Default::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .unwrap();

        // Parse hotkey from config
        let mut mods = HOT_KEY_MODIFIERS(0);
        let mut vk = 0u32;
        for part in cfg.hotkey.split('+') {
            let part = part.trim().to_uppercase();
            match part.as_str() {
                "CTRL" => mods |= MOD_CONTROL,
                "ALT" => mods |= MOD_ALT,
                "SHIFT" => mods |= MOD_SHIFT,
                "WIN" => mods |= MOD_WIN,
                _ => {
                    if part.len() == 1 {
                        vk = part.chars().next().unwrap() as u32;
                    }
                }
            }
        }

        if vk == 0 {
            // fallback if badly configured
            vk = 0x54; // 'T'
            mods = MOD_CONTROL | MOD_ALT;
        }

        if RegisterHotKey(Some(hwnd), HOTKEY_ID, mods, vk).is_err() {
            eprintln!("Failed to register hotkey -- it may be in use by another app.");
        }

        // Initialize System Tray
        let mut tray_opt = TrayItem::new("Timezone Picker", IconSource::Resource("app-icon"));

        if let Ok(ref mut tray) = tray_opt {
            tray.add_menu_item("Settings", || {
                if let Ok(appdata) = env::var("APPDATA") {
                    let mut path = std::path::PathBuf::from(appdata);
                    path.push("timezone-picker");
                    path.push("config.toml");
                    let _ = Command::new("notepad.exe").arg(path).spawn();
                }
            })
            .unwrap_or_else(|e| eprintln!("Tray settings error: {}", e));

            tray.add_menu_item("Quit", || {
                std::process::exit(0);
            })
            .unwrap_or_else(|e| eprintln!("Tray quit error: {}", e));
        } else {
            eprintln!("Failed to initialize tray item. Missing icon resource?");
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_HOTKEY if wparam.0 as i32 == HOTKEY_ID => {
                run_pipeline();
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn run_pipeline() {
    let Some(rect) = overlay::run_selection() else {
        return; // user cancelled
    };

    let center_x = (rect.left + rect.right) / 2;
    let center_y = (rect.top + rect.bottom) / 2;

    // --- Primary path: UI Automation ---
    let mut extracted = uia::text_at_point(center_x, center_y).unwrap_or_default();

    if extracted.trim().is_empty()
        && let Some(ocr_text) = ocr::extract_text(rect)
    {
        extracted = ocr_text;
    }

    // Delegate parsing and display to the interactive popup
    popup::show(&extracted, center_x, center_y);
}
