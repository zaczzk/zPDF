// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: PDFium Safe RAII FFI & Dynamic Function Table

#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::{c_char, c_int, c_void, CString};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::ptr;
use std::sync::Arc;

// Win32 API handles for dynamic loading
type HMODULE = *mut c_void;
type FARPROC = *mut c_void;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryExW(lp_lib_file_name: *const u16, h_file: *mut c_void, dw_flags: u32) -> HMODULE;
    fn GetProcAddress(h_module: HMODULE, lp_proc_name: *const c_char) -> FARPROC;
    fn FreeLibrary(h_module: HMODULE) -> i32;
}

const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x00000008;

// PDFium Types & Constants
pub type FPDF_DOCUMENT = *mut c_void;
pub type FPDF_PAGE = *mut c_void;
pub type FPDF_TEXTPAGE = *mut c_void;
pub type FPDF_BITMAP = *mut c_void;
pub type FPDF_PAGEOBJECT = *mut c_void;
pub type FPDF_SCHHANDLE = *mut c_void;
pub type FPDF_FONT = *mut c_void;

pub const FPDF_PAGEOBJ_UNKNOWN: c_int = 0;
pub const FPDF_PAGEOBJ_TEXT: c_int = 1;
pub const FPDF_PAGEOBJ_PATH: c_int = 2;
pub const FPDF_PAGEOBJ_IMAGE: c_int = 3;
pub const FPDF_PAGEOBJ_SHADING: c_int = 4;
pub const FPDF_PAGEOBJ_FORMS: c_int = 5;

// Bitmap formats
pub const FPDFBitmap_Gray: c_int = 1;
pub const FPDFBitmap_BGR: c_int = 2;
pub const FPDFBitmap_BGRx: c_int = 3;
pub const FPDFBitmap_BGRA: c_int = 4;

// Render flags
pub const FPDF_ANNOT: c_int = 0x01;
pub const FPDF_LCD_TEXT: c_int = 0x02;
pub const FPDF_NO_NATIVETEXT: c_int = 0x04;
pub const FPDF_GRAYSCALE: c_int = 0x08;
pub const FPDF_RENDER_FORCEHALFTONE: c_int = 0x20;
pub const FPDF_PRINTING: c_int = 0x800;
pub const FPDF_RENDER_NO_SMOOTHTEXT: c_int = 0x1000;
pub const FPDF_RENDER_NO_SMOOTHIMAGE: c_int = 0x2000;
pub const FPDF_RENDER_NO_SMOOTHLINE: c_int = 0x4000;

#[repr(C)]
pub struct FPDF_FILEWRITE {
    pub version: c_int,
    pub write_block: unsafe extern "C" fn(
        p_this: *mut FPDF_FILEWRITE,
        p_data: *const c_void,
        size: usize,
    ) -> c_int,
}

#[repr(C)]
pub struct FPDF_LIBRARY_CONFIG {
    pub version: c_int,
    pub m_p_user_font_paths: *mut *const c_char,
    pub m_p_isolate: *mut c_void,
    pub m_v8_embedder_slot: c_uint,
}
type c_uint = u32;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectF {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RectF {
    pub fn width(&self) -> f32 {
        (self.right - self.left).abs()
    }

    pub fn height(&self) -> f32 {
        (self.top - self.bottom).abs()
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        let min_x = self.left.min(self.right);
        let max_x = self.left.max(self.right);
        let min_y = self.bottom.min(self.top);
        let max_y = self.bottom.max(self.top);
        x >= min_x && x <= max_x && y >= min_y && y <= max_y
    }
}

// Dynamic Function Table Struct
pub struct PdfiumApi {
    pub FPDF_InitLibraryWithConfig: unsafe extern "C" fn(*const FPDF_LIBRARY_CONFIG),
    pub FPDF_DestroyLibrary: unsafe extern "C" fn(),
    pub FPDF_LoadDocument: unsafe extern "C" fn(*const c_char, *const c_char) -> FPDF_DOCUMENT,
    pub FPDF_CreateNewDocument: unsafe extern "C" fn() -> FPDF_DOCUMENT,
    pub FPDF_CloseDocument: unsafe extern "C" fn(FPDF_DOCUMENT),
    pub FPDF_GetPageCount: unsafe extern "C" fn(FPDF_DOCUMENT) -> c_int,
    pub FPDF_LoadPage: unsafe extern "C" fn(FPDF_DOCUMENT, c_int) -> FPDF_PAGE,
    pub FPDF_ClosePage: unsafe extern "C" fn(FPDF_PAGE),
    pub FPDF_GetPageWidthF: Option<unsafe extern "C" fn(FPDF_PAGE) -> f32>,
    pub FPDF_GetPageHeightF: Option<unsafe extern "C" fn(FPDF_PAGE) -> f32>,
    pub FPDF_GetPageWidth: unsafe extern "C" fn(FPDF_PAGE) -> f64,
    pub FPDF_GetPageHeight: unsafe extern "C" fn(FPDF_PAGE) -> f64,
    pub FPDF_GetPageRotation: unsafe extern "C" fn(FPDF_PAGE) -> c_int,
    pub FPDFPage_SetRotation: unsafe extern "C" fn(FPDF_PAGE, c_int),
    pub FPDFPage_Delete: unsafe extern "C" fn(FPDF_DOCUMENT, c_int),
    pub FPDFPage_New: unsafe extern "C" fn(FPDF_DOCUMENT, c_int, f64, f64) -> FPDF_PAGE,
    pub FPDFPage_GenerateContent: unsafe extern "C" fn(FPDF_PAGE) -> c_int,

    // Bitmap rendering
    pub FPDFBitmap_CreateEx: unsafe extern "C" fn(c_int, c_int, c_int, *mut c_void, c_int) -> FPDF_BITMAP,
    pub FPDFBitmap_Destroy: unsafe extern "C" fn(FPDF_BITMAP),
    pub FPDFBitmap_FillRect: unsafe extern "C" fn(FPDF_BITMAP, c_int, c_int, c_int, c_int, u32),
    pub FPDFBitmap_GetBuffer: unsafe extern "C" fn(FPDF_BITMAP) -> *mut u8,
    pub FPDFBitmap_GetWidth: unsafe extern "C" fn(FPDF_BITMAP) -> c_int,
    pub FPDFBitmap_GetHeight: unsafe extern "C" fn(FPDF_BITMAP) -> c_int,
    pub FPDFBitmap_GetStride: unsafe extern "C" fn(FPDF_BITMAP) -> c_int,
    pub FPDF_RenderPageBitmap: unsafe extern "C" fn(FPDF_BITMAP, FPDF_PAGE, c_int, c_int, c_int, c_int, c_int, c_int),

    // Text Page & Search
    pub FPDFText_LoadPage: unsafe extern "C" fn(FPDF_PAGE) -> FPDF_TEXTPAGE,
    pub FPDFText_ClosePage: unsafe extern "C" fn(FPDF_TEXTPAGE),
    pub FPDFText_CountChars: unsafe extern "C" fn(FPDF_TEXTPAGE) -> c_int,
    pub FPDFText_GetUnicode: unsafe extern "C" fn(FPDF_TEXTPAGE, c_int) -> c_uint,
    pub FPDFText_GetCharBox: unsafe extern "C" fn(FPDF_TEXTPAGE, c_int, *mut f64, *mut f64, *mut f64, *mut f64) -> c_int,
    pub FPDFText_GetCharIndexAtPos: unsafe extern "C" fn(FPDF_TEXTPAGE, f64, f64, f64, f64) -> c_int,
    pub FPDFText_GetText: unsafe extern "C" fn(FPDF_TEXTPAGE, c_int, c_int, *mut u16) -> c_int,
    pub FPDFText_FindStart: unsafe extern "C" fn(FPDF_TEXTPAGE, *const u16, c_uint, c_int) -> FPDF_SCHHANDLE,
    pub FPDFText_FindNext: unsafe extern "C" fn(FPDF_SCHHANDLE) -> c_int,
    pub FPDFText_GetSchIdx: unsafe extern "C" fn(FPDF_SCHHANDLE) -> c_int,
    pub FPDFText_GetSchResultCnt: unsafe extern "C" fn(FPDF_SCHHANDLE) -> c_int,
    pub FPDFText_FindClose: unsafe extern "C" fn(FPDF_SCHHANDLE),

    // Page Objects & Editing
    pub FPDFPage_CountObjects: unsafe extern "C" fn(FPDF_PAGE) -> c_int,
    pub FPDFPage_GetObject: unsafe extern "C" fn(FPDF_PAGE, c_int) -> FPDF_PAGEOBJECT,
    pub FPDFPageObj_GetType: unsafe extern "C" fn(FPDF_PAGEOBJECT) -> c_int,
    pub FPDFPageObj_GetBounds: unsafe extern "C" fn(FPDF_PAGEOBJECT, *mut f32, *mut f32, *mut f32, *mut f32) -> c_int,
    pub FPDFTextObj_GetText: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, FPDF_TEXTPAGE, *mut u16, c_uint) -> c_uint>,
    pub FPDFTextObj_GetFontSize: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, *mut f32) -> c_int>,
    pub FPDFText_SetText: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, *const u16) -> c_int>,

    // Saving
    pub FPDF_SaveAsCopy: unsafe extern "C" fn(FPDF_DOCUMENT, *mut FPDF_FILEWRITE, c_int) -> c_int,
}

pub struct PdfiumLibrary {
    module: HMODULE,
    pub api: Arc<PdfiumApi>,
}

unsafe impl Send for PdfiumLibrary {}
unsafe impl Sync for PdfiumLibrary {}

impl PdfiumLibrary {
    pub fn load(dll_path: &Path) -> Result<Self, String> {
        let path_str: Vec<u16> = dll_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let module = unsafe { LoadLibraryExW(path_str.as_ptr(), ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
        if module.is_null() {
            return Err(format!(
                "Failed to load PDFium DLL from: {} (Win32 Error: {})",
                dll_path.display(),
                std::io::Error::last_os_error()
            ));
        }

        unsafe {
            macro_rules! resolve {
                ($sym:ident) => {{
                    let c_name = CString::new(stringify!($sym)).unwrap();
                    let proc = GetProcAddress(module, c_name.as_ptr());
                    if proc.is_null() {
                        FreeLibrary(module);
                        return Err(format!("Missing required PDFium export: {}", stringify!($sym)));
                    }
                    std::mem::transmute(proc)
                }};
            }

            macro_rules! resolve_opt {
                ($sym:ident) => {{
                    let c_name = CString::new(stringify!($sym)).unwrap();
                    let proc = GetProcAddress(module, c_name.as_ptr());
                    if proc.is_null() {
                        None
                    } else {
                        Some(std::mem::transmute(proc))
                    }
                }};
            }

            let api = PdfiumApi {
                FPDF_InitLibraryWithConfig: resolve!(FPDF_InitLibraryWithConfig),
                FPDF_DestroyLibrary: resolve!(FPDF_DestroyLibrary),
                FPDF_LoadDocument: resolve!(FPDF_LoadDocument),
                FPDF_CreateNewDocument: resolve!(FPDF_CreateNewDocument),
                FPDF_CloseDocument: resolve!(FPDF_CloseDocument),
                FPDF_GetPageCount: resolve!(FPDF_GetPageCount),
                FPDF_LoadPage: resolve!(FPDF_LoadPage),
                FPDF_ClosePage: resolve!(FPDF_ClosePage),
                FPDF_GetPageWidthF: resolve_opt!(FPDF_GetPageWidthF),
                FPDF_GetPageHeightF: resolve_opt!(FPDF_GetPageHeightF),
                FPDF_GetPageWidth: resolve!(FPDF_GetPageWidth),
                FPDF_GetPageHeight: resolve!(FPDF_GetPageHeight),
                FPDF_GetPageRotation: resolve!(FPDF_GetPageRotation),
                FPDFPage_SetRotation: resolve!(FPDFPage_SetRotation),
                FPDFPage_Delete: resolve!(FPDFPage_Delete),
                FPDFPage_New: resolve!(FPDFPage_New),
                FPDFPage_GenerateContent: resolve!(FPDFPage_GenerateContent),

                FPDFBitmap_CreateEx: resolve!(FPDFBitmap_CreateEx),
                FPDFBitmap_Destroy: resolve!(FPDFBitmap_Destroy),
                FPDFBitmap_FillRect: resolve!(FPDFBitmap_FillRect),
                FPDFBitmap_GetBuffer: resolve!(FPDFBitmap_GetBuffer),
                FPDFBitmap_GetWidth: resolve!(FPDFBitmap_GetWidth),
                FPDFBitmap_GetHeight: resolve!(FPDFBitmap_GetHeight),
                FPDFBitmap_GetStride: resolve!(FPDFBitmap_GetStride),
                FPDF_RenderPageBitmap: resolve!(FPDF_RenderPageBitmap),

                FPDFText_LoadPage: resolve!(FPDFText_LoadPage),
                FPDFText_ClosePage: resolve!(FPDFText_ClosePage),
                FPDFText_CountChars: resolve!(FPDFText_CountChars),
                FPDFText_GetUnicode: resolve!(FPDFText_GetUnicode),
                FPDFText_GetCharBox: resolve!(FPDFText_GetCharBox),
                FPDFText_GetCharIndexAtPos: resolve!(FPDFText_GetCharIndexAtPos),
                FPDFText_GetText: resolve!(FPDFText_GetText),
                FPDFText_FindStart: resolve!(FPDFText_FindStart),
                FPDFText_FindNext: resolve!(FPDFText_FindNext),
                FPDFText_GetSchIdx: resolve!(FPDFText_GetSchIdx),
                FPDFText_GetSchResultCnt: resolve!(FPDFText_GetSchResultCnt),
                FPDFText_FindClose: resolve!(FPDFText_FindClose),

                FPDFPage_CountObjects: resolve!(FPDFPage_CountObjects),
                FPDFPage_GetObject: resolve!(FPDFPage_GetObject),
                FPDFPageObj_GetType: resolve!(FPDFPageObj_GetType),
                FPDFPageObj_GetBounds: resolve!(FPDFPageObj_GetBounds),
                FPDFTextObj_GetText: resolve_opt!(FPDFTextObj_GetText),
                FPDFTextObj_GetFontSize: resolve_opt!(FPDFTextObj_GetFontSize),
                FPDFText_SetText: resolve_opt!(FPDFText_SetText),

                FPDF_SaveAsCopy: resolve!(FPDF_SaveAsCopy),
            };

            let config = FPDF_LIBRARY_CONFIG {
                version: 2,
                m_p_user_font_paths: ptr::null_mut(),
                m_p_isolate: ptr::null_mut(),
                m_v8_embedder_slot: 0,
            };
            (api.FPDF_InitLibraryWithConfig)(&config);

            Ok(Self {
                module,
                api: Arc::new(api),
            })
        }
    }

    pub fn open_document(&self, path: &Path, password: Option<&str>) -> Result<PdfDocument, String> {
        let path_str = path.to_str().ok_or_else(|| "Invalid UTF-8 path".to_string())?;
        let c_path = CString::new(path_str).map_err(|e| e.to_string())?;
        let c_pass = match password {
            Some(p) => Some(CString::new(p).map_err(|e| e.to_string())?),
            None => None,
        };
        let pass_ptr = c_pass.as_ref().map_or(ptr::null(), |p| p.as_ptr());

        let doc = unsafe { (self.api.FPDF_LoadDocument)(c_path.as_ptr(), pass_ptr) };
        if doc.is_null() {
            return Err(format!("PDFium failed to open document: {}", path.display()));
        }

        Ok(PdfDocument {
            raw: doc,
            api: Arc::clone(&self.api),
        })
    }

    pub fn create_empty_document(&self) -> Result<PdfDocument, String> {
        let doc = unsafe { (self.api.FPDF_CreateNewDocument)() };
        if doc.is_null() {
            return Err("Failed to create new PDF document".to_string());
        }
        Ok(PdfDocument {
            raw: doc,
            api: Arc::clone(&self.api),
        })
    }
}

impl Drop for PdfiumLibrary {
    fn drop(&mut self) {
        unsafe {
            (self.api.FPDF_DestroyLibrary)();
            FreeLibrary(self.module);
        }
    }
}

// Helper trait to encode wide strings from standard library
use std::os::windows::ffi::OsStrExt;

// Safe Document RAII
pub struct PdfDocument {
    pub raw: FPDF_DOCUMENT,
    api: Arc<PdfiumApi>,
}

unsafe impl Send for PdfDocument {}
unsafe impl Sync for PdfDocument {}

impl Drop for PdfDocument {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                (self.api.FPDF_CloseDocument)(self.raw);
            }
            self.raw = ptr::null_mut();
        }
    }
}

impl PdfDocument {
    pub fn page_count(&self) -> usize {
        let count = unsafe { (self.api.FPDF_GetPageCount)(self.raw) };
        if count < 0 {
            0
        } else {
            count as usize
        }
    }

    pub fn load_page(&self, page_index: usize) -> Result<PdfPage, String> {
        let page = unsafe { (self.api.FPDF_LoadPage)(self.raw, page_index as c_int) };
        if page.is_null() {
            return Err(format!("Failed to load page index {}", page_index));
        }
        Ok(PdfPage {
            raw: page,
            api: Arc::clone(&self.api),
            index: page_index,
        })
    }

    pub fn new_page(&self, index: usize, width: f64, height: f64) -> Result<PdfPage, String> {
        let page = unsafe { (self.api.FPDFPage_New)(self.raw, index as c_int, width, height) };
        if page.is_null() {
            return Err("Failed to create new page in document".to_string());
        }
        Ok(PdfPage {
            raw: page,
            api: Arc::clone(&self.api),
            index,
        })
    }

    pub fn delete_page(&self, page_index: usize) -> bool {
        if page_index < self.page_count() {
            unsafe { (self.api.FPDFPage_Delete)(self.raw, page_index as c_int) };
            true
        } else {
            false
        }
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let bytes = self.save_to_bytes()?;
        let mut file = File::create(path).map_err(|e| format!("Failed to create output file: {}", e))?;
        file.write_all(&bytes).map_err(|e| format!("Failed to write output file: {}", e))?;
        Ok(())
    }

    pub fn save_to_bytes(&self) -> Result<Vec<u8>, String> {
        struct StreamWriter {
            writer: FPDF_FILEWRITE,
            buffer: Vec<u8>,
        }

        unsafe extern "C" fn write_callback(
            p_this: *mut FPDF_FILEWRITE,
            p_data: *const c_void,
            size: usize,
        ) -> c_int {
            let stream = &mut *(p_this as *mut StreamWriter);
            let slice = std::slice::from_raw_parts(p_data as *const u8, size);
            stream.buffer.extend_from_slice(slice);
            1 // success
        }

        let mut stream = StreamWriter {
            writer: FPDF_FILEWRITE {
                version: 1,
                write_block: write_callback,
            },
            buffer: Vec::with_capacity(64 * 1024),
        };

        let ok = unsafe {
            (self.api.FPDF_SaveAsCopy)(
                self.raw,
                &mut stream.writer as *mut FPDF_FILEWRITE,
                0x01, // FPDF_INCREMENTAL or 0
            )
        };

        if ok != 1 {
            return Err("FPDF_SaveAsCopy failed to export PDF stream".to_string());
        }

        Ok(stream.buffer)
    }
}

// Safe Page RAII
pub struct PdfPage {
    pub raw: FPDF_PAGE,
    api: Arc<PdfiumApi>,
    pub index: usize,
}

unsafe impl Send for PdfPage {}
unsafe impl Sync for PdfPage {}

impl Drop for PdfPage {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                (self.api.FPDF_ClosePage)(self.raw);
            }
            self.raw = ptr::null_mut();
        }
    }
}

impl PdfPage {
    pub fn dimensions(&self) -> (f64, f64) {
        if let (Some(w_fn), Some(h_fn)) = (self.api.FPDF_GetPageWidthF, self.api.FPDF_GetPageHeightF) {
            let w = unsafe { w_fn(self.raw) } as f64;
            let h = unsafe { h_fn(self.raw) } as f64;
            (w, h)
        } else {
            let w = unsafe { (self.api.FPDF_GetPageWidth)(self.raw) };
            let h = unsafe { (self.api.FPDF_GetPageHeight)(self.raw) };
            (w, h)
        }
    }

    pub fn rotation(&self) -> i32 {
        unsafe { (self.api.FPDF_GetPageRotation)(self.raw) }
    }

    pub fn set_rotation(&self, rotation: i32) {
        let norm = (rotation % 4 + 4) % 4;
        unsafe { (self.api.FPDFPage_SetRotation)(self.raw, norm) };
    }

    pub fn rotate_clockwise(&self) {
        let current = self.rotation();
        self.set_rotation((current + 1) % 4);
    }

    pub fn generate_content(&self) -> bool {
        let ret = unsafe { (self.api.FPDFPage_GenerateContent)(self.raw) };
        ret != 0
    }

    pub fn load_text_page(&self) -> Result<PdfTextPage, String> {
        let tp = unsafe { (self.api.FPDFText_LoadPage)(self.raw) };
        if tp.is_null() {
            return Err("Failed to load TextPage from PDF page".to_string());
        }
        Ok(PdfTextPage {
            raw: tp,
            api: Arc::clone(&self.api),
        })
    }

    /// Render page into an allocated BGRA byte buffer.
    pub fn render_into_buffer(
        &self,
        buffer: &mut [u8],
        width: i32,
        height: i32,
        stride: i32,
        rotate: i32,
        flags: i32,
    ) -> Result<(), String> {
        let required = (stride * height) as usize;
        if buffer.len() < required {
            return Err(format!("Buffer too small: {} < {}", buffer.len(), required));
        }

        unsafe {
            let bitmap = (self.api.FPDFBitmap_CreateEx)(
                width,
                height,
                FPDFBitmap_BGRA,
                buffer.as_mut_ptr() as *mut c_void,
                stride,
            );
            if bitmap.is_null() {
                return Err("FPDFBitmap_CreateEx failed".to_string());
            }

            // Fill with solid white (0xFFFFFFFF)
            (self.api.FPDFBitmap_FillRect)(bitmap, 0, 0, width, height, 0xFFFFFFFF);

            (self.api.FPDF_RenderPageBitmap)(
                bitmap,
                self.raw,
                0,
                0,
                width,
                height,
                rotate,
                flags | FPDF_ANNOT | FPDF_LCD_TEXT,
            );

            (self.api.FPDFBitmap_Destroy)(bitmap);
        }

        Ok(())
    }

    pub fn count_objects(&self) -> usize {
        let count = unsafe { (self.api.FPDFPage_CountObjects)(self.raw) };
        if count < 0 {
            0
        } else {
            count as usize
        }
    }

    pub fn get_object(&self, index: usize) -> Option<PdfPageObject> {
        let obj = unsafe { (self.api.FPDFPage_GetObject)(self.raw, index as c_int) };
        if obj.is_null() {
            None
        } else {
            Some(PdfPageObject {
                raw: obj,
                api: Arc::clone(&self.api),
            })
        }
    }
}

// Safe TextPage RAII
pub struct PdfTextPage {
    pub raw: FPDF_TEXTPAGE,
    api: Arc<PdfiumApi>,
}

unsafe impl Send for PdfTextPage {}
unsafe impl Sync for PdfTextPage {}

impl Drop for PdfTextPage {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                (self.api.FPDFText_ClosePage)(self.raw);
            }
            self.raw = ptr::null_mut();
        }
    }
}

impl PdfTextPage {
    pub fn count_chars(&self) -> usize {
        let c = unsafe { (self.api.FPDFText_CountChars)(self.raw) };
        if c < 0 {
            0
        } else {
            c as usize
        }
    }

    pub fn get_char_unicode(&self, index: usize) -> Option<char> {
        let u = unsafe { (self.api.FPDFText_GetUnicode)(self.raw, index as c_int) };
        char::from_u32(u)
    }

    pub fn get_char_box(&self, index: usize) -> Option<RectF> {
        let mut left = 0.0f64;
        let mut right = 0.0f64;
        let mut bottom = 0.0f64;
        let mut top = 0.0f64;

        let ret = unsafe {
            (self.api.FPDFText_GetCharBox)(
                self.raw,
                index as c_int,
                &mut left,
                &mut right,
                &mut bottom,
                &mut top,
            )
        };

        if ret != 0 {
            Some(RectF {
                left: left as f32,
                top: top as f32,
                right: right as f32,
                bottom: bottom as f32,
            })
        } else {
            None
        }
    }

    pub fn get_char_index_at_pos(&self, x: f64, y: f64, x_tol: f64, y_tol: f64) -> Option<usize> {
        let idx = unsafe { (self.api.FPDFText_GetCharIndexAtPos)(self.raw, x, y, x_tol, y_tol) };
        if idx >= 0 {
            Some(idx as usize)
        } else {
            None
        }
    }

    pub fn get_all_text(&self) -> String {
        let count = self.count_chars();
        if count == 0 {
            return String::new();
        }
        let mut buf: Vec<u16> = vec![0; count + 1];
        let written = unsafe { (self.api.FPDFText_GetText)(self.raw, 0, count as c_int, buf.as_mut_ptr()) };
        if written > 0 {
            let actual_len = (written as usize).min(count);
            String::from_utf16_lossy(&buf[..actual_len])
        } else {
            String::new()
        }
    }

    pub fn search(&self, query: &str, match_case: bool) -> Vec<RectF> {
        if query.is_empty() {
            return Vec::new();
        }

        let wide_query: Vec<u16> = query.encode_utf16().chain(std::iter::once(0)).collect();
        let flags: c_uint = if match_case { 0x01 } else { 0x00 };

        let handle = unsafe { (self.api.FPDFText_FindStart)(self.raw, wide_query.as_ptr(), flags, 0) };
        if handle.is_null() {
            return Vec::new();
        }

        let mut results = Vec::new();

        unsafe {
            while (self.api.FPDFText_FindNext)(handle) != 0 {
                let start_idx = (self.api.FPDFText_GetSchIdx)(handle);
                let count = (self.api.FPDFText_GetSchResultCnt)(handle);
                if start_idx >= 0 && count > 0 {
                    let mut b_left = f32::MAX;
                    let mut b_top = f32::MIN;
                    let mut b_right = f32::MIN;
                    let mut b_bottom = f32::MAX;

                    for i in 0..count {
                        if let Some(r) = self.get_char_box((start_idx + i) as usize) {
                            b_left = b_left.min(r.left.min(r.right));
                            b_right = b_right.max(r.left.max(r.right));
                            b_bottom = b_bottom.min(r.bottom.min(r.top));
                            b_top = b_top.max(r.bottom.max(r.top));
                        }
                    }

                    if b_left < b_right && b_bottom < b_top {
                        results.push(RectF {
                            left: b_left,
                            top: b_top,
                            right: b_right,
                            bottom: b_bottom,
                        });
                    }
                }
            }

            (self.api.FPDFText_FindClose)(handle);
        }

        results
    }
}

// Page Object (direct text mutation)
pub struct PdfPageObject {
    pub raw: FPDF_PAGEOBJECT,
    api: Arc<PdfiumApi>,
}

impl PdfPageObject {
    pub fn get_type(&self) -> c_int {
        unsafe { (self.api.FPDFPageObj_GetType)(self.raw) }
    }

    pub fn is_text(&self) -> bool {
        self.get_type() == FPDF_PAGEOBJ_TEXT
    }

    pub fn get_bounds(&self) -> Option<RectF> {
        let mut left = 0.0f32;
        let mut bottom = 0.0f32;
        let mut right = 0.0f32;
        let mut top = 0.0f32;

        let ret = unsafe {
            (self.api.FPDFPageObj_GetBounds)(
                self.raw,
                &mut left,
                &mut bottom,
                &mut right,
                &mut top,
            )
        };

        if ret != 0 {
            Some(RectF {
                left,
                top,
                right,
                bottom,
            })
        } else {
            None
        }
    }

    pub fn get_text(&self, text_page: &PdfTextPage) -> Option<String> {
        if let Some(getter) = self.api.FPDFTextObj_GetText {
            let required = unsafe { getter(self.raw, text_page.raw, ptr::null_mut(), 0) };
            if required == 0 {
                return Some(String::new());
            }
            let mut buf = vec![0u16; required as usize];
            let written = unsafe { getter(self.raw, text_page.raw, buf.as_mut_ptr(), required) };
            if written > 0 {
                let actual = if buf.ends_with(&[0]) {
                    &buf[..buf.len() - 1]
                } else {
                    &buf[..]
                };
                return Some(String::from_utf16_lossy(actual));
            }
        }
        None
    }

    pub fn get_font_size(&self) -> Option<f32> {
        if let Some(size_fn) = self.api.FPDFTextObj_GetFontSize {
            let mut size = 0.0f32;
            let ret = unsafe { size_fn(self.raw, &mut size) };
            if ret != 0 {
                return Some(size);
            }
        }
        None
    }

    pub fn set_text(&self, text: &str) -> bool {
        if let Some(setter) = self.api.FPDFText_SetText {
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let ret = unsafe { setter(self.raw, wide.as_ptr()) };
            ret != 0
        } else {
            false
        }
    }
}
