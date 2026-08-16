use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::Buffer;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::System::WinRT::IBufferByteAccess;
use windows::core::Interface;

pub fn extract_text(rect: RECT) -> Option<String> {
    unsafe {
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return None;
        }

        let hdc_screen = GetDC(None);
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        let hbm = CreateCompatibleBitmap(hdc_screen, width, height);
        let old_obj = SelectObject(hdc_mem, hbm.into());

        BitBlt(
            hdc_mem,
            0,
            0,
            width,
            height,
            Some(hdc_screen),
            rect.left,
            rect.top,
            SRCCOPY,
        )
        .ok()?;

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let size = (width * height * 4) as u32;
        let mut pixels = vec![0u8; size as usize];

        GetDIBits(
            hdc_screen,
            hbm,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(hdc_mem, old_obj);
        let _ = DeleteObject(hbm.into());
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_screen);

        let buffer = Buffer::Create(size).ok()?;
        buffer.SetLength(size).ok()?;

        let byte_access: IBufferByteAccess = buffer.cast().ok()?;
        let dest_ptr = byte_access.Buffer().ok()?;

        std::ptr::copy_nonoverlapping(pixels.as_ptr(), dest_ptr, size as usize);

        let bitmap =
            SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, width, height)
                .ok()?;

        let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok()?;

        // .get() blocks on the IAsyncOperation
        let op = engine.RecognizeAsync(&bitmap).ok()?;
        while op.Status().ok()? == windows_future::AsyncStatus::Started {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let result = op.GetResults().ok()?;

        let text = result.Text().ok()?;
        let text_str = text.to_string();
        if text_str.trim().is_empty() {
            None
        } else {
            Some(text_str)
        }
    }
}
