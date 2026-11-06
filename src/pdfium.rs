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

pub type FPDF_FORMHANDLE = *mut c_void;
pub type FPDF_BOOL = c_int;

pub const FPDF_FILLMODE_NONE: c_int = 0;
pub const FPDF_FILLMODE_ALTERNATE: c_int = 1;
pub const FPDF_FILLMODE_WINDING: c_int = 2;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FPDF_SYSTEMTIME {
    pub wYear: u16,
    pub wMonth: u16,
    pub wDayOfWeek: u16,
    pub wDay: u16,
    pub wHour: u16,
    pub wMinute: u16,
    pub wSecond: u16,
    pub wMilliseconds: u16,
}

#[repr(C)]
pub struct FPDF_FORMFILLINFO {
    pub version: c_int,
    pub Release: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO)>,
    pub FFI_Invalidate: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, FPDF_PAGE, f64, f64, f64, f64)>,
    pub FFI_OutputSelectedRect: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, FPDF_PAGE, f64, f64, f64, f64)>,
    pub FFI_SetCursor: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, c_int)>,
    pub FFI_SetTimer: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, c_int, Option<unsafe extern "C" fn(c_int)>) -> c_int>,
    pub FFI_KillTimer: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, c_int)>,
    pub FFI_GetLocalTime: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO) -> FPDF_SYSTEMTIME>,
    pub FFI_OnChange: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO)>,
    pub FFI_GetPage: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, FPDF_DOCUMENT, c_int) -> FPDF_PAGE>,
    pub FFI_GetCurrentPage: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, FPDF_DOCUMENT) -> FPDF_PAGE>,
    pub FFI_GetRotation: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, FPDF_PAGE) -> c_int>,
    pub FFI_ExecuteNamedAction: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, *const c_char)>,
    pub FFI_SetTextFieldFocus: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, *const u16, u32, FPDF_BOOL)>,
    pub FFI_DoURIAction: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, *const c_char)>,
    pub FFI_DoGoToAction: Option<unsafe extern "C" fn(*mut FPDF_FORMFILLINFO, c_int, c_int, *mut f32, c_int)>,
}

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
        size: std::ffi::c_ulong,
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
    pub FPDF_LoadMemDocument: Option<unsafe extern "C" fn(*const c_void, c_int, *const c_char) -> FPDF_DOCUMENT>,
    pub FPDF_CreateNewDocument: unsafe extern "C" fn() -> FPDF_DOCUMENT,
    pub FPDF_CloseDocument: unsafe extern "C" fn(FPDF_DOCUMENT),
    pub FPDF_GetPageCount: unsafe extern "C" fn(FPDF_DOCUMENT) -> c_int,
    pub FPDF_LoadPage: unsafe extern "C" fn(FPDF_DOCUMENT, c_int) -> FPDF_PAGE,
    pub FPDF_ClosePage: unsafe extern "C" fn(FPDF_PAGE),
    pub FPDF_GetPageWidthF: Option<unsafe extern "C" fn(FPDF_PAGE) -> f32>,
    pub FPDF_GetPageHeightF: Option<unsafe extern "C" fn(FPDF_PAGE) -> f32>,
    pub FPDF_GetPageWidth: unsafe extern "C" fn(FPDF_PAGE) -> f64,
    pub FPDF_GetPageHeight: unsafe extern "C" fn(FPDF_PAGE) -> f64,
    pub FPDFPage_GetRotation: unsafe extern "C" fn(FPDF_PAGE) -> c_int,
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
    pub FPDFText_GetSchResultIndex: unsafe extern "C" fn(FPDF_SCHHANDLE) -> c_int,
    pub FPDFText_GetSchCount: unsafe extern "C" fn(FPDF_SCHHANDLE) -> c_int,
    pub FPDFText_FindClose: unsafe extern "C" fn(FPDF_SCHHANDLE),

    // Page Objects & Editing
    pub FPDFPage_CountObjects: unsafe extern "C" fn(FPDF_PAGE) -> c_int,
    pub FPDFPage_GetObject: unsafe extern "C" fn(FPDF_PAGE, c_int) -> FPDF_PAGEOBJECT,
    pub FPDFPageObj_GetType: unsafe extern "C" fn(FPDF_PAGEOBJECT) -> c_int,
    pub FPDFPageObj_GetBounds: unsafe extern "C" fn(FPDF_PAGEOBJECT, *mut f32, *mut f32, *mut f32, *mut f32) -> c_int,
    pub FPDFTextObj_GetText: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, FPDF_TEXTPAGE, *mut u16, c_uint) -> c_uint>,
    pub FPDFTextObj_GetFontSize: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, *mut f32) -> c_int>,
    pub FPDFText_SetText: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, *const u16) -> c_int>,
    pub FPDFText_GetTextObject: Option<unsafe extern "C" fn(FPDF_TEXTPAGE, c_int) -> FPDF_PAGEOBJECT>,
    pub FPDFText_LoadStandardFont: Option<unsafe extern "C" fn(FPDF_DOCUMENT, *const c_char) -> FPDF_FONT>,
    pub FPDFPageObj_CreateTextObj: Option<unsafe extern "C" fn(FPDF_DOCUMENT, FPDF_FONT, f32) -> FPDF_PAGEOBJECT>,
    pub FPDFPageObj_Transform: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, f64, f64, f64, f64, f64, f64)>,
    pub FPDFPage_InsertObject: Option<unsafe extern "C" fn(FPDF_PAGE, FPDF_PAGEOBJECT)>,

    // AcroForms & Form Filling
    pub FPDFDOC_InitFormFillEnvironment: Option<unsafe extern "C" fn(FPDF_DOCUMENT, *mut FPDF_FORMFILLINFO) -> FPDF_FORMHANDLE>,
    pub FPDFDOC_ExitFormFillEnvironment: Option<unsafe extern "C" fn(FPDF_FORMHANDLE)>,
    pub FORM_DoDocumentJSAction: Option<unsafe extern "C" fn(FPDF_FORMHANDLE)>,
    pub FORM_DoDocumentOpenAction: Option<unsafe extern "C" fn(FPDF_FORMHANDLE)>,
    pub FORM_OnLButtonDown: Option<unsafe extern "C" fn(FPDF_FORMHANDLE, FPDF_PAGE, c_int, f64, f64) -> FPDF_BOOL>,
    pub FORM_OnLButtonUp: Option<unsafe extern "C" fn(FPDF_FORMHANDLE, FPDF_PAGE, c_int, f64, f64) -> FPDF_BOOL>,
    pub FORM_OnMouseMove: Option<unsafe extern "C" fn(FPDF_FORMHANDLE, FPDF_PAGE, c_int, f64, f64) -> FPDF_BOOL>,
    pub FORM_OnKeyDown: Option<unsafe extern "C" fn(FPDF_FORMHANDLE, FPDF_PAGE, c_int, c_int) -> FPDF_BOOL>,
    pub FORM_OnKeyUp: Option<unsafe extern "C" fn(FPDF_FORMHANDLE, FPDF_PAGE, c_int, c_int) -> FPDF_BOOL>,
    pub FORM_OnChar: Option<unsafe extern "C" fn(FPDF_FORMHANDLE, FPDF_PAGE, c_int, c_int) -> FPDF_BOOL>,
    pub FPDF_FFLDraw: Option<unsafe extern "C" fn(FPDF_FORMHANDLE, FPDF_BITMAP, FPDF_PAGE, c_int, c_int, c_int, c_int, c_int, c_int)>,

    // Image & Vector Path Objects
    pub FPDFPageObj_NewImageObj: Option<unsafe extern "C" fn(FPDF_DOCUMENT) -> FPDF_PAGEOBJECT>,
    pub FPDFImageObj_SetBitmap: Option<unsafe extern "C" fn(*mut FPDF_PAGE, c_int, FPDF_PAGEOBJECT, FPDF_BITMAP) -> FPDF_BOOL>,
    pub FPDFImageObj_SetMatrix: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, f64, f64, f64, f64, f64, f64) -> FPDF_BOOL>,
    pub FPDFImageObj_LoadJpegFile: Option<unsafe extern "C" fn(*mut FPDF_PAGE, c_int, FPDF_PAGEOBJECT, *mut c_void) -> FPDF_BOOL>,
    pub FPDFPageObj_CreateNewPath: Option<unsafe extern "C" fn(f32, f32) -> FPDF_PAGEOBJECT>,
    pub FPDFPath_MoveTo: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, f32, f32) -> FPDF_BOOL>,
    pub FPDFPath_LineTo: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, f32, f32) -> FPDF_BOOL>,
    pub FPDFPath_BezierTo: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, f32, f32, f32, f32, f32, f32) -> FPDF_BOOL>,
    pub FPDFPath_Close: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT) -> FPDF_BOOL>,
    pub FPDFPath_SetDrawMode: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, c_int, FPDF_BOOL) -> FPDF_BOOL>,
    pub FPDFPageObj_SetFillColor: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, c_uint, c_uint, c_uint, c_uint) -> FPDF_BOOL>,
    pub FPDFPageObj_SetStrokeColor: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, c_uint, c_uint, c_uint, c_uint) -> FPDF_BOOL>,
    pub FPDFPageObj_SetStrokeWidth: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT, f32) -> FPDF_BOOL>,

    // Object Removal & Destruction
    pub FPDFPage_RemoveObject: Option<unsafe extern "C" fn(FPDF_PAGE, FPDF_PAGEOBJECT) -> FPDF_BOOL>,
    pub FPDFPageObj_Destroy: Option<unsafe extern "C" fn(FPDF_PAGEOBJECT)>,

    // Device Coordinate Conversions
    pub FPDF_DeviceToPage: Option<unsafe extern "C" fn(FPDF_PAGE, c_int, c_int, c_int, c_int, c_int, c_int, c_int, *mut f64, *mut f64) -> c_int>,
    pub FPDF_PageToDevice: Option<unsafe extern "C" fn(FPDF_PAGE, c_int, c_int, c_int, c_int, c_int, f64, f64, *mut c_int, *mut c_int) -> c_int>,

    // Saving
    pub FPDF_SaveAsCopy: unsafe extern "C" fn(FPDF_DOCUMENT, *mut FPDF_FILEWRITE, c_int) -> c_int,
    pub FPDF_GetLastError: unsafe extern "C" fn() -> std::ffi::c_ulong,
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
                FPDF_LoadMemDocument: resolve_opt!(FPDF_LoadMemDocument),
                FPDF_CreateNewDocument: resolve!(FPDF_CreateNewDocument),
                FPDF_CloseDocument: resolve!(FPDF_CloseDocument),
                FPDF_GetPageCount: resolve!(FPDF_GetPageCount),
                FPDF_LoadPage: resolve!(FPDF_LoadPage),
                FPDF_ClosePage: resolve!(FPDF_ClosePage),
                FPDF_GetPageWidthF: resolve_opt!(FPDF_GetPageWidthF),
                FPDF_GetPageHeightF: resolve_opt!(FPDF_GetPageHeightF),
                FPDF_GetPageWidth: resolve!(FPDF_GetPageWidth),
                FPDF_GetPageHeight: resolve!(FPDF_GetPageHeight),
                FPDFPage_GetRotation: resolve!(FPDFPage_GetRotation),
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
                FPDFText_GetSchResultIndex: resolve!(FPDFText_GetSchResultIndex),
                FPDFText_GetSchCount: resolve!(FPDFText_GetSchCount),
                FPDFText_FindClose: resolve!(FPDFText_FindClose),

                FPDFPage_CountObjects: resolve!(FPDFPage_CountObjects),
                FPDFPage_GetObject: resolve!(FPDFPage_GetObject),
                FPDFPageObj_GetType: resolve!(FPDFPageObj_GetType),
                FPDFPageObj_GetBounds: resolve!(FPDFPageObj_GetBounds),
                FPDFTextObj_GetText: resolve_opt!(FPDFTextObj_GetText),
                FPDFTextObj_GetFontSize: resolve_opt!(FPDFTextObj_GetFontSize),
                FPDFText_SetText: resolve_opt!(FPDFText_SetText),
                FPDFText_GetTextObject: resolve_opt!(FPDFText_GetTextObject),
                FPDFText_LoadStandardFont: resolve_opt!(FPDFText_LoadStandardFont),
                FPDFPageObj_CreateTextObj: resolve_opt!(FPDFPageObj_CreateTextObj),
                FPDFPageObj_Transform: resolve_opt!(FPDFPageObj_Transform),
                FPDFPage_InsertObject: resolve_opt!(FPDFPage_InsertObject),
                FPDF_DeviceToPage: resolve_opt!(FPDF_DeviceToPage),
                FPDF_PageToDevice: resolve_opt!(FPDF_PageToDevice),

                FPDFDOC_InitFormFillEnvironment: resolve_opt!(FPDFDOC_InitFormFillEnvironment),
                FPDFDOC_ExitFormFillEnvironment: resolve_opt!(FPDFDOC_ExitFormFillEnvironment),
                FORM_DoDocumentJSAction: resolve_opt!(FORM_DoDocumentJSAction),
                FORM_DoDocumentOpenAction: resolve_opt!(FORM_DoDocumentOpenAction),
                FORM_OnLButtonDown: resolve_opt!(FORM_OnLButtonDown),
                FORM_OnLButtonUp: resolve_opt!(FORM_OnLButtonUp),
                FORM_OnMouseMove: resolve_opt!(FORM_OnMouseMove),
                FORM_OnKeyDown: resolve_opt!(FORM_OnKeyDown),
                FORM_OnKeyUp: resolve_opt!(FORM_OnKeyUp),
                FORM_OnChar: resolve_opt!(FORM_OnChar),
                FPDF_FFLDraw: resolve_opt!(FPDF_FFLDraw),

                FPDFPageObj_NewImageObj: resolve_opt!(FPDFPageObj_NewImageObj),
                FPDFImageObj_SetBitmap: resolve_opt!(FPDFImageObj_SetBitmap),
                FPDFImageObj_SetMatrix: resolve_opt!(FPDFImageObj_SetMatrix),
                FPDFImageObj_LoadJpegFile: resolve_opt!(FPDFImageObj_LoadJpegFile),
                FPDFPageObj_CreateNewPath: resolve_opt!(FPDFPageObj_CreateNewPath),
                FPDFPath_MoveTo: resolve_opt!(FPDFPath_MoveTo),
                FPDFPath_LineTo: resolve_opt!(FPDFPath_LineTo),
                FPDFPath_BezierTo: resolve_opt!(FPDFPath_BezierTo),
                FPDFPath_Close: resolve_opt!(FPDFPath_Close),
                FPDFPath_SetDrawMode: resolve_opt!(FPDFPath_SetDrawMode),
                FPDFPageObj_SetFillColor: resolve_opt!(FPDFPageObj_SetFillColor),
                FPDFPageObj_SetStrokeColor: resolve_opt!(FPDFPageObj_SetStrokeColor),
                FPDFPageObj_SetStrokeWidth: resolve_opt!(FPDFPageObj_SetStrokeWidth),

                FPDFPage_RemoveObject: resolve_opt!(FPDFPage_RemoveObject),
                FPDFPageObj_Destroy: resolve_opt!(FPDFPageObj_Destroy),

                FPDF_SaveAsCopy: resolve!(FPDF_SaveAsCopy),
                FPDF_GetLastError: resolve!(FPDF_GetLastError),
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
            _backing_bytes: None,
        })
    }

    pub fn open_document_from_bytes(&self, bytes: Vec<u8>) -> Result<PdfDocument, String> {
        let load_mem = self
            .api
            .FPDF_LoadMemDocument
            .ok_or_else(|| "FPDF_LoadMemDocument not available in PDFium DLL".to_string())?;
        let boxed: Box<[u8]> = bytes.into_boxed_slice();
        let doc = unsafe {
            load_mem(
                boxed.as_ptr() as *const c_void,
                boxed.len() as c_int,
                ptr::null(),
            )
        };
        if doc.is_null() {
            return Err("PDFium failed to load document from memory buffer".to_string());
        }
        Ok(PdfDocument {
            raw: doc,
            api: Arc::clone(&self.api),
            _backing_bytes: Some(boxed),
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
            _backing_bytes: None,
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
    pub api: Arc<PdfiumApi>,
    _backing_bytes: Option<Box<[u8]>>,
}

unsafe impl Send for PdfDocument {}
unsafe impl Sync for PdfDocument {}

pub struct PdfFormEnv {
    pub raw: FPDF_FORMHANDLE,
    _form_info: Box<FPDF_FORMFILLINFO>,
    api: Arc<PdfiumApi>,
}

unsafe impl Send for PdfFormEnv {}
unsafe impl Sync for PdfFormEnv {}

impl Drop for PdfFormEnv {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            if let Some(exit_fn) = self.api.FPDFDOC_ExitFormFillEnvironment {
                unsafe { exit_fn(self.raw) };
            }
            self.raw = ptr::null_mut();
        }
    }
}

impl PdfFormEnv {
    pub fn do_document_js_action(&self) {
        if let Some(js_fn) = self.api.FORM_DoDocumentJSAction {
            unsafe { js_fn(self.raw) };
        }
    }

    pub fn do_document_open_action(&self) {
        if let Some(open_fn) = self.api.FORM_DoDocumentOpenAction {
            unsafe { open_fn(self.raw) };
        }
    }

    pub fn on_lbutton_down(&self, page: &PdfPage, modifier: i32, page_x: f64, page_y: f64) -> bool {
        if let Some(btn_down) = self.api.FORM_OnLButtonDown {
            unsafe { btn_down(self.raw, page.raw, modifier as c_int, page_x, page_y) != 0 }
        } else {
            false
        }
    }

    pub fn on_lbutton_up(&self, page: &PdfPage, modifier: i32, page_x: f64, page_y: f64) -> bool {
        if let Some(btn_up) = self.api.FORM_OnLButtonUp {
            unsafe { btn_up(self.raw, page.raw, modifier as c_int, page_x, page_y) != 0 }
        } else {
            false
        }
    }

    pub fn on_mouse_move(&self, page: &PdfPage, modifier: i32, page_x: f64, page_y: f64) -> bool {
        if let Some(mm_fn) = self.api.FORM_OnMouseMove {
            unsafe { mm_fn(self.raw, page.raw, modifier as c_int, page_x, page_y) != 0 }
        } else {
            false
        }
    }

    pub fn on_key_down(&self, page: &PdfPage, key_code: i32, modifier: i32) -> bool {
        if let Some(kd_fn) = self.api.FORM_OnKeyDown {
            unsafe { kd_fn(self.raw, page.raw, key_code as c_int, modifier as c_int) != 0 }
        } else {
            false
        }
    }

    pub fn on_key_up(&self, page: &PdfPage, key_code: i32, modifier: i32) -> bool {
        if let Some(ku_fn) = self.api.FORM_OnKeyUp {
            unsafe { ku_fn(self.raw, page.raw, key_code as c_int, modifier as c_int) != 0 }
        } else {
            false
        }
    }

    pub fn on_char(&self, page: &PdfPage, char_code: i32, modifier: i32) -> bool {
        if let Some(char_fn) = self.api.FORM_OnChar {
            unsafe { char_fn(self.raw, page.raw, char_code as c_int, modifier as c_int) != 0 }
        } else {
            false
        }
    }
}

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

    pub fn load_standard_font(&self, font_name: &str) -> Option<FPDF_FONT> {
        let loader = self.api.FPDFText_LoadStandardFont?;
        let c_name = CString::new(font_name).ok()?;
        let font = unsafe { loader(self.raw, c_name.as_ptr()) };
        if font.is_null() {
            None
        } else {
            Some(font)
        }
    }

    pub fn create_text_object(&self, font: FPDF_FONT, font_size: f32) -> Option<PdfPageObject> {
        let creator = self.api.FPDFPageObj_CreateTextObj?;
        let obj = unsafe { creator(self.raw, font, font_size) };
        if obj.is_null() {
            None
        } else {
            Some(PdfPageObject {
                raw: obj,
                api: Arc::clone(&self.api),
            })
        }
    }

    pub fn create_image_object(&self) -> Option<PdfPageObject> {
        let creator = self.api.FPDFPageObj_NewImageObj?;
        let obj = unsafe { creator(self.raw) };
        if obj.is_null() {
            None
        } else {
            Some(PdfPageObject {
                raw: obj,
                api: Arc::clone(&self.api),
            })
        }
    }

    pub fn create_path(&self, x: f32, y: f32) -> Option<PdfPageObject> {
        let creator = self.api.FPDFPageObj_CreateNewPath?;
        let obj = unsafe { creator(x, y) };
        if obj.is_null() {
            None
        } else {
            Some(PdfPageObject {
                raw: obj,
                api: Arc::clone(&self.api),
            })
        }
    }

    pub fn init_form_env(&self) -> Option<PdfFormEnv> {
        let init_fn = self.api.FPDFDOC_InitFormFillEnvironment?;
        let mut form_info = Box::new(FPDF_FORMFILLINFO {
            version: 1,
            Release: None,
            FFI_Invalidate: None,
            FFI_OutputSelectedRect: None,
            FFI_SetCursor: None,
            FFI_SetTimer: None,
            FFI_KillTimer: None,
            FFI_GetLocalTime: None,
            FFI_OnChange: None,
            FFI_GetPage: None,
            FFI_GetCurrentPage: None,
            FFI_GetRotation: None,
            FFI_ExecuteNamedAction: None,
            FFI_SetTextFieldFocus: None,
            FFI_DoURIAction: None,
            FFI_DoGoToAction: None,
        });

        let handle = unsafe { init_fn(self.raw, form_info.as_mut() as *mut FPDF_FORMFILLINFO) };
        if handle.is_null() {
            None
        } else {
            let env = PdfFormEnv {
                raw: handle,
                _form_info: form_info,
                api: Arc::clone(&self.api),
            };
            env.do_document_open_action();
            env.do_document_js_action();
            Some(env)
        }
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let bytes = self.save_to_bytes()?;
        let mut file = File::create(path).map_err(|e| format!("Failed to create output file: {}", e))?;
        file.write_all(&bytes).map_err(|e| format!("Failed to write output file: {}", e))?;
        Ok(())
    }

    pub fn save_to_bytes(&self) -> Result<Vec<u8>, String> {
        #[repr(C)]
        struct StreamWriter {
            writer: FPDF_FILEWRITE,
            buffer: Vec<u8>,
        }

        unsafe extern "C" fn write_callback(
            p_this: *mut FPDF_FILEWRITE,
            p_data: *const c_void,
            size: std::ffi::c_ulong,
        ) -> c_int {
            let stream = &mut *(p_this as *mut StreamWriter);
            let slice = std::slice::from_raw_parts(p_data as *const u8, size as usize);
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
                0, // 0 for full export
            )
        };

        if ok != 1 {
            let err = unsafe { (self.api.FPDF_GetLastError)() };
            return Err(format!("FPDF_SaveAsCopy failed (ok={}, error_code={})", ok, err));
        }

        Ok(stream.buffer)
    }
}

// Safe Page RAII
pub struct PdfPage {
    pub raw: FPDF_PAGE,
    pub api: Arc<PdfiumApi>,
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
        unsafe { (self.api.FPDFPage_GetRotation)(self.raw) }
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

    pub fn select_text_in_rect(&self, rect: RectF) -> (String, Vec<RectF>) {
        if let Ok(text_page) = self.load_text_page() {
            text_page.select_text_in_rect(rect)
        } else {
            (String::new(), Vec::new())
        }
    }

    pub fn remove_object(&self, obj: &PdfPageObject) -> bool {
        if let Some(remover) = self.api.FPDFPage_RemoveObject {
            unsafe { remover(self.raw, obj.raw) != 0 }
        } else {
            false
        }
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
        self.render_into_buffer_ex(buffer, width, height, stride, rotate, flags, None)
    }

    pub fn render_into_buffer_ex(
        &self,
        buffer: &mut [u8],
        width: i32,
        height: i32,
        stride: i32,
        rotate: i32,
        flags: i32,
        form_handle: Option<FPDF_FORMHANDLE>,
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

            if let Some(h_form) = form_handle {
                if let Some(draw_fn) = self.api.FPDF_FFLDraw {
                    draw_fn(
                        h_form,
                        bitmap,
                        self.raw,
                        0,
                        0,
                        width,
                        height,
                        rotate,
                        flags | FPDF_ANNOT,
                    );
                }
            }

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

    pub fn insert_object(&self, obj: &PdfPageObject) -> bool {
        if let Some(inserter) = self.api.FPDFPage_InsertObject {
            unsafe { inserter(self.raw, obj.raw) };
            true
        } else {
            false
        }
    }

    pub fn device_to_page(
        &self,
        start_x: i32,
        start_y: i32,
        size_x: i32,
        size_y: i32,
        rotate: i32,
        device_x: i32,
        device_y: i32,
    ) -> Option<(f64, f64)> {
        let d2p = self.api.FPDF_DeviceToPage?;
        let mut page_x = 0.0f64;
        let mut page_y = 0.0f64;
        let ret = unsafe {
            d2p(
                self.raw,
                start_x,
                start_y,
                size_x,
                size_y,
                rotate,
                device_x,
                device_y,
                &mut page_x,
                &mut page_y,
            )
        };
        if ret != 0 {
            Some((page_x, page_y))
        } else {
            None
        }
    }

    pub fn page_to_device(
        &self,
        start_x: i32,
        start_y: i32,
        size_x: i32,
        size_y: i32,
        rotate: i32,
        page_x: f64,
        page_y: f64,
    ) -> Option<(i32, i32)> {
        let p2d = self.api.FPDF_PageToDevice?;
        let mut dev_x = 0i32;
        let mut dev_y = 0i32;
        let ret = unsafe {
            p2d(
                self.raw,
                start_x,
                start_y,
                size_x,
                size_y,
                rotate,
                page_x,
                page_y,
                &mut dev_x,
                &mut dev_y,
            )
        };
        if ret != 0 {
            Some((dev_x, dev_y))
        } else {
            None
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

    pub fn get_text_object(&self, char_index: usize) -> Option<PdfPageObject> {
        let getter = self.api.FPDFText_GetTextObject?;
        let obj = unsafe { getter(self.raw, char_index as c_int) };
        if obj.is_null() {
            None
        } else {
            Some(PdfPageObject {
                raw: obj,
                api: Arc::clone(&self.api),
            })
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

    pub fn get_text_range(&self, start_idx: usize, char_count: usize) -> String {
        if char_count == 0 {
            return String::new();
        }
        let mut buf: Vec<u16> = vec![0; char_count + 1];
        let written = unsafe {
            (self.api.FPDFText_GetText)(
                self.raw,
                start_idx as c_int,
                char_count as c_int,
                buf.as_mut_ptr(),
            )
        };
        if written > 0 {
            let actual_len = (written as usize).saturating_sub(1).min(char_count);
            String::from_utf16_lossy(&buf[..actual_len])
        } else {
            String::new()
        }
    }

    /// Selects characters within `rect` (or between start/end points) and returns
    /// both the extracted text string and merged per-line highlight bounding rects.
    pub fn select_text_in_rect(&self, rect: RectF) -> (String, Vec<RectF>) {
        let count = self.count_chars();
        if count == 0 {
            return (String::new(), Vec::new());
        }

        let r_left = rect.left.min(rect.right);
        let r_right = rect.left.max(rect.right);
        let r_bottom = rect.bottom.min(rect.top);
        let r_top = rect.bottom.max(rect.top);

        // Try endpoint character lookup first for natural text-flow selection,
        // and also collect all characters intersecting the selection box.
        let mut matched_indices = Vec::new();
        for i in 0..count {
            if let Some(cb) = self.get_char_box(i) {
                let c_left = cb.left.min(cb.right);
                let c_right = cb.left.max(cb.right);
                let c_bottom = cb.bottom.min(cb.top);
                let c_top = cb.bottom.max(cb.top);
                if c_right <= c_left || c_top <= c_bottom {
                    continue;
                }
                let cx = (c_left + c_right) * 0.5;
                let cy = (c_bottom + c_top) * 0.5;
                let intersects = (cx >= r_left && cx <= r_right && cy >= r_bottom && cy <= r_top)
                    || (c_left <= r_right && c_right >= r_left && c_bottom <= r_top && c_top >= r_bottom);
                if intersects {
                    matched_indices.push((i, RectF {
                        left: c_left,
                        bottom: c_bottom,
                        right: c_right,
                        top: c_top,
                    }));
                }
            }
        }

        if matched_indices.is_empty() {
            return (String::new(), Vec::new());
        }

        // Extract text and merge adjacent character boxes on the same line
        let first_idx = matched_indices.first().unwrap().0;
        let last_idx = matched_indices.last().unwrap().0;

        // Build extracted string from matched characters (preserving line breaks)
        let mut out_text = String::new();
        let mut merged_rects: Vec<RectF> = Vec::new();
        let mut prev_idx: Option<usize> = None;
        let mut prev_rect: Option<RectF> = None;

        // If selection is a contiguous range or simple drag, include spaces between words on the same line
        let _ = (first_idx, last_idx);
        for &(idx, cbox) in &matched_indices {
            if let Some(p_idx) = prev_idx {
                if idx > p_idx + 1 {
                    // Check if intermediate chars are whitespace on the same line
                    if let Some(pr) = prev_rect {
                        let same_line = (cbox.bottom - pr.bottom).abs() < 6.0;
                        if same_line && idx - p_idx <= 4 {
                            for gap_i in (p_idx + 1)..idx {
                                if let Some(ch) = self.get_char_unicode(gap_i) {
                                    out_text.push(ch);
                                }
                            }
                        } else if !same_line {
                            out_text.push_str("\r\n");
                        } else {
                            out_text.push(' ');
                        }
                    }
                }
            }

            if let Some(ch) = self.get_char_unicode(idx) {
                if ch != '\0' {
                    out_text.push(ch);
                }
            }

            if let Some(last_r) = merged_rects.last_mut() {
                let same_line = (cbox.bottom - last_r.bottom).abs() < 5.0
                    && (cbox.left - last_r.right).abs() < 18.0;
                if same_line {
                    last_r.left = last_r.left.min(cbox.left);
                    last_r.right = last_r.right.max(cbox.right);
                    last_r.bottom = last_r.bottom.min(cbox.bottom);
                    last_r.top = last_r.top.max(cbox.top);
                } else {
                    merged_rects.push(cbox);
                }
            } else {
                merged_rects.push(cbox);
            }

            prev_idx = Some(idx);
            prev_rect = Some(cbox);
        }

        (out_text.trim().to_string(), merged_rects)
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
                let start_idx = (self.api.FPDFText_GetSchResultIndex)(handle);
                let count = (self.api.FPDFText_GetSchCount)(handle);
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
                let s = String::from_utf16_lossy(&buf);
                let clean = s.trim_matches('\0').to_string();
                return Some(clean);
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

    pub fn transform(&self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        if let Some(transformer) = self.api.FPDFPageObj_Transform {
            unsafe { transformer(self.raw, a, b, c, d, e, f) };
        }
    }

    pub fn destroy(self) {
        if let Some(destroy_fn) = self.api.FPDFPageObj_Destroy {
            unsafe { destroy_fn(self.raw) };
        }
    }

    pub fn path_move_to(&self, x: f32, y: f32) -> bool {
        if let Some(f) = self.api.FPDFPath_MoveTo {
            unsafe { f(self.raw, x, y) != 0 }
        } else {
            false
        }
    }

    pub fn path_line_to(&self, x: f32, y: f32) -> bool {
        if let Some(f) = self.api.FPDFPath_LineTo {
            unsafe { f(self.raw, x, y) != 0 }
        } else {
            false
        }
    }

    pub fn path_bezier_to(&self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) -> bool {
        if let Some(f) = self.api.FPDFPath_BezierTo {
            unsafe { f(self.raw, x1, y1, x2, y2, x3, y3) != 0 }
        } else {
            false
        }
    }

    pub fn path_close(&self) -> bool {
        if let Some(f) = self.api.FPDFPath_Close {
            unsafe { f(self.raw) != 0 }
        } else {
            false
        }
    }

    pub fn path_set_draw_mode(&self, fill_mode: i32, stroke: bool) -> bool {
        if let Some(f) = self.api.FPDFPath_SetDrawMode {
            unsafe { f(self.raw, fill_mode as c_int, if stroke { 1 } else { 0 }) != 0 }
        } else {
            false
        }
    }

    pub fn set_fill_color(&self, r: u8, g: u8, b: u8, a: u8) -> bool {
        if let Some(f) = self.api.FPDFPageObj_SetFillColor {
            unsafe { f(self.raw, r as u32, g as u32, b as u32, a as u32) != 0 }
        } else {
            false
        }
    }

    pub fn set_stroke_color(&self, r: u8, g: u8, b: u8, a: u8) -> bool {
        if let Some(f) = self.api.FPDFPageObj_SetStrokeColor {
            unsafe { f(self.raw, r as u32, g as u32, b as u32, a as u32) != 0 }
        } else {
            false
        }
    }

    pub fn set_stroke_width(&self, width: f32) -> bool {
        if let Some(f) = self.api.FPDFPageObj_SetStrokeWidth {
            unsafe { f(self.raw, width) != 0 }
        } else {
            false
        }
    }

    pub fn set_image_bitmap(&self, pages: &mut [FPDF_PAGE], bitmap: FPDF_BITMAP) -> bool {
        if let Some(f) = self.api.FPDFImageObj_SetBitmap {
            let p_ptr = if pages.is_empty() { ptr::null_mut() } else { pages.as_mut_ptr() };
            unsafe { f(p_ptr, pages.len() as c_int, self.raw, bitmap) != 0 }
        } else {
            false
        }
    }

    pub fn set_image_matrix(&self, a: f64, b: f64, c: f64, d: f64, e: f64, f_coord: f64) -> bool {
        if let Some(m_fn) = self.api.FPDFImageObj_SetMatrix {
            unsafe { m_fn(self.raw, a, b, c, d, e, f_coord) != 0 }
        } else {
            false
        }
    }
}
