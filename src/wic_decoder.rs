// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Windows Imaging Component (WIC) COM Decoder & Clipboard Image Extraction
// Strictly zero external crates: pure Rust std + Win32 / COM FFI

#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

// --- Win32 COM & Basic Types ---

pub type HRESULT = i32;
pub type ULONG = u32;

pub const S_OK: HRESULT = 0;
pub const S_FALSE: HRESULT = 1;
pub const RPC_E_CHANGED_MODE: HRESULT = 0x80010106u32 as i32;

pub const CLSCTX_INPROC_SERVER: u32 = 1;
pub const GENERIC_READ: u32 = 0x80000000;

pub const WICDecodeMetadataCacheOnDemand: u32 = 0;
pub const WICBitmapDitherTypeNone: u32 = 0;
pub const WICBitmapPaletteTypeCustom: u32 = 0;

pub const WINCODEC_SDK_VERSION1: u32 = 0x0236;
pub const WINCODEC_SDK_VERSION2: u32 = 0x0237;

pub const CF_DIB: u32 = 8;
pub const CF_DIBV5: u32 = 17;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GUID {
    pub Data1: u32,
    pub Data2: u16,
    pub Data3: u16,
    pub Data4: [u8; 8],
}

pub const CLSID_WICImagingFactory: GUID = GUID {
    Data1: 0xcacaf262,
    Data2: 0x9370,
    Data3: 0x4615,
    Data4: [0xa1, 0x3b, 0x9f, 0x55, 0x39, 0xda, 0x4c, 0x0a],
};

pub const CLSID_WICImagingFactory1: GUID = CLSID_WICImagingFactory;

pub const CLSID_WICImagingFactory2: GUID = GUID {
    Data1: 0x317d06e8,
    Data2: 0x5f24,
    Data3: 0x433d,
    Data4: [0xbd, 0xf7, 0x79, 0xce, 0x68, 0xd8, 0xab, 0xc2],
};

pub const IID_IWICImagingFactory: GUID = GUID {
    Data1: 0xec5ec8a9,
    Data2: 0xc395,
    Data3: 0x4314,
    Data4: [0x9c, 0x77, 0x54, 0xd7, 0xa9, 0x35, 0xff, 0x70],
};

pub const IID_IWICBitmapDecoder: GUID = GUID {
    Data1: 0x9edde9e7,
    Data2: 0x8dee,
    Data3: 0x47ea,
    Data4: [0x99, 0xdf, 0xe6, 0xfa, 0xf2, 0xed, 0x44, 0xbf],
};

pub const IID_IWICBitmapFrameDecode: GUID = GUID {
    Data1: 0x3b16811b,
    Data2: 0x6a43,
    Data3: 0x4ec9,
    Data4: [0xa8, 0x13, 0x3d, 0x93, 0x0c, 0x13, 0xb9, 0x40],
};

pub const IID_IWICFormatConverter: GUID = GUID {
    Data1: 0x00000301,
    Data2: 0xa8f2,
    Data3: 0x4877,
    Data4: [0xba, 0x0a, 0xfd, 0x2b, 0x66, 0x45, 0xfb, 0x94],
};

pub const IID_IWICStream: GUID = GUID {
    Data1: 0x135ff860,
    Data2: 0x22b7,
    Data3: 0x4ddf,
    Data4: [0xb0, 0xf6, 0x21, 0x8f, 0x4f, 0x29, 0x9a, 0x43],
};

pub const GUID_WICPixelFormat32bppBGRA: GUID = GUID {
    Data1: 0x6fddc324,
    Data2: 0x4e03,
    Data3: 0x4bfe,
    Data4: [0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0f],
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WICRect {
    pub X: i32,
    pub Y: i32,
    pub Width: i32,
    pub Height: i32,
}

// --- COM Vtables ---

#[repr(C)]
pub struct IWICImagingFactoryVtbl {
    // IUnknown
    pub QueryInterface: unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, ppvObject: *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    // IWICImagingFactory
    pub CreateDecoderFromFilename: unsafe extern "system" fn(
        this: *mut c_void,
        wzFilename: *const u16,
        pguidVendor: *const GUID,
        dwDesiredAccess: u32,
        metadataOptions: u32,
        ppIDecoder: *mut *mut IWICBitmapDecoder,
    ) -> HRESULT,
    pub CreateDecoderFromStream: unsafe extern "system" fn(
        this: *mut c_void,
        pIStream: *mut c_void,
        pguidVendor: *const GUID,
        metadataOptions: u32,
        ppIDecoder: *mut *mut IWICBitmapDecoder,
    ) -> HRESULT,
    pub CreateDecoderFromFileHandle: unsafe extern "system" fn(
        this: *mut c_void,
        hFile: usize,
        pguidVendor: *const GUID,
        metadataOptions: u32,
        ppIDecoder: *mut *mut IWICBitmapDecoder,
    ) -> HRESULT,
    pub CreateComponentInfo: unsafe extern "system" fn(
        this: *mut c_void,
        clsidComponent: *const GUID,
        ppIInfo: *mut *mut c_void,
    ) -> HRESULT,
    pub CreateDecoder: unsafe extern "system" fn(
        this: *mut c_void,
        guidContainerFormat: *const GUID,
        pguidVendor: *const GUID,
        ppIDecoder: *mut *mut IWICBitmapDecoder,
    ) -> HRESULT,
    pub CreateEncoder: unsafe extern "system" fn(
        this: *mut c_void,
        guidContainerFormat: *const GUID,
        pguidVendor: *const GUID,
        ppIEncoder: *mut *mut c_void,
    ) -> HRESULT,
    pub CreatePalette: unsafe extern "system" fn(
        this: *mut c_void,
        ppIPalette: *mut *mut c_void,
    ) -> HRESULT,
    pub CreateFormatConverter: unsafe extern "system" fn(
        this: *mut c_void,
        ppIFormatConverter: *mut *mut IWICFormatConverter,
    ) -> HRESULT,
    pub CreateBitmapScaler: unsafe extern "system" fn(
        this: *mut c_void,
        ppIBitmapScaler: *mut *mut c_void,
    ) -> HRESULT,
    pub CreateBitmapClipper: unsafe extern "system" fn(
        this: *mut c_void,
        ppIBitmapClipper: *mut *mut c_void,
    ) -> HRESULT,
    pub CreateBitmapFlipRotator: unsafe extern "system" fn(
        this: *mut c_void,
        ppIBitmapFlipRotator: *mut *mut c_void,
    ) -> HRESULT,
    pub CreateStream: unsafe extern "system" fn(
        this: *mut c_void,
        ppIWICStream: *mut *mut IWICStream,
    ) -> HRESULT,
    pub CreateColorContext: unsafe extern "system" fn(this: *mut c_void, ppIWICColorContext: *mut *mut c_void) -> HRESULT,
    pub CreateColorTransformer: unsafe extern "system" fn(this: *mut c_void, ppIWICColorTransform: *mut *mut c_void) -> HRESULT,
    pub CreateBitmap: unsafe extern "system" fn(this: *mut c_void, uiWidth: u32, uiHeight: u32, pixelFormat: *const GUID, option: u32, ppIBitmap: *mut *mut c_void) -> HRESULT,
    pub CreateBitmapFromSource: unsafe extern "system" fn(this: *mut c_void, piBitmapSource: *mut c_void, option: u32, ppIBitmap: *mut *mut c_void) -> HRESULT,
    pub CreateBitmapFromSourceRect: unsafe extern "system" fn(this: *mut c_void, piBitmapSource: *mut c_void, x: u32, y: u32, width: u32, height: u32, ppIBitmap: *mut *mut c_void) -> HRESULT,
    pub CreateBitmapFromMemory: unsafe extern "system" fn(this: *mut c_void, uiWidth: u32, uiHeight: u32, pixelFormat: *const GUID, cbStride: u32, cbBufferSize: u32, pbBuffer: *mut u8, ppIBitmap: *mut *mut c_void) -> HRESULT,
    pub CreateBitmapFromHBITMAP: unsafe extern "system" fn(this: *mut c_void, hBitmap: *mut c_void, hPalette: *mut c_void, options: u32, ppIBitmap: *mut *mut c_void) -> HRESULT,
    pub CreateBitmapFromHICON: unsafe extern "system" fn(this: *mut c_void, hIcon: *mut c_void, ppIBitmap: *mut *mut c_void) -> HRESULT,
    pub CreateComponentEnumerator: unsafe extern "system" fn(this: *mut c_void, componentTypes: u32, options: u32, ppIEnumUnknown: *mut *mut c_void) -> HRESULT,
    pub CreateFastMetadataEncoderFromDecoder: unsafe extern "system" fn(this: *mut c_void, pIDecoder: *mut c_void, ppIFastEncoder: *mut *mut c_void) -> HRESULT,
    pub CreateFastMetadataEncoderFromFrameDecode: unsafe extern "system" fn(this: *mut c_void, pIFrameDecoder: *mut c_void, ppIFastEncoder: *mut *mut c_void) -> HRESULT,
    pub CreateQueryWriter: unsafe extern "system" fn(this: *mut c_void, guidMetadataFormat: *const GUID, pguidVendor: *const GUID, ppIQueryWriter: *mut *mut c_void) -> HRESULT,
    pub CreateQueryWriterFromReader: unsafe extern "system" fn(this: *mut c_void, pIQueryReader: *mut c_void, pguidVendor: *const GUID, ppIQueryWriter: *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
pub struct IWICImagingFactory {
    pub lpVtbl: *const IWICImagingFactoryVtbl,
}

#[repr(C)]
pub struct IWICBitmapDecoderVtbl {
    // IUnknown
    pub QueryInterface: unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, ppvObject: *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    // IWICBitmapDecoder
    pub QueryCapability: unsafe extern "system" fn(this: *mut c_void, pIStream: *mut c_void, pdwCapability: *mut u32) -> HRESULT,
    pub Initialize: unsafe extern "system" fn(this: *mut c_void, pIStream: *mut c_void, cacheOptions: u32) -> HRESULT,
    pub GetContainerFormat: unsafe extern "system" fn(this: *mut c_void, pguidContainerFormat: *mut GUID) -> HRESULT,
    pub GetDecoderInfo: unsafe extern "system" fn(this: *mut c_void, ppIDecoderInfo: *mut *mut c_void) -> HRESULT,
    pub CopyPalette: unsafe extern "system" fn(this: *mut c_void, pIPalette: *mut c_void) -> HRESULT,
    pub GetMetadataQueryReader: unsafe extern "system" fn(this: *mut c_void, ppIMetadataQueryReader: *mut *mut c_void) -> HRESULT,
    pub GetPreview: unsafe extern "system" fn(this: *mut c_void, ppIBitmapSource: *mut *mut c_void) -> HRESULT,
    pub GetColorContexts: unsafe extern "system" fn(this: *mut c_void, cCount: u32, ppIColorContexts: *mut *mut c_void, pcActualCount: *mut u32) -> HRESULT,
    pub GetThumbnail: unsafe extern "system" fn(this: *mut c_void, ppIThumbnail: *mut *mut c_void) -> HRESULT,
    pub GetFrameCount: unsafe extern "system" fn(this: *mut c_void, pCount: *mut u32) -> HRESULT,
    pub GetFrame: unsafe extern "system" fn(this: *mut c_void, index: u32, ppIBitmapFrame: *mut *mut IWICBitmapFrameDecode) -> HRESULT,
}

#[repr(C)]
pub struct IWICBitmapDecoder {
    pub lpVtbl: *const IWICBitmapDecoderVtbl,
}

#[repr(C)]
pub struct IWICBitmapFrameDecodeVtbl {
    // IUnknown
    pub QueryInterface: unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, ppvObject: *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    // IWICBitmapSource
    pub GetSize: unsafe extern "system" fn(this: *mut c_void, puiWidth: *mut u32, puiHeight: *mut u32) -> HRESULT,
    pub GetPixelFormat: unsafe extern "system" fn(this: *mut c_void, pPixelFormat: *mut GUID) -> HRESULT,
    pub GetResolution: unsafe extern "system" fn(this: *mut c_void, pDpiX: *mut f64, pDpiY: *mut f64) -> HRESULT,
    pub CopyPalette: unsafe extern "system" fn(this: *mut c_void, pIPalette: *mut c_void) -> HRESULT,
    pub CopyPixels: unsafe extern "system" fn(
        this: *mut c_void,
        prc: *const WICRect,
        cbStride: u32,
        cbBufferSize: u32,
        pbBuffer: *mut u8,
    ) -> HRESULT,
    // IWICBitmapFrameDecode
    pub GetMetadataQueryReader: unsafe extern "system" fn(this: *mut c_void, ppIMetadataQueryReader: *mut *mut c_void) -> HRESULT,
    pub GetColorContexts: unsafe extern "system" fn(this: *mut c_void, cCount: u32, ppIColorContexts: *mut *mut c_void, pcActualCount: *mut u32) -> HRESULT,
    pub GetThumbnail: unsafe extern "system" fn(this: *mut c_void, ppIThumbnail: *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
pub struct IWICBitmapFrameDecode {
    pub lpVtbl: *const IWICBitmapFrameDecodeVtbl,
}

#[repr(C)]
pub struct IWICFormatConverterVtbl {
    // IUnknown
    pub QueryInterface: unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, ppvObject: *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    // IWICBitmapSource
    pub GetSize: unsafe extern "system" fn(this: *mut c_void, puiWidth: *mut u32, puiHeight: *mut u32) -> HRESULT,
    pub GetPixelFormat: unsafe extern "system" fn(this: *mut c_void, pPixelFormat: *mut GUID) -> HRESULT,
    pub GetResolution: unsafe extern "system" fn(this: *mut c_void, pDpiX: *mut f64, pDpiY: *mut f64) -> HRESULT,
    pub CopyPalette: unsafe extern "system" fn(this: *mut c_void, pIPalette: *mut c_void) -> HRESULT,
    pub CopyPixels: unsafe extern "system" fn(
        this: *mut c_void,
        prc: *const WICRect,
        cbStride: u32,
        cbBufferSize: u32,
        pbBuffer: *mut u8,
    ) -> HRESULT,
    // IWICFormatConverter
    pub Initialize: unsafe extern "system" fn(
        this: *mut c_void,
        pISource: *mut c_void,
        dstFormat: *const GUID,
        dither: u32,
        pIPalette: *mut c_void,
        alphaThresholdPercent: f64,
        paletteTranslate: u32,
    ) -> HRESULT,
    pub CanConvert: unsafe extern "system" fn(
        this: *mut c_void,
        srcPixelFormat: *const GUID,
        dstPixelFormat: *const GUID,
        pfCanConvert: *mut i32,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IWICFormatConverter {
    pub lpVtbl: *const IWICFormatConverterVtbl,
}

#[repr(C)]
pub struct IWICStreamVtbl {
    // IUnknown
    pub QueryInterface: unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, ppvObject: *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    // ISequentialStream
    pub Read: unsafe extern "system" fn(this: *mut c_void, pv: *mut c_void, cb: u32, pcbRead: *mut u32) -> HRESULT,
    pub Write: unsafe extern "system" fn(this: *mut c_void, pv: *const c_void, cb: u32, pcbWritten: *mut u32) -> HRESULT,
    // IStream
    pub Seek: unsafe extern "system" fn(this: *mut c_void, dlibMove: i64, dwOrigin: u32, plibNewPosition: *mut u64) -> HRESULT,
    pub SetSize: unsafe extern "system" fn(this: *mut c_void, libNewSize: u64) -> HRESULT,
    pub CopyTo: unsafe extern "system" fn(this: *mut c_void, pstm: *mut c_void, cb: u64, pcbRead: *mut u64, pcbWritten: *mut u64) -> HRESULT,
    pub Commit: unsafe extern "system" fn(this: *mut c_void, grfCommitFlags: u32) -> HRESULT,
    pub Revert: unsafe extern "system" fn(this: *mut c_void) -> HRESULT,
    pub LockRegion: unsafe extern "system" fn(this: *mut c_void, libOffset: u64, cb: u64, dwLockType: u32) -> HRESULT,
    pub UnlockRegion: unsafe extern "system" fn(this: *mut c_void, libOffset: u64, cb: u64, dwLockType: u32) -> HRESULT,
    pub Stat: unsafe extern "system" fn(this: *mut c_void, pstatstg: *mut c_void, grfStatFlag: u32) -> HRESULT,
    pub Clone: unsafe extern "system" fn(this: *mut c_void, ppstm: *mut *mut c_void) -> HRESULT,
    // IWICStream
    pub InitializeFromIStream: unsafe extern "system" fn(this: *mut c_void, pIStream: *mut c_void) -> HRESULT,
    pub InitializeFromFilename: unsafe extern "system" fn(this: *mut c_void, wzFileName: *const u16, dwAccessMode: u32) -> HRESULT,
    pub InitializeFromMemory: unsafe extern "system" fn(this: *mut c_void, pbBuffer: *mut u8, cbBufferSize: u32) -> HRESULT,
    pub InitializeFromIStreamRegion: unsafe extern "system" fn(this: *mut c_void, pIStream: *mut c_void, ulOffset: u64, ulMaxSize: u64) -> HRESULT,
}

#[repr(C)]
pub struct IWICStream {
    pub lpVtbl: *const IWICStreamVtbl,
}

// --- FFI External Declarations ---

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> HRESULT;
    fn CoUninitialize();
    fn CoCreateInstance(
        rclsid: *const GUID,
        pUnkOuter: *mut c_void,
        dwClsContext: u32,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT;
}

#[link(name = "windowscodecs")]
extern "system" {
    fn WICCreateImagingFactory_Proxy(
        SDKVersion: u32,
        ppIImagingFactory: *mut *mut IWICImagingFactory,
    ) -> HRESULT;
}

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hWndNewOwner: *mut c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn GetClipboardData(uFormat: u32) -> *mut c_void;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalLock(hMem: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(hMem: *mut c_void) -> i32;
    fn GlobalSize(hMem: *mut c_void) -> usize;
}

// --- Scoped RAII COM Helpers ---

struct ComGuard {
    need_uninit: bool,
}

impl ComGuard {
    fn init() -> Self {
        unsafe {
            // COINIT_MULTITHREADED = 0x0
            let hr = CoInitializeEx(ptr::null_mut(), 0);
            let need_uninit = hr == S_OK || hr == S_FALSE;
            Self { need_uninit }
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.need_uninit {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

struct ComPtr<T> {
    ptr: *mut T,
}

impl<T> ComPtr<T> {
    fn new(ptr: *mut T) -> Self {
        Self { ptr }
    }

    fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

impl<T> Drop for ComPtr<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                // In COM, Release is the 3rd function in the vtable (offset 2: QueryInterface, AddRef, Release)
                let vtable = *(self.ptr as *mut *mut usize);
                let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                    std::mem::transmute(*vtable.add(2));
                release_fn(self.ptr as *mut c_void);
            }
            self.ptr = ptr::null_mut();
        }
    }
}

// Creates an IWICImagingFactory using either CoCreateInstance or WICCreateImagingFactory_Proxy
unsafe fn create_imaging_factory() -> Result<ComPtr<IWICImagingFactory>, String> {
    let mut factory: *mut IWICImagingFactory = ptr::null_mut();

    let mut hr = CoCreateInstance(
        &CLSID_WICImagingFactory,
        ptr::null_mut(),
        CLSCTX_INPROC_SERVER,
        &IID_IWICImagingFactory,
        &mut factory as *mut *mut IWICImagingFactory as *mut *mut c_void,
    );

    if hr < 0 || factory.is_null() {
        hr = CoCreateInstance(
            &CLSID_WICImagingFactory2,
            ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IWICImagingFactory,
            &mut factory as *mut *mut IWICImagingFactory as *mut *mut c_void,
        );
    }

    if hr < 0 || factory.is_null() {
        hr = WICCreateImagingFactory_Proxy(WINCODEC_SDK_VERSION2, &mut factory);
        if hr < 0 || factory.is_null() {
            hr = WICCreateImagingFactory_Proxy(WINCODEC_SDK_VERSION1, &mut factory);
        }
    }

    if hr < 0 || factory.is_null() {
        return Err(format!("Failed to instantiate IWICImagingFactory (0x{:08X})", hr));
    }

    Ok(ComPtr::new(factory))
}

// Internal helper to decode a frame into 32bpp BGRA
unsafe fn decode_frame_to_bgra(
    factory: *mut IWICImagingFactory,
    frame: *mut IWICBitmapFrameDecode,
) -> Result<(u32, u32, Vec<u8>), String> {
    let mut converter: *mut IWICFormatConverter = ptr::null_mut();
    let hr = ((*(*factory).lpVtbl).CreateFormatConverter)(
        factory as *mut c_void,
        &mut converter,
    );
    if hr < 0 || converter.is_null() {
        return Err(format!("CreateFormatConverter failed (0x{:08X})", hr));
    }
    let converter = ComPtr::new(converter);

    let hr = ((*(*converter.as_ptr()).lpVtbl).Initialize)(
        converter.as_ptr() as *mut c_void,
        frame as *mut c_void,
        &GUID_WICPixelFormat32bppBGRA,
        WICBitmapDitherTypeNone,
        ptr::null_mut(),
        0.0,
        WICBitmapPaletteTypeCustom,
    );
    if hr < 0 {
        return Err(format!("IWICFormatConverter::Initialize failed (0x{:08X})", hr));
    }

    let mut width: u32 = 0;
    let mut height: u32 = 0;
    let hr = ((*(*converter.as_ptr()).lpVtbl).GetSize)(
        converter.as_ptr() as *mut c_void,
        &mut width,
        &mut height,
    );
    if hr < 0 {
        return Err(format!("IWICFormatConverter::GetSize failed (0x{:08X})", hr));
    }

    if width == 0 || height == 0 {
        return Err("Decoded image has zero dimensions".to_string());
    }

    let stride = width
        .checked_mul(4)
        .ok_or_else(|| "Image width causes overflow".to_string())?;
    let buffer_size = (stride as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| "Image dimensions cause buffer overflow".to_string())?;

    let mut buffer = vec![0u8; buffer_size];

    let hr = ((*(*converter.as_ptr()).lpVtbl).CopyPixels)(
        converter.as_ptr() as *mut c_void,
        ptr::null(),
        stride,
        buffer_size as u32,
        buffer.as_mut_ptr(),
    );
    if hr < 0 {
        return Err(format!("IWICFormatConverter::CopyPixels failed (0x{:08X})", hr));
    }

    Ok((width, height, buffer))
}

// --- Public WIC Functions ---

/// Decodes an image from a file path (PNG, JPEG, BMP, TIFF, GIF, etc.) into 32bpp BGRA raw bytes.
pub fn decode_image_to_bgra(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    if !path.exists() {
        return Err(format!("File does not exist: {:?}", path));
    }

    let _com = ComGuard::init();

    unsafe {
        let factory = create_imaging_factory()?;

        let mut path_u16: Vec<u16> = path.as_os_str().encode_wide().collect();
        path_u16.push(0);

        let mut decoder: *mut IWICBitmapDecoder = ptr::null_mut();
        let hr = ((*(*factory.as_ptr()).lpVtbl).CreateDecoderFromFilename)(
            factory.as_ptr() as *mut c_void,
            path_u16.as_ptr(),
            ptr::null(),
            GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
            &mut decoder,
        );
        if hr < 0 || decoder.is_null() {
            return Err(format!(
                "CreateDecoderFromFilename failed for {:?} (0x{:08X})",
                path, hr
            ));
        }
        let decoder = ComPtr::new(decoder);

        let mut frame: *mut IWICBitmapFrameDecode = ptr::null_mut();
        let hr = ((*(*decoder.as_ptr()).lpVtbl).GetFrame)(
            decoder.as_ptr() as *mut c_void,
            0,
            &mut frame,
        );
        if hr < 0 || frame.is_null() {
            return Err(format!("IWICBitmapDecoder::GetFrame(0) failed (0x{:08X})", hr));
        }
        let frame = ComPtr::new(frame);

        decode_frame_to_bgra(factory.as_ptr(), frame.as_ptr())
    }
}

/// Decodes an image from an in-memory buffer (PNG, JPEG, BMP, TIFF, GIF, etc.) into 32bpp BGRA raw bytes.
pub fn decode_image_memory_to_bgra(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if bytes.len() < 14 {
        return Err("Input byte buffer is too small to contain a valid image".to_string());
    }

    let _com = ComGuard::init();

    unsafe {
        let factory = create_imaging_factory()?;

        let mut stream: *mut IWICStream = ptr::null_mut();
        let hr = ((*(*factory.as_ptr()).lpVtbl).CreateStream)(
            factory.as_ptr() as *mut c_void,
            &mut stream,
        );
        if hr < 0 || stream.is_null() {
            return Err(format!("CreateStream failed (0x{:08X})", hr));
        }
        let stream = ComPtr::new(stream);

        let hr = ((*(*stream.as_ptr()).lpVtbl).InitializeFromMemory)(
            stream.as_ptr() as *mut c_void,
            bytes.as_ptr() as *mut u8,
            bytes.len() as u32,
        );
        if hr < 0 {
            return Err(format!("IWICStream::InitializeFromMemory failed (0x{:08X})", hr));
        }

        let mut decoder: *mut IWICBitmapDecoder = ptr::null_mut();
        let hr = ((*(*factory.as_ptr()).lpVtbl).CreateDecoderFromStream)(
            factory.as_ptr() as *mut c_void,
            stream.as_ptr() as *mut c_void,
            ptr::null(),
            WICDecodeMetadataCacheOnDemand,
            &mut decoder,
        );
        if hr < 0 || decoder.is_null() {
            return Err(format!("CreateDecoderFromStream failed (0x{:08X})", hr));
        }
        let decoder = ComPtr::new(decoder);

        let mut frame: *mut IWICBitmapFrameDecode = ptr::null_mut();
        let hr = ((*(*decoder.as_ptr()).lpVtbl).GetFrame)(
            decoder.as_ptr() as *mut c_void,
            0,
            &mut frame,
        );
        if hr < 0 || frame.is_null() {
            return Err(format!("IWICBitmapDecoder::GetFrame(0) failed (0x{:08X})", hr));
        }
        let frame = ComPtr::new(frame);

        decode_frame_to_bgra(factory.as_ptr(), frame.as_ptr())
    }
}

// --- Native DIB Parser & Clipboard Extraction ---

/// Parses a Device Independent Bitmap (DIB / CF_DIB) byte buffer into 32bpp BGRA format.
/// Handles bottom-up vs top-down, 32bpp, 24bpp, 8bpp indexed, and automatic 0-alpha fixup.
pub fn parse_dib_to_bgra(dib: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if dib.len() < 40 {
        return None;
    }

    let bi_size = u32::from_le_bytes(dib[0..4].try_into().ok()?);
    if bi_size < 40 || (bi_size as usize) > dib.len() {
        return None;
    }

    let bi_width = i32::from_le_bytes(dib[4..8].try_into().ok()?);
    let bi_height = i32::from_le_bytes(dib[8..12].try_into().ok()?);
    let bi_bit_count = u16::from_le_bytes(dib[14..16].try_into().ok()?);
    let bi_compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);
    let bi_clr_used = u32::from_le_bytes(dib[32..36].try_into().ok()?);

    let width = bi_width.unsigned_abs();
    let height = bi_height.unsigned_abs();
    if width == 0 || height == 0 {
        return None;
    }

    let is_bottom_up = bi_height > 0;

    let pixel_offset = if bi_compression == 3 {
        // BI_BITFIELDS
        if bi_size == 40 {
            40 + 12 + (bi_clr_used as usize * 4)
        } else {
            bi_size as usize + (bi_clr_used as usize * 4)
        }
    } else {
        let num_colors = if bi_clr_used > 0 {
            bi_clr_used as usize
        } else if bi_bit_count <= 8 {
            1 << bi_bit_count
        } else {
            0
        };
        bi_size as usize + num_colors * 4
    };

    if pixel_offset > dib.len() {
        return None;
    }

    let total_pixels = (width as usize).checked_mul(height as usize)?;
    let total_bgra_bytes = total_pixels.checked_mul(4)?;

    // Fast path: 32bpp uncompressed or bitfields
    if bi_bit_count == 32 && (bi_compression == 0 || bi_compression == 3) {
        let row_stride = (width as usize) * 4;
        let required_bytes = row_stride.checked_mul(height as usize)?;
        if dib.len() < pixel_offset + required_bytes {
            return None;
        }

        // Check if any pixel has non-zero alpha
        let mut has_non_zero_alpha = false;
        for y in 0..height as usize {
            let row_start = pixel_offset + y * row_stride;
            for x in 0..width as usize {
                if dib[row_start + x * 4 + 3] > 0 {
                    has_non_zero_alpha = true;
                    break;
                }
            }
            if has_non_zero_alpha {
                break;
            }
        }

        let mut bgra = vec![0u8; total_bgra_bytes];
        for y in 0..height as usize {
            let src_y = if is_bottom_up {
                height as usize - 1 - y
            } else {
                y
            };
            let src_row = pixel_offset + src_y * row_stride;
            let dst_row = y * ((width as usize) * 4);

            for x in 0..width as usize {
                let b = dib[src_row + x * 4];
                let g = dib[src_row + x * 4 + 1];
                let r = dib[src_row + x * 4 + 2];
                let a = if has_non_zero_alpha {
                    dib[src_row + x * 4 + 3]
                } else {
                    255
                };
                let dst_idx = dst_row + x * 4;
                bgra[dst_idx] = b;
                bgra[dst_idx + 1] = g;
                bgra[dst_idx + 2] = r;
                bgra[dst_idx + 3] = a;
            }
        }

        return Some((width, height, bgra));
    }

    // Fast path: 24bpp uncompressed
    if bi_bit_count == 24 && bi_compression == 0 {
        let row_stride = ((width as usize * 3 + 3) / 4) * 4;
        let required_bytes = row_stride.checked_mul(height as usize)?;
        if dib.len() < pixel_offset + required_bytes {
            return None;
        }

        let mut bgra = vec![0u8; total_bgra_bytes];
        for y in 0..height as usize {
            let src_y = if is_bottom_up {
                height as usize - 1 - y
            } else {
                y
            };
            let src_row = pixel_offset + src_y * row_stride;
            let dst_row = y * ((width as usize) * 4);

            for x in 0..width as usize {
                let b = dib[src_row + x * 3];
                let g = dib[src_row + x * 3 + 1];
                let r = dib[src_row + x * 3 + 2];
                let dst_idx = dst_row + x * 4;
                bgra[dst_idx] = b;
                bgra[dst_idx + 1] = g;
                bgra[dst_idx + 2] = r;
                bgra[dst_idx + 3] = 255;
            }
        }

        return Some((width, height, bgra));
    }

    // Fast path: 8bpp indexed with palette
    if bi_bit_count == 8 && bi_compression == 0 {
        let row_stride = ((width as usize + 3) / 4) * 4;
        let required_bytes = row_stride.checked_mul(height as usize)?;
        if dib.len() < pixel_offset + required_bytes {
            return None;
        }

        let mut bgra = vec![0u8; total_bgra_bytes];
        for y in 0..height as usize {
            let src_y = if is_bottom_up {
                height as usize - 1 - y
            } else {
                y
            };
            let src_row = pixel_offset + src_y * row_stride;
            let dst_row = y * ((width as usize) * 4);

            for x in 0..width as usize {
                let idx = dib[src_row + x] as usize;
                let entry = (bi_size as usize) + idx * 4;
                if entry + 3 <= dib.len() {
                    let b = dib[entry];
                    let g = dib[entry + 1];
                    let r = dib[entry + 2];
                    let dst_idx = dst_row + x * 4;
                    bgra[dst_idx] = b;
                    bgra[dst_idx + 1] = g;
                    bgra[dst_idx + 2] = r;
                    bgra[dst_idx + 3] = 255;
                }
            }
        }

        return Some((width, height, bgra));
    }

    // General fallback: prepend 14-byte BITMAPFILEHEADER and decode via WIC memory decoder
    let mut bmp_data = Vec::with_capacity(14 + dib.len());
    bmp_data.extend_from_slice(b"BM");
    let total_file_size = (14 + dib.len()) as u32;
    bmp_data.extend_from_slice(&total_file_size.to_le_bytes());
    bmp_data.extend_from_slice(&[0, 0, 0, 0]); // Reserved
    let off_bits = (14 + pixel_offset) as u32;
    bmp_data.extend_from_slice(&off_bits.to_le_bytes());
    bmp_data.extend_from_slice(dib);

    decode_image_memory_to_bgra(&bmp_data).ok()
}

/// Inspects the Windows Clipboard for `CF_DIB` image data and extracts it into 32bpp BGRA raw bytes.
pub fn get_clipboard_image_bgra() -> Option<(u32, u32, Vec<u8>)> {
    unsafe {
        if OpenClipboard(ptr::null_mut()) == 0 {
            return None;
        }
    }

    struct ClipboardGuard;
    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }
    let _clip_guard = ClipboardGuard;

    let h_data = unsafe { GetClipboardData(CF_DIB) };
    if h_data.is_null() {
        return None;
    }

    let ptr_data = unsafe { GlobalLock(h_data) };
    if ptr_data.is_null() {
        return None;
    }

    let total_size = unsafe { GlobalSize(h_data) };

    struct LockGuard(*mut c_void);
    impl Drop for LockGuard {
        fn drop(&mut self) {
            unsafe {
                GlobalUnlock(self.0);
            }
        }
    }
    let _lock_guard = LockGuard(h_data);

    if total_size < 40 {
        return None;
    }

    let slice = unsafe { std::slice::from_raw_parts(ptr_data as *const u8, total_size) };
    parse_dib_to_bgra(slice)
}

// --- Unit Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructs a valid 2x2 24bpp BMP byte array
    fn build_test_2x2_bmp() -> Vec<u8> {
        let mut data = Vec::new();
        // BITMAPFILEHEADER (14 bytes)
        data.extend_from_slice(b"BM");
        let file_size: u32 = 14 + 40 + 16; // 70 bytes
        data.extend_from_slice(&file_size.to_le_bytes());
        data.extend_from_slice(&[0, 0, 0, 0]);
        let off_bits: u32 = 54;
        data.extend_from_slice(&off_bits.to_le_bytes());

        // BITMAPINFOHEADER (40 bytes)
        data.extend_from_slice(&40u32.to_le_bytes()); // biSize
        data.extend_from_slice(&2i32.to_le_bytes()); // biWidth = 2
        data.extend_from_slice(&2i32.to_le_bytes()); // biHeight = 2 (bottom-up)
        data.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
        data.extend_from_slice(&24u16.to_le_bytes()); // biBitCount = 24
        data.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
        data.extend_from_slice(&16u32.to_le_bytes()); // biSizeImage
        data.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
        data.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
        data.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
        data.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

        // Row 1 (bottom row, y=1 in top-down):
        // Pixel (0,1): Red (BGR: 0, 0, 255)
        // Pixel (1,1): Green (BGR: 0, 255, 0)
        // 2 bytes padding to align to 4-byte boundary
        data.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);

        // Row 0 (top row, y=0 in top-down):
        // Pixel (0,0): Blue (BGR: 255, 0, 0)
        // Pixel (1,0): White (BGR: 255, 255, 255)
        // 2 bytes padding
        data.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]);

        data
    }

    #[test]
    fn test_wic_decode_memory_valid_bmp() {
        let bmp_bytes = build_test_2x2_bmp();
        let result = decode_image_memory_to_bgra(&bmp_bytes);
        assert!(result.is_ok(), "WIC decode_image_memory_to_bgra should succeed: {:?}", result.err());

        let (width, height, bgra) = result.unwrap();
        assert_eq!(width, 2);
        assert_eq!(height, 2);
        assert_eq!(bgra.len(), 16);

        // Pixel (0,0): Blue -> [B, G, R, A] = [255, 0, 0, 255]
        assert_eq!(&bgra[0..4], &[255, 0, 0, 255]);
        // Pixel (1,0): White -> [255, 255, 255, 255]
        assert_eq!(&bgra[4..8], &[255, 255, 255, 255]);
        // Pixel (0,1): Red -> [0, 0, 255, 255]
        assert_eq!(&bgra[8..12], &[0, 0, 255, 255]);
        // Pixel (1,1): Green -> [0, 255, 0, 255]
        assert_eq!(&bgra[12..16], &[0, 255, 0, 255]);
    }

    #[test]
    fn test_wic_decode_file_valid_bmp() {
        let bmp_bytes = build_test_2x2_bmp();
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("zpdf_test_sample.bmp");
        std::fs::write(&temp_file, bmp_bytes).expect("Failed to write temp bmp file");

        let result = decode_image_to_bgra(&temp_file);
        let _ = std::fs::remove_file(&temp_file);

        assert!(result.is_ok(), "WIC decode_image_to_bgra should succeed: {:?}", result.err());
        let (width, height, bgra) = result.unwrap();
        assert_eq!(width, 2);
        assert_eq!(height, 2);
        assert_eq!(bgra.len(), 16);
    }

    #[test]
    fn test_wic_decode_empty_and_invalid() {
        assert!(decode_image_memory_to_bgra(&[]).is_err());
        assert!(decode_image_memory_to_bgra(&[0x01, 0x02, 0x03]).is_err());
        assert!(decode_image_to_bgra(Path::new("C:\\non_existent_zpdf_image_123.png")).is_err());
    }

    #[test]
    fn test_parse_dib_24bpp() {
        let bmp_bytes = build_test_2x2_bmp();
        // DIB data starts right after 14-byte BITMAPFILEHEADER
        let dib_bytes = &bmp_bytes[14..];

        let result = parse_dib_to_bgra(dib_bytes);
        assert!(result.is_some());
        let (width, height, bgra) = result.unwrap();
        assert_eq!(width, 2);
        assert_eq!(height, 2);
        assert_eq!(bgra.len(), 16);

        // Pixel (0,0): Blue
        assert_eq!(&bgra[0..4], &[255, 0, 0, 255]);
        // Pixel (1,0): White
        assert_eq!(&bgra[4..8], &[255, 255, 255, 255]);
        // Pixel (0,1): Red
        assert_eq!(&bgra[8..12], &[0, 0, 255, 255]);
        // Pixel (1,1): Green
        assert_eq!(&bgra[12..16], &[0, 255, 0, 255]);
    }

    #[test]
    fn test_parse_dib_32bpp_zero_alpha_fixup() {
        let mut dib = Vec::new();
        // BITMAPINFOHEADER (40 bytes)
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes()); // 1x1
        dib.extend_from_slice(&1i32.to_le_bytes()); // bottom-up
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes()); // 32bpp
        dib.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
        dib.extend_from_slice(&4u32.to_le_bytes());
        dib.extend_from_slice(&0i32.to_le_bytes());
        dib.extend_from_slice(&0i32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());

        // 1 pixel with alpha = 0 (classic RGBX clipboard format)
        dib.extend_from_slice(&[10, 20, 30, 0]);

        let result = parse_dib_to_bgra(&dib);
        assert!(result.is_some());
        let (width, height, bgra) = result.unwrap();
        assert_eq!(width, 1);
        assert_eq!(height, 1);
        // Alpha should be fixed up to 255!
        assert_eq!(&bgra, &[10, 20, 30, 255]);
    }

    #[test]
    fn test_parse_dib_32bpp_preserve_existing_alpha() {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&4u32.to_le_bytes());
        dib.extend_from_slice(&0i32.to_le_bytes());
        dib.extend_from_slice(&0i32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());

        // 1 pixel with explicit alpha = 128
        dib.extend_from_slice(&[10, 20, 30, 128]);

        let result = parse_dib_to_bgra(&dib);
        assert!(result.is_some());
        let (_width, _height, bgra) = result.unwrap();
        assert_eq!(&bgra, &[10, 20, 30, 128]);
    }

    #[test]
    fn test_clipboard_image_does_not_panic() {
        // Calling get_clipboard_image_bgra should return Option without panicking
        let _ = get_clipboard_image_bgra();
    }
}
