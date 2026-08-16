// Fullscreen, click-through-disabled, semi-transparent overlay that lets
// the user drag a selection rectangle. Blocks (runs its own message loop)
// until the user releases the mouse button or presses Escape.

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, EndPaint, GetStockObject, HBRUSH, InvalidateRect, NULL_BRUSH,
    PAINTSTRUCT, PS_SOLID, Rectangle, SelectObject, SetBkMode, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

const OVERLAY_CLASS: windows::core::PCWSTR = w!("TZPickerOverlay");

struct OverlayState {
    dragging: bool,
    start: POINT,
    current: POINT,
    result: Option<RECT>,
    cancelled: bool,
}

/// Runs the overlay and blocks until the user finishes a drag (Some(rect))
/// or cancels with Escape/right-click (None).
pub fn run_selection() -> Option<RECT> {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: OVERLAY_CLASS,
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            ..Default::default()
        };
        RegisterClassW(&wc);

        // Virtual screen bounds (covers all monitors).
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        let mut state = Box::new(OverlayState {
            dragging: false,
            start: POINT::default(),
            current: POINT::default(),
            result: None,
            cancelled: false,
        });

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
            OVERLAY_CLASS,
            w!("Timezone Picker"),
            WS_POPUP | WS_VISIBLE,
            vx,
            vy,
            vw,
            vh,
            None,
            None,
            Some(hinstance.into()),
            Some(state.as_mut() as *mut _ as *const _),
        )
        .unwrap();

        // ~40% opaque dimming so the overlay is visibly "on" without
        // hiding the content underneath.
        SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), 90, LWA_ALPHA)
            .ok();

        let _ = SetForegroundWindow(hwnd);
        SetCapture(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_LBUTTONUP
                || msg.message == WM_KEYDOWN && msg.wParam.0 == VK_ESCAPE.0 as usize
            {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        ReleaseCapture().ok();
        DestroyWindow(hwnd).ok();
        UnregisterClassW(OVERLAY_CLASS, Some(hinstance.into())).ok();

        state.result
    }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;

        match msg {
            WM_NCCREATE => {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                LRESULT(1)
            }
            WM_LBUTTONDOWN => {
                if let Some(state) = state_ptr.as_mut() {
                    state.dragging = true;
                    state.start = point_from_lparam(lparam);
                    state.current = state.start;
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if let Some(state) = state_ptr.as_mut()
                    && state.dragging
                {
                    state.current = point_from_lparam(lparam);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if let Some(state) = state_ptr.as_mut() {
                    state.dragging = false;
                    let end = point_from_lparam(lparam);
                    let rect = RECT {
                        left: state.start.x.min(end.x),
                        top: state.start.y.min(end.y),
                        right: state.start.x.max(end.x),
                        bottom: state.start.y.max(end.y),
                    };
                    // Ignore accidental pixel-sized "clicks".
                    if (rect.right - rect.left).abs() > 3 || (rect.bottom - rect.top).abs() > 3 {
                        state.result = Some(rect);
                    } else {
                        // Treat a plain click as "select nothing here,
                        // just use this point" -- small rect around it.
                        state.result = Some(RECT {
                            left: end.x - 2,
                            top: end.y - 2,
                            right: end.x + 2,
                            bottom: end.y + 2,
                        });
                    }
                }
                LRESULT(0)
            }
            WM_KEYDOWN if wparam.0 as u16 == VK_ESCAPE.0 => {
                if let Some(state) = state_ptr.as_mut() {
                    state.cancelled = true;
                    state.result = None;
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if let Some(state) = state_ptr.as_ref()
                    && state.dragging
                {
                    let pen = CreatePen(
                        PS_SOLID,
                        2,
                        windows::Win32::Foundation::COLORREF(0x00FFFF00),
                    );
                    let old_pen = SelectObject(hdc, pen.into());
                    let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
                    SetBkMode(hdc, TRANSPARENT);
                    let _ = Rectangle(
                        hdc,
                        state.start.x.min(state.current.x),
                        state.start.y.min(state.current.y),
                        state.start.x.max(state.current.x),
                        state.start.y.max(state.current.y),
                    );
                    SelectObject(hdc, old_pen);
                    SelectObject(hdc, old_brush);
                }
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn point_from_lparam(lparam: LPARAM) -> POINT {
    POINT {
        x: (lparam.0 & 0xFFFF) as i16 as i32,
        y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    }
}
