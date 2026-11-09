use crate::wide;
use std::{
    ffi::c_void,
    io,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    slice,
};
use windows_sys::{
    core::{IUnknown_Vtbl, GUID},
    Win32::{
        Foundation::HWND,
        Graphics::{
            Gdi::{DeleteObject, GetObjectW, BITMAP, HBITMAP},
            GdiPlus::{
                GdipCreateBitmapFromHBITMAP, GdipDisposeImage, GdipSaveImageToStream,
                GdiplusShutdown, GdiplusStartup, GdiplusStartupInput, GpImage,
            },
        },
        System::{
            DataExchange::{
                CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
                RegisterClipboardFormatW,
            },
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
            Ole::{CF_BITMAP, CF_UNICODETEXT},
        },
        UI::{
            Shell::{IStream_Read, IStream_Reset, IStream_Size, SHCreateMemStream},
            WindowsAndMessaging::{CopyImage, IMAGE_BITMAP, LR_CREATEDIBSECTION},
        },
    },
};

const MAX_PNG_BYTES: usize = 32 * 1024 * 1024;
const MAX_PIXELS: u64 = 40_000_000;
const PNG_ENCODER: GUID = GUID::from_u128(0x557cf406_1a04_11d3_9a73_0000f81ef32e);

struct OpenClipboardGuard;

impl Drop for OpenClipboardGuard {
    fn drop(&mut self) {
        unsafe { CloseClipboard() };
    }
}

struct OwnedBitmap(HBITMAP);

impl Drop for OwnedBitmap {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.0) };
    }
}

struct ImagingSession(usize);

impl Drop for ImagingSession {
    fn drop(&mut self) {
        unsafe { GdiplusShutdown(self.0) };
    }
}

struct Image(*mut GpImage);

impl Drop for Image {
    fn drop(&mut self) {
        unsafe { GdipDisposeImage(self.0) };
    }
}

struct Stream(*mut c_void);

impl Drop for Stream {
    fn drop(&mut self) {
        // IStream begins with the IUnknown vtable, including Release.
        unsafe { ((**self.0.cast::<*const IUnknown_Vtbl>()).Release)(self.0) };
    }
}

pub fn read_image(owner: HWND, prefer_text: bool) -> io::Result<Option<Vec<u8>>> {
    unsafe {
        let png_format = RegisterClipboardFormatW(wide("PNG").as_ptr());
        if OpenClipboard(owner) == 0 {
            return Err(io::Error::other(
                "The clipboard is busy. Try pasting again.",
            ));
        }
        let clipboard = OpenClipboardGuard;
        if prefer_text && IsClipboardFormatAvailable(CF_UNICODETEXT as u32) != 0 {
            return Ok(None);
        }

        let has_png = png_format != 0 && IsClipboardFormatAvailable(png_format) != 0;
        if has_png {
            if let Some(png) = copy_png(png_format) {
                return Ok(Some(png));
            }
        }
        // Windows can synthesize CF_BITMAP from either DIB clipboard format.
        if IsClipboardFormatAvailable(CF_BITMAP as u32) == 0 {
            return if has_png {
                Err(io::Error::other(
                    "The clipboard PNG is invalid or too large.",
                ))
            } else {
                Ok(None)
            };
        }

        let source = GetClipboardData(CF_BITMAP as u32);
        let mut info: BITMAP = zeroed();
        if source.is_null()
            || GetObjectW(
                source,
                size_of::<BITMAP>() as i32,
                (&mut info as *mut BITMAP).cast(),
            ) == 0
        {
            return Err(io::Error::other(
                "Windows could not read the clipboard image.",
            ));
        }
        if info.bmWidth <= 0 || !supported_size(info.bmWidth as u32, info.bmHeight.unsigned_abs()) {
            return Err(io::Error::other("This screenshot is too large."));
        }
        let bitmap = CopyImage(source, IMAGE_BITMAP, 0, 0, LR_CREATEDIBSECTION);
        if bitmap.is_null() {
            return Err(io::Error::other(
                "Windows could not copy the clipboard image.",
            ));
        }
        let bitmap = OwnedBitmap(bitmap);
        drop(clipboard);
        encode_bitmap(&bitmap).map(Some)
    }
}

fn supported_size(width: u32, height: u32) -> bool {
    let pixels = u64::from(width) * u64::from(height);
    pixels > 0 && pixels <= MAX_PIXELS
}

unsafe fn copy_png(format: u32) -> Option<Vec<u8>> {
    let handle = GetClipboardData(format);
    if handle.is_null() {
        return None;
    }
    let size = GlobalSize(handle);
    if size == 0 || size > MAX_PNG_BYTES {
        return None;
    }
    let bytes = GlobalLock(handle).cast::<u8>();
    if bytes.is_null() {
        return None;
    }
    let data = slice::from_raw_parts(bytes, size);
    let png = png_length(data).map(|len| data[..len].to_vec());
    GlobalUnlock(handle);
    png
}

fn png_length(bytes: &[u8]) -> Option<usize> {
    if bytes.get(..16)? != b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
    let height = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
    if !supported_size(width, height) {
        return None;
    }

    // Clipboard allocations can include padding. Copy exactly through IEND.
    let mut offset = 8;
    let mut has_pixels = false;
    while let Some(header) = bytes.get(offset..offset + 8) {
        let length = u32::from_be_bytes(header[..4].try_into().ok()?) as usize;
        offset = offset.checked_add(length)?.checked_add(12)?;
        if offset > bytes.len() {
            return None;
        }
        match &header[4..] {
            b"IDAT" => has_pixels = true,
            b"IEND" if length == 0 && has_pixels => return Some(offset),
            _ => {}
        }
    }
    None
}

unsafe fn encode_bitmap(bitmap: &OwnedBitmap) -> io::Result<Vec<u8>> {
    let encoding_error = || io::Error::other("Windows could not encode the screenshot as PNG.");
    let input = GdiplusStartupInput {
        GdiplusVersion: 1,
        SuppressExternalCodecs: 1,
        ..Default::default()
    };
    let mut token = 0;
    if GdiplusStartup(&mut token, &input, null_mut()) != 0 {
        return Err(encoding_error());
    }
    let _session = ImagingSession(token);
    let mut image = null_mut();
    if GdipCreateBitmapFromHBITMAP(bitmap.0, null_mut(), &mut image) != 0 || image.is_null() {
        return Err(encoding_error());
    }
    let image = Image(image.cast());
    let stream = SHCreateMemStream(null(), 0);
    if stream.is_null() {
        return Err(encoding_error());
    }
    let stream = Stream(stream);
    let mut size = 0;
    if GdipSaveImageToStream(image.0, stream.0, &PNG_ENCODER, null()) != 0
        || IStream_Size(stream.0, &mut size) != 0
        || size == 0
    {
        return Err(encoding_error());
    }
    if size > MAX_PNG_BYTES as u64 {
        return Err(io::Error::other(
            "The screenshot exceeds the 32 MiB PNG limit.",
        ));
    }
    let mut png = vec![0; size as usize];
    if IStream_Reset(stream.0) != 0
        || IStream_Read(stream.0, png.as_mut_ptr().cast(), size as u32) != 0
    {
        return Err(encoding_error());
    }
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Graphics::Gdi::CreateBitmap;

    #[test]
    fn windows_bitmap_encoder_produces_complete_png() {
        let pixels = [0x003366ccu32, 0x0077aa33];
        let bitmap = unsafe { CreateBitmap(2, 1, 1, 32, pixels.as_ptr().cast()) };
        assert!(!bitmap.is_null());
        let png = unsafe { encode_bitmap(&OwnedBitmap(bitmap)) }.unwrap();
        assert_eq!(png_length(&png), Some(png.len()));
        assert_eq!(&png[16..24], &[0, 0, 0, 2, 0, 0, 0, 1]);
    }
}
