// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Windows Imaging Component (WIC) COM Decoder & Clipboard Image Extraction
// Strictly zero external crates: pure Rust std + Win32 / COM FFI

// COM/Win32 names are kept verbatim so they can be grepped against the SDK headers.
#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

type HRESULT = i32;

const S_OK: HRESULT = 0;
const S_FALSE: HRESULT = 1;
const COINIT_MULTITHREADED: u32 = 0;
const CLSCTX_INPROC_SERVER: u32 = 1;
const GENERIC_READ: u32 = 0x8000_0000;
const WICDecodeMetadataCacheOnDemand: u32 = 0;
const WICBitmapDitherTypeNone: u32 = 0;
const WICBitmapPaletteTypeCustom: u32 = 0;
const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;

#[repr(C)]
struct GUID {
    Data1: u32,
    Data2: u16,
    Data3: u16,
    Data4: [u8; 8],
}

/// CLSID_WICImagingFactory2 (Windows 8+); it also serves IWICImagingFactory.
const CLSID_WICImagingFactory2: GUID = GUID {
    Data1: 0x317d06e8,
    Data2: 0x5f24,
    Data3: 0x433d,
    Data4: [0xbd, 0xf7, 0x79, 0xce, 0x68, 0xd8, 0xab, 0xc2],
};

const IID_IWICImagingFactory: GUID = GUID {
    Data1: 0xec5ec8a9,
    Data2: 0xc395,
    Data3: 0x4314,
    Data4: [0x9c, 0x77, 0x54, 0xd7, 0xa9, 0x35, 0xff, 0x70],
};

const GUID_WICPixelFormat32bppBGRA: GUID = GUID {
    Data1: 0x6fddc324,
    Data2: 0x4e03,
    Data3: 0x4bfe,
    Data4: [0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0f],
};

// --- COM vtables: only the slots zPDF calls are typed; the rest are `usize`
// padding so every typed slot keeps its exact offset. Slots 0-2 are IUnknown
// (QueryInterface, AddRef, Release); Release is called generically by ComPtr.

#[repr(C)]
struct IWICImagingFactoryVtbl {
    _iunknown: [usize; 3],
    CreateDecoderFromFilename: unsafe extern "system" fn(*mut c_void, *const u16, *const GUID, u32, u32, *mut *mut Com<IWICBitmapDecoderVtbl>) -> HRESULT,
    CreateDecoderFromStream: unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, u32, *mut *mut Com<IWICBitmapDecoderVtbl>) -> HRESULT,
    // CreateDecoderFromFileHandle, CreateComponentInfo, CreateDecoder, CreateEncoder, CreatePalette
    _slots_5_9: [usize; 5],
    CreateFormatConverter: unsafe extern "system" fn(*mut c_void, *mut *mut Com<IWICFormatConverterVtbl>) -> HRESULT,
    // CreateBitmapScaler, CreateBitmapClipper, CreateBitmapFlipRotator
    _slots_11_13: [usize; 3],
    CreateStream: unsafe extern "system" fn(*mut c_void, *mut *mut Com<IWICStreamVtbl>) -> HRESULT,
}

#[repr(C)]
struct IWICBitmapDecoderVtbl {
    _iunknown: [usize; 3],
    // QueryCapability, Initialize, GetContainerFormat, GetDecoderInfo, CopyPalette,
    // GetMetadataQueryReader, GetPreview, GetColorContexts, GetThumbnail, GetFrameCount
    _slots_3_12: [usize; 10],
    GetFrame: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
struct IWICFormatConverterVtbl {
    _iunknown: [usize; 3],
    // IWICBitmapSource
    GetSize: unsafe extern "system" fn(*mut c_void, *mut u32, *mut u32) -> HRESULT,
    // GetPixelFormat, GetResolution, CopyPalette
    _slots_4_6: [usize; 3],
    CopyPixels: unsafe extern "system" fn(*mut c_void, *const c_void, u32, u32, *mut u8) -> HRESULT,
    // IWICFormatConverter
    Initialize: unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, u32, *mut c_void, f64, u32) -> HRESULT,
}

#[repr(C)]
struct IWICStreamVtbl {
    _iunknown: [usize; 3],
    // ISequentialStream: Read, Write; IStream: Seek, SetSize, CopyTo, Commit, Revert,
    // LockRegion, UnlockRegion, Stat, Clone; IWICStream: InitializeFromIStream,
    // InitializeFromFilename
    _slots_3_15: [usize; 13],
    InitializeFromMemory: unsafe extern "system" fn(*mut c_void, *mut u8, u32) -> HRESULT,
}

/// A COM object: a pointer to its vtable.
#[repr(C)]
struct Com<V> {
    vtbl: *const V,
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> HRESULT;
    fn CoUninitialize();
    fn CoCreateInstance(rclsid: *const GUID, pUnkOuter: *mut c_void, dwClsContext: u32, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT;
}

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hWndNewOwner: *mut c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn GetClipboardData(uFormat: u32) -> *mut c_void;
    fn SetClipboardData(uFormat: u32, hMem: *mut c_void) -> *mut c_void;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut c_void;
    fn GlobalFree(hMem: *mut c_void) -> *mut c_void;
    fn GlobalLock(hMem: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(hMem: *mut c_void) -> i32;
    fn GlobalSize(hMem: *mut c_void) -> usize;
}

// --- Scoped RAII COM helpers ---

struct ComGuard {
    need_uninit: bool,
}

impl ComGuard {
    fn init() -> Self {
        // RPC_E_CHANGED_MODE (thread already STA) is fine: COM is usable, just not ours to uninit.
        let hr = unsafe { CoInitializeEx(ptr::null_mut(), COINIT_MULTITHREADED) };
        Self { need_uninit: hr == S_OK || hr == S_FALSE }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.need_uninit {
            unsafe { CoUninitialize() };
        }
    }
}

/// Owning COM pointer; calls IUnknown::Release (vtable slot 2) on drop.
struct ComPtr<V>(*mut Com<V>);

impl<V> ComPtr<V> {
    /// Wraps an out-parameter result, failing on a bad HRESULT or null pointer.
    fn check(hr: HRESULT, raw: *mut Com<V>, what: &str) -> Result<Self, String> {
        if hr < 0 || raw.is_null() {
            return Err(format!("{} failed (0x{:08X})", what, hr));
        }
        Ok(Self(raw))
    }

    fn this(&self) -> *mut c_void {
        self.0 as *mut c_void
    }

    fn vtbl(&self) -> &V {
        unsafe { &*(*self.0).vtbl }
    }
}

impl<V> Drop for ComPtr<V> {
    fn drop(&mut self) {
        unsafe {
            let vtable = *(self.0 as *const *const usize);
            let release: unsafe extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(*vtable.add(2));
            release(self.0 as *mut c_void);
        }
    }
}

fn hr_check(hr: HRESULT, what: &str) -> Result<(), String> {
    if hr < 0 {
        Err(format!("{} failed (0x{:08X})", what, hr))
    } else {
        Ok(())
    }
}

unsafe fn create_imaging_factory() -> Result<ComPtr<IWICImagingFactoryVtbl>, String> {
    let mut factory: *mut Com<IWICImagingFactoryVtbl> = ptr::null_mut();
    let hr = CoCreateInstance(
        &CLSID_WICImagingFactory2,
        ptr::null_mut(),
        CLSCTX_INPROC_SERVER,
        &IID_IWICImagingFactory,
        &mut factory as *mut _ as *mut *mut c_void,
    );
    ComPtr::check(hr, factory, "CoCreateInstance(WICImagingFactory)")
}

/// Converts frame 0 of `decoder` to 32bpp BGRA.
unsafe fn decode_to_bgra(
    factory: &ComPtr<IWICImagingFactoryVtbl>,
    decoder: ComPtr<IWICBitmapDecoderVtbl>,
) -> Result<(u32, u32, Vec<u8>), String> {
    let mut frame: *mut c_void = ptr::null_mut();
    hr_check((decoder.vtbl().GetFrame)(decoder.this(), 0, &mut frame), "IWICBitmapDecoder::GetFrame(0)")?;
    let frame = ComPtr::<()>::check(S_OK, frame as *mut Com<()>, "IWICBitmapDecoder::GetFrame(0)")?;

    let mut conv = ptr::null_mut();
    let hr = (factory.vtbl().CreateFormatConverter)(factory.this(), &mut conv);
    let conv = ComPtr::check(hr, conv, "CreateFormatConverter")?;
    let hr = (conv.vtbl().Initialize)(
        conv.this(),
        frame.this(),
        &GUID_WICPixelFormat32bppBGRA,
        WICBitmapDitherTypeNone,
        ptr::null_mut(),
        0.0,
        WICBitmapPaletteTypeCustom,
    );
    hr_check(hr, "IWICFormatConverter::Initialize")?;

    let (mut width, mut height) = (0u32, 0u32);
    hr_check((conv.vtbl().GetSize)(conv.this(), &mut width, &mut height), "IWICFormatConverter::GetSize")?;
    if width == 0 || height == 0 {
        return Err("Decoded image has zero dimensions".to_string());
    }
    let stride = width.checked_mul(4).ok_or("Image width causes overflow")?;
    let size = (stride as usize).checked_mul(height as usize).filter(|&s| s <= u32::MAX as usize).ok_or("Image too large")?;
    let mut buffer = vec![0u8; size];
    let hr = (conv.vtbl().CopyPixels)(conv.this(), ptr::null(), stride, size as u32, buffer.as_mut_ptr());
    hr_check(hr, "IWICFormatConverter::CopyPixels")?;
    Ok((width, height, buffer))
}

// --- Public WIC functions ---

/// Decodes an image file (PNG, JPEG, BMP, TIFF, GIF, ...) into 32bpp BGRA.
pub fn decode_image_to_bgra(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    if !path.exists() {
        return Err(format!("File does not exist: {:?}", path));
    }
    let _com = ComGuard::init();
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        let factory = create_imaging_factory()?;
        let mut decoder = ptr::null_mut();
        let hr = (factory.vtbl().CreateDecoderFromFilename)(
            factory.this(),
            wide.as_ptr(),
            ptr::null(),
            GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
            &mut decoder,
        );
        let decoder = ComPtr::check(hr, decoder, "CreateDecoderFromFilename")?;
        decode_to_bgra(&factory, decoder)
    }
}

/// Decodes an in-memory image (PNG, JPEG, BMP, TIFF, GIF, ...) into 32bpp BGRA.
pub fn decode_image_memory_to_bgra(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if bytes.len() < 14 || bytes.len() > u32::MAX as usize {
        return Err("Input byte buffer is not a valid image size".to_string());
    }
    let _com = ComGuard::init();
    unsafe {
        let factory = create_imaging_factory()?;
        let mut stream = ptr::null_mut();
        let hr = (factory.vtbl().CreateStream)(factory.this(), &mut stream);
        let stream = ComPtr::check(hr, stream, "CreateStream")?;
        // WIC only reads from the buffer; it outlives the stream.
        let hr = (stream.vtbl().InitializeFromMemory)(stream.this(), bytes.as_ptr() as *mut u8, bytes.len() as u32);
        hr_check(hr, "IWICStream::InitializeFromMemory")?;
        let mut decoder = ptr::null_mut();
        let hr = (factory.vtbl().CreateDecoderFromStream)(factory.this(), stream.this(), ptr::null(), WICDecodeMetadataCacheOnDemand, &mut decoder);
        let decoder = ComPtr::check(hr, decoder, "CreateDecoderFromStream")?;
        decode_to_bgra(&factory, decoder)
    }
}

// --- DIB parsing & clipboard ---

/// Parses a packed DIB (CF_DIB: BITMAPINFOHEADER + palette/masks + pixels) into
/// top-down 32bpp BGRA. 32bpp DIBs are converted directly so that the common
/// "all alpha bytes zero" clipboard case can be fixed up to opaque; every other
/// format is decoded by WIC after prepending a BITMAPFILEHEADER.
pub fn parse_dib_to_bgra(dib: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let u32_at = |o: usize| Some(u32::from_le_bytes(dib.get(o..o + 4)?.try_into().ok()?));
    let header_size = u32_at(0)? as usize;
    if header_size < 40 || header_size > dib.len() {
        return None;
    }
    let raw_height = u32_at(8)? as i32;
    let width = (u32_at(4)? as i32).unsigned_abs() as usize;
    let height = raw_height.unsigned_abs() as usize;
    let bit_count = u16::from_le_bytes(dib[14..16].try_into().ok()?);
    let compression = u32_at(16)?;
    let colors_used = u32_at(32)? as usize;
    if width == 0 || height == 0 {
        return None;
    }

    const BI_RGB: u32 = 0;
    const BI_BITFIELDS: u32 = 3;
    let palette_len = if bit_count <= 8 && colors_used == 0 { 1usize << bit_count } else { colors_used };
    // A 40-byte header with BI_BITFIELDS is followed by three DWORD color masks.
    let masks_len = if compression == BI_BITFIELDS && header_size == 40 { 12 } else { 0 };
    let pixel_offset = header_size + masks_len + palette_len * 4;
    if pixel_offset > dib.len() {
        return None;
    }

    if bit_count == 32 && (compression == BI_RGB || compression == BI_BITFIELDS) {
        let row = width.checked_mul(4)?;
        let src = dib.get(pixel_offset..pixel_offset.checked_add(row.checked_mul(height)?)?)?;
        let has_alpha = src.chunks_exact(4).any(|p| p[3] != 0);
        let mut bgra = Vec::with_capacity(src.len());
        for y in 0..height {
            let src_y = if raw_height > 0 { height - 1 - y } else { y };
            for px in src[src_y * row..][..row].chunks_exact(4) {
                bgra.extend_from_slice(&[px[0], px[1], px[2], if has_alpha { px[3] } else { 0xFF }]);
            }
        }
        return Some((width as u32, height as u32, bgra));
    }

    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&((14 + pixel_offset) as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    decode_image_memory_to_bgra(&bmp).ok()
}

/// Closes the clipboard when dropped.
struct ClipboardGuard;

impl ClipboardGuard {
    fn open() -> Option<Self> {
        (unsafe { OpenClipboard(ptr::null_mut()) } != 0).then_some(ClipboardGuard)
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe { CloseClipboard() };
    }
}

/// Extracts a `CF_DIB` image from the Windows clipboard as 32bpp BGRA.
pub fn get_clipboard_image_bgra() -> Option<(u32, u32, Vec<u8>)> {
    let _clip = ClipboardGuard::open()?;
    unsafe {
        let h_data = GetClipboardData(CF_DIB);
        if h_data.is_null() {
            return None;
        }
        let data = GlobalLock(h_data) as *const u8;
        if data.is_null() {
            return None;
        }
        let result = parse_dib_to_bgra(std::slice::from_raw_parts(data, GlobalSize(h_data)));
        GlobalUnlock(h_data);
        result
    }
}

/// Copies a string onto the Windows clipboard as `CF_UNICODETEXT`.
pub fn set_clipboard_text(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let Some(_clip) = ClipboardGuard::open() else { return false };
    unsafe {
        EmptyClipboard();
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
        if h_mem.is_null() {
            return false;
        }
        let dst = GlobalLock(h_mem) as *mut u16;
        if dst.is_null() {
            GlobalFree(h_mem);
            return false;
        }
        ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
        GlobalUnlock(h_mem);
        // On success the clipboard owns h_mem.
        if SetClipboardData(CF_UNICODETEXT, h_mem).is_null() {
            GlobalFree(h_mem);
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2x2 24bpp bottom-up BMP: top row blue, white; bottom row red, green.
    fn build_test_2x2_bmp() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"BM");
        data.extend_from_slice(&70u32.to_le_bytes()); // file size
        data.extend_from_slice(&[0, 0, 0, 0]);
        data.extend_from_slice(&54u32.to_le_bytes()); // pixel offset
        data.extend_from_slice(&40u32.to_le_bytes()); // biSize
        data.extend_from_slice(&2i32.to_le_bytes()); // width
        data.extend_from_slice(&2i32.to_le_bytes()); // height (bottom-up)
        data.extend_from_slice(&1u16.to_le_bytes()); // planes
        data.extend_from_slice(&24u16.to_le_bytes()); // bpp
        data.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
        data.extend_from_slice(&16u32.to_le_bytes()); // image size
        data.extend_from_slice(&[0; 16]); // ppm x/y, clr used/important
        data.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]); // bottom row: red, green
        data.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]); // top row: blue, white
        data
    }

    fn assert_2x2_pixels(bgra: &[u8]) {
        assert_eq!(&bgra[0..4], &[255, 0, 0, 255]); // blue
        assert_eq!(&bgra[4..8], &[255, 255, 255, 255]); // white
        assert_eq!(&bgra[8..12], &[0, 0, 255, 255]); // red
        assert_eq!(&bgra[12..16], &[0, 255, 0, 255]); // green
    }

    fn dib_32bpp_1x1(pixel: [u8; 4]) -> Vec<u8> {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&4u32.to_le_bytes());
        dib.extend_from_slice(&[0; 16]);
        dib.extend_from_slice(&pixel);
        dib
    }

    #[test]
    fn test_wic_decode_memory_valid_bmp() {
        let (w, h, bgra) = decode_image_memory_to_bgra(&build_test_2x2_bmp()).expect("decode");
        assert_eq!((w, h, bgra.len()), (2, 2, 16));
        assert_2x2_pixels(&bgra);
    }

    #[test]
    fn test_wic_decode_file_valid_bmp() {
        let temp_file = std::env::temp_dir().join(format!("zpdf_test_sample_{}.bmp", std::process::id()));
        std::fs::write(&temp_file, build_test_2x2_bmp()).expect("write temp bmp");
        let result = decode_image_to_bgra(&temp_file);
        let _ = std::fs::remove_file(&temp_file);
        let (w, h, bgra) = result.expect("decode");
        assert_eq!((w, h, bgra.len()), (2, 2, 16));
    }

    #[test]
    fn test_wic_decode_empty_and_invalid() {
        assert!(decode_image_memory_to_bgra(&[]).is_err());
        assert!(decode_image_memory_to_bgra(&[0x01, 0x02, 0x03]).is_err());
        assert!(decode_image_to_bgra(Path::new("C:\\non_existent_zpdf_image_123.png")).is_err());
    }

    #[test]
    fn test_parse_dib_24bpp() {
        let (w, h, bgra) = parse_dib_to_bgra(&build_test_2x2_bmp()[14..]).expect("parse");
        assert_eq!((w, h, bgra.len()), (2, 2, 16));
        assert_2x2_pixels(&bgra);
    }

    #[test]
    fn test_parse_dib_32bpp_zero_alpha_fixup() {
        let (w, h, bgra) = parse_dib_to_bgra(&dib_32bpp_1x1([10, 20, 30, 0])).expect("parse");
        assert_eq!((w, h), (1, 1));
        assert_eq!(&bgra, &[10, 20, 30, 255]);
    }

    #[test]
    fn test_parse_dib_32bpp_preserve_existing_alpha() {
        let (_, _, bgra) = parse_dib_to_bgra(&dib_32bpp_1x1([10, 20, 30, 128])).expect("parse");
        assert_eq!(&bgra, &[10, 20, 30, 128]);
    }

    #[test]
    fn test_clipboard_image_does_not_panic() {
        let _ = get_clipboard_image_bgra();
    }
}
