// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Native Win32 / GDI double-buffered GUI with Windows 11 dark chrome

#![allow(non_snake_case, non_camel_case_types, dead_code, unused_variables)]

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Arc;

use crate::editor::{
    apply_text_mutation, hit_test_text_object, ActiveTextEditor, Annotation, EditAction, ToolMode,
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
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
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

// Win32 Constants
pub const WM_CREATE: u32 = 0x0001;
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_SIZE: u32 = 0x0005;
pub const WM_SETFOCUS: u32 = 0x0007;
pub const WM_KILLFOCUS: u32 = 0x0008;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WM_SETCURSOR: u32 = 0x0020;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
pub const WM_CHAR: u32 = 0x0102;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_MOUSEMOVE: u32 = 0x0200;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_RBUTTONDOWN: u32 = 0x0204;
pub const WM_RBUTTONUP: u32 = 0x0205;
pub const WM_MOUSEWHEEL: u32 = 0x020A;
pub const WM_MOUSEHWHEEL: u32 = 0x020E;

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
pub const IDC_SIZEALL: *const u16 = 32646 as *const u16;

pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
pub const OFN_FILEMUSTEXIST: u32 = 0x00001000;
pub const OFN_PATHMUSTEXIST: u32 = 0x00000800;
pub const OFN_OVERWRITEPROMPT: u32 = 0x00000002;

#[link(name = "user32")]
extern "system" {
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
    fn MessageBoxW(hWnd: HWND, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
    fn SetCapture(hWnd: HWND) -> HWND;
    fn ReleaseCapture() -> i32;
    fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: *const u16) -> HCURSOR;
    fn SetCursor(hCursor: HCURSOR) -> HCURSOR;
    fn SetFocus(hWnd: HWND) -> HWND;
    fn SetWindowPos(hWnd: HWND, hWndInsertAfter: HWND, X: i32, Y: i32, cx: i32, cy: i32, uFlags: u32) -> i32;
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
    fn SetTextColor(hdc: HDC, color: COLORREF) -> COLORREF;
    fn TextOutW(hdc: HDC, x: i32, y: i32, lpString: *const u16, c: i32) -> i32;
    fn CreateSolidBrush(color: COLORREF) -> HBRUSH;
    fn FillRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    fn FrameRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    fn CreatePen(iStyle: i32, cWidth: i32, color: COLORREF) -> HPEN;
    fn Rectangle(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> i32;
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

    // UI state
    pub sidebar_width: i32,
    pub toolbar_height: i32,
    pub zoom_pct: u32, // 100 = 100%
    pub scroll_x: i32,
    pub scroll_y: i32,
    pub tool_mode: ToolMode,
    pub page_layouts: Vec<PageLayout>,
    pub total_content_height: i32,

    // Interaction
    pub is_dragging: bool,
    pub drag_start_x: i32,
    pub drag_start_y: i32,
    pub drag_start_scroll_x: i32,
    pub drag_start_scroll_y: i32,
    pub active_editor: Option<ActiveTextEditor>,
    pub edit_hwnd: HWND,

    // Vector annotations
    pub annotations: Vec<(usize, Annotation)>, // (page_idx, annotation)

    // Search
    pub search_query: String,
    pub search_results: Vec<(usize, RectF)>,
    pub search_active_idx: usize,
}

static mut GLOBAL_STATE: *mut AppState = ptr::null_mut();

impl AppState {
    pub fn new(hwnd: HWND, pdfium: Arc<PdfiumLibrary>) -> Self {
        Self {
            hwnd,
            pdfium,
            doc: None,
            current_file: None,
            tile_cache: TileCache::new(128),
            thumb_cache: ThumbnailCache::new(),
            undo_manager: UndoManager::new(),
            sidebar_width: 170,
            toolbar_height: 44,
            zoom_pct: 100,
            scroll_x: 0,
            scroll_y: 0,
            tool_mode: ToolMode::Pan,
            page_layouts: Vec::new(),
            total_content_height: 0,
            is_dragging: false,
            drag_start_x: 0,
            drag_start_y: 0,
            drag_start_scroll_x: 0,
            drag_start_scroll_y: 0,
            active_editor: None,
            edit_hwnd: ptr::null_mut(),
            annotations: Vec::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_active_idx: 0,
        }
    }

    pub fn open_file(&mut self, path: &Path) -> Result<(), String> {
        let doc = self.pdfium.open_document(path, None)?;
        self.doc = Some(doc);
        self.current_file = Some(path.to_path_buf());
        self.tile_cache.clear();
        self.thumb_cache.clear();
        self.annotations.clear();
        self.recompute_layouts();
        self.scroll_y = 0;
        self.scroll_x = 0;
        self.update_title();
        self.invalidate();
        Ok(())
    }

    pub fn recompute_layouts(&mut self) {
        self.page_layouts.clear();
        let Some(doc) = &self.doc else {
            self.total_content_height = 0;
            return;
        };

        let page_count = doc.page_count();
        let scale = self.zoom_pct as f64 / 100.0;
        let mut y = 20; // top margin
        let gap = 20;

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
            }
        }

        self.total_content_height = y;
    }

    pub fn update_title(&self) {
        let name = self
            .current_file
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled");

        let pages = self.doc.as_ref().map_or(0, |d| d.page_count());
        let title = format!("zPDF — {} ({} pages, {}%)", name, pages, self.zoom_pct);
        let wide = to_wide_null(&title);
        unsafe { SetWindowTextW(self.hwnd, wide.as_ptr()) };
    }

    pub fn invalidate(&self) {
        unsafe { InvalidateRect(self.hwnd, ptr::null(), 0) };
    }

    pub fn save_current(&mut self) -> Result<(), String> {
        if let (Some(doc), Some(path)) = (&self.doc, &self.current_file) {
            doc.save_to_file(path)?;
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
        ofn.Flags = OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST;

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
        if let Some(layout) = self.page_layouts.first() {
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
        let Some(doc) = &mut self.doc else { return };
        if doc.page_count() <= 1 {
            return; // don't delete sole page
        }
        if doc.delete_page(0) {
            self.tile_cache.clear();
            self.thumb_cache.clear();
            self.recompute_layouts();
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

// Win32 Window Procedure
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
            // Enable dark mode title bar
            let dark_mode: i32 = 1;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const _ as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );
            0
        }

        WM_SIZE => {
            let _w = (lparam & 0xFFFF) as i32;
            let _h = ((lparam >> 16) & 0xFFFF) as i32;
            state.invalidate();
            0
        }

        WM_ERASEBKGND => 1, // Double-buffered, avoid flickering

        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint_viewport(state, hdc);
            EndPaint(hwnd, &ps);
            0
        }

        WM_LBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            // Check if click was in toolbar
            if y < state.toolbar_height {
                handle_toolbar_click(state, x, y);
                return 0;
            }

            // Check if click was in sidebar
            if x < state.sidebar_width {
                handle_sidebar_click(state, x, y);
                return 0;
            }

            match state.tool_mode {
                ToolMode::Pan => {
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

            if state.is_dragging {
                let dx = x - state.drag_start_x;
                let dy = y - state.drag_start_y;
                state.scroll_x = state.drag_start_scroll_x - dx;
                state.scroll_y = (state.drag_start_scroll_y - dy).max(0);
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
            let ctrl = (wparam & 0x0008) != 0; // MK_CONTROL

            if ctrl {
                // Zoom
                if delta > 0 {
                    state.set_zoom(state.zoom_pct + 15);
                } else {
                    state.set_zoom(state.zoom_pct.saturating_sub(15));
                }
            } else {
                // Scroll vertically
                let scroll_step = 60;
                if delta > 0 {
                    state.scroll_y = state.scroll_y.saturating_sub(scroll_step);
                } else {
                    state.scroll_y = state.scroll_y.saturating_add(scroll_step);
                }
                state.invalidate();
            }
            0
        }

        WM_KEYDOWN => {
            let vk = wparam as i32;
            let ctrl = (unsafe { GetKeyState(0x11) } as u16 & 0x8000) != 0; // VK_CONTROL

            if ctrl {
                match vk {
                    0x4F => { state.prompt_open_file(); }       // Ctrl+O
                    0x53 => { let _ = state.save_current(); }   // Ctrl+S
                    0x5A => { handle_undo(state); }             // Ctrl+Z
                    0x59 => { handle_redo(state); }             // Ctrl+Y
                    0xBB | 0x6B => { state.set_zoom(state.zoom_pct + 20); } // Ctrl +
                    0xBD | 0x6D => { state.set_zoom(state.zoom_pct.saturating_sub(20)); } // Ctrl -
                    0x30 => { state.set_zoom(100); }            // Ctrl+0
                    _ => {}
                }
            } else {
                match vk {
                    0x52 => { state.rotate_current_page(); }     // R: Rotate
                    0x2E => { state.delete_current_page(); }     // Del: Delete
                    0x48 => { state.tool_mode = ToolMode::Pan; state.invalidate(); } // H: Hand/Pan
                    0x54 => { state.tool_mode = ToolMode::EditTextObject; state.invalidate(); } // T: Text edit
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

#[link(name = "user32")]
extern "system" {
    fn GetKeyState(nVirtKey: i32) -> i16;
}

fn handle_undo(state: &mut AppState) {
    if let Some(action) = state.undo_manager.pop_undo() {
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
            _ => {}
        }
    }
}

fn handle_redo(state: &mut AppState) {
    if let Some(action) = state.undo_manager.pop_redo() {
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
            _ => {}
        }
    }
}

fn handle_toolbar_click(state: &mut AppState, x: i32, _y: i32) {
    // Toolbar buttons: [Open] [Save] | [Undo] [Redo] | [Pan] [Text Edit] | [Rotate] [Zoom-] [Zoom+] [Fit]
    let btn_w = 64;
    let idx = x / btn_w;

    match idx {
        0 => state.prompt_open_file(),
        1 => { let _ = state.save_current(); },
        2 => handle_undo(state),
        3 => handle_redo(state),
        4 => { state.tool_mode = ToolMode::Pan; state.invalidate(); },
        5 => { state.tool_mode = ToolMode::EditTextObject; state.invalidate(); },
        6 => state.rotate_current_page(),
        7 => state.set_zoom(state.zoom_pct.saturating_sub(15)),
        8 => state.set_zoom(state.zoom_pct + 15),
        9 => state.set_zoom(100),
        _ => {}
    }
}

fn handle_sidebar_click(state: &mut AppState, _x: i32, y: i32) {
    let rel_y = y - state.toolbar_height;
    let thumb_h = 160;
    let clicked_page = (rel_y / thumb_h) as usize;

    if let Some(layout) = state.page_layouts.get(clicked_page) {
        state.scroll_y = layout.y_offset;
        state.invalidate();
    }
}

fn handle_text_edit_click(state: &mut AppState, client_x: i32, client_y: i32) {
    let Some(doc) = &state.doc else { return };

    let canvas_x = client_x - state.sidebar_width + state.scroll_x;
    let canvas_y = client_y - state.toolbar_height + state.scroll_y;

    for layout in &state.page_layouts {
        if canvas_y >= layout.y_offset && canvas_y <= layout.y_offset + layout.render_h {
            let page_rel_x = canvas_x - 30; // 30px page margin
            let page_rel_y = canvas_y - layout.y_offset;

            if page_rel_x >= 0 && page_rel_x <= layout.render_w {
                // Map screen coords back to PDF points
                let scale = state.zoom_pct as f64 / 100.0 * 1.333333;
                let pdf_x = (page_rel_x as f64 / scale) as f32;
                let pdf_y = (layout.height_pt - (page_rel_y as f64 / scale)) as f32;

                if let Ok(page) = doc.load_page(layout.index) {
                    if let Some((obj_idx, text, bounds)) = hit_test_text_object(&page, pdf_x, pdf_y, 4.0) {
                        // Prompt inline or simple text modification
                        let msg = format!("Current Text:\n{}\n\n(Edit text in zPDF)", text);
                        let w_msg = to_wide_null(&msg);
                        let w_title = to_wide_null("PDF Text Object In-Place");
                        unsafe {
                            MessageBoxW(state.hwnd, w_msg.as_ptr(), w_title.as_ptr(), 0);
                        }
                    }
                }
            }
            break;
        }
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

    unsafe {
        // Offscreen backbuffer
        let mem_dc = CreateCompatibleDC(hdc);
        let mem_bmp = CreateCompatibleBitmap(hdc, width, height);
        let old_bmp = SelectObject(mem_dc, mem_bmp as HGDIOBJ);

        // 1. Fill main background with dark gray (#1e1e1e)
        let bg_brush = CreateSolidBrush(0x001E1E1E);
        FillRect(mem_dc, &rect, bg_brush);
        DeleteObject(bg_brush as HGDIOBJ);

        // 2. Render Document Pages
        paint_pages(state, mem_dc, width, height);

        // 3. Render Left Sidebar (Thumbnails)
        paint_sidebar(state, mem_dc, height);

        // 4. Render Top Toolbar
        paint_toolbar(state, mem_dc, width);

        // Blit backbuffer to screen in 1 frame
        BitBlt(hdc, 0, 0, width, height, mem_dc, 0, 0, SRCCOPY);

        SelectObject(mem_dc, old_bmp);
        DeleteObject(mem_bmp as HGDIOBJ);
        DeleteDC(mem_dc);
    }
}

fn paint_toolbar(state: &AppState, hdc: HDC, width: i32) {
    let tb_rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: state.toolbar_height,
    };

    unsafe {
        let tb_brush = CreateSolidBrush(0x00262525); // #252526
        FillRect(hdc, &tb_rect, tb_brush);
        DeleteObject(tb_brush as HGDIOBJ);

        // Separator line
        let sep_brush = CreateSolidBrush(0x00423E3E);
        let sep_rect = RECT {
            left: 0,
            top: state.toolbar_height - 1,
            right: width,
            bottom: state.toolbar_height,
        };
        FillRect(hdc, &sep_rect, sep_brush);
        DeleteObject(sep_brush as HGDIOBJ);

        // Buttons
        let buttons = [
            "Open", "Save", "Undo", "Redo", "Pan", "Edit", "Rotate", "Zoom -", "Zoom +", "100%",
        ];
        let btn_w = 64;

        SetBkMode(hdc, TRANSPARENT);
        let font = CreateFontW(16, 0, 0, 0, 500, 0, 0, 0, 1, 0, 0, 5, 0, to_wide_null("Segoe UI").as_ptr());
        let old_font = SelectObject(hdc, font as HGDIOBJ);

        for (i, &btn_text) in buttons.iter().enumerate() {
            let bx = i as i32 * btn_w;
            let is_active = (i == 4 && state.tool_mode == ToolMode::Pan)
                || (i == 5 && state.tool_mode == ToolMode::EditTextObject);

            if is_active {
                let active_brush = CreateSolidBrush(0x007ACC00);
                let btn_r = RECT { left: bx + 2, top: 4, right: bx + btn_w - 2, bottom: state.toolbar_height - 4 };
                FillRect(hdc, &btn_r, active_brush);
                DeleteObject(active_brush as HGDIOBJ);
                SetTextColor(hdc, 0x00FFFFFF);
            } else {
                SetTextColor(hdc, 0x00D4D4D4);
            }

            let wide = to_wide_null(btn_text);
            TextOutW(hdc, bx + 10, 12, wide.as_ptr(), (wide.len() - 1) as i32);
        }

        SelectObject(hdc, old_font);
        DeleteObject(font as HGDIOBJ);
    }
}

fn paint_sidebar(state: &mut AppState, hdc: HDC, height: i32) {
    let sb_rect = RECT {
        left: 0,
        top: state.toolbar_height,
        right: state.sidebar_width,
        bottom: height,
    };

    unsafe {
        let sb_brush = CreateSolidBrush(0x00302D2D); // #2d2d30
        FillRect(hdc, &sb_rect, sb_brush);
        DeleteObject(sb_brush as HGDIOBJ);

        // Border line
        let border_brush = CreateSolidBrush(0x00423E3E);
        let border_rect = RECT {
            left: state.sidebar_width - 1,
            top: state.toolbar_height,
            right: state.sidebar_width,
            bottom: height,
        };
        FillRect(hdc, &border_rect, border_brush);
        DeleteObject(border_brush as HGDIOBJ);

        let Some(doc) = &state.doc else { return };
        let count = doc.page_count();
        let thumb_h = 160;

        let font = CreateFontW(14, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, to_wide_null("Segoe UI").as_ptr());
        let old_font = SelectObject(hdc, font as HGDIOBJ);
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, 0x00AAAAAA);

        for i in 0..count {
            let ty = state.toolbar_height + (i as i32 * thumb_h) + 10;
            if ty > height {
                break;
            }

            let box_rect = RECT {
                left: 15,
                top: ty,
                right: state.sidebar_width - 15,
                bottom: ty + thumb_h - 25,
            };

            let white_brush = CreateSolidBrush(0x00FFFFFF);
            FillRect(hdc, &box_rect, white_brush);
            DeleteObject(white_brush as HGDIOBJ);

            let label = format!("Page {}", i + 1);
            let wide = to_wide_null(&label);
            TextOutW(hdc, 20, ty + thumb_h - 22, wide.as_ptr(), (wide.len() - 1) as i32);
        }

        SelectObject(hdc, old_font);
        DeleteObject(font as HGDIOBJ);
    }
}

fn paint_pages(state: &mut AppState, hdc: HDC, viewport_w: i32, viewport_h: i32) {
    let Some(doc) = &state.doc else { return };

    let canvas_origin_x = state.sidebar_width - state.scroll_x + 30;
    let canvas_origin_y = state.toolbar_height - state.scroll_y;

    for layout in &state.page_layouts {
        let page_y = canvas_origin_y + layout.y_offset;

        // Virtualized culling: skip pages completely off-screen
        if page_y + layout.render_h < state.toolbar_height || page_y > viewport_h {
            continue;
        }

        let page_x = canvas_origin_x;
        let page_w = layout.render_w;
        let page_h = layout.render_h;

        // Page drop shadow
        let shadow_rect = RECT {
            left: page_x + 4,
            top: page_y + 4,
            right: page_x + page_w + 4,
            bottom: page_y + page_h + 4,
        };
        let shadow_brush = unsafe { CreateSolidBrush(0x00111111) };
        unsafe {
            FillRect(hdc, &shadow_rect, shadow_brush);
            DeleteObject(shadow_brush as HGDIOBJ);
        }

        // Check if page bitmap is already in LRU cache
        let rot = if let Ok(p) = doc.load_page(layout.index) {
            p.rotation()
        } else {
            0
        };

        let cached = state.tile_cache.get(layout.index, state.zoom_pct, rot).is_some();

        if !cached {
            // Render into cache
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
                    biHeight: -bitmap.height, // Top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0, // BI_RGB
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
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
            }
        }
    }
}

pub fn run_gui(pdfium: Arc<PdfiumLibrary>, initial_pdf: Option<PathBuf>) -> Result<(), String> {
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let class_name = to_wide_null("zPDF_MainWindowClass");

    let wnd_class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0x0002 | 0x0001, // CS_HREDRAW | CS_VREDRAW
        lpfnWndProc: window_proc,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) },
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: ptr::null_mut(),
    };

    unsafe {
        let atom = RegisterClassExW(&wnd_class);
        if atom == 0 {
            return Err("Failed to register Win32 window class".to_string());
        }

        let title = to_wide_null("zPDF — Zero-Dependency Native PDF Viewer & Editor");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1280,
            800,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err("Failed to create Win32 window".to_string());
        }

        let mut app_state = Box::new(AppState::new(hwnd, pdfium));
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
