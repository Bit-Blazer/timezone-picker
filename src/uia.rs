// UI Automation text extraction.
//
// Strategy: given a screen point (center of the user's drag-selection),
// find the UI Automation element there and pull whatever text it exposes.
// This covers native Win32, WPF, most Chromium/Firefox content, and Office.
// Returns None if the element has no usable text (custom-drawn UI, images,
// games, RDP sessions, etc.) -- caller should fall back to OCR in that case.

use windows::Win32::Foundation::POINT;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, UIA_NamePropertyId, UIA_ValueValuePropertyId,
};
use windows::core::Result;

/// Must be called once per thread before using UIA. Safe to call more than
/// once (subsequent calls are no-ops / return an already-initialized error
/// that we ignore).
pub fn init_com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

pub fn text_at_point(x: i32, y: i32) -> Option<String> {
    init_com();

    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok()?;

    let point = POINT { x, y };
    let element = unsafe { automation.ElementFromPoint(point) }.ok()?;

    // Prefer TextPattern if the control supports it (browsers, editors,
    // PDF readers) -- it gives us surrounding text, not just this one
    // element's own Name/Value.
    if let Ok(text) = try_text_pattern(&automation, &element)
        && !text.trim().is_empty()
    {
        return Some(text);
    }

    // Fall back to simple properties: Value (edit controls, list items)
    // then Name (labels, buttons, calendar cells).
    if let Ok(value) = unsafe { element.GetCurrentPropertyValue(UIA_ValueValuePropertyId) } {
        let s = unsafe {
            if value.Anonymous.Anonymous.vt == windows::Win32::System::Variant::VT_BSTR {
                value.Anonymous.Anonymous.Anonymous.bstrVal.to_string()
            } else {
                String::new()
            }
        };
        if !s.trim().is_empty() {
            return Some(s);
        }
    }

    if let Ok(name) = unsafe { element.GetCurrentPropertyValue(UIA_NamePropertyId) } {
        let s = unsafe {
            if name.Anonymous.Anonymous.vt == windows::Win32::System::Variant::VT_BSTR {
                name.Anonymous.Anonymous.Anonymous.bstrVal.to_string()
            } else {
                String::new()
            }
        };
        if !s.trim().is_empty() {
            return Some(s);
        }
    }

    None
}

fn try_text_pattern(
    _automation: &IUIAutomation,
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> Result<String> {
    use windows::Win32::UI::Accessibility::{IUIAutomationTextPattern, UIA_TextPatternId};

    let pattern =
        unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }?;
    let doc_range = unsafe { pattern.DocumentRange() }?;
    let text = unsafe { doc_range.GetText(4000) }?; // cap length; we only need a snippet
    Ok(text.to_string())
}
