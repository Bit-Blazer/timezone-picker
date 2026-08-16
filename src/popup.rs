use crate::{clipboard, parse};
use chrono::{Datelike, Local, TimeZone};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateFontW, CreateSolidBrush, DeleteObject, FW_NORMAL};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const POPUP_CLASS: PCWSTR = w!("TZPickerPopup");
const EDIT_ID: i32 = 101;
const STATIC_ID: i32 = 102;
const EM_SETSEL: u32 = 0x00B1;

struct PopupState {
    edit_hwnd: HWND,
    static_hwnd: HWND,
    current_result: String,
    font: windows::Win32::Graphics::Gdi::HFONT,
}

pub fn show(initial_text: &str, x: i32, y: i32) {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: POPUP_CLASS,
            hbrBackground: CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00221E1A)), // Darker background
            ..Default::default()
        };
        RegisterClassW(&wc);

        let width = 350;
        let height = 160;

        let mut state = Box::new(PopupState {
            edit_hwnd: HWND::default(),
            static_hwnd: HWND::default(),
            current_result: String::new(),
            font: CreateFontW(
                16,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                windows::Win32::Graphics::Gdi::DEFAULT_CHARSET,
                windows::Win32::Graphics::Gdi::OUT_DEFAULT_PRECIS,
                windows::Win32::Graphics::Gdi::CLIP_DEFAULT_PRECIS,
                windows::Win32::Graphics::Gdi::DEFAULT_QUALITY,
                windows::Win32::Graphics::Gdi::FF_DONTCARE.0 as u32,
                w!("Segoe UI"),
            ),
        });

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            POPUP_CLASS,
            w!("TZPicker Interactive"),
            WS_POPUP | WS_VISIBLE,
            x - width / 2,
            y + 16,
            width,
            height,
            None,
            None,
            Some(hinstance.into()),
            Some(state.as_mut() as *mut _ as *const _),
        )
        .unwrap();

        SetLayeredWindowAttributes(
            hwnd,
            windows::Win32::Foundation::COLORREF(0),
            245,
            LWA_ALPHA,
        )
        .ok();

        // Create EDIT control
        state.edit_hwnd = CreateWindowExW(
            Default::default(),
            w!("EDIT"),
            None,
            WS_CHILD
                | WS_VISIBLE
                | WS_BORDER
                | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            10,
            10,
            width - 20,
            24,
            Some(hwnd),
            Some(HMENU(EDIT_ID as *mut _)),
            Some(hinstance.into()),
            None,
        )
        .unwrap();

        SendMessageW(
            state.edit_hwnd,
            WM_SETFONT,
            Some(WPARAM(state.font.0 as usize)),
            Some(LPARAM(1)),
        );

        // Create STATIC control
        state.static_hwnd = CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            None,
            WS_CHILD | WS_VISIBLE,
            10,
            40,
            width - 20,
            110,
            Some(hwnd),
            Some(HMENU(STATIC_ID as *mut _)),
            Some(hinstance.into()),
            None,
        )
        .unwrap();

        SendMessageW(
            state.static_hwnd,
            WM_SETFONT,
            Some(WPARAM(state.font.0 as usize)),
            Some(LPARAM(1)),
        );

        // Subclass STATIC to make it dark themed? Default is ugly gray, but let's just intercept WM_CTLCOLORSTATIC in wnd_proc.

        // Pre-fill
        let wide: Vec<u16> = initial_text
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        SetWindowTextW(state.edit_hwnd, PCWSTR(wide.as_ptr())).ok();
        SendMessageW(
            state.edit_hwnd,
            EM_SETSEL,
            Some(WPARAM(wide.len() - 1)),
            Some(LPARAM(wide.len() as isize - 1)),
        ); // Cursor at end
        let _ = SetFocus(Some(state.edit_hwnd));
        let _ = SetForegroundWindow(hwnd);

        // Run an initial conversion
        update_conversion(hwnd, &mut state);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_KEYDOWN {
                if msg.wParam.0 == VK_RETURN.0 as usize {
                    // Enter pressed! Copy & Exit
                    if !state.current_result.is_empty() {
                        clipboard::set_text(&state.current_result);
                    }
                    DestroyWindow(hwnd).ok();
                    break;
                } else if msg.wParam.0 == VK_ESCAPE.0 as usize {
                    // Escape pressed! Exit
                    DestroyWindow(hwnd).ok();
                    break;
                }
            }

            // To make the edit control handle arrows properly when we intercept keys,
            // IsDialogMessage is highly recommended but simple Translate/Dispatch usually works for simple EDITs.
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            if !IsWindow(Some(hwnd)).as_bool() {
                break;
            }
        }

        let _ = DeleteObject(state.font.into());
        UnregisterClassW(POPUP_CLASS, Some(hinstance.into())).ok();
    }
}

fn update_conversion(_hwnd: HWND, state: &mut PopupState) {
    unsafe {
        let len = GetWindowTextLengthW(state.edit_hwnd);
        if len == 0 {
            set_static_text(state, "Type a time or instruction...");
            state.current_result.clear();
            return;
        }

        let mut buf = vec![0u16; (len + 1) as usize];
        GetWindowTextW(state.edit_hwnd, &mut buf);
        let text = String::from_utf16_lossy(&buf[..len as usize]);

        let this_year = Local::now().year();

        if let Some(parsed) = parse::extract_request(&text, this_year) {
            let targets = parsed
                .target_tzs
                .unwrap_or_else(|| crate::config::CONFIG.target_tzs.clone());

            if let Some(source_dt) = parsed
                .source_tz
                .from_local_datetime(&parsed.datetime)
                .earliest()
            {
                let mut results = Vec::new();
                for target in targets {
                    let converted = source_dt.with_timezone(&target);
                    results.push(format!(
                        "{}  ({})",
                        converted.format("%b %d, %Y  %I:%M %p"),
                        target
                    ));
                }
                let result = results.join("\r\n");
                set_static_text(state, &result);
                state.current_result = result;
            } else {
                set_static_text(state, "Ambiguous local time (DST fold)");
                state.current_result.clear();
            }
        } else {
            set_static_text(state, "Couldn't parse datetime");
            state.current_result.clear();
        }
    }
}

unsafe fn set_static_text(state: &PopupState, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        SetWindowTextW(state.static_hwnd, PCWSTR(wide.as_ptr())).ok();
    }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PopupState;

        match msg {
            WM_NCCREATE => {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                LRESULT(1)
            }
            WM_COMMAND => {
                // Check if it's our edit control and it's an EN_CHANGE notification
                let control_id = (wparam.0 & 0xFFFF) as i32;
                let notification = ((wparam.0 >> 16) & 0xFFFF) as u32;

                if control_id == EDIT_ID
                    && notification == EN_CHANGE
                    && let Some(state) = state_ptr.as_mut()
                {
                    update_conversion(hwnd, state);
                }
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC => {
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as _);
                windows::Win32::Graphics::Gdi::SetBkMode(
                    hdc,
                    windows::Win32::Graphics::Gdi::TRANSPARENT,
                );
                windows::Win32::Graphics::Gdi::SetTextColor(
                    hdc,
                    windows::Win32::Foundation::COLORREF(0x00E8F0F0),
                );
                // Return a dark brush
                let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00221E1A));
                LRESULT(brush.0 as isize)
            }
            WM_CTLCOLOREDIT => {
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as _);
                windows::Win32::Graphics::Gdi::SetBkMode(
                    hdc,
                    windows::Win32::Graphics::Gdi::OPAQUE,
                );
                windows::Win32::Graphics::Gdi::SetBkColor(
                    hdc,
                    windows::Win32::Foundation::COLORREF(0x00333333),
                );
                windows::Win32::Graphics::Gdi::SetTextColor(
                    hdc,
                    windows::Win32::Foundation::COLORREF(0x00FFFFFF),
                );
                let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00333333));
                LRESULT(brush.0 as isize)
            }
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
