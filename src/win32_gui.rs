// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Native Win32 / GDI double-buffered GUI with Modern Fluent Dark UI

#![allow(non_snake_case, non_camel_case_types, dead_code, unused_variables)]

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Arc;

use crate::editor::{
    apply_text_mutation, hit_test_text_object, ActiveTextEditor, EditAction, ToolMode,
    UndoManager,
};
use crate::pdfium::{PdfDocument, PdfiumLibrary, RectF};
use crate::tile_cache::{ThumbnailCache, TileCache};

// Win32 Types
pub type HWND = *mut c_void;
pub type HDC = *mut c_void;
pub type HBITMAP = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HFONT = *mut c_void;
pub type HPEN = *mut c_void;
pub type HCURSOR = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type HGDIOBJ = *mut c_void;
pub type COLORREF = u32;
pub type LPARAM = isize;
pub type WPARAM = usize;
pub type LRESULT = isize;

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RECT {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

#[repr(C)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: i32,
    pub rcPaint: RECT,
    pub fRestore: i32,
    pub fIncUpdate: i32,
    pub rgbReserved: [u8; 32],
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
}

pub type WNDPROC = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: *mut c_void,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: *mut c_void,
}

#[repr(C)]
pub struct BITMAPINFOHEADER {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
}

#[repr(C)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [u32; 1],
}

#[repr(C)]
pub struct OPENFILENAMEW {
    pub lStructSize: u32,
    pub hwndOwner: HWND,
    pub hInstance: HINSTANCE,
    pub lpstrFilter: *const u16,
    pub lpstrCustomFilter: *mut u16,
    pub nMaxCustFilter: u32,
    pub nFilterIndex: u32,
    pub lpstrFile: *mut u16,
    pub nMaxFile: u32,
    pub lpstrFileTitle: *mut u16,
    pub nMaxFileTitle: u32,
    pub lpstrInitialDir: *const u16,
    pub lpstrTitle: *const u16,
    pub Flags: u32,
    pub nFileOffset: u16,
    pub nFileExtension: u16,
    pub lpstrDefExt: *const u16,
    pub lCustData: LPARAM,
    pub lpfnHook: *mut c_void,
    pub lpTemplateName: *const u16,
    pub pvReserved: *mut c_void,
    pub dwReserved: u32,
    pub FlagsEx: u32,
}

#[repr(C)]
pub struct TRACKMOUSEEVENT {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub hwndTrack: HWND,
    pub dwHoverTime: u32,
}

// Win32 Constants
pub const WM_CREATE: u32 = 0x0001;
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_SIZE: u32 = 0x0005;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WM_SETCURSOR: u32 = 0x0020;
pub const WM_SETFONT: u32 = 0x0030;
pub const WM_SETICON: u32 = 0x0080;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
pub const WM_CHAR: u32 = 0x0102;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_CTLCOLOREDIT: u32 = 0x0133;
pub const WM_CTLCOLORSTATIC: u32 = 0x0138;
pub const WM_MOUSEMOVE: u32 = 0x0200;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_RBUTTONDOWN: u32 = 0x0204;
pub const WM_RBUTTONUP: u32 = 0x0205;
pub const WM_MOUSEWHEEL: u32 = 0x020A;
pub const WM_DROPFILES: u32 = 0x0233;
pub const WM_MOUSELEAVE: u32 = 0x02A3;

pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
pub const WS_VISIBLE: u32 = 0x10000000;
pub const WS_CHILD: u32 = 0x40000000;
pub const WS_CLIPCHILDREN: u32 = 0x02000000;
pub const WS_BORDER: u32 = 0x00800000;
pub const ES_AUTOHSCROLL: u32 = 0x0080;

pub const CW_USEDEFAULT: i32 = 0x80000000u32 as i32;
pub const SW_SHOW: i32 = 5;
pub const SRCCOPY: u32 = 0x00CC0020;
pub const DIB_RGB_COLORS: u32 = 0;
pub const TRANSPARENT: i32 = 1;
pub const OPAQUE: i32 = 2;

pub const IDC_ARROW: *const u16 = 32512 as *const u16;
pub const IDC_IBEAM: *const u16 = 32513 as *const u16;
pub const IDC_HAND: *const u16 = 32649 as *const u16;

pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
pub const OFN_FILEMUSTEXIST: u32 = 0x00001000;
pub const OFN_PATHMUSTEXIST: u32 = 0x00000800;
pub const OFN_OVERWRITEPROMPT: u32 = 0x00000002;
pub const TME_LEAVE: u32 = 0x00000002;
pub const EN_KILLFOCUS: u16 = 0x0200;
pub const GWLP_WNDPROC: i32 = -4;

#[link(name = "user32")]
extern "system" {
    fn SetProcessDpiAwarenessContext(value: isize) -> i32;
    fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> u16;
    fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: *mut c_void,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    fn DestroyWindow(hWnd: HWND) -> i32;
    fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> i32;
    fn UpdateWindow(hWnd: HWND) -> i32;
    fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn PostQuitMessage(nExitCode: i32);
    fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    fn TranslateMessage(lpMsg: *const MSG) -> i32;
    fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> i32;
    fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: i32) -> i32;
    fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> i32;
    fn SetWindowTextW(hWnd: HWND, lpString: *const u16) -> i32;
    fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
    fn SetCapture(hWnd: HWND) -> HWND;
    fn ReleaseCapture() -> i32;
    fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: *const u16) -> HCURSOR;
    fn LoadIconW(hInstance: HINSTANCE, lpIconName: *const u16) -> *mut c_void;
    fn SetCursor(hCursor: HCURSOR) -> HCURSOR;
    fn SetFocus(hWnd: HWND) -> HWND;
    fn SendMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: isize) -> isize;
    fn CallWindowProcW(lpPrevWndFunc: isize, hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn TrackMouseEvent(lpEventTrack: *mut TRACKMOUSEEVENT) -> i32;
    fn GetKeyState(nVirtKey: i32) -> i16;
    fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> i32;
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    fn MessageBoxW(hWnd: HWND, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
    fn DrawIconEx(
        hdc: HDC,
        xLeft: i32,
        yTop: i32,
        hIcon: *mut c_void,
        cxWidth: i32,
        cyWidth: i32,
        istepIfAniCur: u32,
        hbrFlickerFreeDraw: HBRUSH,
        diFlags: u32,
    ) -> i32;
}

pub fn show_message_box(hwnd: HWND, caption: &str, text: &str, u_type: u32) -> i32 {
    let wide_caption = to_wide_null(caption);
    let wide_text = to_wide_null(text);
    unsafe { MessageBoxW(hwnd, wide_text.as_ptr(), wide_caption.as_ptr(), u_type) }
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: HDC) -> HDC;
    fn DeleteDC(hdc: HDC) -> i32;
    fn CreateCompatibleBitmap(hdc: HDC, cx: i32, cy: i32) -> HBITMAP;
    fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    fn DeleteObject(ho: HGDIOBJ) -> i32;
    fn BitBlt(hdc: HDC, x: i32, y: i32, cx: i32, cy: i32, hdcSrc: HDC, x1: i32, y1: i32, rop: u32) -> i32;
    fn StretchDIBits(
        hdc: HDC,
        xDest: i32,
        yDest: i32,
        DestWidth: i32,
        DestHeight: i32,
        xSrc: i32,
        ySrc: i32,
        SrcWidth: i32,
        SrcHeight: i32,
        lpBits: *const c_void,
        lpbmi: *const BITMAPINFO,
        iUsage: u32,
        rop: u32,
    ) -> i32;
    fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    fn SetBkColor(hdc: HDC, color: COLORREF) -> COLORREF;
    fn SetTextColor(hdc: HDC, color: COLORREF) -> COLORREF;
    fn TextOutW(hdc: HDC, x: i32, y: i32, lpString: *const u16, c: i32) -> i32;
    fn CreateSolidBrush(color: COLORREF) -> HBRUSH;
    fn FillRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    fn FrameRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    fn CreatePen(iStyle: i32, cWidth: i32, color: COLORREF) -> HPEN;
    fn RoundRect(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32, width: i32, height: i32) -> i32;
    fn CreateFontW(
        cHeight: i32,
        cWidth: i32,
        cEscapement: i32,
        cOrientation: i32,
        cWeight: i32,
        bItalic: u32,
        bUnderline: u32,
        bStrikeOut: u32,
        iCharSet: u32,
        iOutPrecision: u32,
        iClipPrecision: u32,
        iQuality: u32,
        iPitchAndFamily: u32,
        pszFaceName: *const u16,
    ) -> HFONT;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: HWND,
        dwAttribute: u32,
        pvAttribute: *const c_void,
        cbAttribute: u32,
    ) -> i32;
}

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
    fn GetSaveFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn DragAcceptFiles(hWnd: HWND, fAccept: i32);
    fn DragQueryFileW(hDrop: *mut c_void, iFile: u32, lpszFile: *mut u16, cch: u32) -> u32;
    fn DragFinish(hDrop: *mut c_void);
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
}

fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct PageLayout {
    pub index: usize,
    pub width_pt: f64,
    pub height_pt: f64,
    pub render_w: i32,
    pub render_h: i32,
    pub y_offset: i32,
}

pub struct AppState {
    pub hwnd: HWND,
    pub pdfium: Arc<PdfiumLibrary>,
    pub doc: Option<PdfDocument>,
    pub current_file: Option<PathBuf>,
    pub tile_cache: TileCache,
    pub thumb_cache: ThumbnailCache,
    pub undo_manager: UndoManager,

    // Layout
    pub sidebar_width: i32,
    pub toolbar_height: i32,
    pub status_height: i32,
    pub zoom_pct: u32,
    pub scroll_x: i32,
    pub scroll_y: i32,
    pub sidebar_scroll_y: i32,
    pub sidebar_total_height: i32,
    pub active_page_idx: usize,
    pub viewport_width: i32,
    pub viewport_height: i32,
    pub tool_mode: ToolMode,
    pub page_layouts: Vec<PageLayout>,
    pub total_content_height: i32,

    // Mouse & Interaction
    pub hovered_button: Option<usize>,
    pub hovered_edit_rect: Option<RECT>,
    pub is_dragging: bool,
    pub drag_start_x: i32,
    pub drag_start_y: i32,
    pub drag_start_scroll_x: i32,
    pub drag_start_scroll_y: i32,
    pub active_editor: Option<ActiveTextEditor>,
    pub edit_hwnd: HWND,
    pub edit_font: HFONT,
    pub edit_bg_brush: HBRUSH,
    pub welcome_btn_rect: RECT,
    pub app_icon: *mut c_void,
}

static mut GLOBAL_STATE: *mut AppState = ptr::null_mut();
static mut OLD_EDIT_PROC: isize = 0;

unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_KEYDOWN => {
            if wparam == 0x0D {
                // Enter: commit edit
                if !GLOBAL_STATE.is_null() {
                    commit_active_text_editor(&mut *GLOBAL_STATE);
                }
                return 0;
            } else if wparam == 0x1B {
                // Escape: cancel edit
                if !GLOBAL_STATE.is_null() {
                    cancel_active_text_editor(&mut *GLOBAL_STATE);
                }
                return 0;
            }
        }
        WM_CHAR => {
            // Suppress error beeps on Enter/Esc in single-line edit
            if wparam == 0x0D || wparam == 0x1B {
                return 0;
            }
        }
        _ => {}
    }
    CallWindowProcW(OLD_EDIT_PROC, hwnd, msg, wparam, lparam)
}

impl AppState {
    pub fn new(hwnd: HWND, pdfium: Arc<PdfiumLibrary>, app_icon: *mut c_void) -> Self {
        Self {
            hwnd,
            pdfium,
            doc: None,
            current_file: None,
            tile_cache: TileCache::new(128),
            thumb_cache: ThumbnailCache::new(),
            undo_manager: UndoManager::new(),
            sidebar_width: 200,
            toolbar_height: 44,
            status_height: 24,
            zoom_pct: 100,
            scroll_x: 0,
            scroll_y: 0,
            sidebar_scroll_y: 0,
            sidebar_total_height: 0,
            active_page_idx: 0,
            viewport_width: 1280,
            viewport_height: 820,
            tool_mode: ToolMode::Pan,
            page_layouts: Vec::new(),
            total_content_height: 0,
            hovered_button: None,
            hovered_edit_rect: None,
            is_dragging: false,
            drag_start_x: 0,
            drag_start_y: 0,
            drag_start_scroll_x: 0,
            drag_start_scroll_y: 0,
            active_editor: None,
            edit_hwnd: ptr::null_mut(),
            edit_font: ptr::null_mut(),
            edit_bg_brush: unsafe { CreateSolidBrush(0x00262020) }, // #202026 dark background
            welcome_btn_rect: RECT::default(),
            app_icon,
        }
    }

    pub fn open_file(&mut self, path: &Path) -> Result<(), String> {
        let doc = self.pdfium.open_document(path, None)?;
        self.doc = Some(doc);
        self.current_file = Some(path.to_path_buf());
        self.tile_cache.clear();
        self.thumb_cache.clear();
        self.recompute_layouts();
        self.scroll_y = 0;
        self.scroll_x = 0;
        self.sidebar_scroll_y = 0;
        self.active_page_idx = 0;
        self.update_title();
        self.invalidate();
        Ok(())
    }

    pub fn clamp_scroll(&mut self) {
        let avail_w = self.viewport_width - self.sidebar_width;
        let max_page_w = self.page_layouts.iter().map(|l| l.render_w).max().unwrap_or(0);
        let max_scroll_x = (max_page_w + 48 - avail_w).max(0);
        self.scroll_x = self.scroll_x.clamp(0, max_scroll_x);

        let avail_h = self.viewport_height - self.toolbar_height - self.status_height;
        let max_scroll_y = (self.total_content_height - avail_h).max(0);
        self.scroll_y = self.scroll_y.clamp(0, max_scroll_y);

        let max_sidebar_scroll = (self.sidebar_total_height - avail_h).max(0);
        self.sidebar_scroll_y = self.sidebar_scroll_y.clamp(0, max_sidebar_scroll);
    }

    pub fn recompute_layouts(&mut self) {
        self.page_layouts.clear();
        let Some(doc) = &self.doc else {
            self.total_content_height = 0;
            self.sidebar_total_height = 0;
            self.scroll_x = 0;
            self.scroll_y = 0;
            self.sidebar_scroll_y = 0;
            return;
        };

        let page_count = doc.page_count();
        let scale = self.zoom_pct as f64 / 100.0;
        let mut y = 24;
        let gap = 24;

        let mut sidebar_y = 28;
        let thumb_max_w = 144;
        let thumb_max_h = 160;

        for i in 0..page_count {
            if let Ok(page) = doc.load_page(i) {
                let (w_pt, h_pt) = page.dimensions();
                let rot = page.rotation();
                let (eff_w, eff_h) = if rot == 1 || rot == 3 {
                    (h_pt, w_pt)
                } else {
                    (w_pt, h_pt)
                };

                let render_w = (eff_w * scale * 1.333333).round() as i32;
                let render_h = (eff_h * scale * 1.333333).round() as i32;

                self.page_layouts.push(PageLayout {
                    index: i,
                    width_pt: eff_w,
                    height_pt: eff_h,
                    render_w,
                    render_h,
                    y_offset: y,
                });

                y += render_h + gap;

                let aspect = eff_h / eff_w.max(1.0);
                let mut th = ((thumb_max_w as f64) * aspect).round() as i32;
                if th > thumb_max_h { th = thumb_max_h; }
                if th < 40 { th = 40; }
                let slot_h = th + 34;
                sidebar_y += slot_h;
            }
        }
        self.total_content_height = y;
        self.sidebar_total_height = sidebar_y;
        self.clamp_scroll();
    }

    pub fn update_title(&self) {
        let title = match &self.current_file {
            Some(path) => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Untitled.pdf");
                let pages = self.doc.as_ref().map_or(0, |d| d.page_count());
                format!("{} ({} pages, {}%) — zPDF", name, pages, self.zoom_pct)
            }
            None => "zPDF".to_string(),
        };
        let wide = to_wide_null(&title);
        unsafe { SetWindowTextW(self.hwnd, wide.as_ptr()) };
    }

    pub fn invalidate(&self) {
        unsafe { InvalidateRect(self.hwnd, ptr::null(), 0) };
    }

    pub fn save_current(&mut self) -> Result<(), String> {
        if let (Some(doc), Some(path)) = (&self.doc, &self.current_file) {
            doc.save_to_file(path)?;
            self.update_title();
            Ok(())
        } else {
            self.save_as()
        }
    }

    pub fn save_as(&mut self) -> Result<(), String> {
        let Some(doc) = &self.doc else {
            return Err("No open document to save".to_string());
        };

        let mut file_buf = [0u16; 1024];
        let filter = to_wide_null("PDF Files (*.pdf)\0*.pdf\0All Files (*.*)\0*.*\0");
        let def_ext = to_wide_null("pdf");
        let title = to_wide_null("Save PDF As");

        let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = self.hwnd;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = file_buf.as_mut_ptr();
        ofn.nMaxFile = file_buf.len() as u32;
        ofn.lpstrDefExt = def_ext.as_ptr();
        ofn.lpstrTitle = title.as_ptr();
        ofn.Flags = OFN_OVERWRITEPROMPT;

        if unsafe { GetSaveFileNameW(&mut ofn) } != 0 {
            let end = file_buf.iter().position(|&c| c == 0).unwrap_or(file_buf.len());
            let path_str = String::from_utf16_lossy(&file_buf[..end]);
            let path = PathBuf::from(path_str);
            doc.save_to_file(&path)?;
            self.current_file = Some(path);
            self.update_title();
            Ok(())
        } else {
            Ok(())
        }
    }

    pub fn prompt_open_file(&mut self) {
        let mut file_buf = [0u16; 1024];
        let filter = to_wide_null("PDF Files (*.pdf)\0*.pdf\0All Files (*.*)\0*.*\0");
        let title = to_wide_null("Open PDF Document");

        let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = self.hwnd;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = file_buf.as_mut_ptr();
        ofn.nMaxFile = file_buf.len() as u32;
        ofn.lpstrTitle = title.as_ptr();
        ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST;

        if unsafe { GetOpenFileNameW(&mut ofn) } != 0 {
            let end = file_buf.iter().position(|&c| c == 0).unwrap_or(file_buf.len());
            let path_str = String::from_utf16_lossy(&file_buf[..end]);
            let path = PathBuf::from(path_str);
            let _ = self.open_file(&path);
        }
    }

    pub fn rotate_current_page(&mut self) {
        let Some(doc) = &mut self.doc else { return };
        if let Some(layout) = self.page_layouts.get(self.active_page_idx) {
            let idx = layout.index;
            if let Ok(page) = doc.load_page(idx) {
                let old_rot = page.rotation();
                page.rotate_clockwise();
                let new_rot = page.rotation();
                self.undo_manager.push(EditAction::RotatePage {
                    page_index: idx,
                    old_rotation: old_rot,
                    new_rotation: new_rot,
                });
                self.tile_cache.invalidate_page(idx);
                self.thumb_cache.invalidate(idx);
                self.recompute_layouts();
                self.invalidate();
            }
        }
    }

    pub fn delete_current_page(&mut self) {
        let (ok, count) = {
            let Some(doc) = &mut self.doc else { return };
            if doc.page_count() <= 1 {
                return;
            }
            let del_idx = self.active_page_idx.min(doc.page_count() - 1);
            let ok = doc.delete_page(del_idx);
            (ok, doc.page_count())
        };
        if ok {
            self.tile_cache.clear();
            self.thumb_cache.clear();
            self.recompute_layouts();
            if self.active_page_idx >= count {
                self.active_page_idx = count.saturating_sub(1);
            }
            self.invalidate();
        }
    }

    pub fn set_zoom(&mut self, new_zoom: u32) {
        let clamped = new_zoom.clamp(25, 500);
        if clamped != self.zoom_pct {
            self.zoom_pct = clamped;
            self.recompute_layouts();
            self.update_title();
            self.invalidate();
        }
    }
}

// In-Place Text Editing
pub fn commit_active_text_editor(state: &mut AppState) {
    if state.edit_hwnd.is_null() {
        return;
    }

    let mut buf = [0u16; 4096];
    let len = unsafe { GetWindowTextW(state.edit_hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    let new_text = if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        String::new()
    };

    if let (Some(editor), Some(doc)) = (state.active_editor.take(), &state.doc) {
        if let Some(obj_idx) = editor.object_index {
            // Existing text object mutation
            if new_text != editor.original_text {
                let _ = apply_text_mutation(doc, editor.page_index, obj_idx, &new_text);
                state.undo_manager.push(EditAction::EditTextRun {
                    page_index: editor.page_index,
                    object_index: obj_idx,
                    old_text: editor.original_text,
                    new_text,
                });
                state.tile_cache.invalidate_page(editor.page_index);
                state.thumb_cache.invalidate(editor.page_index);
                state.invalidate();
            }
        } else if !new_text.trim().is_empty() {
            // New text object insertion
            if let Ok(new_obj_idx) = crate::editor::insert_new_text_run(
                doc,
                editor.page_index,
                &new_text,
                editor.insert_pos.0,
                editor.insert_pos.1,
                editor.font_size,
            ) {
                state.undo_manager.push(EditAction::InsertTextRun {
                    page_index: editor.page_index,
                    object_index: new_obj_idx,
                    text: new_text,
                });
                state.tile_cache.invalidate_page(editor.page_index);
                state.thumb_cache.invalidate(editor.page_index);
                state.invalidate();
            }
        }
    }

    unsafe {
        DestroyWindow(state.edit_hwnd);
        state.edit_hwnd = ptr::null_mut();
        if !state.edit_font.is_null() {
            DeleteObject(state.edit_font as HGDIOBJ);
            state.edit_font = ptr::null_mut();
        }
        SetFocus(state.hwnd);
    }
}

pub fn cancel_active_text_editor(state: &mut AppState) {
    state.active_editor = None;
    if !state.edit_hwnd.is_null() {
        unsafe {
            DestroyWindow(state.edit_hwnd);
            state.edit_hwnd = ptr::null_mut();
            if !state.edit_font.is_null() {
                DeleteObject(state.edit_font as HGDIOBJ);
                state.edit_font = ptr::null_mut();
            }
            SetFocus(state.hwnd);
        }
    }
}

pub fn handle_undo(state: &mut AppState) {
    commit_active_text_editor(state);
    let Some(action) = state.undo_manager.pop_undo() else { return };
    match action {
        EditAction::RotatePage { page_index, old_rotation, .. } => {
            if let Some(doc) = &state.doc {
                if let Ok(page) = doc.load_page(page_index) {
                    page.set_rotation(old_rotation);
                    state.tile_cache.invalidate_page(page_index);
                    state.thumb_cache.invalidate(page_index);
                    state.recompute_layouts();
                    state.invalidate();
                }
            }
        }
        EditAction::EditTextRun { page_index, object_index, old_text, .. } => {
            if let Some(doc) = &state.doc {
                let _ = apply_text_mutation(doc, page_index, object_index, &old_text);
                state.tile_cache.invalidate_page(page_index);
                state.thumb_cache.invalidate(page_index);
                state.invalidate();
            }
        }
        EditAction::InsertTextRun { page_index, object_index, .. } => {
            if let Some(doc) = &state.doc {
                let _ = apply_text_mutation(doc, page_index, object_index, "");
                state.tile_cache.invalidate_page(page_index);
                state.thumb_cache.invalidate(page_index);
                state.invalidate();
            }
        }
        _ => {}
    }
}

pub fn handle_redo(state: &mut AppState) {
    commit_active_text_editor(state);
    let Some(action) = state.undo_manager.pop_redo() else { return };
    match action {
        EditAction::RotatePage { page_index, new_rotation, .. } => {
            if let Some(doc) = &state.doc {
                if let Ok(page) = doc.load_page(page_index) {
                    page.set_rotation(new_rotation);
                    state.tile_cache.invalidate_page(page_index);
                    state.thumb_cache.invalidate(page_index);
                    state.recompute_layouts();
                    state.invalidate();
                }
            }
        }
        EditAction::EditTextRun { page_index, object_index, new_text, .. } => {
            if let Some(doc) = &state.doc {
                let _ = apply_text_mutation(doc, page_index, object_index, &new_text);
                state.tile_cache.invalidate_page(page_index);
                state.thumb_cache.invalidate(page_index);
                state.invalidate();
            }
        }
        EditAction::InsertTextRun { page_index, object_index, text } => {
            if let Some(doc) = &state.doc {
                let _ = apply_text_mutation(doc, page_index, object_index, &text);
                state.tile_cache.invalidate_page(page_index);
                state.thumb_cache.invalidate(page_index);
                state.invalidate();
            }
        }
        _ => {}
    }
}

fn handle_text_edit_click(state: &mut AppState, client_x: i32, client_y: i32) {
    commit_active_text_editor(state);
    let Some(doc) = &state.doc else { return };

    let avail_w = state.viewport_width - state.sidebar_width;

    for layout in &state.page_layouts {
        let page_x = if avail_w > layout.render_w {
            state.sidebar_width + (avail_w - layout.render_w) / 2
        } else {
            state.sidebar_width + 24 - state.scroll_x
        };
        let page_y = state.toolbar_height - state.scroll_y + layout.y_offset;

        if client_x >= page_x
            && client_x <= page_x + layout.render_w
            && client_y >= page_y
            && client_y <= page_y + layout.render_h
        {
            let Ok(page) = doc.load_page(layout.index) else { break };
            let rot = page.rotation();
            let dev_x = client_x - page_x;
            let dev_y = client_y - page_y;

            let Some((pdf_x, pdf_y)) = page.device_to_page(0, 0, layout.render_w, layout.render_h, rot, dev_x, dev_y) else {
                break;
            };

            let hit = hit_test_text_object(&page, pdf_x as f32, pdf_y as f32, 8.0);

            let (obj_idx, initial_text, screen_x, screen_y, screen_w, screen_h, bounds, font_size): (Option<usize>, String, i32, i32, i32, i32, RectF, f32) = if let Some((idx, text, bounds)) = hit {
                let pt1 = page.page_to_device(0, 0, layout.render_w, layout.render_h, rot, bounds.left as f64, bounds.top as f64);
                let pt2 = page.page_to_device(0, 0, layout.render_w, layout.render_h, rot, bounds.right as f64, bounds.bottom as f64);

                let (bx, by, bw, bh) = match (pt1, pt2) {
                    (Some((x1, y1)), Some((x2, y2))) => {
                        let sx = page_x + x1.min(x2);
                        let sy = page_y + y1.min(y2);
                        let sw = (x1 - x2).abs().max(60) + 12;
                        let sh = (y1 - y2).abs().max(20) + 6;
                        (sx, sy, sw, sh)
                    }
                    _ => (client_x - 4, client_y - 10, 160, 26),
                };

                let fsize = page.get_object(idx).and_then(|o| o.get_font_size()).unwrap_or(14.0);
                (Some(idx), text, bx, by, bw, bh, bounds, fsize)
            } else {
                let default_bounds = RectF {
                    left: pdf_x as f32,
                    bottom: pdf_y as f32,
                    right: (pdf_x + 120.0) as f32,
                    top: (pdf_y + 16.0) as f32,
                };
                (None, String::new(), client_x, client_y, 160, 26, default_bounds, 14.0)
            };

            let edit_class = to_wide_null("EDIT");
            let text_w = to_wide_null(&initial_text);

            unsafe {
                let edit_ctl = CreateWindowExW(
                    0,
                    edit_class.as_ptr(),
                    text_w.as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL,
                    screen_x,
                    screen_y,
                    screen_w,
                    screen_h,
                    state.hwnd,
                    1001 as *mut c_void,
                    GetModuleHandleW(ptr::null()),
                    ptr::null_mut(),
                );

                if !edit_ctl.is_null() {
                    let font = CreateFontW(
                        (screen_h - 6).max(14),
                        0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0,
                        to_wide_null("Segoe UI").as_ptr(),
                    );
                    SendMessageW(edit_ctl, WM_SETFONT, font as WPARAM, 1);
                    state.edit_font = font;
                    OLD_EDIT_PROC = SetWindowLongPtrW(edit_ctl, GWLP_WNDPROC, edit_subclass_proc as *const () as isize);
                    SendMessageW(edit_ctl, 0x00B1, 0, -1);
                    SetFocus(edit_ctl);
                    state.edit_hwnd = edit_ctl;
                    state.active_editor = Some(ActiveTextEditor {
                        page_index: layout.index,
                        object_index: obj_idx,
                        original_text: initial_text.clone(),
                        current_text: initial_text,
                        bounds,
                        insert_pos: (pdf_x, pdf_y),
                        font_size,
                    });
                }
            }
            break;
        }
    }
}

// Window Procedure
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = unsafe {
        if GLOBAL_STATE.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        &mut *GLOBAL_STATE
    };

    match msg {
        WM_CREATE => {
            // Windows 11 Dark Mode Title Bar
            let dark_mode: i32 = 1;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const _ as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );
            DragAcceptFiles(hwnd, 1);
            0
        }

        WM_SIZE => {
            let w = (lparam & 0xFFFF) as i32;
            let h = ((lparam >> 16) & 0xFFFF) as i32;
            state.viewport_width = w;
            state.viewport_height = h;
            state.clamp_scroll();
            state.invalidate();
            0
        }

        WM_SETCURSOR => {
            let hit_test = (lparam & 0xFFFF) as i16;
            if hit_test == 1 { // HTCLIENT
                let mut pt = POINT { x: 0, y: 0 };
                unsafe {
                    GetCursorPos(&mut pt);
                    ScreenToClient(hwnd, &mut pt);
                }

                let cursor = if pt.y < state.toolbar_height || pt.x < state.sidebar_width || state.doc.is_none() {
                    unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) }
                } else {
                    match state.tool_mode {
                        ToolMode::Pan => unsafe { LoadCursorW(ptr::null_mut(), IDC_HAND) },
                        ToolMode::EditTextObject => unsafe { LoadCursorW(ptr::null_mut(), IDC_IBEAM) },
                        _ => unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) },
                    }
                };
                unsafe { SetCursor(cursor) };
                return 1;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        WM_ERASEBKGND => 1,

        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint_viewport(state, hdc);
            EndPaint(hwnd, &ps);
            0
        }

        WM_DROPFILES => {
            let hdrop = wparam as *mut c_void;
            let mut file_buf = [0u16; 1024];
            let count = DragQueryFileW(hdrop, 0, file_buf.as_mut_ptr(), file_buf.len() as u32);
            if count > 0 {
                let path_str = String::from_utf16_lossy(&file_buf[..count as usize]);
                let _ = state.open_file(&PathBuf::from(path_str));
            }
            DragFinish(hdrop);
            0
        }

        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
            let hdc_edit = wparam as HDC;
            SetTextColor(hdc_edit, 0x00FFFFFF);       // Crisp white text
            SetBkColor(hdc_edit, 0x00262020);         // #202026 dark background
            return state.edit_bg_brush as LRESULT;
        }

        WM_LBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            if y < state.toolbar_height {
                handle_toolbar_click(state, x, y);
                return 0;
            }

            if state.doc.is_none() {
                if state.welcome_btn_rect.contains(x, y) {
                    state.prompt_open_file();
                }
                return 0;
            }

            // Check if click was in sidebar
            if x < state.sidebar_width && y >= state.toolbar_height && y < state.viewport_height - state.status_height {
                handle_sidebar_click(state, x, y);
                return 0;
            }

            match state.tool_mode {
                ToolMode::Pan => {
                    commit_active_text_editor(state);
                    state.is_dragging = true;
                    state.drag_start_x = x;
                    state.drag_start_y = y;
                    state.drag_start_scroll_x = state.scroll_x;
                    state.drag_start_scroll_y = state.scroll_y;
                    SetCapture(hwnd);
                }
                ToolMode::EditTextObject => {
                    handle_text_edit_click(state, x, y);
                }
                _ => {}
            }
            0
        }

        WM_MOUSEMOVE => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            let mut tme = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 10,
            };
            TrackMouseEvent(&mut tme);

            if y < state.toolbar_height {
                let new_hover = get_toolbar_button_at(x, y);
                if new_hover != state.hovered_button {
                    state.hovered_button = new_hover;
                    state.invalidate();
                }
            } else if state.doc.is_none() {
                let on_btn = state.welcome_btn_rect.contains(x, y);
                let new_hover = if on_btn { Some(999) } else { None };
                if new_hover != state.hovered_button {
                    state.hovered_button = new_hover;
                    state.invalidate();
                }
            } else if state.hovered_button.is_some() {
                state.hovered_button = None;
                state.invalidate();
            }

            // Live hover highlight in Edit mode
            if state.tool_mode == ToolMode::EditTextObject && !state.is_dragging {
                let mut found_hover_rect = None;
                if let Some(doc) = &state.doc {
                    let avail_w = state.viewport_width - state.sidebar_width;
                    for layout in &state.page_layouts {
                        let page_x = if avail_w > layout.render_w {
                            state.sidebar_width + (avail_w - layout.render_w) / 2
                        } else {
                            state.sidebar_width + 24 - state.scroll_x
                        };
                        let page_y = state.toolbar_height - state.scroll_y + layout.y_offset;

                        if x >= page_x && x <= page_x + layout.render_w && y >= page_y && y <= page_y + layout.render_h {
                            if let Ok(page) = doc.load_page(layout.index) {
                                let rot = page.rotation();
                                let dev_x = x - page_x;
                                let dev_y = y - page_y;
                                if let Some((pdf_x, pdf_y)) = page.device_to_page(0, 0, layout.render_w, layout.render_h, rot, dev_x, dev_y) {
                                    if let Some((_, _, bounds)) = hit_test_text_object(&page, pdf_x as f32, pdf_y as f32, 6.0) {
                                        let pt1 = page.page_to_device(0, 0, layout.render_w, layout.render_h, rot, bounds.left as f64, bounds.top as f64);
                                        let pt2 = page.page_to_device(0, 0, layout.render_w, layout.render_h, rot, bounds.right as f64, bounds.bottom as f64);
                                        if let (Some((x1, y1)), Some((x2, y2))) = (pt1, pt2) {
                                            found_hover_rect = Some(RECT {
                                                left: page_x + x1.min(x2) - 2,
                                                top: page_y + y1.min(y2) - 2,
                                                right: page_x + x1.max(x2) + 2,
                                                bottom: page_y + y1.max(y2) + 2,
                                            });
                                        }
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
                if found_hover_rect != state.hovered_edit_rect {
                    state.hovered_edit_rect = found_hover_rect;
                    state.invalidate();
                }
            } else if state.hovered_edit_rect.is_some() {
                state.hovered_edit_rect = None;
                state.invalidate();
            }

            if state.is_dragging {
                let dx = x - state.drag_start_x;
                let dy = y - state.drag_start_y;
                let avail_w = state.viewport_width - state.sidebar_width;
                let max_page_w = state.page_layouts.iter().map(|l| l.render_w).max().unwrap_or(0);
                let max_scroll_x = (max_page_w + 48 - avail_w).max(0);
                let avail_h = state.viewport_height - state.toolbar_height - state.status_height;
                let max_scroll_y = (state.total_content_height - avail_h).max(0);

                state.scroll_x = (state.drag_start_scroll_x - dx).clamp(0, max_scroll_x);
                state.scroll_y = (state.drag_start_scroll_y - dy).clamp(0, max_scroll_y);
                state.invalidate();
            }
            0
        }

        WM_MOUSELEAVE => {
            let mut need_redraw = false;
            if state.hovered_button.is_some() {
                state.hovered_button = None;
                need_redraw = true;
            }
            if state.hovered_edit_rect.is_some() {
                state.hovered_edit_rect = None;
                need_redraw = true;
            }
            if need_redraw {
                state.invalidate();
            }
            0
        }

        WM_LBUTTONUP => {
            if state.is_dragging {
                state.is_dragging = false;
                ReleaseCapture();
            }
            0
        }

        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) & 0xFFFF) as i16;
            let ctrl = (wparam & 0x0008) != 0;

            let mut pt = POINT {
                x: (lparam & 0xFFFF) as i16 as i32,
                y: ((lparam >> 16) & 0xFFFF) as i16 as i32,
            };
            ScreenToClient(hwnd, &mut pt);

            if ctrl {
                if delta > 0 {
                    state.set_zoom(state.zoom_pct + 15);
                } else {
                    state.set_zoom(state.zoom_pct.saturating_sub(15));
                }
            } else if pt.x < state.sidebar_width && pt.y >= state.toolbar_height && pt.y < state.viewport_height - state.status_height {
                // Scroll thumbnail rail
                let step = 80;
                let avail_h = state.viewport_height - state.toolbar_height - state.status_height;
                let max_scroll = (state.sidebar_total_height - avail_h).max(0);
                if max_scroll > 0 {
                    if delta > 0 {
                        state.sidebar_scroll_y = (state.sidebar_scroll_y - step).clamp(0, max_scroll);
                    } else {
                        state.sidebar_scroll_y = (state.sidebar_scroll_y + step).clamp(0, max_scroll);
                    }
                    state.invalidate();
                }
            } else if state.doc.is_some() {
                commit_active_text_editor(state);
                let step = 64;
                let avail_h = state.viewport_height - state.toolbar_height - state.status_height;
                let max_scroll = (state.total_content_height - avail_h).max(0);
                if max_scroll > 0 {
                    if delta > 0 {
                        state.scroll_y = (state.scroll_y - step).clamp(0, max_scroll);
                    } else {
                        state.scroll_y = (state.scroll_y + step).clamp(0, max_scroll);
                    }
                    state.invalidate();
                }
            }
            0
        }

        WM_COMMAND => {
            let notify = ((wparam >> 16) & 0xFFFF) as u16;
            let cid = (wparam & 0xFFFF) as u16;
            if cid == 1001 && notify == EN_KILLFOCUS {
                commit_active_text_editor(state);
            }
            0
        }

        WM_KEYDOWN => {
            let vk = wparam as i32;
            let ctrl = (unsafe { GetKeyState(0x11) } as u16 & 0x8000) != 0;

            if ctrl {
                match vk {
                    0x4F => { state.prompt_open_file(); }
                    0x53 => { let _ = state.save_current(); }
                    0x5A => { handle_undo(state); }
                    0x59 => { handle_redo(state); }
                    0xBB | 0x6B => { state.set_zoom(state.zoom_pct + 15); }
                    0xBD | 0x6D => { state.set_zoom(state.zoom_pct.saturating_sub(15)); }
                    0x30 | 0x60 => { state.set_zoom(100); }
                    _ => {}
                }
            } else {
                match vk {
                    0x48 => { state.tool_mode = ToolMode::Pan; state.invalidate(); }
                    0x54 => { state.tool_mode = ToolMode::EditTextObject; state.invalidate(); }
                    0x52 => { state.rotate_current_page(); }
                    0x2E => { state.delete_current_page(); }
                    _ => {}
                }
            }
            0
        }

        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[derive(Clone, Copy)]
struct ToolbarButtonDef {
    pub label: &'static str,
    pub width: i32,
    pub is_tool_pan: bool,
    pub is_tool_edit: bool,
    pub is_group_start: bool,
}

const TOOLBAR_BUTTONS: [ToolbarButtonDef; 10] = [
    ToolbarButtonDef { label: "Open", width: 56, is_tool_pan: false, is_tool_edit: false, is_group_start: true },
    ToolbarButtonDef { label: "Save", width: 54, is_tool_pan: false, is_tool_edit: false, is_group_start: false },
    ToolbarButtonDef { label: "Undo", width: 54, is_tool_pan: false, is_tool_edit: false, is_group_start: true },
    ToolbarButtonDef { label: "Redo", width: 54, is_tool_pan: false, is_tool_edit: false, is_group_start: false },
    ToolbarButtonDef { label: "Hand", width: 56, is_tool_pan: true, is_tool_edit: false, is_group_start: true },
    ToolbarButtonDef { label: "Edit", width: 54, is_tool_pan: false, is_tool_edit: true, is_group_start: false },
    ToolbarButtonDef { label: "Rotate", width: 62, is_tool_pan: false, is_tool_edit: false, is_group_start: true },
    ToolbarButtonDef { label: "−", width: 32, is_tool_pan: false, is_tool_edit: false, is_group_start: true },
    ToolbarButtonDef { label: "100%", width: 52, is_tool_pan: false, is_tool_edit: false, is_group_start: false },
    ToolbarButtonDef { label: "+", width: 32, is_tool_pan: false, is_tool_edit: false, is_group_start: false },
];

fn get_toolbar_button_rect(index: usize, toolbar_h: i32) -> RECT {
    let mut current_x = 96; // Brand offset
    for (i, btn) in TOOLBAR_BUTTONS.iter().enumerate() {
        if btn.is_group_start && i > 0 {
            current_x += 12; // Extra divider spacing
        }
        if i == index {
            return RECT {
                left: current_x,
                top: 7,
                right: current_x + btn.width,
                bottom: toolbar_h - 7,
            };
        }
        current_x += btn.width + 4;
    }
    RECT::default()
}

fn get_toolbar_button_at(x: i32, y: i32) -> Option<usize> {
    if y > 44 {
        return None;
    }
    for i in 0..TOOLBAR_BUTTONS.len() {
        let r = get_toolbar_button_rect(i, 44);
        if r.contains(x, y) {
            return Some(i);
        }
    }
    None
}

fn handle_toolbar_click(state: &mut AppState, x: i32, y: i32) {
    if let Some(idx) = get_toolbar_button_at(x, y) {
        match idx {
            0 => state.prompt_open_file(),
            1 => { let _ = state.save_current(); },
            2 => handle_undo(state),
            3 => handle_redo(state),
            4 => { state.tool_mode = ToolMode::Pan; state.invalidate(); },
            5 => { state.tool_mode = ToolMode::EditTextObject; state.invalidate(); },
            6 => state.rotate_current_page(),
            7 => state.set_zoom(state.zoom_pct.saturating_sub(15)),
            8 => state.set_zoom(100),
            9 => state.set_zoom(state.zoom_pct + 15),
            _ => {}
        }
    }
}

fn handle_sidebar_click(state: &mut AppState, _x: i32, y: i32) {
    let Some(doc) = &state.doc else { return };
    let count = doc.page_count();
    let rel_y = y - state.toolbar_height + state.sidebar_scroll_y - 28; // Account for "PAGES" header
    if rel_y < 0 {
        return;
    }

    let mut current_y = 0;
    let thumb_max_w = 144;
    let thumb_max_h = 160;

    for i in 0..count {
        let (eff_w, eff_h) = if let Some(layout) = state.page_layouts.get(i) {
            (layout.width_pt, layout.height_pt)
        } else if let Ok(page) = doc.load_page(i) {
            let (w_pt, h_pt) = page.dimensions();
            let rot = page.rotation();
            if rot == 1 || rot == 3 { (h_pt, w_pt) } else { (w_pt, h_pt) }
        } else {
            (612.0, 792.0)
        };
        let aspect = eff_h / eff_w.max(1.0);
        let tw = thumb_max_w;
        let mut th = ((tw as f64) * aspect).round() as i32;
        if th > thumb_max_h { th = thumb_max_h; }
        if th < 40 { th = 40; }
        let slot_h = th + 34;

        if rel_y >= current_y && rel_y <= current_y + slot_h {
            state.active_page_idx = i;
            if let Some(layout) = state.page_layouts.get(i) {
                let avail_h = state.viewport_height - state.toolbar_height - state.status_height;
                let max_scroll_y = (state.total_content_height - avail_h).max(0);
                state.scroll_y = (layout.y_offset - 24).clamp(0, max_scroll_y);
                commit_active_text_editor(state);
                state.invalidate();
            }
            return;
        }
        current_y += slot_h;
    }
}

// Double-Buffered Painting
fn paint_viewport(state: &mut AppState, hdc: HDC) {
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(state.hwnd, &mut rect) };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return;
    }

    state.viewport_width = width;
    state.viewport_height = height;

    unsafe {
        let mem_dc = CreateCompatibleDC(hdc);
        let mem_bmp = CreateCompatibleBitmap(hdc, width, height);
        let old_bmp = SelectObject(mem_dc, mem_bmp as HGDIOBJ);

        // Deep Obsidian Slate Backdrop (#0D0D11)
        let bg_brush = CreateSolidBrush(0x00110D0D);
        FillRect(mem_dc, &rect, bg_brush);
        DeleteObject(bg_brush as HGDIOBJ);

        if state.doc.is_some() {
            // Render Document Pages
            paint_pages(state, mem_dc, width, height);
            // Render Left Sidebar (Live Thumbnails)
            paint_sidebar(state, mem_dc, height);
        } else {
            // Render Minimalist Welcome Dropzone
            paint_empty_state(state, mem_dc, width, height);
        }

        // Render Modern Segmented Top Toolbar
        paint_toolbar(state, mem_dc, width);

        // Render Minimalist Status Bar
        paint_status_bar(state, mem_dc, width, height);

        BitBlt(hdc, 0, 0, width, height, mem_dc, 0, 0, SRCCOPY);

        SelectObject(mem_dc, old_bmp);
        DeleteObject(mem_bmp as HGDIOBJ);
        DeleteDC(mem_dc);
    }
}

fn paint_empty_state(state: &mut AppState, hdc: HDC, width: i32, height: i32) {
    unsafe {
        let card_w = 460;
        let card_h = 280;
        let card_x = (width - card_w) / 2;
        let card_y = (height - card_h) / 2;

        // Container Card (#16161B with subtle 1px border #26262F)
        let card_brush = CreateSolidBrush(0x001B1616);
        let border_pen = CreatePen(0, 1, 0x002F2626);
        let old_brush = SelectObject(hdc, card_brush as HGDIOBJ);
        let old_pen = SelectObject(hdc, border_pen as HGDIOBJ);
        RoundRect(hdc, card_x, card_y, card_x + card_w, card_y + card_h, 16, 16);
        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        DeleteObject(card_brush as HGDIOBJ);
        DeleteObject(border_pen as HGDIOBJ);

        SetBkMode(hdc, TRANSPARENT);

        // Application Icon centered
        let icon_size = 48;
        let icon_x = card_x + (card_w - icon_size) / 2;
        let icon_y = card_y + 36;
        if !state.app_icon.is_null() {
            DrawIconEx(hdc, icon_x, icon_y, state.app_icon, icon_size, icon_size, 0, ptr::null_mut(), 0x0003);
        }

        // Crisp Title
        let title_font = CreateFontW(22, 0, 0, 0, 600, 0, 0, 0, 1, 0, 0, 5, 0, to_wide_null("Segoe UI").as_ptr());
        let old_f = SelectObject(hdc, title_font as HGDIOBJ);
        SetTextColor(hdc, 0x00F0EDED); // Crisp off-white #EDEDF0
        let title_w = to_wide_null("zPDF");
        TextOutW(hdc, card_x + 208, card_y + 98, title_w.as_ptr(), (title_w.len() - 1) as i32);
        SelectObject(hdc, old_f);
        DeleteObject(title_font as HGDIOBJ);

        // Subtitle
        let sub_font = CreateFontW(14, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, to_wide_null("Segoe UI").as_ptr());
        let old_f = SelectObject(hdc, sub_font as HGDIOBJ);
        SetTextColor(hdc, 0x009A9090); // Muted cool grey #90909A
        let sub_w = to_wide_null("Drop a PDF anywhere to begin");
        TextOutW(hdc, card_x + 138, card_y + 130, sub_w.as_ptr(), (sub_w.len() - 1) as i32);

        // Action CTA: "Open File"
        let btn_w = 150;
        let btn_h = 36;
        let btn_x = card_x + (card_w - btn_w) / 2;
        let btn_y = card_y + 172;
        state.welcome_btn_rect = RECT { left: btn_x, top: btn_y, right: btn_x + btn_w, bottom: btn_y + btn_h };

        let is_hovered = state.hovered_button == Some(999);
        let btn_color = if is_hovered { 0x003A3232 } else { 0x002D2424 }; // Sleek dark slate pill
        let btn_border = if is_hovered { 0x004C3E3E } else { 0x003B2F2F };
        let btn_brush = CreateSolidBrush(btn_color);
        let btn_pen = CreatePen(0, 1, btn_border);
        let old_b = SelectObject(hdc, btn_brush as HGDIOBJ);
        let old_p = SelectObject(hdc, btn_pen as HGDIOBJ);
        RoundRect(hdc, btn_x, btn_y, btn_x + btn_w, btn_y + btn_h, 8, 8);
        SelectObject(hdc, old_b);
        SelectObject(hdc, old_p);
        DeleteObject(btn_brush as HGDIOBJ);
        DeleteObject(btn_pen as HGDIOBJ);

        let btn_font = CreateFontW(14, 0, 0, 0, 600, 0, 0, 0, 1, 0, 0, 5, 0, to_wide_null("Segoe UI").as_ptr());
        let old_f2 = SelectObject(hdc, btn_font as HGDIOBJ);
        SetTextColor(hdc, 0x00F0EDED);
        let open_w = to_wide_null("Open File");
        TextOutW(hdc, btn_x + 44, btn_y + 8, open_w.as_ptr(), (open_w.len() - 1) as i32);
        SelectObject(hdc, old_f2);
        DeleteObject(btn_font as HGDIOBJ);

        // Keyboard Shortcut hint
        SetTextColor(hdc, 0x006C5E5E); // Subtle #5E5E6C
        let hint_w = to_wide_null("or press Ctrl + O");
        TextOutW(hdc, card_x + 180, btn_y + 44, hint_w.as_ptr(), (hint_w.len() - 1) as i32);

        SelectObject(hdc, old_f);
        DeleteObject(sub_font as HGDIOBJ);
    }
}

fn paint_toolbar(state: &AppState, hdc: HDC, width: i32) {
    unsafe {
        // Toolbar background (#141418)
        let tb_rect = RECT { left: 0, top: 0, right: width, bottom: state.toolbar_height };
        let tb_brush = CreateSolidBrush(0x00181414);
        FillRect(hdc, &tb_rect, tb_brush);
        DeleteObject(tb_brush as HGDIOBJ);

        // Bottom hairline border (#23232B)
        let sep_brush = CreateSolidBrush(0x002B2323);
        let sep_rect = RECT { left: 0, top: state.toolbar_height - 1, right: width, bottom: state.toolbar_height };
        FillRect(hdc, &sep_rect, sep_brush);

        SetBkMode(hdc, TRANSPARENT);

        // Branded Logo / Icon (22x22)
        if !state.app_icon.is_null() {
            DrawIconEx(hdc, 16, 11, state.app_icon, 22, 22, 0, ptr::null_mut(), 0x0003);
        }
        let brand_font = CreateFontW(15, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, to_wide_null("Segoe UI").as_ptr());
        let old_font = SelectObject(hdc, brand_font as HGDIOBJ);
        SetTextColor(hdc, 0x00F0EDED);
        let brand_text = to_wide_null("zPDF");
        TextOutW(hdc, 44, 11, brand_text.as_ptr(), (brand_text.len() - 1) as i32);
        SelectObject(hdc, old_font);
        DeleteObject(brand_font as HGDIOBJ);

        // Separator after brand
        let brand_sep = RECT { left: 86, top: 10, right: 87, bottom: state.toolbar_height - 10 };
        FillRect(hdc, &brand_sep, sep_brush);

        // Render Toolbar Buttons
        let btn_font = CreateFontW(13, 0, 0, 0, 500, 0, 0, 0, 1, 0, 0, 5, 0, to_wide_null("Segoe UI").as_ptr());
        let old_btn_f = SelectObject(hdc, btn_font as HGDIOBJ);

        for (i, btn) in TOOLBAR_BUTTONS.iter().enumerate() {
            let btn_rect = get_toolbar_button_rect(i, state.toolbar_height);

            // Group divider before button
            if btn.is_group_start && i > 0 {
                let div_x = btn_rect.left - 7;
                let div_rect = RECT { left: div_x, top: 12, right: div_x + 1, bottom: state.toolbar_height - 12 };
                FillRect(hdc, &div_rect, sep_brush);
            }

            let is_hovered = state.hovered_button == Some(i);
            let is_active = (btn.is_tool_pan && state.tool_mode == ToolMode::Pan)
                || (btn.is_tool_edit && state.tool_mode == ToolMode::EditTextObject);

            if is_active {
                // Active button pill (Sleek slate indigo #2B384E with #3B82F6 border)
                let active_brush = CreateSolidBrush(0x004E382B);
                let active_pen = CreatePen(0, 1, 0x00F6823B);
                let old_b = SelectObject(hdc, active_brush as HGDIOBJ);
                let old_p = SelectObject(hdc, active_pen as HGDIOBJ);
                RoundRect(hdc, btn_rect.left, btn_rect.top, btn_rect.right, btn_rect.bottom, 6, 6);
                SelectObject(hdc, old_b);
                SelectObject(hdc, old_p);
                DeleteObject(active_brush as HGDIOBJ);
                DeleteObject(active_pen as HGDIOBJ);
                SetTextColor(hdc, 0x00FFFFFF);
            } else if is_hovered {
                // Hover highlight (#262630)
                let hover_brush = CreateSolidBrush(0x00302626);
                let old_b = SelectObject(hdc, hover_brush as HGDIOBJ);
                RoundRect(hdc, btn_rect.left, btn_rect.top, btn_rect.right, btn_rect.bottom, 6, 6);
                SelectObject(hdc, old_b);
                DeleteObject(hover_brush as HGDIOBJ);
                SetTextColor(hdc, 0x00FFFFFF);
            } else {
                SetTextColor(hdc, 0x00C8C0C0);
            }

            let wide_label = to_wide_null(btn.label);
            // Center text inside button
            let text_x = if btn.width <= 36 {
                btn_rect.left + (btn.width - 10) / 2
            } else {
                btn_rect.left + 10
            };
            TextOutW(hdc, text_x, 13, wide_label.as_ptr(), (wide_label.len() - 1) as i32);
        }

        DeleteObject(sep_brush as HGDIOBJ);
        SelectObject(hdc, old_btn_f);
        DeleteObject(btn_font as HGDIOBJ);
    }
}

fn paint_status_bar(state: &AppState, hdc: HDC, width: i32, height: i32) {
    let sb_top = height - state.status_height;
    let sb_rect = RECT { left: 0, top: sb_top, right: width, bottom: height };

    unsafe {
        // Status bar background (#111114)
        let bar_brush = CreateSolidBrush(0x00141111);
        FillRect(hdc, &sb_rect, bar_brush);
        DeleteObject(bar_brush as HGDIOBJ);

        // Top hairline separator
        let sep_brush = CreateSolidBrush(0x00262020);
        let sep_r = RECT { left: 0, top: sb_top, right: width, bottom: sb_top + 1 };
        FillRect(hdc, &sep_r, sep_brush);
        DeleteObject(sep_brush as HGDIOBJ);

        SetBkMode(hdc, TRANSPARENT);
        let font = CreateFontW(12, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, to_wide_null("Segoe UI").as_ptr());
        let old_f = SelectObject(hdc, font as HGDIOBJ);
        SetTextColor(hdc, 0x008E8888);

        // Left: Clean document name
        let left_msg = match &state.current_file {
            Some(p) => p.file_name().and_then(|n| n.to_str()).unwrap_or("Document.pdf").to_string(),
            None => "Ready".to_string(),
        };
        let w_left = to_wide_null(&left_msg);
        TextOutW(hdc, 16, sb_top + 4, w_left.as_ptr(), (w_left.len() - 1) as i32);

        // Center: Page indicator
        if let Some(doc) = &state.doc {
            let total = doc.page_count();
            let current = (state.active_page_idx + 1).min(total);
            let center_msg = format!("Page {} of {}", current, total);
            let w_center = to_wide_null(&center_msg);
            let center_x = (width - 80) / 2;
            TextOutW(hdc, center_x, sb_top + 4, w_center.as_ptr(), (w_center.len() - 1) as i32);
        }

        // Right: Zoom and Tool
        let mode_str = match state.tool_mode {
            ToolMode::Pan => "Hand",
            ToolMode::EditTextObject => "Edit",
            _ => "View",
        };
        let right_msg = format!("{}%  •  {}", state.zoom_pct, mode_str);
        let w_right = to_wide_null(&right_msg);
        let right_x = (width - 120).max(width / 2 + 100);
        TextOutW(hdc, right_x, sb_top + 4, w_right.as_ptr(), (w_right.len() - 1) as i32);

        SelectObject(hdc, old_f);
        DeleteObject(font as HGDIOBJ);
    }
}

fn paint_sidebar(state: &mut AppState, hdc: HDC, height: i32) {
    let sb_rect = RECT {
        left: 0,
        top: state.toolbar_height,
        right: state.sidebar_width,
        bottom: height - state.status_height,
    };

    unsafe {
        // Sidebar background (#141418)
        let sb_brush = CreateSolidBrush(0x00181414);
        FillRect(hdc, &sb_rect, sb_brush);
        DeleteObject(sb_brush as HGDIOBJ);

        // Right hairline border (#23232B)
        let border_brush = CreateSolidBrush(0x002B2323);
        let border_rect = RECT {
            left: state.sidebar_width - 1,
            top: state.toolbar_height,
            right: state.sidebar_width,
            bottom: height - state.status_height,
        };
        FillRect(hdc, &border_rect, border_brush);
        DeleteObject(border_brush as HGDIOBJ);

        let Some(doc) = &state.doc else { return };
        let count = doc.page_count();

        // Header: "PAGES"
        SetBkMode(hdc, TRANSPARENT);
        let header_font = CreateFontW(11, 0, 0, 0, 600, 0, 0, 0, 1, 0, 0, 5, 0, to_wide_null("Segoe UI").as_ptr());
        let old_h_f = SelectObject(hdc, header_font as HGDIOBJ);
        SetTextColor(hdc, 0x007C6E6E); // Subtle grey #6E6E7C
        let header_text = to_wide_null("PAGES");
        TextOutW(hdc, 18, state.toolbar_height + 8, header_text.as_ptr(), (header_text.len() - 1) as i32);
        SelectObject(hdc, old_h_f);
        DeleteObject(header_font as HGDIOBJ);

        // Page number font
        let font = CreateFontW(12, 0, 0, 0, 500, 0, 0, 0, 1, 0, 0, 5, 0, to_wide_null("Segoe UI").as_ptr());
        let old_font = SelectObject(hdc, font as HGDIOBJ);

        let mut current_slot_y = 28; // Start below header
        let thumb_max_w = 144;
        let thumb_max_h = 160;

        for i in 0..count {
            // Compute true aspect ratio
            let (eff_w, eff_h) = if let Some(layout) = state.page_layouts.get(i) {
                (layout.width_pt, layout.height_pt)
            } else if let Ok(page) = doc.load_page(i) {
                let (w_pt, h_pt) = page.dimensions();
                let rot = page.rotation();
                if rot == 1 || rot == 3 { (h_pt, w_pt) } else { (w_pt, h_pt) }
            } else {
                (612.0, 792.0)
            };

            let aspect = eff_h / eff_w.max(1.0);
            let tw = thumb_max_w;
            let mut th = ((tw as f64) * aspect).round() as i32;
            if th > thumb_max_h {
                th = thumb_max_h;
            }
            if th < 40 {
                th = 40;
            }
            let slot_h = th + 34;

            let visual_y = state.toolbar_height + current_slot_y - state.sidebar_scroll_y;

            // Virtualized culling: skip rendering if outside visible sidebar bounds
            if visual_y + slot_h >= state.toolbar_height && visual_y <= height - state.status_height {
                let tx = (state.sidebar_width - tw) / 2;
                let ty = visual_y + 4;

                // 1. Drop shadow behind thumbnail card
                let shadow_rect = RECT {
                    left: tx + 2,
                    top: ty + 2,
                    right: tx + tw + 2,
                    bottom: ty + th + 2,
                };
                let shadow_brush = CreateSolidBrush(0x000E0A0A);
                FillRect(hdc, &shadow_rect, shadow_brush);
                DeleteObject(shadow_brush as HGDIOBJ);

                // 2. White paper card base (#FFFFFF)
                let card_rect = RECT { left: tx, top: ty, right: tx + tw, bottom: ty + th };
                let paper_brush = CreateSolidBrush(0x00FFFFFF);
                FillRect(hdc, &card_rect, paper_brush);
                DeleteObject(paper_brush as HGDIOBJ);

                // 3. Ensure thumbnail is rendered in cache
                if state.thumb_cache.get(i).is_none() {
                    if let Ok(page) = doc.load_page(i) {
                        let stride = tw * 4;
                        let mut buffer = vec![0u8; (stride * th) as usize];
                        let rot = page.rotation();
                        if page.render_into_buffer(&mut buffer, tw, th, stride, rot, 0).is_ok() {
                            state.thumb_cache.insert(i, tw, th, buffer);
                        }
                    }
                }

                // 4. Render cached thumbnail bitmap
                if let Some((_, _, pixels)) = state.thumb_cache.get(i) {
                    let bmi = BITMAPINFO {
                        bmiHeader: BITMAPINFOHEADER {
                            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                            biWidth: tw,
                            biHeight: -th, // Negative for top-down BGRA
                            biPlanes: 1,
                            biBitCount: 32,
                            biCompression: 0,
                            biSizeImage: 0,
                            biXPelsPerMeter: 0,
                            biYPelsPerMeter: 0,
                            biClrUsed: 0,
                            biClrImportant: 0,
                        },
                        bmiColors: [0],
                    };

                    StretchDIBits(
                        hdc,
                        tx,
                        ty,
                        tw,
                        th,
                        0,
                        0,
                        tw,
                        th,
                        pixels.as_ptr() as *const c_void,
                        &bmi,
                        0,
                        SRCCOPY,
                    );
                }

                // 5. Border / Selection ring
                let is_active = i == state.active_page_idx;
                let border_rect = RECT { left: tx, top: ty, right: tx + tw, bottom: ty + th };
                if is_active {
                    // 2px active selection ring (#3B82F6 Electric Blue)
                    let ring_brush = CreateSolidBrush(0x00F6823B);
                    FrameRect(hdc, &border_rect, ring_brush);
                    let inner_r = RECT { left: tx + 1, top: ty + 1, right: tx + tw - 1, bottom: ty + th - 1 };
                    FrameRect(hdc, &inner_r, ring_brush);
                    DeleteObject(ring_brush as HGDIOBJ);
                } else {
                    // 1px hairline border (#2A2A34)
                    let normal_brush = CreateSolidBrush(0x00342A2A);
                    FrameRect(hdc, &border_rect, normal_brush);
                    DeleteObject(normal_brush as HGDIOBJ);
                }

                // 6. Clean page number badge
                SetTextColor(hdc, if is_active { 0x00FFFFFF } else { 0x008E8282 });
                let label = format!("{}", i + 1);
                let wide = to_wide_null(&label);
                let label_x = tx + (tw - 8) / 2;
                TextOutW(hdc, label_x, ty + th + 6, wide.as_ptr(), (wide.len() - 1) as i32);
            }

            current_slot_y += slot_h;
        }

        state.sidebar_total_height = current_slot_y;

        SelectObject(hdc, old_font);
        DeleteObject(font as HGDIOBJ);
    }
}

fn paint_pages(state: &mut AppState, hdc: HDC, viewport_w: i32, viewport_h: i32) {
    let Some(doc) = &state.doc else { return };

    let avail_w = viewport_w - state.sidebar_width;
    let canvas_origin_y = state.toolbar_height - state.scroll_y;

    // Track active page currently visible in viewport
    let mut closest_dist = i32::MAX;
    let mut current_page = state.active_page_idx;

    for layout in &state.page_layouts {
        let page_y = canvas_origin_y + layout.y_offset;

        // Dynamic page horizontal centering
        let page_x = if avail_w > layout.render_w {
            state.sidebar_width + (avail_w - layout.render_w) / 2
        } else {
            state.sidebar_width + 24 - state.scroll_x
        };

        // Determine active page
        let dist = (page_y - state.toolbar_height).abs();
        if dist < closest_dist {
            closest_dist = dist;
            current_page = layout.index;
        }

        if page_y + layout.render_h < state.toolbar_height || page_y > viewport_h - state.status_height {
            continue;
        }

        let page_w = layout.render_w;
        let page_h = layout.render_h;

        // Soft, ambient multi-layer drop shadow (#08080C)
        let shadow_rect = RECT {
            left: page_x + 4,
            top: page_y + 4,
            right: page_x + page_w + 4,
            bottom: page_y + page_h + 4,
        };
        let shadow_brush = unsafe { CreateSolidBrush(0x000C0808) };
        unsafe {
            FillRect(hdc, &shadow_rect, shadow_brush);
            DeleteObject(shadow_brush as HGDIOBJ);
        }

        let rot = if let Ok(p) = doc.load_page(layout.index) {
            p.rotation()
        } else {
            0
        };

        let cached = state.tile_cache.get(layout.index, state.zoom_pct, rot).is_some();

        if !cached {
            if let Ok(page) = doc.load_page(layout.index) {
                let stride = page_w * 4;
                let buf_len = (stride * page_h) as usize;
                let mut buffer = state.tile_cache.acquire_buffer(buf_len);

                if page.render_into_buffer(&mut buffer, page_w, page_h, stride, rot, 0).is_ok() {
                    state.tile_cache.insert(
                        layout.index,
                        state.zoom_pct,
                        rot,
                        page_w,
                        page_h,
                        stride,
                        buffer,
                    );
                }
            }
        }

        if let Some(bitmap) = state.tile_cache.get(layout.index, state.zoom_pct, rot) {
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: bitmap.width,
                    biHeight: -bitmap.height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [0],
            };

            unsafe {
                StretchDIBits(
                    hdc,
                    page_x,
                    page_y,
                    page_w,
                    page_h,
                    0,
                    0,
                    bitmap.width,
                    bitmap.height,
                    bitmap.pixels.as_ptr() as *const c_void,
                    &bmi,
                    0,
                    SRCCOPY,
                );

                // Subtle 1px page outline (#282832) for crisp contrast against dark canvas
                let outline_rect = RECT {
                    left: page_x,
                    top: page_y,
                    right: page_x + page_w,
                    bottom: page_y + page_h,
                };
                let outline_brush = CreateSolidBrush(0x00322828);
                FrameRect(hdc, &outline_rect, outline_brush);
                DeleteObject(outline_brush as HGDIOBJ);
            }
        }
    }

    state.active_page_idx = current_page;

    // Draw active hover highlight for editable text objects
    if let Some(r) = state.hovered_edit_rect {
        unsafe {
            let hover_brush = CreateSolidBrush(0x00F6823B); // Electric Blue accent
            FrameRect(hdc, &r, hover_brush);
            DeleteObject(hover_brush as HGDIOBJ);
        }
    }
}

pub fn run_gui(pdfium: Arc<PdfiumLibrary>, initial_pdf: Option<PathBuf>) -> Result<(), String> {
    unsafe {
        // Enable Windows 10/11 Per-Monitor v2 DPI Awareness (-4)
        let _ = SetProcessDpiAwarenessContext(-4);
    }

    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let class_name = to_wide_null("zPDF_MainWindowClass");
    let h_icon = unsafe { LoadIconW(instance, 1 as *const u16) };

    let wnd_class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0x0002 | 0x0001,
        lpfnWndProc: window_proc,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: h_icon,
        hCursor: unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) },
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: h_icon,
    };

    unsafe {
        let atom = RegisterClassExW(&wnd_class);
        if atom == 0 {
            return Err("Failed to register Win32 window class".to_string());
        }

        let title = to_wide_null("zPDF");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1280,
            820,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err("Failed to create Win32 window".to_string());
        }

        if !h_icon.is_null() {
            SendMessageW(hwnd, WM_SETICON, 1, h_icon as LPARAM);
            SendMessageW(hwnd, WM_SETICON, 0, h_icon as LPARAM);
        }

        let mut app_state = Box::new(AppState::new(hwnd, pdfium, h_icon));
        if let Some(ref path) = initial_pdf {
            let _ = app_state.open_file(path);
        }
        GLOBAL_STATE = Box::into_raw(app_state);

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if !GLOBAL_STATE.is_null() {
            drop(Box::from_raw(GLOBAL_STATE));
            GLOBAL_STATE = ptr::null_mut();
        }
    }

    Ok(())
}
