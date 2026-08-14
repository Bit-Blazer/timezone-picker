// Small, borderless, always-on-top popup shown near the cursor with the
// converted time. Auto-dismisses after a couple seconds or on click.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT, DT_CENTER, DT_SINGLELINE, DT_VCENTER,
    FW_NORMAL, PAINTSTRUCT,
};
use windows::Win32::UI::WindowsAndMessaging::*;

const POPUP_CLASS: PCWSTR = w!("TZPickerPopup");
const TIMER_ID: usize = 1;
const AUTO_DISMISS_MS: u32 = 3500;

pub fn show(text: &str, x: i32, y: i32) {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();

        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: POPUP_CLASS,
            hbrBackground: CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00332B1E)), // dark background
            ..Default::default()
        };
        RegisterClassW(&wc);

        let width = 60 + (text.len() as i32) * 8;
        let height = 44;

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            POPUP_CLASS,
            PCWSTR(wide.as_ptr()),
            WS_POPUP | WS_VISIBLE,
            x - width / 2,
            y + 16,
            width,
            height,
            None,
            None,
            hinstance,
            None,
        )
        .unwrap();

        SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), 235, LWA_ALPHA).ok();
        SetTimer(hwnd, TIMER_ID, AUTO_DISMISS_MS, None);

        // Store the text pointer for WM_PAINT via window text (simplest route).
        SetWindowTextW(hwnd, PCWSTR(wide.as_ptr())).ok();

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if !IsWindow(hwnd).as_bool() {
                break;
            }
        }

        UnregisterClassW(POPUP_CLASS, hinstance).ok();
    }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                GetClientRect(hwnd, &mut rect).ok();

                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00E8F0F0));

                let font = CreateFontW(
                    18, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                    windows::Win32::Graphics::Gdi::DEFAULT_CHARSET,
                    windows::Win32::Graphics::Gdi::OUT_DEFAULT_PRECIS,
                    windows::Win32::Graphics::Gdi::CLIP_DEFAULT_PRECIS,
                    windows::Win32::Graphics::Gdi::DEFAULT_QUALITY,
                    windows::Win32::Graphics::Gdi::FF_DONTCARE.0 as u32,
                    w!("Segoe UI"),
                );
                let old_font = SelectObject(hdc, font);

                let len = GetWindowTextLengthW(hwnd);
                let mut buf = vec![0u16; (len + 1) as usize];
                GetWindowTextW(hwnd, &mut buf);

                DrawTextW(hdc, &mut buf[..len as usize], &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

                SelectObject(hdc, old_font);
                DeleteObject(font).ok();
                EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_TIMER | WM_LBUTTONDOWN => {
                DestroyWindow(hwnd).ok();
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
