// Prevents a console window from flashing on launch (GUI subsystem).
#![windows_subsystem = "windows"]

mod clipboard;
mod overlay;
mod parse;
mod popup;
mod tz;
mod uia;
// mod ocr; // TODO: bitmap capture + Windows.Media.Ocr fallback -- see README.

use chrono::{Datelike, Local};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::*;

const HOTKEY_ID: i32 = 1;
const WM_APP_TRIGGER: u32 = WM_APP + 1;

fn main() {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();

        let class_name = w!("TZPickerHidden");
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
            0, 0, 0, 0,
            None, None, hinstance, None,
        )
        .unwrap();

        // Ctrl+Alt+T. Change here if it collides with something on your
        // system, or make this configurable later.
        let vk_t = 0x54u32; // 'T'
        if RegisterHotKey(hwnd, HOTKEY_ID, MOD_CONTROL | MOD_ALT, vk_t).is_err() {
            eprintln!("Failed to register hotkey -- it may be in use by another app.");
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
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
    let extracted = uia::text_at_point(center_x, center_y);

    // --- Fallback path (TODO): if `extracted` is None, or if it's Some
    // but parse::extract_request() below fails to find a datetime in it,
    // fall back to: capture `rect` via BitBlt, upscale, run through
    // Windows.Media.Ocr.OcrEngine, and retry parsing on the OCR'd text.
    // Stubbed for now so the UIA-first path can be validated independently.

    let Some(text) = extracted else {
        popup::show("No text found here (OCR fallback not yet implemented)", center_x, center_y);
        return;
    };

    let this_year = Local::now().year();

    let Some(parsed) = parse::extract_request(&text, this_year) else {
        popup::show("Couldn't find a datetime in the selected text", center_x, center_y);
        return;
    };

    let target = parsed.target_tz.unwrap_or_else(tz::default_target_tz);

    let source_dt = parsed
        .source_tz
        .from_local_datetime(&parsed.datetime)
        .earliest();

    let Some(source_dt) = source_dt else {
        popup::show("Ambiguous local time (DST fold)", center_x, center_y);
        return;
    };

    let converted = source_dt.with_timezone(&target);
    let result = format!(
        "{}  ({})",
        converted.format("%b %d, %Y  %I:%M %p"),
        target
    );

    clipboard::set_text(&result).ok();
    popup::show(&result, center_x, center_y);
}

// Bring TimeZone trait into scope for `from_local_datetime`.
use chrono::TimeZone;
