// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: PDFium safe RAII wrappers over a dynamically loaded export table

// PDFium's C names are kept verbatim so they can be grepped against the headers.
#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void, CString};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use std::sync::Arc;

type HMODULE = *mut c_void;
type FARPROC = *mut c_void;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryExW(lp_lib_file_name: *const u16, h_file: *mut c_void, dw_flags: u32) -> HMODULE;
    fn GetProcAddress(h_module: HMODULE, lp_proc_name: *const c_char) -> FARPROC;
    fn FreeLibrary(h_module: HMODULE) -> i32;
}

const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;

// --- PDFium handle types & constants (values from the public PDFium headers) ---

pub type FPDF_DOCUMENT = *mut c_void;
pub type FPDF_PAGE = *mut c_void;
pub type FPDF_TEXTPAGE = *mut c_void;
pub type FPDF_BITMAP = *mut c_void;
pub type FPDF_PAGEOBJECT = *mut c_void;
pub type FPDF_SCHHANDLE = *mut c_void;
pub type FPDF_FONT = *mut c_void;
pub type FPDF_FORMHANDLE = *mut c_void;
pub type FPDF_ANNOTATION = *mut c_void;
pub type FPDF_BOOL = c_int;

pub const FPDF_PAGEOBJ_TEXT: c_int = 1;
pub const FPDF_PAGEOBJ_PATH: c_int = 2;
pub const FPDF_PAGEOBJ_IMAGE: c_int = 3;

pub const FPDF_FILLMODE_NONE: c_int = 0;
pub const FPDF_FILLMODE_WINDING: c_int = 2;

pub const FPDFBitmap_Gray: c_int = 1;
pub const FPDFBitmap_BGR: c_int = 2;
pub const FPDFBitmap_BGRx: c_int = 3;
pub const FPDFBitmap_BGRA: c_int = 4;

/// Render flags.
pub const FPDF_ANNOT: c_int = 0x01;
pub const FPDF_LCD_TEXT: c_int = 0x02;

/// Annotation subtype / color type.
pub const FPDF_ANNOT_HIGHLIGHT: c_int = 9;
const FPDFANNOT_COLORTYPE_Color: c_int = 0;

/// Modifier flags for the FORM_On* event methods (fpdf_fwlevent.h).
pub const FWL_EVENTFLAG_ShiftKey: i32 = 1 << 0;
pub const FWL_EVENTFLAG_ControlKey: i32 = 1 << 1;

const FPDF_NO_INCREMENTAL: c_int = 2;

/// FPDF_FORMFILLINFO, version 1 layout (fpdf_formfill.h). zPDF installs no
/// callbacks, so the 15 typed callback slots between `version` and
/// `m_pJsPlatform` are kept as nullable pointers in header order.
#[repr(C)]
pub(crate) struct FPDF_FORMFILLINFO {
    version: c_int,
    // Release, FFI_Invalidate, FFI_OutputSelectedRect, FFI_SetCursor, FFI_SetTimer,
    // FFI_KillTimer, FFI_GetLocalTime, FFI_OnChange, FFI_GetPage, FFI_GetCurrentPage,
    // FFI_GetRotation, FFI_ExecuteNamedAction, FFI_SetTextFieldFocus, FFI_DoURIAction,
    // FFI_DoGoToAction
    callbacks: [Option<unsafe extern "C" fn()>; 15],
    m_pJsPlatform: *mut c_void,
}

#[repr(C)]
pub(crate) struct FPDF_FILEWRITE {
    version: c_int,
    WriteBlock: unsafe extern "C" fn(*mut FPDF_FILEWRITE, *const c_void, c_ulong) -> c_int,
}

/// FPDF_LIBRARY_CONFIG, version 2 layout.
#[repr(C)]
pub(crate) struct FPDF_LIBRARY_CONFIG {
    version: c_int,
    m_pUserFontPaths: *mut *const c_char,
    m_pIsolate: *mut c_void,
    m_v8EmbedderSlot: c_uint,
}

#[repr(C)]
#[derive(Default)]
pub(crate) struct FS_SIZEF {
    width: f32,
    height: f32,
}

#[repr(C)]
#[derive(Default)]
pub(crate) struct FS_MATRIX {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
}

#[repr(C)]
pub(crate) struct FS_QUADPOINTSF {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    x3: f32,
    y3: f32,
    x4: f32,
    y4: f32,
}

/// A rectangle in PDF page space. `repr(C)` with the same field order as
/// PDFium's `FS_RECTF`, so it is passed to PDFium directly.
#[repr(C)]
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

    /// Same rectangle with left <= right and bottom <= top.
    pub fn normalized(&self) -> RectF {
        RectF {
            left: self.left.min(self.right),
            right: self.left.max(self.right),
            bottom: self.bottom.min(self.top),
            top: self.bottom.max(self.top),
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.normalized();
        x >= r.left && x <= r.right && y >= r.bottom && y <= r.top
    }

    /// True when the two rectangles overlap (edges touching counts).
    pub fn intersects(&self, other: &RectF) -> bool {
        let a = self.normalized();
        let b = other.normalized();
        a.left <= b.right && a.right >= b.left && a.bottom <= b.top && a.top >= b.bottom
    }
}

// --- Export table: one line per PDFium export (field + resolution generated) ---

macro_rules! pdfium_exports {
    ($( fn $name:ident ( $($arg:ty),* $(,)? ) $(-> $ret:ty)? ; )*) => {
        /// Function table resolved from pdfium.dll. It owns the module handle:
        /// dropping the last `Arc<PdfiumApi>` (held by the library and by every
        /// document, page, text page, object and form environment) calls
        /// `FPDF_DestroyLibrary` and unloads the DLL.
        pub struct PdfiumApi {
            module: HMODULE,
            $( pub(crate) $name: unsafe extern "C" fn($($arg),*) $(-> $ret)?, )*
        }

        impl PdfiumApi {
            unsafe fn resolve(module: HMODULE) -> Result<Self, String> {
                Ok(Self {
                    module,
                    $( $name: {
                        let p = GetProcAddress(module, concat!(stringify!($name), "\0").as_ptr() as *const c_char);
                        if p.is_null() {
                            return Err(concat!("Missing required PDFium export: ", stringify!($name)).to_string());
                        }
                        std::mem::transmute::<FARPROC, unsafe extern "C" fn($($arg),*) $(-> $ret)?>(p)
                    }, )*
                })
            }
        }
    };
}

pdfium_exports! {
    fn FPDF_InitLibraryWithConfig(*const FPDF_LIBRARY_CONFIG);
    fn FPDF_DestroyLibrary();
    fn FPDF_GetLastError() -> c_ulong;
    fn FPDF_LoadMemDocument64(*const c_void, usize, *const c_char) -> FPDF_DOCUMENT;
    fn FPDF_CreateNewDocument() -> FPDF_DOCUMENT;
    fn FPDF_CloseDocument(FPDF_DOCUMENT);
    fn FPDF_SaveAsCopy(FPDF_DOCUMENT, *mut FPDF_FILEWRITE, c_int) -> FPDF_BOOL;
    fn FPDF_GetPageCount(FPDF_DOCUMENT) -> c_int;
    fn FPDF_GetPageSizeByIndexF(FPDF_DOCUMENT, c_int, *mut FS_SIZEF) -> FPDF_BOOL;
    fn FPDF_LoadPage(FPDF_DOCUMENT, c_int) -> FPDF_PAGE;
    fn FPDF_ClosePage(FPDF_PAGE);
    fn FPDF_GetPageWidthF(FPDF_PAGE) -> f32;
    fn FPDF_GetPageHeightF(FPDF_PAGE) -> f32;
    fn FPDFPage_GetRotation(FPDF_PAGE) -> c_int;
    fn FPDFPage_SetRotation(FPDF_PAGE, c_int);
    fn FPDFPage_New(FPDF_DOCUMENT, c_int, f64, f64) -> FPDF_PAGE;
    fn FPDFPage_Delete(FPDF_DOCUMENT, c_int);
    fn FPDFPage_GenerateContent(FPDF_PAGE) -> FPDF_BOOL;
    fn FPDF_DeviceToPage(FPDF_PAGE, c_int, c_int, c_int, c_int, c_int, c_int, c_int, *mut f64, *mut f64) -> FPDF_BOOL;
    fn FPDF_PageToDevice(FPDF_PAGE, c_int, c_int, c_int, c_int, c_int, f64, f64, *mut c_int, *mut c_int) -> FPDF_BOOL;

    fn FPDFBitmap_Create(c_int, c_int, c_int) -> FPDF_BITMAP;
    fn FPDFBitmap_CreateEx(c_int, c_int, c_int, *mut c_void, c_int) -> FPDF_BITMAP;
    fn FPDFBitmap_Destroy(FPDF_BITMAP);
    fn FPDFBitmap_FillRect(FPDF_BITMAP, c_int, c_int, c_int, c_int, c_ulong) -> FPDF_BOOL;
    fn FPDFBitmap_GetBuffer(FPDF_BITMAP) -> *mut u8;
    fn FPDFBitmap_GetWidth(FPDF_BITMAP) -> c_int;
    fn FPDFBitmap_GetHeight(FPDF_BITMAP) -> c_int;
    fn FPDFBitmap_GetStride(FPDF_BITMAP) -> c_int;
    fn FPDFBitmap_GetFormat(FPDF_BITMAP) -> c_int;
    fn FPDF_RenderPageBitmap(FPDF_BITMAP, FPDF_PAGE, c_int, c_int, c_int, c_int, c_int, c_int);

    fn FPDFText_LoadPage(FPDF_PAGE) -> FPDF_TEXTPAGE;
    fn FPDFText_ClosePage(FPDF_TEXTPAGE);
    fn FPDFText_CountChars(FPDF_TEXTPAGE) -> c_int;
    fn FPDFText_GetUnicode(FPDF_TEXTPAGE, c_int) -> c_uint;
    fn FPDFText_GetCharBox(FPDF_TEXTPAGE, c_int, *mut f64, *mut f64, *mut f64, *mut f64) -> FPDF_BOOL;
    fn FPDFText_GetCharIndexAtPos(FPDF_TEXTPAGE, f64, f64, f64, f64) -> c_int;
    fn FPDFText_FindStart(FPDF_TEXTPAGE, *const u16, c_ulong, c_int) -> FPDF_SCHHANDLE;
    fn FPDFText_FindNext(FPDF_SCHHANDLE) -> FPDF_BOOL;
    fn FPDFText_GetSchResultIndex(FPDF_SCHHANDLE) -> c_int;
    fn FPDFText_GetSchCount(FPDF_SCHHANDLE) -> c_int;
    fn FPDFText_FindClose(FPDF_SCHHANDLE);

    fn FPDFPage_CountObjects(FPDF_PAGE) -> c_int;
    fn FPDFPage_GetObject(FPDF_PAGE, c_int) -> FPDF_PAGEOBJECT;
    fn FPDFPage_InsertObject(FPDF_PAGE, FPDF_PAGEOBJECT);
    fn FPDFPage_RemoveObject(FPDF_PAGE, FPDF_PAGEOBJECT) -> FPDF_BOOL;
    fn FPDFPageObj_Destroy(FPDF_PAGEOBJECT);
    fn FPDFPageObj_GetType(FPDF_PAGEOBJECT) -> c_int;
    fn FPDFPageObj_GetBounds(FPDF_PAGEOBJECT, *mut f32, *mut f32, *mut f32, *mut f32) -> FPDF_BOOL;
    fn FPDFPageObj_GetMatrix(FPDF_PAGEOBJECT, *mut FS_MATRIX) -> FPDF_BOOL;
    fn FPDFPageObj_Transform(FPDF_PAGEOBJECT, f64, f64, f64, f64, f64, f64);
    fn FPDFPageObj_SetFillColor(FPDF_PAGEOBJECT, c_uint, c_uint, c_uint, c_uint) -> FPDF_BOOL;
    fn FPDFPageObj_SetStrokeColor(FPDF_PAGEOBJECT, c_uint, c_uint, c_uint, c_uint) -> FPDF_BOOL;
    fn FPDFPageObj_SetStrokeWidth(FPDF_PAGEOBJECT, f32) -> FPDF_BOOL;
    fn FPDFTextObj_GetText(FPDF_PAGEOBJECT, FPDF_TEXTPAGE, *mut u16, c_ulong) -> c_ulong;
    fn FPDFTextObj_GetFontSize(FPDF_PAGEOBJECT, *mut f32) -> FPDF_BOOL;
    fn FPDFText_SetText(FPDF_PAGEOBJECT, *const u16) -> FPDF_BOOL;
    fn FPDFText_LoadStandardFont(FPDF_DOCUMENT, *const c_char) -> FPDF_FONT;
    fn FPDFFont_Close(FPDF_FONT);
    fn FPDFPageObj_CreateTextObj(FPDF_DOCUMENT, FPDF_FONT, f32) -> FPDF_PAGEOBJECT;
    fn FPDFPageObj_NewImageObj(FPDF_DOCUMENT) -> FPDF_PAGEOBJECT;
    fn FPDFImageObj_SetBitmap(*mut FPDF_PAGE, c_int, FPDF_PAGEOBJECT, FPDF_BITMAP) -> FPDF_BOOL;
    fn FPDFImageObj_GetBitmap(FPDF_PAGEOBJECT) -> FPDF_BITMAP;
    fn FPDFImageObj_SetMatrix(FPDF_PAGEOBJECT, f64, f64, f64, f64, f64, f64) -> FPDF_BOOL;
    fn FPDFPageObj_CreateNewPath(f32, f32) -> FPDF_PAGEOBJECT;
    fn FPDFPath_MoveTo(FPDF_PAGEOBJECT, f32, f32) -> FPDF_BOOL;
    fn FPDFPath_LineTo(FPDF_PAGEOBJECT, f32, f32) -> FPDF_BOOL;
    fn FPDFPath_Close(FPDF_PAGEOBJECT) -> FPDF_BOOL;
    fn FPDFPath_SetDrawMode(FPDF_PAGEOBJECT, c_int, FPDF_BOOL) -> FPDF_BOOL;

    fn FPDFPage_CreateAnnot(FPDF_PAGE, c_int) -> FPDF_ANNOTATION;
    fn FPDFPage_GetAnnotCount(FPDF_PAGE) -> c_int;
    fn FPDFPage_GetAnnot(FPDF_PAGE, c_int) -> FPDF_ANNOTATION;
    fn FPDFPage_GetAnnotIndex(FPDF_PAGE, FPDF_ANNOTATION) -> c_int;
    fn FPDFPage_CloseAnnot(FPDF_ANNOTATION);
    fn FPDFPage_RemoveAnnot(FPDF_PAGE, c_int) -> FPDF_BOOL;
    fn FPDFAnnot_SetColor(FPDF_ANNOTATION, c_int, c_uint, c_uint, c_uint, c_uint) -> FPDF_BOOL;
    fn FPDFAnnot_AppendAttachmentPoints(FPDF_ANNOTATION, *const FS_QUADPOINTSF) -> FPDF_BOOL;
    fn FPDFAnnot_SetRect(FPDF_ANNOTATION, *const RectF) -> FPDF_BOOL;
    fn FPDFAnnot_GetRect(FPDF_ANNOTATION, *mut RectF) -> FPDF_BOOL;

    fn FPDFDOC_InitFormFillEnvironment(FPDF_DOCUMENT, *mut FPDF_FORMFILLINFO) -> FPDF_FORMHANDLE;
    fn FPDFDOC_ExitFormFillEnvironment(FPDF_FORMHANDLE);
    fn FORM_DoDocumentJSAction(FPDF_FORMHANDLE);
    fn FORM_DoDocumentOpenAction(FPDF_FORMHANDLE);
    fn FORM_OnAfterLoadPage(FPDF_PAGE, FPDF_FORMHANDLE);
    fn FORM_OnBeforeClosePage(FPDF_PAGE, FPDF_FORMHANDLE);
    fn FORM_OnLButtonDown(FPDF_FORMHANDLE, FPDF_PAGE, c_int, f64, f64) -> FPDF_BOOL;
    fn FORM_OnLButtonUp(FPDF_FORMHANDLE, FPDF_PAGE, c_int, f64, f64) -> FPDF_BOOL;
    fn FORM_OnMouseMove(FPDF_FORMHANDLE, FPDF_PAGE, c_int, f64, f64) -> FPDF_BOOL;
    fn FORM_OnKeyDown(FPDF_FORMHANDLE, FPDF_PAGE, c_int, c_int) -> FPDF_BOOL;
    fn FORM_OnKeyUp(FPDF_FORMHANDLE, FPDF_PAGE, c_int, c_int) -> FPDF_BOOL;
    fn FORM_OnChar(FPDF_FORMHANDLE, FPDF_PAGE, c_int, c_int) -> FPDF_BOOL;
    fn FORM_ForceToKillFocus(FPDF_FORMHANDLE) -> FPDF_BOOL;
    fn FORM_GetFocusedAnnot(FPDF_FORMHANDLE, *mut c_int, *mut FPDF_ANNOTATION) -> FPDF_BOOL;
    fn FPDF_FFLDraw(FPDF_FORMHANDLE, FPDF_BITMAP, FPDF_PAGE, c_int, c_int, c_int, c_int, c_int, c_int);
}

impl Drop for PdfiumApi {
    fn drop(&mut self) {
        unsafe {
            (self.FPDF_DestroyLibrary)();
            FreeLibrary(self.module);
        }
    }
}

fn last_error_message(api: &PdfiumApi) -> &'static str {
    match unsafe { (api.FPDF_GetLastError)() } {
        2 => "file not found or could not be opened",
        3 => "file is not a PDF or is corrupted",
        4 => "password required or incorrect password",
        5 => "unsupported security scheme",
        6 => "page not found or content error",
        _ => "unknown error",
    }
}

// --- Library ---

pub struct PdfiumLibrary {
    api: Arc<PdfiumApi>,
}

impl PdfiumLibrary {
    pub fn load(dll_path: &Path) -> Result<Self, String> {
        let wide: Vec<u16> = dll_path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let module = unsafe { LoadLibraryExW(wide.as_ptr(), ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
        if module.is_null() {
            return Err(format!(
                "Failed to load PDFium DLL from: {} (Win32 Error: {})",
                dll_path.display(),
                std::io::Error::last_os_error()
            ));
        }
        let api = match unsafe { PdfiumApi::resolve(module) } {
            Ok(api) => api,
            Err(e) => {
                unsafe { FreeLibrary(module) };
                return Err(e);
            }
        };
        let config = FPDF_LIBRARY_CONFIG {
            version: 2,
            m_pUserFontPaths: ptr::null_mut(),
            m_pIsolate: ptr::null_mut(),
            m_v8EmbedderSlot: 0,
        };
        unsafe { (api.FPDF_InitLibraryWithConfig)(&config) };
        Ok(Self { api: Arc::new(api) })
    }

    /// Reads the whole file into memory and opens it from there, so the file on
    /// disk is never held open (and can be safely overwritten by `save_to_file`).
    pub fn open_document(&self, path: &Path, password: Option<&str>) -> Result<PdfDocument, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        self.load_from_memory(bytes, password)
            .map_err(|e| format!("PDFium failed to open {}: {}", path.display(), e))
    }

    pub fn open_document_from_bytes(&self, bytes: Vec<u8>) -> Result<PdfDocument, String> {
        self.load_from_memory(bytes, None)
            .map_err(|e| format!("PDFium failed to load document from memory: {}", e))
    }

    fn load_from_memory(&self, bytes: Vec<u8>, password: Option<&str>) -> Result<PdfDocument, String> {
        let c_pass = password.map(CString::new).transpose().map_err(|e| e.to_string())?;
        let pass_ptr = c_pass.as_ref().map_or(ptr::null(), |p| p.as_ptr());
        let doc = unsafe { (self.api.FPDF_LoadMemDocument64)(bytes.as_ptr() as *const c_void, bytes.len(), pass_ptr) };
        if doc.is_null() {
            return Err(last_error_message(&self.api).to_string());
        }
        Ok(PdfDocument {
            raw: doc,
            api: Arc::clone(&self.api),
            _backing_bytes: bytes,
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
            _backing_bytes: Vec::new(),
        })
    }
}

// --- Document ---

pub struct PdfDocument {
    pub raw: FPDF_DOCUMENT,
    pub(crate) api: Arc<PdfiumApi>,
    /// Memory the document was loaded from. PDFium reads it lazily, so it must
    /// outlive `raw`; `Drop` closes the document before fields are dropped.
    _backing_bytes: Vec<u8>,
}

impl Drop for PdfDocument {
    fn drop(&mut self) {
        unsafe { (self.api.FPDF_CloseDocument)(self.raw) };
    }
}

impl PdfDocument {
    pub fn page_count(&self) -> usize {
        unsafe { (self.api.FPDF_GetPageCount)(self.raw) }.max(0) as usize
    }

    /// Page size in points without parsing the page. Like `PdfPage::dimensions`,
    /// it already accounts for the page's /Rotate (width/height swapped for 90/270).
    pub fn page_size(&self, index: usize) -> Option<(f32, f32)> {
        let mut size = FS_SIZEF::default();
        let ok = unsafe { (self.api.FPDF_GetPageSizeByIndexF)(self.raw, index as c_int, &mut size) };
        (ok != 0).then_some((size.width, size.height))
    }

    pub fn load_page(&self, page_index: usize) -> Result<PdfPage, String> {
        let page = unsafe { (self.api.FPDF_LoadPage)(self.raw, page_index as c_int) };
        if page.is_null() {
            return Err(format!("Failed to load page index {}", page_index));
        }
        Ok(PdfPage { raw: page, api: Arc::clone(&self.api) })
    }

    pub fn new_page(&self, index: usize, width: f64, height: f64) -> Result<PdfPage, String> {
        let page = unsafe { (self.api.FPDFPage_New)(self.raw, index as c_int, width, height) };
        if page.is_null() {
            return Err("Failed to create new page in document".to_string());
        }
        Ok(PdfPage { raw: page, api: Arc::clone(&self.api) })
    }

    pub fn delete_page(&self, page_index: usize) -> bool {
        if page_index >= self.page_count() {
            return false;
        }
        unsafe { (self.api.FPDFPage_Delete)(self.raw, page_index as c_int) };
        true
    }

    /// Loads one of the 14 standard PDF fonts. The caller owns the handle and
    /// should release it with `close_font` once every text object created from
    /// it has been inserted into a page.
    pub fn load_standard_font(&self, font_name: &str) -> Option<FPDF_FONT> {
        let c_name = CString::new(font_name).ok()?;
        let font = unsafe { (self.api.FPDFText_LoadStandardFont)(self.raw, c_name.as_ptr()) };
        (!font.is_null()).then_some(font)
    }

    /// Releases a font handle returned by `load_standard_font`.
    pub fn close_font(&self, font: FPDF_FONT) {
        unsafe { (self.api.FPDFFont_Close)(font) };
    }

    fn wrap_object(&self, raw: FPDF_PAGEOBJECT) -> Option<PdfPageObject> {
        (!raw.is_null()).then(|| PdfPageObject { raw, api: Arc::clone(&self.api) })
    }

    pub fn create_text_object(&self, font: FPDF_FONT, font_size: f32) -> Option<PdfPageObject> {
        self.wrap_object(unsafe { (self.api.FPDFPageObj_CreateTextObj)(self.raw, font, font_size) })
    }

    pub fn create_image_object(&self) -> Option<PdfPageObject> {
        self.wrap_object(unsafe { (self.api.FPDFPageObj_NewImageObj)(self.raw) })
    }

    pub fn create_path(&self, x: f32, y: f32) -> Option<PdfPageObject> {
        self.wrap_object(unsafe { (self.api.FPDFPageObj_CreateNewPath)(x, y) })
    }

    pub fn init_form_env(&self) -> Option<PdfFormEnv> {
        let mut form_info = Box::new(FPDF_FORMFILLINFO {
            version: 1,
            callbacks: [None; 15],
            m_pJsPlatform: ptr::null_mut(),
        });
        let handle = unsafe { (self.api.FPDFDOC_InitFormFillEnvironment)(self.raw, &mut *form_info) };
        if handle.is_null() {
            return None;
        }
        unsafe {
            (self.api.FORM_DoDocumentOpenAction)(handle);
            (self.api.FORM_DoDocumentJSAction)(handle);
        }
        Some(PdfFormEnv {
            raw: handle,
            _form_info: form_info,
            api: Arc::clone(&self.api),
        })
    }

    /// Saves atomically: the PDF is written to a temporary file in the target's
    /// directory, flushed, and then renamed over the target (replacing it).
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let bytes = self.save_to_bytes()?;
        let file_name = path.file_name().ok_or_else(|| format!("Invalid output path: {}", path.display()))?;
        let mut tmp_name = file_name.to_os_string();
        tmp_name.push(format!(".{}.zpdf-tmp", std::process::id()));
        let tmp_path = path.with_file_name(tmp_name);

        let write = || -> std::io::Result<()> {
            let mut file = std::fs::File::create(&tmp_path)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&tmp_path, path)
        };
        write().map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            format!("Failed to save {}: {}", path.display(), e)
        })
    }

    pub fn save_to_bytes(&self) -> Result<Vec<u8>, String> {
        #[repr(C)]
        struct StreamWriter {
            writer: FPDF_FILEWRITE,
            buffer: Vec<u8>,
        }

        unsafe extern "C" fn write_block(this: *mut FPDF_FILEWRITE, data: *const c_void, size: c_ulong) -> c_int {
            // `writer` is the first field of the repr(C) StreamWriter.
            let stream = &mut *(this as *mut StreamWriter);
            stream.buffer.extend_from_slice(std::slice::from_raw_parts(data as *const u8, size as usize));
            1
        }

        let mut stream = StreamWriter {
            writer: FPDF_FILEWRITE { version: 1, WriteBlock: write_block },
            buffer: Vec::with_capacity(64 * 1024),
        };
        let ok = unsafe { (self.api.FPDF_SaveAsCopy)(self.raw, &mut stream.writer, FPDF_NO_INCREMENTAL) };
        if ok == 0 {
            return Err(format!("FPDF_SaveAsCopy failed: {}", last_error_message(&self.api)));
        }
        Ok(stream.buffer)
    }
}

// --- Form fill environment ---

pub struct PdfFormEnv {
    pub raw: FPDF_FORMHANDLE,
    /// PDFium keeps a pointer to this struct for the lifetime of the handle.
    _form_info: Box<FPDF_FORMFILLINFO>,
    api: Arc<PdfiumApi>,
}

impl Drop for PdfFormEnv {
    fn drop(&mut self) {
        unsafe { (self.api.FPDFDOC_ExitFormFillEnvironment)(self.raw) };
    }
}

impl PdfFormEnv {
    pub fn on_after_load_page(&self, page: &PdfPage) {
        unsafe { (self.api.FORM_OnAfterLoadPage)(page.raw, self.raw) };
    }

    pub fn on_before_close_page(&self, page: &PdfPage) {
        unsafe { (self.api.FORM_OnBeforeClosePage)(page.raw, self.raw) };
    }

    pub fn on_lbutton_down(&self, page: &PdfPage, modifier: i32, page_x: f64, page_y: f64) -> bool {
        unsafe { (self.api.FORM_OnLButtonDown)(self.raw, page.raw, modifier, page_x, page_y) != 0 }
    }

    pub fn on_lbutton_up(&self, page: &PdfPage, modifier: i32, page_x: f64, page_y: f64) -> bool {
        unsafe { (self.api.FORM_OnLButtonUp)(self.raw, page.raw, modifier, page_x, page_y) != 0 }
    }

    pub fn on_mouse_move(&self, page: &PdfPage, modifier: i32, page_x: f64, page_y: f64) -> bool {
        unsafe { (self.api.FORM_OnMouseMove)(self.raw, page.raw, modifier, page_x, page_y) != 0 }
    }

    pub fn on_key_down(&self, page: &PdfPage, key_code: i32, modifier: i32) -> bool {
        unsafe { (self.api.FORM_OnKeyDown)(self.raw, page.raw, key_code, modifier) != 0 }
    }

    pub fn on_key_up(&self, page: &PdfPage, key_code: i32, modifier: i32) -> bool {
        unsafe { (self.api.FORM_OnKeyUp)(self.raw, page.raw, key_code, modifier) != 0 }
    }

    pub fn on_char(&self, page: &PdfPage, char_code: i32, modifier: i32) -> bool {
        unsafe { (self.api.FORM_OnChar)(self.raw, page.raw, char_code, modifier) != 0 }
    }

    /// Removes focus from the focused form field (commits its value).
    pub fn kill_focus(&self) -> bool {
        unsafe { (self.api.FORM_ForceToKillFocus)(self.raw) != 0 }
    }

    /// True when a form field / annotation currently has keyboard focus.
    pub fn has_focus(&self) -> bool {
        let mut page_index: c_int = -1;
        let mut annot: FPDF_ANNOTATION = ptr::null_mut();
        let ok = unsafe { (self.api.FORM_GetFocusedAnnot)(self.raw, &mut page_index, &mut annot) };
        if annot.is_null() {
            return false;
        }
        unsafe { (self.api.FPDFPage_CloseAnnot)(annot) };
        ok != 0
    }
}

// --- Page ---

pub struct PdfPage {
    pub raw: FPDF_PAGE,
    api: Arc<PdfiumApi>,
}

impl Drop for PdfPage {
    fn drop(&mut self) {
        unsafe { (self.api.FPDF_ClosePage)(self.raw) };
    }
}

impl PdfPage {
    /// Page size in points, already accounting for /Rotate (see `PdfDocument::page_size`).
    pub fn dimensions(&self) -> (f64, f64) {
        unsafe { ((self.api.FPDF_GetPageWidthF)(self.raw) as f64, (self.api.FPDF_GetPageHeightF)(self.raw) as f64) }
    }

    /// Page /Rotate as quarter turns clockwise (0..=3).
    pub fn rotation(&self) -> i32 {
        unsafe { (self.api.FPDFPage_GetRotation)(self.raw) }
    }

    pub fn set_rotation(&self, rotation: i32) {
        unsafe { (self.api.FPDFPage_SetRotation)(self.raw, rotation.rem_euclid(4)) };
    }

    pub fn rotate_clockwise(&self) {
        self.set_rotation(self.rotation() + 1);
    }

    pub fn generate_content(&self) -> bool {
        unsafe { (self.api.FPDFPage_GenerateContent)(self.raw) != 0 }
    }

    pub fn load_text_page(&self) -> Result<PdfTextPage, String> {
        let tp = unsafe { (self.api.FPDFText_LoadPage)(self.raw) };
        if tp.is_null() {
            return Err("Failed to load TextPage from PDF page".to_string());
        }
        Ok(PdfTextPage { raw: tp, api: Arc::clone(&self.api) })
    }

    pub fn count_objects(&self) -> usize {
        unsafe { (self.api.FPDFPage_CountObjects)(self.raw) }.max(0) as usize
    }

    pub fn get_object(&self, index: usize) -> Option<PdfPageObject> {
        let raw = unsafe { (self.api.FPDFPage_GetObject)(self.raw, index as c_int) };
        (!raw.is_null()).then(|| PdfPageObject { raw, api: Arc::clone(&self.api) })
    }

    /// Transfers ownership of `obj` to the page.
    pub fn insert_object(&self, obj: &PdfPageObject) -> bool {
        unsafe { (self.api.FPDFPage_InsertObject)(self.raw, obj.raw) };
        true
    }

    /// Detaches `obj` from the page; the caller must then `destroy()` it.
    pub fn remove_object(&self, obj: &PdfPageObject) -> bool {
        unsafe { (self.api.FPDFPage_RemoveObject)(self.raw, obj.raw) != 0 }
    }

    pub fn annot_count(&self) -> usize {
        unsafe { (self.api.FPDFPage_GetAnnotCount)(self.raw) }.max(0) as usize
    }

    /// Bounding rect of annotation `index`.
    pub fn annot_rect(&self, index: usize) -> Option<RectF> {
        let annot = unsafe { (self.api.FPDFPage_GetAnnot)(self.raw, index as c_int) };
        if annot.is_null() {
            return None;
        }
        let mut rect = RectF::default();
        let ok = unsafe { (self.api.FPDFAnnot_GetRect)(annot, &mut rect) };
        unsafe { (self.api.FPDFPage_CloseAnnot)(annot) };
        (ok != 0).then_some(rect)
    }

    pub fn remove_annot(&self, index: usize) -> bool {
        unsafe { (self.api.FPDFPage_RemoveAnnot)(self.raw, index as c_int) != 0 }
    }

    /// Creates a highlight annotation covering `rects` (one quad per rect) with
    /// RGBA `color`. Returns its annotation index. PDFium generates the
    /// appearance stream from /QuadPoints and /C when rendering and saving.
    pub fn create_highlight_annot(&self, rects: &[RectF], color: (u8, u8, u8, u8)) -> Result<usize, String> {
        if rects.is_empty() {
            return Err("Highlight needs at least one rectangle".to_string());
        }
        let api = &self.api;
        unsafe {
            let annot = (api.FPDFPage_CreateAnnot)(self.raw, FPDF_ANNOT_HIGHLIGHT);
            if annot.is_null() {
                return Err("FPDFPage_CreateAnnot(HIGHLIGHT) failed".to_string());
            }
            let mut ok = (api.FPDFAnnot_SetColor)(
                annot,
                FPDFANNOT_COLORTYPE_Color,
                color.0 as c_uint,
                color.1 as c_uint,
                color.2 as c_uint,
                color.3 as c_uint,
            ) != 0;
            let mut bbox = rects[0].normalized();
            for r in rects {
                let r = r.normalized();
                bbox.left = bbox.left.min(r.left);
                bbox.right = bbox.right.max(r.right);
                bbox.bottom = bbox.bottom.min(r.bottom);
                bbox.top = bbox.top.max(r.top);
                // Quad order per the PDF spec: top-left, top-right, bottom-left, bottom-right.
                let quad = FS_QUADPOINTSF {
                    x1: r.left,
                    y1: r.top,
                    x2: r.right,
                    y2: r.top,
                    x3: r.left,
                    y3: r.bottom,
                    x4: r.right,
                    y4: r.bottom,
                };
                ok &= (api.FPDFAnnot_AppendAttachmentPoints)(annot, &quad) != 0;
            }
            ok &= (api.FPDFAnnot_SetRect)(annot, &bbox) != 0;
            let index = (api.FPDFPage_GetAnnotIndex)(self.raw, annot);
            (api.FPDFPage_CloseAnnot)(annot);
            if !ok || index < 0 {
                if index >= 0 {
                    (api.FPDFPage_RemoveAnnot)(self.raw, index);
                }
                return Err("Failed to configure highlight annotation".to_string());
            }
            Ok(index as usize)
        }
    }

    /// Renders the page into a `width`×`height` BGRA buffer. `rotate` is an
    /// extra rotation on top of the page's own /Rotate (which PDFium applies itself).
    pub fn render_into_buffer(&self, buffer: &mut [u8], width: i32, height: i32, stride: i32, rotate: i32, flags: i32) -> Result<(), String> {
        self.render_impl(buffer, width, height, stride, (0, 0, width, height), rotate, flags, None)
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
        self.render_impl(buffer, width, height, stride, (0, 0, width, height), rotate, flags, form_handle)
    }

    /// Renders the page scaled to `size_x`×`size_y` device pixels with its
    /// top-left corner at (`start_x`, `start_y`) (may be negative) into a
    /// `buf_w`×`buf_h` BGRA buffer, i.e. a clipped region of the full page.
    /// The page's /Rotate is applied by PDFium; no extra rotation is added.
    pub fn render_region(
        &self,
        buffer: &mut [u8],
        buf_w: i32,
        buf_h: i32,
        stride: i32,
        start_x: i32,
        start_y: i32,
        size_x: i32,
        size_y: i32,
        form: Option<FPDF_FORMHANDLE>,
    ) -> Result<(), String> {
        self.render_impl(buffer, buf_w, buf_h, stride, (start_x, start_y, size_x, size_y), 0, 0, form)
    }

    fn render_impl(
        &self,
        buffer: &mut [u8],
        buf_w: i32,
        buf_h: i32,
        stride: i32,
        (start_x, start_y, size_x, size_y): (i32, i32, i32, i32),
        rotate: i32,
        flags: i32,
        form: Option<FPDF_FORMHANDLE>,
    ) -> Result<(), String> {
        if buf_w <= 0 || buf_h <= 0 || stride < buf_w * 4 {
            return Err(format!("Invalid bitmap geometry {}x{} stride {}", buf_w, buf_h, stride));
        }
        let required = stride as usize * buf_h as usize;
        if buffer.len() < required {
            return Err(format!("Buffer too small: {} < {}", buffer.len(), required));
        }
        let api = &self.api;
        unsafe {
            let bitmap = (api.FPDFBitmap_CreateEx)(buf_w, buf_h, FPDFBitmap_BGRA, buffer.as_mut_ptr() as *mut c_void, stride);
            if bitmap.is_null() {
                return Err("FPDFBitmap_CreateEx failed".to_string());
            }
            (api.FPDFBitmap_FillRect)(bitmap, 0, 0, buf_w, buf_h, 0xFFFF_FFFF);
            let flags = flags | FPDF_ANNOT | FPDF_LCD_TEXT;
            (api.FPDF_RenderPageBitmap)(bitmap, self.raw, start_x, start_y, size_x, size_y, rotate, flags);
            if let Some(h_form) = form {
                (api.FPDF_FFLDraw)(h_form, bitmap, self.raw, start_x, start_y, size_x, size_y, rotate, flags);
            }
            (api.FPDFBitmap_Destroy)(bitmap);
        }
        Ok(())
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
        let (mut x, mut y) = (0.0f64, 0.0f64);
        let ok = unsafe {
            (self.api.FPDF_DeviceToPage)(self.raw, start_x, start_y, size_x, size_y, rotate, device_x, device_y, &mut x, &mut y)
        };
        (ok != 0).then_some((x, y))
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
        let (mut x, mut y) = (0i32, 0i32);
        let ok = unsafe {
            (self.api.FPDF_PageToDevice)(self.raw, start_x, start_y, size_x, size_y, rotate, page_x, page_y, &mut x, &mut y)
        };
        (ok != 0).then_some((x, y))
    }
}

// --- Text page ---

pub struct PdfTextPage {
    pub raw: FPDF_TEXTPAGE,
    api: Arc<PdfiumApi>,
}

impl Drop for PdfTextPage {
    fn drop(&mut self) {
        unsafe { (self.api.FPDFText_ClosePage)(self.raw) };
    }
}

impl PdfTextPage {
    pub fn count_chars(&self) -> usize {
        unsafe { (self.api.FPDFText_CountChars)(self.raw) }.max(0) as usize
    }

    pub fn get_char_unicode(&self, index: usize) -> Option<char> {
        char::from_u32(unsafe { (self.api.FPDFText_GetUnicode)(self.raw, index as c_int) })
    }

    pub fn get_char_box(&self, index: usize) -> Option<RectF> {
        let (mut l, mut r, mut b, mut t) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let ok = unsafe { (self.api.FPDFText_GetCharBox)(self.raw, index as c_int, &mut l, &mut r, &mut b, &mut t) };
        (ok != 0).then_some(RectF { left: l as f32, top: t as f32, right: r as f32, bottom: b as f32 })
    }

    pub fn get_char_index_at_pos(&self, x: f64, y: f64, x_tol: f64, y_tol: f64) -> Option<usize> {
        let idx = unsafe { (self.api.FPDFText_GetCharIndexAtPos)(self.raw, x, y, x_tol, y_tol) };
        (idx >= 0).then_some(idx as usize)
    }

    /// Selects characters intersecting `rect` and returns the extracted text
    /// plus merged per-line highlight rects.
    pub fn select_text_in_rect(&self, rect: RectF) -> (String, Vec<RectF>) {
        let sel = rect.normalized();
        let mut matched = Vec::new();
        for i in 0..self.count_chars() {
            if let Some(cb) = self.get_char_box(i) {
                let cb = cb.normalized();
                if cb.right > cb.left && cb.top > cb.bottom && cb.intersects(&sel) {
                    matched.push((i, cb));
                }
            }
        }

        let mut out_text = String::new();
        let mut merged: Vec<RectF> = Vec::new();
        let mut prev: Option<(usize, RectF)> = None;
        for &(idx, cbox) in &matched {
            if let Some((p_idx, pr)) = prev {
                if idx > p_idx + 1 {
                    let same_line = (cbox.bottom - pr.bottom).abs() < 6.0;
                    if same_line && idx - p_idx <= 4 {
                        // Keep the (box-less) spaces between words on the same line.
                        out_text.extend(((p_idx + 1)..idx).filter_map(|g| self.get_char_unicode(g)));
                    } else if !same_line {
                        out_text.push_str("\r\n");
                    } else {
                        out_text.push(' ');
                    }
                }
            }
            if let Some(ch) = self.get_char_unicode(idx).filter(|&c| c != '\0') {
                out_text.push(ch);
            }
            match merged.last_mut() {
                Some(last) if (cbox.bottom - last.bottom).abs() < 5.0 && (cbox.left - last.right).abs() < 18.0 => {
                    last.left = last.left.min(cbox.left);
                    last.right = last.right.max(cbox.right);
                    last.bottom = last.bottom.min(cbox.bottom);
                    last.top = last.top.max(cbox.top);
                }
                _ => merged.push(cbox),
            }
            prev = Some((idx, cbox));
        }
        (out_text.trim().to_string(), merged)
    }

    /// Returns the bounding rect of every match of `query`.
    pub fn search(&self, query: &str, match_case: bool) -> Vec<RectF> {
        if query.is_empty() {
            return Vec::new();
        }
        let wide: Vec<u16> = query.encode_utf16().chain(std::iter::once(0)).collect();
        let flags: c_ulong = if match_case { 0x01 } else { 0x00 };
        let api = &self.api;
        let handle = unsafe { (api.FPDFText_FindStart)(self.raw, wide.as_ptr(), flags, 0) };
        if handle.is_null() {
            return Vec::new();
        }
        let mut results = Vec::new();
        unsafe {
            while (api.FPDFText_FindNext)(handle) != 0 {
                let start = (api.FPDFText_GetSchResultIndex)(handle);
                let count = (api.FPDFText_GetSchCount)(handle);
                let mut bbox: Option<RectF> = None;
                for i in start.max(0)..(start + count).max(0) {
                    if let Some(r) = self.get_char_box(i as usize) {
                        let r = r.normalized();
                        bbox = Some(match bbox {
                            None => r,
                            Some(b) => RectF {
                                left: b.left.min(r.left),
                                right: b.right.max(r.right),
                                bottom: b.bottom.min(r.bottom),
                                top: b.top.max(r.top),
                            },
                        });
                    }
                }
                if let Some(b) = bbox.filter(|b| b.left < b.right && b.bottom < b.top) {
                    results.push(b);
                }
            }
            (api.FPDFText_FindClose)(handle);
        }
        results
    }
}

// --- Page object ---

/// A page object handle. Objects returned by `PdfPage::get_object` are owned by
/// the page; objects from `PdfDocument::create_*` are owned by the caller until
/// inserted (`PdfPage::insert_object`) or destroyed (`destroy`).
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
        let (mut l, mut b, mut r, mut t) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let ok = unsafe { (self.api.FPDFPageObj_GetBounds)(self.raw, &mut l, &mut b, &mut r, &mut t) };
        (ok != 0).then_some(RectF { left: l, top: t, right: r, bottom: b })
    }

    /// Object transform matrix (a, b, c, d, e, f).
    pub fn get_matrix(&self) -> Option<[f32; 6]> {
        let mut m = FS_MATRIX::default();
        let ok = unsafe { (self.api.FPDFPageObj_GetMatrix)(self.raw, &mut m) };
        (ok != 0).then_some([m.a, m.b, m.c, m.d, m.e, m.f])
    }

    pub fn get_text(&self, text_page: &PdfTextPage) -> Option<String> {
        let getter = self.api.FPDFTextObj_GetText;
        // Length is in bytes, including the UTF-16 terminator.
        let bytes = unsafe { getter(self.raw, text_page.raw, ptr::null_mut(), 0) };
        if bytes == 0 {
            return None;
        }
        let mut buf = vec![0u16; (bytes as usize).div_ceil(2)];
        let written = unsafe { getter(self.raw, text_page.raw, buf.as_mut_ptr(), bytes) };
        if written == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf).trim_matches('\0').to_string())
    }

    pub fn get_font_size(&self) -> Option<f32> {
        let mut size = 0.0f32;
        let ok = unsafe { (self.api.FPDFTextObj_GetFontSize)(self.raw, &mut size) };
        (ok != 0).then_some(size)
    }

    pub fn set_text(&self, text: &str) -> bool {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe { (self.api.FPDFText_SetText)(self.raw, wide.as_ptr()) != 0 }
    }

    pub fn transform(&self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        unsafe { (self.api.FPDFPageObj_Transform)(self.raw, a, b, c, d, e, f) };
    }

    /// Frees an object that is not (or no longer) owned by a page.
    pub fn destroy(self) {
        unsafe { (self.api.FPDFPageObj_Destroy)(self.raw) };
    }

    pub fn path_move_to(&self, x: f32, y: f32) -> bool {
        unsafe { (self.api.FPDFPath_MoveTo)(self.raw, x, y) != 0 }
    }

    pub fn path_line_to(&self, x: f32, y: f32) -> bool {
        unsafe { (self.api.FPDFPath_LineTo)(self.raw, x, y) != 0 }
    }

    pub fn path_close(&self) -> bool {
        unsafe { (self.api.FPDFPath_Close)(self.raw) != 0 }
    }

    pub fn path_set_draw_mode(&self, fill_mode: i32, stroke: bool) -> bool {
        unsafe { (self.api.FPDFPath_SetDrawMode)(self.raw, fill_mode, stroke as FPDF_BOOL) != 0 }
    }

    pub fn set_fill_color(&self, r: u8, g: u8, b: u8, a: u8) -> bool {
        unsafe { (self.api.FPDFPageObj_SetFillColor)(self.raw, r as c_uint, g as c_uint, b as c_uint, a as c_uint) != 0 }
    }

    pub fn set_stroke_color(&self, r: u8, g: u8, b: u8, a: u8) -> bool {
        unsafe { (self.api.FPDFPageObj_SetStrokeColor)(self.raw, r as c_uint, g as c_uint, b as c_uint, a as c_uint) != 0 }
    }

    pub fn set_stroke_width(&self, width: f32) -> bool {
        unsafe { (self.api.FPDFPageObj_SetStrokeWidth)(self.raw, width) != 0 }
    }

    /// Replaces the image object's pixel data (the image XObject stream is
    /// rewritten in place, so the previous data is not kept in the document).
    pub fn set_image_bitmap(&self, pages: &mut [FPDF_PAGE], bitmap: FPDF_BITMAP) -> bool {
        let p_ptr = if pages.is_empty() { ptr::null_mut() } else { pages.as_mut_ptr() };
        unsafe { (self.api.FPDFImageObj_SetBitmap)(p_ptr, pages.len() as c_int, self.raw, bitmap) != 0 }
    }

    /// Decoded image pixels as a PDFium bitmap (caller owns it).
    pub fn get_image_bitmap(&self) -> Option<PdfBitmap> {
        let raw = unsafe { (self.api.FPDFImageObj_GetBitmap)(self.raw) };
        (!raw.is_null()).then(|| PdfBitmap { raw, api: Arc::clone(&self.api) })
    }

    pub fn set_image_matrix(&self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> bool {
        unsafe { (self.api.FPDFImageObj_SetMatrix)(self.raw, a, b, c, d, e, f) != 0 }
    }
}

// --- Owned bitmap ---

/// A PDFium-allocated bitmap, destroyed on drop.
pub struct PdfBitmap {
    pub raw: FPDF_BITMAP,
    api: Arc<PdfiumApi>,
}

impl Drop for PdfBitmap {
    fn drop(&mut self) {
        unsafe { (self.api.FPDFBitmap_Destroy)(self.raw) };
    }
}

impl PdfBitmap {
    /// New PDFium-owned bitmap; `alpha` selects BGRA instead of BGRx.
    pub fn new(doc: &PdfDocument, width: i32, height: i32, alpha: bool) -> Option<PdfBitmap> {
        let raw = unsafe { (doc.api.FPDFBitmap_Create)(width, height, alpha as c_int) };
        (!raw.is_null()).then(|| PdfBitmap { raw, api: Arc::clone(&doc.api) })
    }

    pub fn width(&self) -> i32 {
        unsafe { (self.api.FPDFBitmap_GetWidth)(self.raw) }
    }

    pub fn height(&self) -> i32 {
        unsafe { (self.api.FPDFBitmap_GetHeight)(self.raw) }
    }

    pub fn stride(&self) -> i32 {
        unsafe { (self.api.FPDFBitmap_GetStride)(self.raw) }
    }

    /// One of the `FPDFBitmap_*` format constants.
    pub fn format(&self) -> c_int {
        unsafe { (self.api.FPDFBitmap_GetFormat)(self.raw) }
    }

    /// Bytes per pixel for the bitmap's format.
    pub fn bytes_per_pixel(&self) -> usize {
        match self.format() {
            FPDFBitmap_Gray => 1,
            FPDFBitmap_BGR => 3,
            FPDFBitmap_BGRx | FPDFBitmap_BGRA => 4,
            _ => 0,
        }
    }

    /// The pixel buffer (`stride * height` bytes).
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        let len = self.stride().max(0) as usize * self.height().max(0) as usize;
        let ptr = unsafe { (self.api.FPDFBitmap_GetBuffer)(self.raw) };
        if ptr.is_null() || len == 0 {
            return &mut [];
        }
        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }
}
