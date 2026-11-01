// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Native Win32 / GDI double-buffered GUI with Modern Fluent Dark UI

#![allow(non_snake_case, non_camel_case_types, dead_code, unused_variables)]

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Arc;

use crate::editor::{
    apply_redaction, apply_text_mutation, hit_test_text_object, insert_image_object,
    insert_stamp_checkmark, insert_stamp_cross, remove_page_object, ActiveImageGizmo,
    ActiveTextEditor, EditAction, GizmoHandle, ToolMode, UndoManager,
};
use crate::pdfium::{PdfDocument, PdfFormEnv, PdfiumLibrary, RectF};
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
pub type HMENU = *mut c_void;
pub type COLORREF = u32;
pub type LPARAM = isize;
pub type WPARAM = usize;
pub type LRESULT = isize;

#[inline]
pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
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
#[derive(Clone, Copy, Default, Debug)]
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
pub const WM_TIMER: u32 = 0x0113;
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
pub const WM_DPICHANGED: u32 = 0x02E0;

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
pub const IDC_CROSS: *const u16 = 32515 as *const u16;
pub const IDC_SIZEALL: *const u16 = 32646 as *const u16;
pub const IDC_SIZENWSE: *const u16 = 32642 as *const u16;
pub const IDC_SIZENESW: *const u16 = 32643 as *const u16;
pub const IDC_SIZEWE: *const u16 = 32644 as *const u16;
pub const IDC_SIZENS: *const u16 = 32645 as *const u16;

pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
pub const OFN_FILEMUSTEXIST: u32 = 0x00001000;
pub const OFN_PATHMUSTEXIST: u32 = 0x00000800;
pub const OFN_OVERWRITEPROMPT: u32 = 0x00000002;
pub const TME_LEAVE: u32 = 0x00000002;
pub const EN_KILLFOCUS: u16 = 0x0200;
pub const EN_CHANGE: u16 = 0x0300;
pub const EM_CANUNDO: u32 = 0x00C6;
pub const GWLP_WNDPROC: i32 = -4;

pub const MF_STRING: u32 = 0x0000;
pub const MF_GRAYED: u32 = 0x0001;
pub const MF_SEPARATOR: u32 = 0x0800;
pub const TPM_RIGHTBUTTON: u32 = 0x0002;
pub const TPM_RETURNCMD: u32 = 0x0100;
pub const TIMER_SAVE_TOAST: usize = 2001;

pub const VK_PRIOR: i32 = 0x21; // Page Up
pub const VK_NEXT: i32 = 0x22;  // Page Down
pub const VK_END: i32 = 0x23;   // End
pub const VK_HOME: i32 = 0x24;  // Home

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolbarControl {
    Open,
    Save,
    Undo,
    Redo,
    ToolPan,
    ToolEdit,
    ToolStampCheckmark,
    ToolStampCross,
    ToolImage,
    ToolRedactBlackline,
    ZoomOut,
    ZoomReset,
    ZoomIn,
    SearchToggle,
    SearchPrev,
    SearchNext,
    SearchClose,
    WelcomeOpen,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchMatch {
    pub page_index: usize,
    pub rect: RectF,
}

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
    fn ClientToScreen(hWnd: HWND, lpPoint: *mut POINT) -> i32;
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
    fn CreatePopupMenu() -> HMENU;
    fn DestroyMenu(hMenu: HMENU) -> i32;
    fn AppendMenuW(hMenu: HMENU, uFlags: u32, uIDNewItem: usize, lpNewItem: *const u16) -> i32;
    fn TrackPopupMenu(
        hMenu: HMENU,
        uFlags: u32,
        x: i32,
        y: i32,
        nReserved: i32,
        hWnd: HWND,
        prcRect: *const RECT,
    ) -> i32;
    fn SetTimer(hWnd: HWND, nIDEvent: usize, uElapse: u32, lpTimerFunc: *mut c_void) -> usize;
    fn KillTimer(hWnd: HWND, uIDEvent: usize) -> i32;
    fn InvertRect(hDC: HDC, lprc: *const RECT) -> i32;
    fn MoveWindow(hWnd: HWND, X: i32, Y: i32, nWidth: i32, nHeight: i32, bRepaint: i32) -> i32;
    fn GetDpiForWindow(hWnd: HWND) -> u32;
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
    fn MoveToEx(hdc: HDC, x: i32, y: i32, lppt: *mut POINT) -> i32;
    fn LineTo(hdc: HDC, x: i32, y: i32) -> i32;
    fn Polyline(hdc: HDC, apt: *const POINT, cpt: i32) -> i32;
    fn Polygon(hdc: HDC, apt: *const POINT, cpt: i32) -> i32;
    fn Rectangle(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn Arc(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32, xr1: i32, yr1: i32, xr2: i32, yr2: i32) -> i32;
    fn Ellipse(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn GetStockObject(i: i32) -> HGDIOBJ;
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

#[inline]
pub fn scale_dpi(val: i32, dpi: u32) -> i32 {
    let effective_dpi = if dpi == 0 { 96 } else { dpi };
    ((val as f32 * effective_dpi as f32 / 96.0) + 0.5) as i32
}

pub fn create_ui_font(pt: f32, weight: i32, dpi: u32) -> HFONT {
    let effective_dpi = if dpi == 0 { 96 } else { dpi };
    // In Win32 GDI, a negative height requests character (em) height rather than bounding cell height.
    // This accurately matches standard typography and point sizes.
    let height = -((pt * effective_dpi as f32 / 72.0) + 0.5) as i32;
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            1, // DEFAULT_CHARSET
            0,
            0,
            5, // CLEARTYPE_QUALITY
            0,
            to_wide_null("Segoe UI").as_ptr(),
        )
    }
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

    // Dirty state & Save toast tracking
    pub is_dirty: bool,
    pub save_toast_active: bool,

    // Floating Search state
    pub search_open: bool,
    pub search_query: String,
    pub search_matches: Vec<SearchMatch>,
    pub current_search_idx: Option<usize>,
    pub search_edit_hwnd: HWND,
    pub search_font: HFONT,

    // Layout & DPI
    pub dpi: u32,
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
    pub hovered_control: Option<ToolbarControl>,
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

    // Cached Toolbar & Search Control Rects
    pub tb_open_rect: RECT,
    pub tb_save_rect: RECT,
    pub tb_undo_rect: RECT,
    pub tb_redo_rect: RECT,
    pub tb_hand_rect: RECT,
    pub tb_edit_rect: RECT,
    pub tb_zoom_out_rect: RECT,
    pub tb_zoom_reset_rect: RECT,
    pub tb_zoom_in_rect: RECT,
    pub tb_search_rect: RECT,

    pub search_card_rect: RECT,
    pub search_prev_rect: RECT,
    pub search_next_rect: RECT,
    pub search_close_rect: RECT,

    // Backbuffer Caching
    pub backbuffer_dc: HDC,
    pub backbuffer_bmp: HBITMAP,
    pub backbuffer_old_bmp: HGDIOBJ,
    pub backbuffer_w: i32,
    pub backbuffer_h: i32,

    // AcroForms
    pub form_env: Option<PdfFormEnv>,

    // Image Gizmo & Transform
    pub active_image_gizmo: Option<ActiveImageGizmo>,

    // Redaction Drag Rect
    pub redact_drag_rect: Option<(usize, RectF)>,

    // Floating Default App Banner
    pub show_default_app_banner: bool,
    pub banner_rect: RECT,
    pub banner_set_btn_rect: RECT,
    pub banner_never_btn_rect: RECT,
    pub banner_close_btn_rect: RECT,

    // New Toolbar Action Rects
    pub tb_stamp_v_rect: RECT,
    pub tb_stamp_x_rect: RECT,
    pub tb_image_rect: RECT,
    pub tb_redact_rect: RECT,
}

static mut GLOBAL_STATE: *mut AppState = ptr::null_mut();
static mut OLD_EDIT_PROC: isize = 0;
static mut OLD_SEARCH_PROC: isize = 0;

unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_KEYDOWN => {
            let ctrl = (GetKeyState(0x11) as u16 & 0x8000) != 0;
            let shift = (GetKeyState(0x10) as u16 & 0x8000) != 0;

            if ctrl {
                match wparam as i32 {
                    0x53 => {
                        // Ctrl+S or Ctrl+Shift+S
                        if !GLOBAL_STATE.is_null() {
                            let state = &mut *GLOBAL_STATE;
                            commit_active_text_editor(state);
                            if shift {
                                let _ = state.save_as();
                            } else {
                                let _ = state.save_current();
                            }
                        }
                        return 0;
                    }
                    0x4F => {
                        // Ctrl+O
                        if !GLOBAL_STATE.is_null() {
                            let state = &mut *GLOBAL_STATE;
                            commit_active_text_editor(state);
                            state.prompt_open_file();
                        }
                        return 0;
                    }
                    0x46 => {
                        // Ctrl+F
                        if !GLOBAL_STATE.is_null() {
                            let state = &mut *GLOBAL_STATE;
                            commit_active_text_editor(state);
                            state.open_search();
                        }
                        return 0;
                    }
                    0x5A => {
                        // Ctrl+Z / Ctrl+Shift+Z
                        if shift {
                            if !GLOBAL_STATE.is_null() {
                                let state = &mut *GLOBAL_STATE;
                                commit_active_text_editor(state);
                                handle_redo(state);
                            }
                            return 0;
                        } else {
                            let can_undo = SendMessageW(hwnd, EM_CANUNDO, 0, 0);
                            if can_undo == 0 {
                                if !GLOBAL_STATE.is_null() {
                                    let state = &mut *GLOBAL_STATE;
                                    cancel_active_text_editor(state);
                                    handle_undo(state);
                                }
                                return 0;
                            }
                        }
                    }
                    0x59 => {
                        // Ctrl+Y
                        if !GLOBAL_STATE.is_null() {
                            let state = &mut *GLOBAL_STATE;
                            commit_active_text_editor(state);
                            handle_redo(state);
                        }
                        return 0;
                    }
                    _ => {}
                }
            } else {
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
        }
        WM_CHAR => {
            // Suppress error beeps on Enter, Esc, Ctrl+S, Ctrl+F, Ctrl+O, Ctrl+Z, Ctrl+Y
            if wparam == 0x0D
                || wparam == 0x1B
                || wparam == 0x13
                || wparam == 0x06
                || wparam == 0x0F
                || wparam == 0x1A
                || wparam == 0x19
            {
                return 0;
            }
        }
        _ => {}
    }
    CallWindowProcW(OLD_EDIT_PROC, hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn search_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_KEYDOWN => {
            if wparam == 0x0D {
                // Enter: next match (or prev match if Shift pressed)
                let shift = (GetKeyState(0x10) as u16 & 0x8000) != 0;
                if !GLOBAL_STATE.is_null() {
                    let state = &mut *GLOBAL_STATE;
                    if shift {
                        state.search_prev();
                    } else {
                        state.search_next();
                    }
                }
                return 0;
            } else if wparam == 0x1B {
                // Escape: close search
                if !GLOBAL_STATE.is_null() {
                    let state = &mut *GLOBAL_STATE;
                    state.close_search();
                }
                return 0;
            }
        }
        WM_CHAR => {
            if wparam == 0x0D || wparam == 0x1B {
                return 0;
            }
        }
        _ => {}
    }
    CallWindowProcW(OLD_SEARCH_PROC, hwnd, msg, wparam, lparam)
}

impl AppState {
    pub fn new(hwnd: HWND, pdfium: Arc<PdfiumLibrary>, app_icon: *mut c_void) -> Self {
        let dpi = if !hwnd.is_null() {
            let d = unsafe { GetDpiForWindow(hwnd) };
            if d == 0 { 96 } else { d }
        } else {
            96
        };
        let sidebar_width = scale_dpi(200, dpi);
        let toolbar_height = scale_dpi(48, dpi);
        let status_height = scale_dpi(26, dpi);

        Self {
            hwnd,
            pdfium,
            doc: None,
            current_file: None,
            tile_cache: TileCache::new(128),
            thumb_cache: ThumbnailCache::new(),
            undo_manager: UndoManager::new(),
            is_dirty: false,
            save_toast_active: false,
            search_open: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_search_idx: None,
            search_edit_hwnd: ptr::null_mut(),
            search_font: ptr::null_mut(),
            dpi,
            sidebar_width,
            toolbar_height,
            status_height,
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
            hovered_control: None,
            hovered_edit_rect: None,
            is_dragging: false,
            drag_start_x: 0,
            drag_start_y: 0,
            drag_start_scroll_x: 0,
            drag_start_scroll_y: 0,
            active_editor: None,
            edit_hwnd: ptr::null_mut(),
            edit_font: ptr::null_mut(),
            edit_bg_brush: unsafe { CreateSolidBrush(rgb(0x20, 0x20, 0x26)) },
            welcome_btn_rect: RECT::default(),
            app_icon,
            tb_open_rect: RECT::default(),
            tb_save_rect: RECT::default(),
            tb_undo_rect: RECT::default(),
            tb_redo_rect: RECT::default(),
            tb_hand_rect: RECT::default(),
            tb_edit_rect: RECT::default(),
            tb_zoom_out_rect: RECT::default(),
            tb_zoom_reset_rect: RECT::default(),
            tb_zoom_in_rect: RECT::default(),
            tb_search_rect: RECT::default(),
            search_card_rect: RECT::default(),
            search_prev_rect: RECT::default(),
            search_next_rect: RECT::default(),
            search_close_rect: RECT::default(),
            backbuffer_dc: ptr::null_mut(),
            backbuffer_bmp: ptr::null_mut(),
            backbuffer_old_bmp: ptr::null_mut(),
            backbuffer_w: 0,
            backbuffer_h: 0,
            form_env: None,
            active_image_gizmo: None,
            redact_drag_rect: None,
            show_default_app_banner: zpdf::default_app::should_prompt_default_app(),
            banner_rect: RECT::default(),
            banner_set_btn_rect: RECT::default(),
            banner_never_btn_rect: RECT::default(),
            banner_close_btn_rect: RECT::default(),
            tb_stamp_v_rect: RECT::default(),
            tb_stamp_x_rect: RECT::default(),
            tb_image_rect: RECT::default(),
            tb_redact_rect: RECT::default(),
        }
    }

    pub fn open_file(&mut self, path: &Path) -> Result<(), String> {
        let doc = self.pdfium.open_document(path, None)?;
        self.form_env = doc.init_form_env();
        self.doc = Some(doc);
        self.current_file = Some(path.to_path_buf());
        self.active_image_gizmo = None;
        self.redact_drag_rect = None;
        self.is_dirty = false;
        self.save_toast_active = false;
        self.tile_cache.clear();
        self.thumb_cache.clear();
        self.recompute_layouts();
        self.scroll_y = 0;
        self.scroll_x = 0;
        self.sidebar_scroll_y = 0;
        self.active_page_idx = 0;
        if self.search_open {
            self.perform_search();
        }
        self.update_title();
        self.invalidate();
        Ok(())
    }

    pub fn update_dpi(&mut self, dpi: u32) {
        self.dpi = if dpi == 0 { 96 } else { dpi };
        self.toolbar_height = scale_dpi(48, self.dpi);
        self.status_height = scale_dpi(26, self.dpi);
        self.sidebar_width = scale_dpi(200, self.dpi);
        self.tile_cache.clear();
        self.thumb_cache.clear();
        if !self.search_font.is_null() {
            unsafe {
                DeleteObject(self.search_font as HGDIOBJ);
            }
            self.search_font = create_ui_font(10.0, 400, self.dpi);
            if !self.search_edit_hwnd.is_null() {
                unsafe {
                    SendMessageW(self.search_edit_hwnd, WM_SETFONT, self.search_font as WPARAM, 1);
                }
            }
        }
        self.recompute_layouts();
        self.clamp_scroll();
        self.invalidate();
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
        let dpi = self.dpi;
        let s = |v: i32| scale_dpi(v, dpi);
        let mut y = s(24);
        let gap = s(24);
        let mut sidebar_y = s(28);
        let thumb_max_w = s(144);
        let thumb_max_h = s(160);
        let base_scale = (self.dpi as f64) / 72.0;

        for i in 0..page_count {
            if let Ok(page) = doc.load_page(i) {
                let (w_pt, h_pt) = page.dimensions();
                let rot = page.rotation();
                let (eff_w, eff_h) = if rot == 1 || rot == 3 {
                    (h_pt, w_pt)
                } else {
                    (w_pt, h_pt)
                };

                let render_w = (eff_w * scale * base_scale).round() as i32;
                let render_h = (eff_h * scale * base_scale).round() as i32;

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
                if th > thumb_max_h {
                    th = thumb_max_h;
                }
                if th < s(40) {
                    th = s(40);
                }
                let slot_h = th + s(34);
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
                let dirty_str = if self.is_dirty { " • Edited" } else { "" };
                format!("{}{dirty_str} ({pages} pages, {}%) — zPDF", name, self.zoom_pct)
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
            self.is_dirty = false;
            self.save_toast_active = true;
            unsafe {
                SetTimer(self.hwnd, TIMER_SAVE_TOAST, 1800, ptr::null_mut());
            }
            self.update_title();
            self.invalidate();
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
            self.is_dirty = false;
            self.save_toast_active = true;
            unsafe {
                SetTimer(self.hwnd, TIMER_SAVE_TOAST, 1800, ptr::null_mut());
            }
            self.update_title();
            self.invalidate();
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

    pub fn prompt_insert_image(&mut self) {
        if self.doc.is_none() {
            return;
        }
        let mut file_buf = [0u16; 1024];
        let filter = to_wide_null("Image Files (*.png;*.jpg;*.jpeg;*.bmp;*.gif;*.tif;*.tiff)\0*.png;*.jpg;*.jpeg;*.bmp;*.gif;*.tif;*.tiff\0All Files (*.*)\0*.*\0");
        let title = to_wide_null("Insert Image");

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
            if let Ok((raw_w, raw_h, bgra)) = zpdf::wic_decoder::decode_image_to_bgra(&path) {
                let p_idx = self.active_page_idx;
                let (page_w_pt, page_h_pt) = if let Some(layout) = self.page_layouts.get(p_idx) {
                    (layout.width_pt, layout.height_pt)
                } else {
                    (612.0, 792.0)
                };
                let aspect = (raw_w as f64) / (raw_h as f64).max(1.0);
                let mut w_pt = 200.0;
                let mut h_pt = w_pt / aspect;
                if h_pt > 250.0 {
                    h_pt = 250.0;
                    w_pt = h_pt * aspect;
                }
                let page_x = (page_w_pt - w_pt) / 2.0;
                let page_y = (page_h_pt - h_pt) / 2.0;
                self.active_image_gizmo = Some(ActiveImageGizmo {
                    page_index: p_idx,
                    object_index: None,
                    page_x,
                    page_y,
                    width_pt: w_pt,
                    height_pt: h_pt,
                    raw_width_px: raw_w,
                    raw_height_px: raw_h,
                    bgra_pixels: bgra,
                    dragging_handle: None,
                    drag_start_mouse: (0, 0),
                    original_bounds: (page_x, page_y, w_pt, h_pt),
                });
                self.invalidate();
            }
        }
    }

    pub fn rotate_page_clockwise(&mut self, page_idx: usize) {
        let Some(doc) = &mut self.doc else { return };
        if let Ok(page) = doc.load_page(page_idx) {
            let old_rot = page.rotation();
            page.rotate_clockwise();
            let new_rot = page.rotation();
            self.undo_manager.push(EditAction::RotatePage {
                page_index: page_idx,
                old_rotation: old_rot,
                new_rotation: new_rot,
            });
            self.is_dirty = true;
            self.tile_cache.invalidate_page(page_idx);
            self.thumb_cache.invalidate(page_idx);
            self.recompute_layouts();
            self.update_title();
            self.invalidate();
        }
    }

    pub fn rotate_page_counter_clockwise(&mut self, page_idx: usize) {
        let Some(doc) = &mut self.doc else { return };
        if let Ok(page) = doc.load_page(page_idx) {
            let old_rot = page.rotation();
            let new_rot = (old_rot + 3) % 4;
            page.set_rotation(new_rot);
            self.undo_manager.push(EditAction::RotatePage {
                page_index: page_idx,
                old_rotation: old_rot,
                new_rotation: new_rot,
            });
            self.is_dirty = true;
            self.tile_cache.invalidate_page(page_idx);
            self.thumb_cache.invalidate(page_idx);
            self.recompute_layouts();
            self.update_title();
            self.invalidate();
        }
    }

    pub fn delete_page(&mut self, page_idx: usize) {
        let (ok, count) = {
            let Some(doc) = &mut self.doc else { return };
            if doc.page_count() <= 1 {
                return;
            }
            let del_idx = page_idx.min(doc.page_count() - 1);
            let ok = doc.delete_page(del_idx);
            (ok, doc.page_count())
        };
        if ok {
            self.is_dirty = true;
            self.tile_cache.clear();
            self.thumb_cache.clear();
            self.recompute_layouts();
            if self.active_page_idx >= count {
                self.active_page_idx = count.saturating_sub(1);
            }
            self.update_title();
            self.invalidate();
        }
    }

    pub fn rotate_current_page(&mut self) {
        self.rotate_page_clockwise(self.active_page_idx);
    }

    pub fn delete_current_page(&mut self) {
        self.delete_page(self.active_page_idx);
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

    // Floating Search Bar Management
    pub fn open_search(&mut self) {
        self.search_open = true;
        if self.search_edit_hwnd.is_null() {
            let edit_class = to_wide_null("EDIT");
            let text_w = to_wide_null(&self.search_query);
            let edit_ctl = unsafe {
                CreateWindowExW(
                    0,
                    edit_class.as_ptr(),
                    text_w.as_ptr(),
                    WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL,
                    0,
                    0,
                    0,
                    0,
                    self.hwnd,
                    1002 as *mut c_void,
                    GetModuleHandleW(ptr::null()),
                    ptr::null_mut(),
                )
            };
            if !edit_ctl.is_null() {
                if self.search_font.is_null() {
                    self.search_font = create_ui_font(10.0, 400, self.dpi);
                }
                unsafe {
                    SendMessageW(edit_ctl, WM_SETFONT, self.search_font as WPARAM, 1);
                    OLD_SEARCH_PROC = SetWindowLongPtrW(
                        edit_ctl,
                        GWLP_WNDPROC,
                        search_subclass_proc as *const () as isize,
                    );
                }
                self.search_edit_hwnd = edit_ctl;
            }
        } else {
            unsafe {
                ShowWindow(self.search_edit_hwnd, SW_SHOW);
            }
        }

        self.update_search_controls_layout();
        if !self.search_edit_hwnd.is_null() {
            unsafe {
                SetFocus(self.search_edit_hwnd);
                SendMessageW(self.search_edit_hwnd, 0x00B1, 0, -1);
            }
        }
        self.perform_search();
        self.invalidate();
    }

    pub fn close_search(&mut self) {
        self.search_open = false;
        if !self.search_edit_hwnd.is_null() {
            unsafe {
                ShowWindow(self.search_edit_hwnd, 0); // SW_HIDE
                SetFocus(self.hwnd);
            }
        }
        self.search_matches.clear();
        self.current_search_idx = None;
        self.invalidate();
    }

    pub fn toggle_search(&mut self) {
        if self.search_open {
            if !self.search_edit_hwnd.is_null() {
                unsafe {
                    SetFocus(self.search_edit_hwnd);
                    SendMessageW(self.search_edit_hwnd, 0x00B1, 0, -1);
                }
            }
        } else {
            self.open_search();
        }
    }

    pub fn on_search_query_changed(&mut self) {
        if self.search_edit_hwnd.is_null() {
            return;
        }
        let mut buf = [0u16; 512];
        let len = unsafe { GetWindowTextW(self.search_edit_hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        self.search_query = if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        };
        self.perform_search();
    }

    pub fn perform_search(&mut self) {
        self.search_matches.clear();
        let query = self.search_query.trim();
        if query.is_empty() {
            self.current_search_idx = None;
            self.invalidate();
            return;
        }

        let Some(doc) = &self.doc else {
            self.current_search_idx = None;
            self.invalidate();
            return;
        };

        let page_count = doc.page_count();
        for i in 0..page_count {
            if let Ok(page) = doc.load_page(i) {
                if let Ok(tp) = page.load_text_page() {
                    let rects = tp.search(query, false);
                    for rect in rects {
                        self.search_matches.push(SearchMatch {
                            page_index: i,
                            rect,
                        });
                    }
                }
            }
        }

        if !self.search_matches.is_empty() {
            let initial_idx = self
                .search_matches
                .iter()
                .position(|m| m.page_index >= self.active_page_idx)
                .unwrap_or(0);
            self.current_search_idx = Some(initial_idx);
            self.scroll_to_match(initial_idx);
        } else {
            self.current_search_idx = None;
        }
        self.invalidate();
    }

    pub fn search_next(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let next = match self.current_search_idx {
            Some(curr) => (curr + 1) % self.search_matches.len(),
            None => 0,
        };
        self.current_search_idx = Some(next);
        self.scroll_to_match(next);
        self.invalidate();
    }

    pub fn search_prev(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let total = self.search_matches.len();
        let prev = match self.current_search_idx {
            Some(curr) => {
                if curr == 0 {
                    total - 1
                } else {
                    curr - 1
                }
            }
            None => total - 1,
        };
        self.current_search_idx = Some(prev);
        self.scroll_to_match(prev);
        self.invalidate();
    }

    pub fn scroll_to_match(&mut self, match_idx: usize) {
        let Some(&m) = self.search_matches.get(match_idx) else { return };
        let Some(layout) = self.page_layouts.get(m.page_index) else { return };
        let Some(doc) = &self.doc else { return };
        let Ok(page) = doc.load_page(m.page_index) else { return };
        let rot = page.rotation();

        let center_pdf_x = (m.rect.left + m.rect.right) as f64 / 2.0;
        let center_pdf_y = (m.rect.top + m.rect.bottom) as f64 / 2.0;

        if let Some((_dev_x, dev_y)) = page.page_to_device(
            0,
            0,
            layout.render_w,
            layout.render_h,
            rot,
            center_pdf_x,
            center_pdf_y,
        ) {
            let avail_h = self.viewport_height - self.toolbar_height - self.status_height;
            let target_canvas_y = layout.y_offset + dev_y;
            let desired_scroll_y = target_canvas_y - (avail_h / 2);
            let max_scroll_y = (self.total_content_height - avail_h).max(0);
            self.scroll_y = desired_scroll_y.clamp(0, max_scroll_y);
            self.active_page_idx = m.page_index;
        }
    }

    pub fn update_search_controls_layout(&mut self) {
        if !self.search_open {
            return;
        }
        let dpi = self.dpi;
        let s = |v: i32| scale_dpi(v, dpi);

        let card_w = s(360);
        let card_h = s(38);
        let card_x = (self.viewport_width - card_w - s(20)).max(self.sidebar_width + s(10));
        let card_y = self.toolbar_height + s(10);

        self.search_card_rect = RECT {
            left: card_x,
            top: card_y,
            right: card_x + card_w,
            bottom: card_y + card_h,
        };

        let btn_sz = s(26);
        let btn_y = card_y + (card_h - btn_sz) / 2;

        self.search_prev_rect = RECT {
            left: card_x + card_w - s(96),
            top: btn_y,
            right: card_x + card_w - s(96) + btn_sz,
            bottom: btn_y + btn_sz,
        };
        self.search_next_rect = RECT {
            left: card_x + card_w - s(64),
            top: btn_y,
            right: card_x + card_w - s(64) + btn_sz,
            bottom: btn_y + btn_sz,
        };
        self.search_close_rect = RECT {
            left: card_x + card_w - s(32),
            top: btn_y,
            right: card_x + card_w - s(32) + btn_sz,
            bottom: btn_y + btn_sz,
        };

        if !self.search_edit_hwnd.is_null() {
            let edit_x = card_x + s(36);
            let edit_h = s(24);
            let edit_y = card_y + (card_h - edit_h) / 2;
            let edit_w = s(120);
            unsafe {
                MoveWindow(self.search_edit_hwnd, edit_x, edit_y, edit_w, edit_h, 1);
            }
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        unsafe {
            if !self.edit_bg_brush.is_null() {
                DeleteObject(self.edit_bg_brush as HGDIOBJ);
                self.edit_bg_brush = ptr::null_mut();
            }
            if !self.edit_font.is_null() {
                DeleteObject(self.edit_font as HGDIOBJ);
                self.edit_font = ptr::null_mut();
            }
            if !self.search_font.is_null() {
                DeleteObject(self.search_font as HGDIOBJ);
                self.search_font = ptr::null_mut();
            }
            if !self.search_edit_hwnd.is_null() {
                DestroyWindow(self.search_edit_hwnd);
                self.search_edit_hwnd = ptr::null_mut();
            }
            if !self.edit_hwnd.is_null() {
                DestroyWindow(self.edit_hwnd);
                self.edit_hwnd = ptr::null_mut();
            }
            KillTimer(self.hwnd, TIMER_SAVE_TOAST);
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
                state.is_dirty = true;
                state.tile_cache.invalidate_page(editor.page_index);
                state.thumb_cache.invalidate(editor.page_index);
                state.update_title();
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
                state.is_dirty = true;
                state.tile_cache.invalidate_page(editor.page_index);
                state.thumb_cache.invalidate(editor.page_index);
                state.update_title();
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
                    state.is_dirty = true;
                    state.tile_cache.invalidate_page(page_index);
                    state.thumb_cache.invalidate(page_index);
                    state.recompute_layouts();
                    state.update_title();
                    state.invalidate();
                }
            }
        }
        EditAction::EditTextRun { page_index, object_index, old_text, .. } => {
            if let Some(doc) = &state.doc {
                let _ = apply_text_mutation(doc, page_index, object_index, &old_text);
                state.is_dirty = true;
                state.tile_cache.invalidate_page(page_index);
                state.thumb_cache.invalidate(page_index);
                state.update_title();
                state.invalidate();
            }
        }
        EditAction::InsertTextRun { page_index, object_index, .. } => {
            if let Some(doc) = &state.doc {
                let _ = apply_text_mutation(doc, page_index, object_index, "");
                state.is_dirty = true;
                state.tile_cache.invalidate_page(page_index);
                state.thumb_cache.invalidate(page_index);
                state.update_title();
                state.invalidate();
            }
        }
        EditAction::InsertStamp { page_index, object_index } | EditAction::InsertImage { page_index, object_index } => {
            if let Some(doc) = &state.doc {
                let _ = remove_page_object(doc, page_index, object_index);
                state.is_dirty = true;
                state.tile_cache.invalidate_page(page_index);
                state.thumb_cache.invalidate(page_index);
                state.update_title();
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
                    state.is_dirty = true;
                    state.tile_cache.invalidate_page(page_index);
                    state.thumb_cache.invalidate(page_index);
                    state.recompute_layouts();
                    state.update_title();
                    state.invalidate();
                }
            }
        }
        EditAction::EditTextRun { page_index, object_index, new_text, .. } => {
            if let Some(doc) = &state.doc {
                let _ = apply_text_mutation(doc, page_index, object_index, &new_text);
                state.is_dirty = true;
                state.tile_cache.invalidate_page(page_index);
                state.thumb_cache.invalidate(page_index);
                state.update_title();
                state.invalidate();
            }
        }
        EditAction::InsertTextRun { page_index, object_index, text } => {
            if let Some(doc) = &state.doc {
                let _ = apply_text_mutation(doc, page_index, object_index, &text);
                state.is_dirty = true;
                state.tile_cache.invalidate_page(page_index);
                state.thumb_cache.invalidate(page_index);
                state.update_title();
                state.invalidate();
            }
        }
        _ => {}
    }
}

pub fn hit_test_gizmo_handle(state: &AppState, client_x: i32, client_y: i32) -> Option<GizmoHandle> {
    let gizmo = state.active_image_gizmo.as_ref()?;
    let doc = state.doc.as_ref()?;
    let layout = state.page_layouts.get(gizmo.page_index)?;
    let page = doc.load_page(gizmo.page_index).ok()?;
    let rot = page.rotation();

    let avail_w = state.viewport_width - state.sidebar_width;
    let page_screen_x = if avail_w > layout.render_w {
        state.sidebar_width + (avail_w - layout.render_w) / 2
    } else {
        state.sidebar_width + 24 - state.scroll_x
    };
    let page_screen_y = state.toolbar_height - state.scroll_y + layout.y_offset;

    let min_x_pt = gizmo.page_x;
    let max_x_pt = gizmo.page_x + gizmo.width_pt;
    let min_y_pt = gizmo.page_y;
    let max_y_pt = gizmo.page_y + gizmo.height_pt;

    let p_bl = page.page_to_device(0, 0, layout.render_w, layout.render_h, rot, min_x_pt, min_y_pt)?;
    let p_tr = page.page_to_device(0, 0, layout.render_w, layout.render_h, rot, max_x_pt, max_y_pt)?;

    let min_dev_x = page_screen_x + p_bl.0.min(p_tr.0);
    let max_dev_x = page_screen_x + p_bl.0.max(p_tr.0);
    let min_dev_y = page_screen_y + p_bl.1.min(p_tr.1);
    let max_dev_y = page_screen_y + p_bl.1.max(p_tr.1);

    let mid_dev_x = (min_dev_x + max_dev_x) / 2;
    let mid_dev_y = (min_dev_y + max_dev_y) / 2;

    let hit_r = scale_dpi(8, state.dpi);
    let near = |px: i32, py: i32| -> bool {
        (client_x - px).abs() <= hit_r && (client_y - py).abs() <= hit_r
    };

    if near(min_dev_x, min_dev_y) {
        Some(GizmoHandle::TopLeft)
    } else if near(max_dev_x, min_dev_y) {
        Some(GizmoHandle::TopRight)
    } else if near(min_dev_x, max_dev_y) {
        Some(GizmoHandle::BottomLeft)
    } else if near(max_dev_x, max_dev_y) {
        Some(GizmoHandle::BottomRight)
    } else if near(mid_dev_x, min_dev_y) {
        Some(GizmoHandle::TopCenter)
    } else if near(mid_dev_x, max_dev_y) {
        Some(GizmoHandle::BottomCenter)
    } else if near(min_dev_x, mid_dev_y) {
        Some(GizmoHandle::MiddleLeft)
    } else if near(max_dev_x, mid_dev_y) {
        Some(GizmoHandle::MiddleRight)
    } else if client_x >= min_dev_x && client_x <= max_dev_x && client_y >= min_dev_y && client_y <= max_dev_y {
        Some(GizmoHandle::Move)
    } else {
        None
    }
}

pub fn commit_active_image_gizmo(state: &mut AppState) {
    if let Some(gizmo) = state.active_image_gizmo.take() {
        if let Some(doc) = &state.doc {
            if let Ok(new_idx) = insert_image_object(
                doc,
                gizmo.page_index,
                gizmo.raw_width_px,
                gizmo.raw_height_px,
                gizmo.bgra_pixels.clone(),
                gizmo.page_x,
                gizmo.page_y,
                gizmo.width_pt,
                gizmo.height_pt,
            ) {
                state.undo_manager.push(EditAction::InsertImage {
                    page_index: gizmo.page_index,
                    object_index: new_idx,
                });
                state.is_dirty = true;
                state.tile_cache.invalidate_page(gizmo.page_index);
                state.thumb_cache.invalidate(gizmo.page_index);
                state.update_title();
                state.invalidate();
            }
        }
    }
}

pub fn cancel_active_image_gizmo(state: &mut AppState) {
    if state.active_image_gizmo.take().is_some() {
        state.invalidate();
    }
}

pub fn insert_stamp_at_center(state: &mut AppState, checkmark: bool) {
    let Some(doc) = &state.doc else { return };
    let page_idx = state.active_page_idx;
    let (w_pt, h_pt) = if let Some(layout) = state.page_layouts.get(page_idx) {
        (layout.width_pt, layout.height_pt)
    } else {
        (612.0, 792.0)
    };
    let center_x = w_pt / 2.0;
    let center_y = h_pt / 2.0;
    let size_pt = 48.0;

    let res = if checkmark {
        insert_stamp_checkmark(doc, page_idx, center_x, center_y, size_pt)
    } else {
        insert_stamp_cross(doc, page_idx, center_x, center_y, size_pt)
    };

    if let Ok(new_idx) = res {
        state.undo_manager.push(EditAction::InsertStamp {
            page_index: page_idx,
            object_index: new_idx,
        });
        state.is_dirty = true;
        state.tile_cache.invalidate_page(page_idx);
        state.thumb_cache.invalidate(page_idx);
        state.update_title();
        state.invalidate();
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

            let Some((pdf_x, pdf_y)) = page.device_to_page(
                0, 0, layout.render_w, layout.render_h, rot, dev_x, dev_y
            ) else {
                break;
            };

            let hit = hit_test_text_object(&page, pdf_x as f32, pdf_y as f32, 8.0);

            let (obj_idx, initial_text, screen_x, screen_y, screen_w, screen_h, bounds, font_size): (
                Option<usize>,
                String,
                i32,
                i32,
                i32,
                i32,
                RectF,
                f32,
            ) = if let Some((idx, text, bounds)) = hit {
                let pt1 = page.page_to_device(
                    0, 0, layout.render_w, layout.render_h, rot, bounds.left as f64, bounds.top as f64
                );
                let pt2 = page.page_to_device(
                    0, 0, layout.render_w, layout.render_h, rot, bounds.right as f64, bounds.bottom as f64
                );

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
                    OLD_EDIT_PROC = SetWindowLongPtrW(
                        edit_ctl,
                        GWLP_WNDPROC,
                        edit_subclass_proc as *const () as isize,
                    );
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
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            state.update_dpi(if dpi == 0 { 96 } else { dpi });
            0
        }

        WM_DPICHANGED => {
            let new_dpi = (wparam & 0xFFFF) as u32;
            state.tile_cache.clear();
            state.thumb_cache.clear();
            state.update_dpi(new_dpi);
            let prc = lparam as *const RECT;
            if !prc.is_null() {
                let rc = unsafe { *prc };
                unsafe {
                    MoveWindow(
                        hwnd,
                        rc.left,
                        rc.top,
                        rc.right - rc.left,
                        rc.bottom - rc.top,
                        1,
                    );
                }
            }
            0
        }

        WM_SIZE => {
            let w = (lparam & 0xFFFF) as i32;
            let h = ((lparam >> 16) & 0xFFFF) as i32;
            state.viewport_width = w;
            state.viewport_height = h;
            state.update_search_controls_layout();
            state.clamp_scroll();
            state.invalidate();
            0
        }

        WM_TIMER => {
            if wparam == TIMER_SAVE_TOAST {
                KillTimer(hwnd, TIMER_SAVE_TOAST);
                state.save_toast_active = false;
                state.invalidate();
                return 0;
            }
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
                } else if state.search_open && state.search_card_rect.contains(pt.x, pt.y) {
                    unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) }
                } else if state.show_default_app_banner && state.banner_rect.contains(pt.x, pt.y) {
                    unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) }
                } else if let Some(handle) = hit_test_gizmo_handle(state, pt.x, pt.y) {
                    match handle {
                        GizmoHandle::TopLeft | GizmoHandle::BottomRight => unsafe { LoadCursorW(ptr::null_mut(), IDC_SIZENWSE) },
                        GizmoHandle::TopRight | GizmoHandle::BottomLeft => unsafe { LoadCursorW(ptr::null_mut(), IDC_SIZENESW) },
                        GizmoHandle::TopCenter | GizmoHandle::BottomCenter => unsafe { LoadCursorW(ptr::null_mut(), IDC_SIZENS) },
                        GizmoHandle::MiddleLeft | GizmoHandle::MiddleRight => unsafe { LoadCursorW(ptr::null_mut(), IDC_SIZEWE) },
                        GizmoHandle::Move => unsafe { LoadCursorW(ptr::null_mut(), IDC_SIZEALL) },
                    }
                } else {
                    match state.tool_mode {
                        ToolMode::Pan => unsafe { LoadCursorW(ptr::null_mut(), IDC_HAND) },
                        ToolMode::EditTextObject => unsafe { LoadCursorW(ptr::null_mut(), IDC_IBEAM) },
                        ToolMode::RedactClear | ToolMode::RedactBlackline | ToolMode::StampCheckmark | ToolMode::StampCross => {
                            unsafe { LoadCursorW(ptr::null_mut(), IDC_CROSS) }
                        }
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

            // Check floating default app banner clicks
            if state.show_default_app_banner && state.banner_rect.contains(x, y) {
                if state.banner_set_btn_rect.contains(x, y) {
                    zpdf::default_app::register_prog_id();
                    zpdf::default_app::launch_default_apps_settings();
                    state.show_default_app_banner = false;
                    state.invalidate();
                    return 0;
                } else if state.banner_never_btn_rect.contains(x, y) {
                    zpdf::default_app::set_never_ask_default(true);
                    state.show_default_app_banner = false;
                    state.invalidate();
                    return 0;
                } else if state.banner_close_btn_rect.contains(x, y) {
                    state.show_default_app_banner = false;
                    state.invalidate();
                    return 0;
                }
                return 0;
            }

            // Check floating search bar clicks
            if state.search_open {
                if state.search_prev_rect.contains(x, y) {
                    state.search_prev();
                    return 0;
                } else if state.search_next_rect.contains(x, y) {
                    state.search_next();
                    return 0;
                } else if state.search_close_rect.contains(x, y) {
                    state.close_search();
                    return 0;
                } else if state.search_card_rect.contains(x, y) {
                    return 0;
                }
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

            // Check image gizmo handle hit
            if state.active_image_gizmo.is_some() {
                if let Some(handle) = hit_test_gizmo_handle(state, x, y) {
                    let gizmo = state.active_image_gizmo.as_mut().unwrap();
                    gizmo.dragging_handle = Some(handle);
                    gizmo.drag_start_mouse = (x, y);
                    gizmo.original_bounds = (gizmo.page_x, gizmo.page_y, gizmo.width_pt, gizmo.height_pt);
                    unsafe { SetCapture(hwnd) };
                    return 0;
                } else {
                    commit_active_image_gizmo(state);
                }
            }

            // Find clicked page and PDF coordinates
            let mut clicked_page_info = None;
            let avail_w = state.viewport_width - state.sidebar_width;
            for layout in &state.page_layouts {
                let page_x = if avail_w > layout.render_w {
                    state.sidebar_width + (avail_w - layout.render_w) / 2
                } else {
                    state.sidebar_width + 24 - state.scroll_x
                };
                let page_y = state.toolbar_height - state.scroll_y + layout.y_offset;

                if x >= page_x && x <= page_x + layout.render_w && y >= page_y && y <= page_y + layout.render_h {
                    if let Some(doc) = &state.doc {
                        if let Ok(page) = doc.load_page(layout.index) {
                            let rot = page.rotation();
                            let dev_x = x - page_x;
                            let dev_y = y - page_y;
                            if let Some((pdf_x, pdf_y)) = page.device_to_page(0, 0, layout.render_w, layout.render_h, rot, dev_x, dev_y) {
                                clicked_page_info = Some((layout.index, pdf_x, pdf_y));
                            }
                        }
                    }
                    break;
                }
            }

            // Route to AcroForms in Pan mode:
            if let Some((p_idx, pdf_x, pdf_y)) = clicked_page_info {
                if state.tool_mode == ToolMode::Pan {
                    if let Some(form_env) = &state.form_env {
                        if let Some(doc) = &state.doc {
                            if let Ok(page) = doc.load_page(p_idx) {
                                let mut modifier = 0;
                                if (unsafe { GetKeyState(0x11) } as u16 & 0x8000) != 0 { modifier |= 1 << 0; }
                                if (unsafe { GetKeyState(0x10) } as u16 & 0x8000) != 0 { modifier |= 1 << 1; }
                                if (unsafe { GetKeyState(0x12) } as u16 & 0x8000) != 0 { modifier |= 1 << 2; }
                                if form_env.on_lbutton_down(&page, modifier, pdf_x, pdf_y) {
                                    state.tile_cache.invalidate_page(p_idx);
                                    state.invalidate();
                                    return 0;
                                }
                            }
                        }
                    }
                }
            }

            match state.tool_mode {
                ToolMode::Pan => {
                    commit_active_text_editor(state);
                    state.is_dragging = true;
                    state.drag_start_x = x;
                    state.drag_start_y = y;
                    state.drag_start_scroll_x = state.scroll_x;
                    state.drag_start_scroll_y = state.scroll_y;
                    unsafe { SetCapture(hwnd) };
                }
                ToolMode::EditTextObject => {
                    handle_text_edit_click(state, x, y);
                }
                ToolMode::StampCheckmark | ToolMode::StampCross => {
                    if let Some((p_idx, pdf_x, pdf_y)) = clicked_page_info {
                        if let Some(doc) = &state.doc {
                            let res = if state.tool_mode == ToolMode::StampCheckmark {
                                insert_stamp_checkmark(doc, p_idx, pdf_x, pdf_y, 48.0)
                            } else {
                                insert_stamp_cross(doc, p_idx, pdf_x, pdf_y, 48.0)
                            };
                            if let Ok(new_idx) = res {
                                state.undo_manager.push(EditAction::InsertStamp {
                                    page_index: p_idx,
                                    object_index: new_idx,
                                });
                                state.is_dirty = true;
                                state.tile_cache.invalidate_page(p_idx);
                                state.thumb_cache.invalidate(p_idx);
                                state.update_title();
                                state.invalidate();
                            }
                        }
                    }
                }
                ToolMode::RedactClear | ToolMode::RedactBlackline => {
                    if let Some((p_idx, pdf_x, pdf_y)) = clicked_page_info {
                        state.redact_drag_rect = Some((
                            p_idx,
                            RectF {
                                left: pdf_x as f32,
                                bottom: pdf_y as f32,
                                right: pdf_x as f32,
                                top: pdf_y as f32,
                            },
                        ));
                        unsafe { SetCapture(hwnd) };
                    }
                }
                _ => {}
            }
            0
        }

        WM_RBUTTONUP => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            // Thumbnail context menu
            if x < state.sidebar_width && y >= state.toolbar_height && y < state.viewport_height - state.status_height {
                let rel_y = y - state.toolbar_height + state.sidebar_scroll_y - 28;
                if rel_y >= 0 {
                    if let Some(doc) = &state.doc {
                        let count = doc.page_count();
                        let mut current_y = 0;
                        let thumb_max_w = 144;
                        let thumb_max_h = 160;
                        let mut clicked_page = None;

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
                                clicked_page = Some(i);
                                break;
                            }
                            current_y += slot_h;
                        }

                        if let Some(page_idx) = clicked_page {
                            state.active_page_idx = page_idx;
                            state.invalidate();

                            unsafe {
                                let hmenu = CreatePopupMenu();
                                if !hmenu.is_null() {
                                    let w_rot_cw = to_wide_null("Rotate Clockwise\t(R)");
                                    let w_rot_ccw = to_wide_null("Rotate Counter-Clockwise");
                                    let w_del = to_wide_null("Delete Page\t(Del)");

                                    AppendMenuW(hmenu, MF_STRING, 1, w_rot_cw.as_ptr());
                                    AppendMenuW(hmenu, MF_STRING, 2, w_rot_ccw.as_ptr());
                                    AppendMenuW(hmenu, MF_SEPARATOR, 0, ptr::null());

                                    let del_flags = if doc.page_count() <= 1 {
                                        MF_STRING | MF_GRAYED
                                    } else {
                                        MF_STRING
                                    };
                                    AppendMenuW(hmenu, del_flags, 3, w_del.as_ptr());

                                    let mut pt = POINT { x, y };
                                    ClientToScreen(hwnd, &mut pt);

                                    let cmd = TrackPopupMenu(
                                        hmenu,
                                        TPM_RIGHTBUTTON | TPM_RETURNCMD,
                                        pt.x,
                                        pt.y,
                                        0,
                                        hwnd,
                                        ptr::null(),
                                    );
                                    DestroyMenu(hmenu);

                                    match cmd {
                                        1 => state.rotate_page_clockwise(page_idx),
                                        2 => state.rotate_page_counter_clockwise(page_idx),
                                        3 => state.delete_page(page_idx),
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
                return 0;
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
            unsafe { TrackMouseEvent(&mut tme) };

            // 1. Gizmo Handle Dragging
            if let Some(gizmo) = &mut state.active_image_gizmo {
                if let Some(handle) = gizmo.dragging_handle {
                    let dx_mouse = x - gizmo.drag_start_mouse.0;
                    let dy_mouse = y - gizmo.drag_start_mouse.1;
                    let scale = if let Some(layout) = state.page_layouts.get(gizmo.page_index) {
                        (layout.render_w as f64) / layout.width_pt.max(1.0)
                    } else {
                        1.0
                    };
                    let dx_pt = (dx_mouse as f64) / scale;
                    let dy_pt = -(dy_mouse as f64) / scale;
                    let (orig_x, orig_y, orig_w, orig_h) = gizmo.original_bounds;

                    match handle {
                        GizmoHandle::Move => {
                            gizmo.page_x = orig_x + dx_pt;
                            gizmo.page_y = orig_y + dy_pt;
                        }
                        GizmoHandle::TopRight => {
                            gizmo.width_pt = (orig_w + dx_pt).max(10.0);
                            gizmo.height_pt = (orig_h + dy_pt).max(10.0);
                        }
                        GizmoHandle::TopLeft => {
                            let new_w = (orig_w - dx_pt).max(10.0);
                            gizmo.page_x = orig_x + (orig_w - new_w);
                            gizmo.width_pt = new_w;
                            gizmo.height_pt = (orig_h + dy_pt).max(10.0);
                        }
                        GizmoHandle::BottomRight => {
                            gizmo.width_pt = (orig_w + dx_pt).max(10.0);
                            let new_h = (orig_h - dy_pt).max(10.0);
                            gizmo.page_y = orig_y + (orig_h - new_h);
                            gizmo.height_pt = new_h;
                        }
                        GizmoHandle::BottomLeft => {
                            let new_w = (orig_w - dx_pt).max(10.0);
                            gizmo.page_x = orig_x + (orig_w - new_w);
                            gizmo.width_pt = new_w;
                            let new_h = (orig_h - dy_pt).max(10.0);
                            gizmo.page_y = orig_y + (orig_h - new_h);
                            gizmo.height_pt = new_h;
                        }
                        GizmoHandle::TopCenter => {
                            gizmo.height_pt = (orig_h + dy_pt).max(10.0);
                        }
                        GizmoHandle::BottomCenter => {
                            let new_h = (orig_h - dy_pt).max(10.0);
                            gizmo.page_y = orig_y + (orig_h - new_h);
                            gizmo.height_pt = new_h;
                        }
                        GizmoHandle::MiddleRight => {
                            gizmo.width_pt = (orig_w + dx_pt).max(10.0);
                        }
                        GizmoHandle::MiddleLeft => {
                            let new_w = (orig_w - dx_pt).max(10.0);
                            gizmo.page_x = orig_x + (orig_w - new_w);
                            gizmo.width_pt = new_w;
                        }
                    }
                    state.invalidate();
                    return 0;
                }
            }

            // 2. Redaction Dragging
            if let Some((p_idx, ref mut r)) = state.redact_drag_rect {
                let avail_w = state.viewport_width - state.sidebar_width;
                if let Some(layout) = state.page_layouts.get(p_idx) {
                    let page_x = if avail_w > layout.render_w {
                        state.sidebar_width + (avail_w - layout.render_w) / 2
                    } else {
                        state.sidebar_width + 24 - state.scroll_x
                    };
                    let page_y = state.toolbar_height - state.scroll_y + layout.y_offset;
                    if let Some(doc) = &state.doc {
                        if let Ok(page) = doc.load_page(p_idx) {
                            let rot = page.rotation();
                            if let Some((pdf_x, pdf_y)) = page.device_to_page(0, 0, layout.render_w, layout.render_h, rot, x - page_x, y - page_y) {
                                r.right = pdf_x as f32;
                                r.top = pdf_y as f32;
                                state.invalidate();
                                return 0;
                            }
                        }
                    }
                }
            }

            if y < state.toolbar_height {
                let new_ctrl = get_toolbar_control_at(state, x, y);
                if new_ctrl != state.hovered_control {
                    state.hovered_control = new_ctrl;
                    state.invalidate();
                }
            } else if state.search_open && state.search_card_rect.contains(x, y) {
                let new_ctrl = get_search_control_at(state, x, y);
                if new_ctrl != state.hovered_control {
                    state.hovered_control = new_ctrl;
                    state.invalidate();
                }
            } else if state.doc.is_none() {
                let on_btn = state.welcome_btn_rect.contains(x, y);
                let new_ctrl = if on_btn { Some(ToolbarControl::WelcomeOpen) } else { None };
                if new_ctrl != state.hovered_control {
                    state.hovered_control = new_ctrl;
                    state.invalidate();
                }
            } else if state.hovered_control.is_some() {
                state.hovered_control = None;
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
            if state.hovered_control.is_some() {
                state.hovered_control = None;
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
            if let Some(gizmo) = &mut state.active_image_gizmo {
                if gizmo.dragging_handle.is_some() {
                    gizmo.dragging_handle = None;
                    unsafe { ReleaseCapture() };
                    return 0;
                }
            }

            if let Some((p_idx, r)) = state.redact_drag_rect.take() {
                unsafe { ReleaseCapture() };
                let left = r.left.min(r.right);
                let right = r.left.max(r.right);
                let bottom = r.bottom.min(r.top);
                let top = r.bottom.max(r.top);
                let normalized = RectF { left, bottom, right, top };
                if (right - left).abs() > 4.0 && (top - bottom).abs() > 4.0 {
                    let blackline = state.tool_mode == ToolMode::RedactBlackline;
                    if let Some(doc) = &state.doc {
                        if let Ok(removed) = apply_redaction(doc, p_idx, normalized, blackline) {
                            if removed > 0 {
                                state.is_dirty = true;
                                state.tile_cache.invalidate_page(p_idx);
                                state.thumb_cache.invalidate(p_idx);
                                state.update_title();
                            }
                        }
                    }
                }
                state.invalidate();
                return 0;
            }

            if state.is_dragging {
                state.is_dragging = false;
                unsafe { ReleaseCapture() };
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
            unsafe { ScreenToClient(hwnd, &mut pt) };

            if ctrl {
                if delta > 0 {
                    state.set_zoom(state.zoom_pct + 15);
                } else {
                    state.set_zoom(state.zoom_pct.saturating_sub(15));
                }
            } else if pt.x < state.sidebar_width && pt.y >= state.toolbar_height && pt.y < state.viewport_height - state.status_height {
                // Scroll thumbnail rail
                let scroll_step = ((delta as f64 / 120.0) * (scale_dpi(80, state.dpi) as f64)).round() as i32;
                let avail_h = state.viewport_height - state.toolbar_height - state.status_height;
                let max_scroll = (state.sidebar_total_height - avail_h).max(0);
                if max_scroll > 0 {
                    state.sidebar_scroll_y = (state.sidebar_scroll_y - scroll_step).clamp(0, max_scroll);
                    state.invalidate();
                }
            } else if state.doc.is_some() {
                commit_active_text_editor(state);
                let scroll_step = ((delta as f64 / 120.0) * (scale_dpi(64, state.dpi) as f64)).round() as i32;
                let avail_h = state.viewport_height - state.toolbar_height - state.status_height;
                let max_scroll = (state.total_content_height - avail_h).max(0);
                if max_scroll > 0 {
                    state.scroll_y = (state.scroll_y - scroll_step).clamp(0, max_scroll);
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
            } else if cid == 1002 && notify == EN_CHANGE {
                state.on_search_query_changed();
            }
            0
        }

        WM_KEYDOWN => {
            let vk = wparam as i32;
            let ctrl = (unsafe { GetKeyState(0x11) } as u16 & 0x8000) != 0;
            let shift = (unsafe { GetKeyState(0x10) } as u16 & 0x8000) != 0;

            if ctrl {
                match vk {
                    0x4F => { state.prompt_open_file(); }
                    0x53 => {
                        if shift {
                            let _ = state.save_as();
                        } else {
                            let _ = state.save_current();
                        }
                    }
                    0x5A => {
                        if shift {
                            handle_redo(state);
                        } else {
                            handle_undo(state);
                        }
                    }
                    0x59 => { handle_redo(state); }
                    0x46 => { state.toggle_search(); }
                    0x56 => {
                        // Ctrl+V: Paste Image from Clipboard
                        if let Some((raw_w, raw_h, bgra)) = zpdf::wic_decoder::get_clipboard_image_bgra() {
                            let p_idx = state.active_page_idx;
                            let (page_w_pt, page_h_pt) = if let Some(layout) = state.page_layouts.get(p_idx) {
                                (layout.width_pt, layout.height_pt)
                            } else {
                                (612.0, 792.0)
                            };
                            let aspect = (raw_w as f64) / (raw_h as f64).max(1.0);
                            let mut w_pt = 200.0;
                            let mut h_pt = w_pt / aspect;
                            if h_pt > 250.0 {
                                h_pt = 250.0;
                                w_pt = h_pt * aspect;
                            }
                            let page_x = (page_w_pt - w_pt) / 2.0;
                            let page_y = (page_h_pt - h_pt) / 2.0;
                            state.active_image_gizmo = Some(ActiveImageGizmo {
                                page_index: p_idx,
                                object_index: None,
                                page_x,
                                page_y,
                                width_pt: w_pt,
                                height_pt: h_pt,
                                raw_width_px: raw_w,
                                raw_height_px: raw_h,
                                bgra_pixels: bgra,
                                dragging_handle: None,
                                drag_start_mouse: (0, 0),
                                original_bounds: (page_x, page_y, w_pt, h_pt),
                            });
                            state.invalidate();
                        }
                    }
                    0xBB | 0x6B => { state.set_zoom(state.zoom_pct + 15); }
                    0xBD | 0x6D => { state.set_zoom(state.zoom_pct.saturating_sub(15)); }
                    0x30 | 0x60 => { state.set_zoom(100); }
                    _ => {}
                }
            } else {
                match vk {
                    0x0D => {
                        if state.active_image_gizmo.is_some() {
                            commit_active_image_gizmo(state);
                            return 0;
                        }
                    }
                    0x1B => {
                        if state.active_image_gizmo.is_some() {
                            cancel_active_image_gizmo(state);
                            return 0;
                        } else if state.search_open {
                            state.close_search();
                            return 0;
                        }
                    }
                    0x56 => { // 'V'
                        insert_stamp_at_center(state, true);
                    }
                    0x58 => { // 'X'
                        insert_stamp_at_center(state, false);
                    }
                    0x48 => { state.tool_mode = ToolMode::Pan; state.invalidate(); }
                    0x54 => { state.tool_mode = ToolMode::EditTextObject; state.invalidate(); }
                    0x52 => { state.rotate_current_page(); }
                    0x2E => { state.delete_current_page(); }
                    VK_PRIOR => {
                        let avail_h = state.viewport_height - state.toolbar_height - state.status_height;
                        state.scroll_y = (state.scroll_y - (avail_h * 4 / 5)).max(0);
                        state.invalidate();
                    }
                    VK_NEXT => {
                        let avail_h = state.viewport_height - state.toolbar_height - state.status_height;
                        let max_scroll_y = (state.total_content_height - avail_h).max(0);
                        state.scroll_y = (state.scroll_y + (avail_h * 4 / 5)).min(max_scroll_y);
                        state.invalidate();
                    }
                    VK_HOME => {
                        state.scroll_y = 0;
                        state.scroll_x = 0;
                        state.invalidate();
                    }
                    VK_END => {
                        let avail_h = state.viewport_height - state.toolbar_height - state.status_height;
                        state.scroll_y = (state.total_content_height - avail_h).max(0);
                        state.invalidate();
                    }
                    _ => {}
                }
            }

            if state.active_editor.is_none() && !state.search_open {
                if let Some(form_env) = &state.form_env {
                    if let Some(doc) = &state.doc {
                        if let Ok(page) = doc.load_page(state.active_page_idx) {
                            let mut modifier = 0;
                            if (unsafe { GetKeyState(0x11) } as u16 & 0x8000) != 0 { modifier |= 1 << 0; }
                            if (unsafe { GetKeyState(0x10) } as u16 & 0x8000) != 0 { modifier |= 1 << 1; }
                            if (unsafe { GetKeyState(0x12) } as u16 & 0x8000) != 0 { modifier |= 1 << 2; }
                            if form_env.on_key_down(&page, vk, modifier) {
                                state.tile_cache.invalidate_page(state.active_page_idx);
                                state.invalidate();
                                return 0;
                            }
                        }
                    }
                }
            }
            0
        }

        WM_KEYUP => {
            let vk = wparam as i32;
            if state.active_editor.is_none() && !state.search_open {
                if let Some(form_env) = &state.form_env {
                    if let Some(doc) = &state.doc {
                        if let Ok(page) = doc.load_page(state.active_page_idx) {
                            let mut modifier = 0;
                            if (unsafe { GetKeyState(0x11) } as u16 & 0x8000) != 0 { modifier |= 1 << 0; }
                            if (unsafe { GetKeyState(0x10) } as u16 & 0x8000) != 0 { modifier |= 1 << 1; }
                            if (unsafe { GetKeyState(0x12) } as u16 & 0x8000) != 0 { modifier |= 1 << 2; }
                            if form_env.on_key_up(&page, vk, modifier) {
                                state.tile_cache.invalidate_page(state.active_page_idx);
                                state.invalidate();
                                return 0;
                            }
                        }
                    }
                }
            }
            0
        }

        WM_CHAR => {
            if state.active_editor.is_none() && !state.search_open {
                if let Some(form_env) = &state.form_env {
                    if let Some(doc) = &state.doc {
                        if let Ok(page) = doc.load_page(state.active_page_idx) {
                            let mut modifier = 0;
                            if (unsafe { GetKeyState(0x11) } as u16 & 0x8000) != 0 { modifier |= 1 << 0; }
                            if (unsafe { GetKeyState(0x10) } as u16 & 0x8000) != 0 { modifier |= 1 << 1; }
                            if (unsafe { GetKeyState(0x12) } as u16 & 0x8000) != 0 { modifier |= 1 << 2; }
                            if form_env.on_char(&page, wparam as i32, modifier) {
                                state.tile_cache.invalidate_page(state.active_page_idx);
                                state.invalidate();
                                return 0;
                            }
                        }
                    }
                }
            }
            0
        }

        WM_DESTROY => {
            if !state.backbuffer_dc.is_null() {
                unsafe {
                    SelectObject(state.backbuffer_dc, state.backbuffer_old_bmp);
                    DeleteObject(state.backbuffer_bmp as HGDIOBJ);
                    DeleteDC(state.backbuffer_dc);
                    state.backbuffer_dc = ptr::null_mut();
                    state.backbuffer_bmp = ptr::null_mut();
                }
            }
            PostQuitMessage(0);
            0
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn get_toolbar_control_at(state: &AppState, x: i32, y: i32) -> Option<ToolbarControl> {
    if state.tb_open_rect.contains(x, y) {
        Some(ToolbarControl::Open)
    } else if state.tb_save_rect.contains(x, y) {
        Some(ToolbarControl::Save)
    } else if state.tb_undo_rect.contains(x, y) {
        Some(ToolbarControl::Undo)
    } else if state.tb_redo_rect.contains(x, y) {
        Some(ToolbarControl::Redo)
    } else if state.tb_hand_rect.contains(x, y) {
        Some(ToolbarControl::ToolPan)
    } else if state.tb_edit_rect.contains(x, y) {
        Some(ToolbarControl::ToolEdit)
    } else if state.tb_stamp_v_rect.contains(x, y) {
        Some(ToolbarControl::ToolStampCheckmark)
    } else if state.tb_stamp_x_rect.contains(x, y) {
        Some(ToolbarControl::ToolStampCross)
    } else if state.tb_image_rect.contains(x, y) {
        Some(ToolbarControl::ToolImage)
    } else if state.tb_redact_rect.contains(x, y) {
        Some(ToolbarControl::ToolRedactBlackline)
    } else if state.tb_zoom_out_rect.contains(x, y) {
        Some(ToolbarControl::ZoomOut)
    } else if state.tb_zoom_reset_rect.contains(x, y) {
        Some(ToolbarControl::ZoomReset)
    } else if state.tb_zoom_in_rect.contains(x, y) {
        Some(ToolbarControl::ZoomIn)
    } else if state.tb_search_rect.contains(x, y) {
        Some(ToolbarControl::SearchToggle)
    } else {
        None
    }
}

fn get_search_control_at(state: &AppState, x: i32, y: i32) -> Option<ToolbarControl> {
    if !state.search_open {
        return None;
    }
    if state.search_prev_rect.contains(x, y) {
        Some(ToolbarControl::SearchPrev)
    } else if state.search_next_rect.contains(x, y) {
        Some(ToolbarControl::SearchNext)
    } else if state.search_close_rect.contains(x, y) {
        Some(ToolbarControl::SearchClose)
    } else {
        None
    }
}

fn handle_toolbar_click(state: &mut AppState, x: i32, y: i32) {
    if state.tb_open_rect.contains(x, y) {
        state.prompt_open_file();
    } else if state.tb_save_rect.contains(x, y) {
        let _ = state.save_current();
    } else if state.tb_undo_rect.contains(x, y) {
        handle_undo(state);
    } else if state.tb_redo_rect.contains(x, y) {
        handle_redo(state);
    } else if state.tb_hand_rect.contains(x, y) {
        state.tool_mode = ToolMode::Pan;
        state.invalidate();
    } else if state.tb_edit_rect.contains(x, y) {
        state.tool_mode = ToolMode::EditTextObject;
        state.invalidate();
    } else if state.tb_stamp_v_rect.contains(x, y) {
        if state.tool_mode == ToolMode::StampCheckmark {
            state.tool_mode = ToolMode::Pan;
        } else {
            state.tool_mode = ToolMode::StampCheckmark;
        }
        state.invalidate();
    } else if state.tb_stamp_x_rect.contains(x, y) {
        if state.tool_mode == ToolMode::StampCross {
            state.tool_mode = ToolMode::Pan;
        } else {
            state.tool_mode = ToolMode::StampCross;
        }
        state.invalidate();
    } else if state.tb_image_rect.contains(x, y) {
        state.prompt_insert_image();
    } else if state.tb_redact_rect.contains(x, y) {
        if state.tool_mode == ToolMode::RedactBlackline {
            state.tool_mode = ToolMode::Pan;
        } else {
            state.tool_mode = ToolMode::RedactBlackline;
        }
        state.invalidate();
    } else if state.tb_zoom_out_rect.contains(x, y) {
        state.set_zoom(state.zoom_pct.saturating_sub(15));
    } else if state.tb_zoom_reset_rect.contains(x, y) {
        state.set_zoom(100);
    } else if state.tb_zoom_in_rect.contains(x, y) {
        state.set_zoom(state.zoom_pct + 15);
    } else if state.tb_search_rect.contains(x, y) {
        state.toggle_search();
    }
}

fn handle_sidebar_click(state: &mut AppState, _x: i32, y: i32) {
    let Some(doc) = &state.doc else { return };
    let count = doc.page_count();
    let dpi = state.dpi;
    let s = |v: i32| scale_dpi(v, dpi);
    let rel_y = y - state.toolbar_height + state.sidebar_scroll_y - s(28);
    if rel_y < 0 {
        return;
    }

    let mut current_y = 0;
    let thumb_max_w = s(144);
    let thumb_max_h = s(160);

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
        if th < s(40) { th = s(40); }
        let slot_h = th + s(34);

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
        if state.backbuffer_dc.is_null() || state.backbuffer_w != width || state.backbuffer_h != height {
            if !state.backbuffer_dc.is_null() {
                SelectObject(state.backbuffer_dc, state.backbuffer_old_bmp);
                DeleteObject(state.backbuffer_bmp as HGDIOBJ);
                DeleteDC(state.backbuffer_dc);
            }
            state.backbuffer_dc = CreateCompatibleDC(hdc);
            state.backbuffer_bmp = CreateCompatibleBitmap(hdc, width, height);
            state.backbuffer_old_bmp = SelectObject(state.backbuffer_dc, state.backbuffer_bmp as HGDIOBJ);
            state.backbuffer_w = width;
            state.backbuffer_h = height;
        }

        let mem_dc = state.backbuffer_dc;

        // Deep Obsidian Slate Backdrop (#0D0D11)
        let bg_brush = CreateSolidBrush(rgb(0x0D, 0x0D, 0x11));
        FillRect(mem_dc, &rect, bg_brush);
        DeleteObject(bg_brush as HGDIOBJ);

        if state.doc.is_some() {
            // Render Document Pages (with live search highlights)
            paint_pages(state, mem_dc, width, height);
            // Render Left Sidebar (Live Thumbnails)
            paint_sidebar(state, mem_dc, height);
        } else {
            // Render Minimalist Welcome Dropzone
            paint_empty_state(state, mem_dc, width, height);
        }

        // Render Modern 3-Zone Linear Top Toolbar
        paint_toolbar(state, mem_dc, width);

        // Render Floating Search Bar if active
        paint_search_bar(state, mem_dc);

        // Render Floating Default App Banner if active
        if state.show_default_app_banner {
            paint_default_app_banner(state, mem_dc, width);
        }

        // Render Minimalist Status Bar
        paint_status_bar(state, mem_dc, width, height);

        BitBlt(hdc, 0, 0, width, height, mem_dc, 0, 0, SRCCOPY);
    }
}

fn paint_empty_state(state: &mut AppState, hdc: HDC, width: i32, height: i32) {
    let dpi = state.dpi;
    let s = |v: i32| scale_dpi(v, dpi);

    unsafe {
        let card_w = s(460);
        let card_h = s(280);
        let card_x = (width - card_w) / 2;
        let card_y = (height - card_h) / 2;

        // Container Card (#16161B with subtle 1px border #26262F)
        let card_brush = CreateSolidBrush(rgb(0x16, 0x16, 0x1B));
        let border_pen = CreatePen(0, 1, rgb(0x26, 0x26, 0x2F));
        let old_brush = SelectObject(hdc, card_brush as HGDIOBJ);
        let old_pen = SelectObject(hdc, border_pen as HGDIOBJ);
        RoundRect(hdc, card_x, card_y, card_x + card_w, card_y + card_h, s(16), s(16));
        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        DeleteObject(card_brush as HGDIOBJ);
        DeleteObject(border_pen as HGDIOBJ);

        SetBkMode(hdc, TRANSPARENT);

        // Application Icon centered
        let icon_size = s(48);
        let icon_x = card_x + (card_w - icon_size) / 2;
        let icon_y = card_y + s(32);
        if !state.app_icon.is_null() {
            DrawIconEx(hdc, icon_x, icon_y, state.app_icon, icon_size, icon_size, 0, ptr::null_mut(), 0x0003);
        }

        // Title: 18pt Bold Segoe UI
        let title_font = create_ui_font(18.0, 700, dpi);
        let old_f = SelectObject(hdc, title_font as HGDIOBJ);
        SetTextColor(hdc, rgb(0xED, 0xED, 0xF0));
        let title_w = to_wide_null("zPDF");
        TextOutW(hdc, card_x + (card_w - s(50)) / 2, card_y + s(94), title_w.as_ptr(), (title_w.len() - 1) as i32);
        SelectObject(hdc, old_f);
        DeleteObject(title_font as HGDIOBJ);

        // Subtitle: 10.5pt Regular Segoe UI
        let sub_font = create_ui_font(10.5, 400, dpi);
        let old_f = SelectObject(hdc, sub_font as HGDIOBJ);
        SetTextColor(hdc, rgb(0x90, 0x90, 0x9A));
        let sub_w = to_wide_null("Drop a PDF anywhere to begin");
        TextOutW(hdc, card_x + (card_w - s(190)) / 2, card_y + s(128), sub_w.as_ptr(), (sub_w.len() - 1) as i32);
        SelectObject(hdc, old_f);
        DeleteObject(sub_font as HGDIOBJ);

        // Action CTA: "Open File"
        let btn_w = s(160);
        let btn_h = s(38);
        let btn_x = card_x + (card_w - btn_w) / 2;
        let btn_y = card_y + s(170);
        state.welcome_btn_rect = RECT { left: btn_x, top: btn_y, right: btn_x + btn_w, bottom: btn_y + btn_h };

        let is_hovered = state.hovered_control == Some(ToolbarControl::WelcomeOpen);
        let btn_color = if is_hovered { rgb(0x32, 0x32, 0x3A) } else { rgb(0x24, 0x24, 0x2D) };
        let btn_border = if is_hovered { rgb(0x3E, 0x3E, 0x4C) } else { rgb(0x2F, 0x2F, 0x3B) };
        let btn_brush = CreateSolidBrush(btn_color);
        let btn_pen = CreatePen(0, 1, btn_border);
        let old_b = SelectObject(hdc, btn_brush as HGDIOBJ);
        let old_p = SelectObject(hdc, btn_pen as HGDIOBJ);
        RoundRect(hdc, btn_x, btn_y, btn_x + btn_w, btn_y + btn_h, s(8), s(8));
        SelectObject(hdc, old_b);
        SelectObject(hdc, old_p);
        DeleteObject(btn_brush as HGDIOBJ);
        DeleteObject(btn_pen as HGDIOBJ);

        let btn_font = create_ui_font(10.5, 600, dpi);
        let old_f2 = SelectObject(hdc, btn_font as HGDIOBJ);
        SetTextColor(hdc, rgb(0xED, 0xED, 0xF0));
        draw_vector_open(hdc, btn_x + s(38), btn_y + btn_h / 2, dpi, rgb(0xED, 0xED, 0xF0));
        let open_w = to_wide_null("Open File");
        TextOutW(hdc, btn_x + s(54), btn_y + s(9), open_w.as_ptr(), (open_w.len() - 1) as i32);
        SelectObject(hdc, old_f2);
        DeleteObject(btn_font as HGDIOBJ);

        // Keyboard Shortcut hint: 9.5pt Regular Segoe UI
        let hint_font = create_ui_font(9.5, 400, dpi);
        let old_hf = SelectObject(hdc, hint_font as HGDIOBJ);
        SetTextColor(hdc, rgb(0x5E, 0x5E, 0x6C));
        let hint_w = to_wide_null("or press Ctrl + O");
        TextOutW(hdc, card_x + (card_w - s(105)) / 2, btn_y + s(44), hint_w.as_ptr(), (hint_w.len() - 1) as i32);
        SelectObject(hdc, old_hf);
        DeleteObject(hint_font as HGDIOBJ);
    }
}

// ==========================================
// Pixel-Perfect Pure GDI Vector Icon Helpers
// ==========================================

const NULL_BRUSH: i32 = 5;

unsafe fn draw_vector_undo(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(2).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    // Arch curving from bottom-right up and over to left
    let arch_pts = [
        POINT { x: cx + s(5), y: cy + s(4) },
        POINT { x: cx + s(5), y: cy },
        POINT { x: cx + s(3), y: cy - s(4) },
        POINT { x: cx,        y: cy - s(6) },
        POINT { x: cx - s(4), y: cy - s(6) },
    ];
    Polyline(hdc, arch_pts.as_ptr(), arch_pts.len() as i32);

    // Prominent arrowhead pointing left
    let arrow_pts = [
        POINT { x: cx - s(2), y: cy - s(10) },
        POINT { x: cx - s(7), y: cy - s(6) },
        POINT { x: cx - s(2), y: cy - s(2) },
    ];
    Polyline(hdc, arrow_pts.as_ptr(), arrow_pts.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_redo(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(2).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    // Arch curving from bottom-left up and over to right
    let arch_pts = [
        POINT { x: cx - s(5), y: cy + s(4) },
        POINT { x: cx - s(5), y: cy },
        POINT { x: cx - s(3), y: cy - s(4) },
        POINT { x: cx,        y: cy - s(6) },
        POINT { x: cx + s(4), y: cy - s(6) },
    ];
    Polyline(hdc, arch_pts.as_ptr(), arch_pts.len() as i32);

    // Prominent arrowhead pointing right
    let arrow_pts = [
        POINT { x: cx + s(2), y: cy - s(10) },
        POINT { x: cx + s(7), y: cy - s(6) },
        POINT { x: cx + s(2), y: cy - s(2) },
    ];
    Polyline(hdc, arrow_pts.as_ptr(), arrow_pts.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_open(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(1).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let folder_outline = [
        POINT { x: cx - s(6), y: cy + s(5) },
        POINT { x: cx - s(6), y: cy - s(4) },
        POINT { x: cx - s(2), y: cy - s(4) },
        POINT { x: cx,        y: cy - s(2) },
        POINT { x: cx + s(6), y: cy - s(2) },
        POINT { x: cx + s(6), y: cy + s(5) },
        POINT { x: cx - s(6), y: cy + s(5) },
    ];
    Polyline(hdc, folder_outline.as_ptr(), folder_outline.len() as i32);

    let flap = [
        POINT { x: cx - s(6), y: cy },
        POINT { x: cx + s(6), y: cy },
    ];
    Polyline(hdc, flap.as_ptr(), flap.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_save(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(1).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let disk_outline = [
        POINT { x: cx - s(6), y: cy - s(6) },
        POINT { x: cx + s(3), y: cy - s(6) },
        POINT { x: cx + s(6), y: cy - s(3) },
        POINT { x: cx + s(6), y: cy + s(6) },
        POINT { x: cx - s(6), y: cy + s(6) },
        POINT { x: cx - s(6), y: cy - s(6) },
    ];
    Polyline(hdc, disk_outline.as_ptr(), disk_outline.len() as i32);

    let shutter = [
        POINT { x: cx - s(3), y: cy - s(6) },
        POINT { x: cx - s(3), y: cy - s(2) },
        POINT { x: cx + s(2), y: cy - s(2) },
        POINT { x: cx + s(2), y: cy - s(6) },
    ];
    Polyline(hdc, shutter.as_ptr(), shutter.len() as i32);

    let label = [
        POINT { x: cx - s(4), y: cy + s(2) },
        POINT { x: cx + s(4), y: cy + s(2) },
        POINT { x: cx + s(4), y: cy + s(6) },
        POINT { x: cx - s(4), y: cy + s(6) },
        POINT { x: cx - s(4), y: cy + s(2) },
    ];
    Polyline(hdc, label.as_ptr(), label.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_hand(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(1).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let hand_pts = [
        POINT { x: cx - s(3), y: cy + s(6) },
        POINT { x: cx + s(4), y: cy + s(6) },
        POINT { x: cx + s(5), y: cy + s(2) },
        POINT { x: cx + s(6), y: cy - s(2) },
        POINT { x: cx + s(4), y: cy - s(2) },
        POINT { x: cx + s(4), y: cy - s(5) },
        POINT { x: cx + s(2), y: cy - s(5) },
        POINT { x: cx + s(2), y: cy - s(7) },
        POINT { x: cx,        y: cy - s(7) },
        POINT { x: cx,        y: cy - s(5) },
        POINT { x: cx - s(2), y: cy - s(5) },
        POINT { x: cx - s(3), y: cy - s(1) },
        POINT { x: cx - s(6), y: cy + s(1) },
        POINT { x: cx - s(6), y: cy + s(3) },
        POINT { x: cx - s(3), y: cy + s(6) },
    ];
    Polyline(hdc, hand_pts.as_ptr(), hand_pts.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_pen(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(1).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let pen_outline = [
        POINT { x: cx - s(5), y: cy + s(5) },
        POINT { x: cx - s(5), y: cy + s(2) },
        POINT { x: cx + s(2), y: cy - s(5) },
        POINT { x: cx + s(5), y: cy - s(2) },
        POINT { x: cx - s(2), y: cy + s(5) },
        POINT { x: cx - s(5), y: cy + s(5) },
    ];
    Polyline(hdc, pen_outline.as_ptr(), pen_outline.len() as i32);

    let cap_line = [
        POINT { x: cx + s(1), y: cy - s(4) },
        POINT { x: cx + s(4), y: cy - s(1) },
    ];
    Polyline(hdc, cap_line.as_ptr(), cap_line.len() as i32);

    let nib_split = [
        POINT { x: cx - s(5), y: cy + s(5) },
        POINT { x: cx - s(2), y: cy + s(2) },
    ];
    Polyline(hdc, nib_split.as_ptr(), nib_split.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_search(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(2).max(1);
    let pen = CreatePen(0, pen_w, color);
    let null_brush = GetStockObject(NULL_BRUSH);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);
    let old_brush = SelectObject(hdc, null_brush);

    Ellipse(hdc, cx - s(6), cy - s(6), cx + s(3), cy + s(3));

    let handle_pts = [
        POINT { x: cx + s(1), y: cy + s(1) },
        POINT { x: cx + s(6), y: cy + s(6) },
    ];
    Polyline(hdc, handle_pts.as_ptr(), handle_pts.len() as i32);

    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_minus(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(2).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let pts = [
        POINT { x: cx - s(4), y: cy },
        POINT { x: cx + s(5), y: cy },
    ];
    Polyline(hdc, pts.as_ptr(), pts.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_plus(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(2).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let h_pts = [
        POINT { x: cx - s(4), y: cy },
        POINT { x: cx + s(5), y: cy },
    ];
    Polyline(hdc, h_pts.as_ptr(), h_pts.len() as i32);

    let v_pts = [
        POINT { x: cx,        y: cy - s(4) },
        POINT { x: cx,        y: cy + s(5) },
    ];
    Polyline(hdc, v_pts.as_ptr(), v_pts.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_chevron(hdc: HDC, cx: i32, cy: i32, is_up: bool, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(2).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let pts = if is_up {
        [
            POINT { x: cx - s(4), y: cy + s(2) },
            POINT { x: cx,        y: cy - s(2) },
            POINT { x: cx + s(4), y: cy + s(2) },
        ]
    } else {
        [
            POINT { x: cx - s(4), y: cy - s(2) },
            POINT { x: cx,        y: cy + s(2) },
            POINT { x: cx + s(4), y: cy - s(2) },
        ]
    };
    Polyline(hdc, pts.as_ptr(), pts.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_close(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(2).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let d1 = [
        POINT { x: cx - s(3), y: cy - s(3) },
        POINT { x: cx + s(4), y: cy + s(4) },
    ];
    Polyline(hdc, d1.as_ptr(), d1.len() as i32);

    let d2 = [
        POINT { x: cx + s(3), y: cy - s(3) },
        POINT { x: cx - s(4), y: cy + s(4) },
    ];
    Polyline(hdc, d2.as_ptr(), d2.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_status_dot(hdc: HDC, cx: i32, cy: i32, dpi: u32, is_dirty: bool, is_toast: bool) {
    let s = |v: i32| scale_dpi(v, dpi);
    let r = s(3).max(2);
    if is_toast || !is_dirty {
        let brush = CreateSolidBrush(rgb(0x10, 0xB9, 0x81));
        let pen = CreatePen(0, 1, rgb(0x10, 0xB9, 0x81));
        let old_b = SelectObject(hdc, brush as HGDIOBJ);
        let old_p = SelectObject(hdc, pen as HGDIOBJ);
        Ellipse(hdc, cx - r, cy - r, cx + r + 1, cy + r + 1);
        SelectObject(hdc, old_b);
        SelectObject(hdc, old_p);
        DeleteObject(brush as HGDIOBJ);
        DeleteObject(pen as HGDIOBJ);
    } else {
        let pen = CreatePen(0, 1, rgb(0xF5, 0x9E, 0x0B));
        let null_brush = GetStockObject(NULL_BRUSH);
        let old_p = SelectObject(hdc, pen as HGDIOBJ);
        let old_b = SelectObject(hdc, null_brush);
        Ellipse(hdc, cx - r, cy - r, cx + r + 1, cy + r + 1);
        SelectObject(hdc, old_p);
        SelectObject(hdc, old_b);
        DeleteObject(pen as HGDIOBJ);
    }
}

unsafe fn draw_vector_checkmark(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(2).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let pts = [
        POINT { x: cx - s(5), y: cy },
        POINT { x: cx - s(1), y: cy + s(4) },
        POINT { x: cx + s(5), y: cy - s(4) },
    ];
    Polyline(hdc, pts.as_ptr(), pts.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_cross(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(2).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let d1 = [
        POINT { x: cx - s(4), y: cy - s(4) },
        POINT { x: cx + s(4), y: cy + s(4) },
    ];
    Polyline(hdc, d1.as_ptr(), d1.len() as i32);

    let d2 = [
        POINT { x: cx + s(4), y: cy - s(4) },
        POINT { x: cx - s(4), y: cy + s(4) },
    ];
    Polyline(hdc, d2.as_ptr(), d2.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_image_icon(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(1).max(1);
    let pen = CreatePen(0, pen_w, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);

    let frame = [
        POINT { x: cx - s(6), y: cy - s(5) },
        POINT { x: cx + s(6), y: cy - s(5) },
        POINT { x: cx + s(6), y: cy + s(5) },
        POINT { x: cx - s(6), y: cy + s(5) },
        POINT { x: cx - s(6), y: cy - s(5) },
    ];
    Polyline(hdc, frame.as_ptr(), frame.len() as i32);

    let m1 = [
        POINT { x: cx - s(5), y: cy + s(3) },
        POINT { x: cx - s(2), y: cy - s(1) },
        POINT { x: cx + s(2), y: cy + s(3) },
    ];
    Polyline(hdc, m1.as_ptr(), m1.len() as i32);

    let m2 = [
        POINT { x: cx,        y: cy + s(3) },
        POINT { x: cx + s(3), y: cy },
        POINT { x: cx + s(5), y: cy + s(3) },
    ];
    Polyline(hdc, m2.as_ptr(), m2.len() as i32);

    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_vector_redact_icon(hdc: HDC, cx: i32, cy: i32, dpi: u32, color: COLORREF) {
    let s = |v: i32| scale_dpi(v, dpi);
    let pen_w = s(1).max(1);
    let pen = CreatePen(0, pen_w, color);
    let brush = CreateSolidBrush(color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);
    let old_b = SelectObject(hdc, brush as HGDIOBJ);

    RoundRect(hdc, cx - s(6), cy - s(3), cx + s(6), cy + s(3), s(2), s(2));

    SelectObject(hdc, old_b);
    SelectObject(hdc, old_pen);
    DeleteObject(brush as HGDIOBJ);
    DeleteObject(pen as HGDIOBJ);
}

fn paint_default_app_banner(state: &mut AppState, hdc: HDC, width: i32) {
    let dpi = state.dpi;
    let s = |v: i32| scale_dpi(v, dpi);

    let card_w = s(540).min(width - s(32));
    let card_h = s(38);
    let card_x = (width - card_w) / 2;
    let card_y = state.toolbar_height + s(8);

    state.banner_rect = RECT {
        left: card_x,
        top: card_y,
        right: card_x + card_w,
        bottom: card_y + card_h,
    };

    unsafe {
        // Drop shadow behind floating banner
        let shadow = RECT {
            left: card_x + s(2),
            top: card_y + s(2),
            right: card_x + card_w + s(2),
            bottom: card_y + card_h + s(2),
        };
        let shadow_brush = CreateSolidBrush(rgb(0x06, 0x06, 0x0A));
        let shadow_pen = CreatePen(0, 1, rgb(0x06, 0x06, 0x0A));
        let old_b = SelectObject(hdc, shadow_brush as HGDIOBJ);
        let old_p = SelectObject(hdc, shadow_pen as HGDIOBJ);
        RoundRect(hdc, shadow.left, shadow.top, shadow.right, shadow.bottom, s(8), s(8));
        SelectObject(hdc, old_b);
        SelectObject(hdc, old_p);
        DeleteObject(shadow_brush as HGDIOBJ);
        DeleteObject(shadow_pen as HGDIOBJ);

        // Container card (#1B1B26 with #333348 border)
        let bg_brush = CreateSolidBrush(rgb(0x1B, 0x1B, 0x26));
        let border_pen = CreatePen(0, 1, rgb(0x33, 0x33, 0x48));
        let old_b = SelectObject(hdc, bg_brush as HGDIOBJ);
        let old_p = SelectObject(hdc, border_pen as HGDIOBJ);
        RoundRect(hdc, card_x, card_y, card_x + card_w, card_y + card_h, s(8), s(8));
        SelectObject(hdc, old_b);
        SelectObject(hdc, old_p);
        DeleteObject(bg_brush as HGDIOBJ);
        DeleteObject(border_pen as HGDIOBJ);

        SetBkMode(hdc, TRANSPARENT);
        let font = create_ui_font(9.5, 500, dpi);
        let old_f = SelectObject(hdc, font as HGDIOBJ);

        // Text prompt
        SetTextColor(hdc, rgb(0xED, 0xED, 0xF0));
        let prompt_text = to_wide_null("Set zPDF as your default PDF viewer?");
        let text_cy = card_y + (card_h - s(14)) / 2;
        TextOutW(hdc, card_x + s(14), text_cy, prompt_text.as_ptr(), (prompt_text.len() - 1) as i32);

        // Buttons on right: [Set as Default] [Never] [✕]
        let close_w = s(24);
        let never_w = s(56);
        let set_w = s(104);
        let btn_h = s(26);
        let btn_y = card_y + (card_h - btn_h) / 2;

        state.banner_close_btn_rect = RECT {
            left: card_x + card_w - s(8) - close_w,
            top: btn_y,
            right: card_x + card_w - s(8),
            bottom: btn_y + btn_h,
        };
        state.banner_never_btn_rect = RECT {
            left: state.banner_close_btn_rect.left - s(6) - never_w,
            top: btn_y,
            right: state.banner_close_btn_rect.left - s(6),
            bottom: btn_y + btn_h,
        };
        state.banner_set_btn_rect = RECT {
            left: state.banner_never_btn_rect.left - s(6) - set_w,
            top: btn_y,
            right: state.banner_never_btn_rect.left - s(6),
            bottom: btn_y + btn_h,
        };

        // 1. "Set as Default" (Accent Blue #2563EB)
        let set_brush = CreateSolidBrush(rgb(0x25, 0x63, 0xEB));
        let set_pen = CreatePen(0, 1, rgb(0x3B, 0x82, 0xF6));
        let old_b2 = SelectObject(hdc, set_brush as HGDIOBJ);
        let old_p2 = SelectObject(hdc, set_pen as HGDIOBJ);
        RoundRect(hdc, state.banner_set_btn_rect.left, state.banner_set_btn_rect.top, state.banner_set_btn_rect.right, state.banner_set_btn_rect.bottom, s(6), s(6));
        SelectObject(hdc, old_b2);
        SelectObject(hdc, old_p2);
        DeleteObject(set_brush as HGDIOBJ);
        DeleteObject(set_pen as HGDIOBJ);
        SetTextColor(hdc, rgb(0xFF, 0xFF, 0xFF));
        let set_text = to_wide_null("Set as Default");
        TextOutW(hdc, state.banner_set_btn_rect.left + s(10), text_cy, set_text.as_ptr(), (set_text.len() - 1) as i32);

        // 2. "Never" button
        let never_brush = CreateSolidBrush(rgb(0x24, 0x24, 0x32));
        let never_pen = CreatePen(0, 1, rgb(0x33, 0x33, 0x48));
        let old_b3 = SelectObject(hdc, never_brush as HGDIOBJ);
        let old_p3 = SelectObject(hdc, never_pen as HGDIOBJ);
        RoundRect(hdc, state.banner_never_btn_rect.left, state.banner_never_btn_rect.top, state.banner_never_btn_rect.right, state.banner_never_btn_rect.bottom, s(6), s(6));
        SelectObject(hdc, old_b3);
        SelectObject(hdc, old_p3);
        DeleteObject(never_brush as HGDIOBJ);
        DeleteObject(never_pen as HGDIOBJ);
        SetTextColor(hdc, rgb(0xC0, 0xC0, 0xD0));
        let never_text = to_wide_null("Never");
        TextOutW(hdc, state.banner_never_btn_rect.left + s(12), text_cy, never_text.as_ptr(), (never_text.len() - 1) as i32);

        // 3. Close "✕" icon
        draw_vector_close(hdc, (state.banner_close_btn_rect.left + state.banner_close_btn_rect.right) / 2, card_y + card_h / 2, dpi, rgb(0x90, 0x90, 0xA0));

        SelectObject(hdc, old_f);
        DeleteObject(font as HGDIOBJ);
    }
}

fn paint_toolbar(state: &mut AppState, hdc: HDC, width: i32) {
    let dpi = state.dpi;
    let s = |v: i32| scale_dpi(v, dpi);
    let tb_height = state.toolbar_height;
    let cy = tb_height / 2;
    let btn_h = s(32);
    let btn_y = (tb_height - btn_h) / 2;

    // 1. Background: Deep Obsidian #121216
    let tb_rect = RECT { left: 0, top: 0, right: width, bottom: tb_height };
    let tb_brush = unsafe { CreateSolidBrush(rgb(0x12, 0x12, 0x16)) };
    unsafe {
        FillRect(hdc, &tb_rect, tb_brush);
        DeleteObject(tb_brush as HGDIOBJ);
    }

    // 2. Hairline bottom border: #23232C
    let hairline_rect = RECT { left: 0, top: tb_height - 1, right: width, bottom: tb_height };
    let hairline_brush = unsafe { CreateSolidBrush(rgb(0x23, 0x23, 0x2C)) };
    unsafe {
        FillRect(hdc, &hairline_rect, hairline_brush);
        DeleteObject(hairline_brush as HGDIOBJ);
    }

    unsafe {
        SetBkMode(hdc, TRANSPARENT);

        // ==========================================
        // LEFT ZONE: Brand & Action Buttons
        // ==========================================
        // Brand Icon
        let icon_sz = s(20);
        let icon_y = (tb_height - icon_sz) / 2;
        if !state.app_icon.is_null() {
            DrawIconEx(hdc, s(14), icon_y, state.app_icon, icon_sz, icon_sz, 0, ptr::null_mut(), 0x0003);
        }

        // Brand Name "zPDF"
        let brand_font = create_ui_font(12.0, 700, dpi);
        let old_f = SelectObject(hdc, brand_font as HGDIOBJ);
        SetTextColor(hdc, rgb(0xED, 0xED, 0xF0));
        let brand_text = to_wide_null("zPDF");
        TextOutW(hdc, s(40), cy - s(8), brand_text.as_ptr(), (brand_text.len() - 1) as i32);
        SelectObject(hdc, old_f);
        DeleteObject(brand_font as HGDIOBJ);

        // Brand hairline separator
        let sep_brush = CreateSolidBrush(rgb(0x28, 0x28, 0x34));
        let brand_sep = RECT { left: s(84), top: s(12), right: s(85), bottom: tb_height - s(12) };
        FillRect(hdc, &brand_sep, sep_brush);
        DeleteObject(sep_brush as HGDIOBJ);

        // Left Action Buttons: Open, Save, Undo, Redo
        let open_w = s(68);
        let save_w = s(66);
        let icon_btn_w = s(34);
        let open_x = s(94);
        state.tb_open_rect = RECT { left: open_x, top: btn_y, right: open_x + open_w, bottom: btn_y + btn_h };
        state.tb_save_rect = RECT { left: state.tb_open_rect.right + s(4), top: btn_y, right: state.tb_open_rect.right + s(4) + save_w, bottom: btn_y + btn_h };
        state.tb_undo_rect = RECT { left: state.tb_save_rect.right + s(6), top: btn_y, right: state.tb_save_rect.right + s(6) + icon_btn_w, bottom: btn_y + btn_h };
        state.tb_redo_rect = RECT { left: state.tb_undo_rect.right + s(4), top: btn_y, right: state.tb_undo_rect.right + s(4) + icon_btn_w, bottom: btn_y + btn_h };

        let btn_font = create_ui_font(10.0, 500, dpi);
        let old_bf = SelectObject(hdc, btn_font as HGDIOBJ);

        // 1. Open Button (Vector Folder + Text)
        let is_open_hover = state.hovered_control == Some(ToolbarControl::Open);
        let open_color = if is_open_hover {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_open_rect.left, state.tb_open_rect.top, state.tb_open_rect.right, state.tb_open_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0xC0, 0xC0, 0xC8)
        };
        draw_vector_open(hdc, state.tb_open_rect.left + s(14), cy, dpi, open_color);
        SetTextColor(hdc, open_color);
        let open_w_str = to_wide_null("Open");
        TextOutW(hdc, state.tb_open_rect.left + s(28), cy - s(7), open_w_str.as_ptr(), (open_w_str.len() - 1) as i32);

        // 2. Save Button (Vector Floppy + Text)
        let is_save_hover = state.hovered_control == Some(ToolbarControl::Save);
        let save_color = if is_save_hover {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_save_rect.left, state.tb_save_rect.top, state.tb_save_rect.right, state.tb_save_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0xC0, 0xC0, 0xC8)
        };
        draw_vector_save(hdc, state.tb_save_rect.left + s(14), cy, dpi, save_color);
        SetTextColor(hdc, save_color);
        let save_w_str = to_wide_null("Save");
        TextOutW(hdc, state.tb_save_rect.left + s(28), cy - s(7), save_w_str.as_ptr(), (save_w_str.len() - 1) as i32);

        // 3. Undo Button (Crisp Centered Vector Curved Arrow)
        let is_undo_hover = state.hovered_control == Some(ToolbarControl::Undo);
        let undo_color = if is_undo_hover {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_undo_rect.left, state.tb_undo_rect.top, state.tb_undo_rect.right, state.tb_undo_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0xC0, 0xC0, 0xC8)
        };
        draw_vector_undo(hdc, (state.tb_undo_rect.left + state.tb_undo_rect.right) / 2, cy, dpi, undo_color);

        // 4. Redo Button (Crisp Centered Vector Curved Arrow)
        let is_redo_hover = state.hovered_control == Some(ToolbarControl::Redo);
        let redo_color = if is_redo_hover {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_redo_rect.left, state.tb_redo_rect.top, state.tb_redo_rect.right, state.tb_redo_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0xC0, 0xC0, 0xC8)
        };
        draw_vector_redo(hdc, (state.tb_redo_rect.left + state.tb_redo_rect.right) / 2, cy, dpi, redo_color);

        SelectObject(hdc, old_bf);
        DeleteObject(btn_font as HGDIOBJ);

        // ==========================================
        // RIGHT ZONE: Segmented Tools, Zoom & Search
        // ==========================================
        // Search Button: [ 🔍 ]
        let search_btn_w = s(34);
        state.tb_search_rect = RECT { left: width - s(14) - search_btn_w, top: btn_y, right: width - s(14), bottom: btn_y + btn_h };

        // Zoom Cluster: [ − | 100% | + ]
        let zoom_btn_w = s(32);
        let zoom_reset_w = s(64);
        let zoom_w = zoom_btn_w * 2 + zoom_reset_w;
        let zoom_right = state.tb_search_rect.left - s(10);
        let zoom_left = zoom_right - zoom_w;
        state.tb_zoom_out_rect = RECT { left: zoom_left, top: btn_y, right: zoom_left + zoom_btn_w, bottom: btn_y + btn_h };
        state.tb_zoom_reset_rect = RECT { left: zoom_left + zoom_btn_w, top: btn_y, right: zoom_right - zoom_btn_w, bottom: btn_y + btn_h };
        state.tb_zoom_in_rect = RECT { left: zoom_right - zoom_btn_w, top: btn_y, right: zoom_right, bottom: btn_y + btn_h };

        // Segmented Tool Pill: [ Hand | Edit ]
        let seg_w = s(76);
        let tool_pill_w = seg_w * 2;
        let tool_pill_left = zoom_left - s(14) - tool_pill_w;
        state.tb_hand_rect = RECT { left: tool_pill_left, top: btn_y, right: tool_pill_left + seg_w, bottom: btn_y + btn_h };
        state.tb_edit_rect = RECT { left: tool_pill_left + seg_w, top: btn_y, right: tool_pill_left + tool_pill_w, bottom: btn_y + btn_h };

        // Action Tools: [ Checkmark | Cross | Image | Redact ]
        let act_btn_w = s(34);
        let act_spacing = s(4);
        let act_total_w = act_btn_w * 4 + act_spacing * 3;
        let act_left = tool_pill_left - s(14) - act_total_w;
        state.tb_stamp_v_rect = RECT { left: act_left, top: btn_y, right: act_left + act_btn_w, bottom: btn_y + btn_h };
        state.tb_stamp_x_rect = RECT { left: state.tb_stamp_v_rect.right + act_spacing, top: btn_y, right: state.tb_stamp_v_rect.right + act_spacing + act_btn_w, bottom: btn_y + btn_h };
        state.tb_image_rect = RECT { left: state.tb_stamp_x_rect.right + act_spacing, top: btn_y, right: state.tb_stamp_x_rect.right + act_spacing + act_btn_w, bottom: btn_y + btn_h };
        state.tb_redact_rect = RECT { left: state.tb_image_rect.right + act_spacing, top: btn_y, right: state.tb_image_rect.right + act_spacing + act_btn_w, bottom: btn_y + btn_h };

        // Render Action Buttons
        // 1. Checkmark Stamp
        let is_stamp_v = state.tool_mode == ToolMode::StampCheckmark;
        let v_color = if is_stamp_v {
            let ab = CreateSolidBrush(rgb(0x1B, 0x38, 0x2B));
            let ap = CreatePen(0, 1, rgb(0x10, 0xB9, 0x81));
            let ob = SelectObject(hdc, ab as HGDIOBJ);
            let op = SelectObject(hdc, ap as HGDIOBJ);
            RoundRect(hdc, state.tb_stamp_v_rect.left, state.tb_stamp_v_rect.top, state.tb_stamp_v_rect.right, state.tb_stamp_v_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            SelectObject(hdc, op);
            DeleteObject(ab as HGDIOBJ);
            DeleteObject(ap as HGDIOBJ);
            rgb(0x10, 0xB9, 0x81)
        } else if state.hovered_control == Some(ToolbarControl::ToolStampCheckmark) {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_stamp_v_rect.left, state.tb_stamp_v_rect.top, state.tb_stamp_v_rect.right, state.tb_stamp_v_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0x10, 0xB9, 0x81)
        } else {
            rgb(0x10, 0xB9, 0x81)
        };
        draw_vector_checkmark(hdc, (state.tb_stamp_v_rect.left + state.tb_stamp_v_rect.right) / 2, cy, dpi, v_color);

        // 2. Cross Stamp
        let is_stamp_x = state.tool_mode == ToolMode::StampCross;
        let x_color = if is_stamp_x {
            let ab = CreateSolidBrush(rgb(0x3A, 0x1E, 0x24));
            let ap = CreatePen(0, 1, rgb(0xEF, 0x44, 0x44));
            let ob = SelectObject(hdc, ab as HGDIOBJ);
            let op = SelectObject(hdc, ap as HGDIOBJ);
            RoundRect(hdc, state.tb_stamp_x_rect.left, state.tb_stamp_x_rect.top, state.tb_stamp_x_rect.right, state.tb_stamp_x_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            SelectObject(hdc, op);
            DeleteObject(ab as HGDIOBJ);
            DeleteObject(ap as HGDIOBJ);
            rgb(0xEF, 0x44, 0x44)
        } else if state.hovered_control == Some(ToolbarControl::ToolStampCross) {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_stamp_x_rect.left, state.tb_stamp_x_rect.top, state.tb_stamp_x_rect.right, state.tb_stamp_x_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xEF, 0x44, 0x44)
        } else {
            rgb(0xEF, 0x44, 0x44)
        };
        draw_vector_cross(hdc, (state.tb_stamp_x_rect.left + state.tb_stamp_x_rect.right) / 2, cy, dpi, x_color);

        // 3. Image Button
        let img_color = if state.hovered_control == Some(ToolbarControl::ToolImage) {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_image_rect.left, state.tb_image_rect.top, state.tb_image_rect.right, state.tb_image_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0xC0, 0xC0, 0xC8)
        };
        draw_vector_image_icon(hdc, (state.tb_image_rect.left + state.tb_image_rect.right) / 2, cy, dpi, img_color);

        // 4. Redact Button
        let is_redact = state.tool_mode == ToolMode::RedactBlackline || state.tool_mode == ToolMode::RedactClear;
        let redact_color = if is_redact {
            let ab = CreateSolidBrush(rgb(0x3A, 0x1E, 0x24));
            let ap = CreatePen(0, 1, rgb(0xEF, 0x44, 0x44));
            let ob = SelectObject(hdc, ab as HGDIOBJ);
            let op = SelectObject(hdc, ap as HGDIOBJ);
            RoundRect(hdc, state.tb_redact_rect.left, state.tb_redact_rect.top, state.tb_redact_rect.right, state.tb_redact_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            SelectObject(hdc, op);
            DeleteObject(ab as HGDIOBJ);
            DeleteObject(ap as HGDIOBJ);
            rgb(0xEF, 0x44, 0x44)
        } else if state.hovered_control == Some(ToolbarControl::ToolRedactBlackline) {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_redact_rect.left, state.tb_redact_rect.top, state.tb_redact_rect.right, state.tb_redact_rect.bottom, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0xC0, 0xC0, 0xC8)
        };
        draw_vector_redact_icon(hdc, (state.tb_redact_rect.left + state.tb_redact_rect.right) / 2, cy, dpi, redact_color);

        // 1. Draw Tool Pill Container
        let tool_bg = CreateSolidBrush(rgb(0x19, 0x19, 0x22));
        let tool_pen = CreatePen(0, 1, rgb(0x28, 0x28, 0x34));
        let old_b = SelectObject(hdc, tool_bg as HGDIOBJ);
        let old_p = SelectObject(hdc, tool_pen as HGDIOBJ);
        RoundRect(hdc, tool_pill_left, btn_y, tool_pill_left + tool_pill_w, btn_y + btn_h, s(8), s(8));
        SelectObject(hdc, old_b);
        SelectObject(hdc, old_p);
        DeleteObject(tool_bg as HGDIOBJ);
        DeleteObject(tool_pen as HGDIOBJ);

        let tools_font = create_ui_font(10.0, 500, dpi);
        let old_tf = SelectObject(hdc, tools_font as HGDIOBJ);

        let is_pan = state.tool_mode == ToolMode::Pan;
        let is_edit = state.tool_mode == ToolMode::EditTextObject;

        // Render Hand Tool Segment
        let hand_color = if is_pan {
            let active_brush = CreateSolidBrush(rgb(0x2C, 0x36, 0x4A));
            let active_pen = CreatePen(0, 1, rgb(0x3B, 0x82, 0xF6));
            let old_b = SelectObject(hdc, active_brush as HGDIOBJ);
            let old_p = SelectObject(hdc, active_pen as HGDIOBJ);
            RoundRect(hdc, state.tb_hand_rect.left + 1, state.tb_hand_rect.top + 1, state.tb_hand_rect.right - 1, state.tb_hand_rect.bottom - 1, s(6), s(6));
            SelectObject(hdc, old_b);
            SelectObject(hdc, old_p);
            DeleteObject(active_brush as HGDIOBJ);
            DeleteObject(active_pen as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else if state.hovered_control == Some(ToolbarControl::ToolPan) {
            let hover_brush = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let old_b = SelectObject(hdc, hover_brush as HGDIOBJ);
            RoundRect(hdc, state.tb_hand_rect.left + 1, state.tb_hand_rect.top + 1, state.tb_hand_rect.right - 1, state.tb_hand_rect.bottom - 1, s(6), s(6));
            SelectObject(hdc, old_b);
            DeleteObject(hover_brush as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0x90, 0x90, 0x9A)
        };
        draw_vector_hand(hdc, tool_pill_left + s(20), cy, dpi, hand_color);
        SetTextColor(hdc, hand_color);
        let w_hand = to_wide_null("Hand");
        TextOutW(hdc, tool_pill_left + s(34), cy - s(7), w_hand.as_ptr(), (w_hand.len() - 1) as i32);

        // Render Edit Tool Segment
        let edit_color = if is_edit {
            let active_brush = CreateSolidBrush(rgb(0x2C, 0x36, 0x4A));
            let active_pen = CreatePen(0, 1, rgb(0x3B, 0x82, 0xF6));
            let old_b = SelectObject(hdc, active_brush as HGDIOBJ);
            let old_p = SelectObject(hdc, active_pen as HGDIOBJ);
            RoundRect(hdc, state.tb_edit_rect.left + 1, state.tb_edit_rect.top + 1, state.tb_edit_rect.right - 1, state.tb_edit_rect.bottom - 1, s(6), s(6));
            SelectObject(hdc, old_b);
            SelectObject(hdc, old_p);
            DeleteObject(active_brush as HGDIOBJ);
            DeleteObject(active_pen as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else if state.hovered_control == Some(ToolbarControl::ToolEdit) {
            let hover_brush = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let old_b = SelectObject(hdc, hover_brush as HGDIOBJ);
            RoundRect(hdc, state.tb_edit_rect.left + 1, state.tb_edit_rect.top + 1, state.tb_edit_rect.right - 1, state.tb_edit_rect.bottom - 1, s(6), s(6));
            SelectObject(hdc, old_b);
            DeleteObject(hover_brush as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0x90, 0x90, 0x9A)
        };
        draw_vector_pen(hdc, tool_pill_left + seg_w + s(20), cy, dpi, edit_color);
        SetTextColor(hdc, edit_color);
        let w_edit = to_wide_null("Edit");
        TextOutW(hdc, tool_pill_left + seg_w + s(34), cy - s(7), w_edit.as_ptr(), (w_edit.len() - 1) as i32);

        // 2. Draw Zoom Segmented Cluster Container
        let z_bg = CreateSolidBrush(rgb(0x19, 0x19, 0x22));
        let z_pen = CreatePen(0, 1, rgb(0x28, 0x28, 0x34));
        let old_zb = SelectObject(hdc, z_bg as HGDIOBJ);
        let old_zp = SelectObject(hdc, z_pen as HGDIOBJ);
        RoundRect(hdc, zoom_left, btn_y, zoom_right, btn_y + btn_h, s(8), s(8));
        SelectObject(hdc, old_zb);
        SelectObject(hdc, old_zp);
        DeleteObject(z_bg as HGDIOBJ);
        DeleteObject(z_pen as HGDIOBJ);

        // Inner vertical hairline dividers
        let div_brush = CreateSolidBrush(rgb(0x28, 0x28, 0x34));
        let div1 = RECT { left: state.tb_zoom_reset_rect.left, top: btn_y + s(5), right: state.tb_zoom_reset_rect.left + 1, bottom: btn_y + btn_h - s(5) };
        let div2 = RECT { left: state.tb_zoom_reset_rect.right - 1, top: btn_y + s(5), right: state.tb_zoom_reset_rect.right, bottom: btn_y + btn_h - s(5) };
        FillRect(hdc, &div1, div_brush);
        FillRect(hdc, &div2, div_brush);
        DeleteObject(div_brush as HGDIOBJ);

        // Zoom Out Button
        let is_zo_hover = state.hovered_control == Some(ToolbarControl::ZoomOut);
        let zo_color = if is_zo_hover {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_zoom_out_rect.left + 1, state.tb_zoom_out_rect.top + 1, state.tb_zoom_out_rect.right - 1, state.tb_zoom_out_rect.bottom - 1, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0xC0, 0xC0, 0xC8)
        };
        draw_vector_minus(hdc, (state.tb_zoom_out_rect.left + state.tb_zoom_out_rect.right) / 2, cy, dpi, zo_color);

        // Zoom Reset / Percentage Display
        let is_zr_hover = state.hovered_control == Some(ToolbarControl::ZoomReset);
        let zr_color = if is_zr_hover {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_zoom_reset_rect.left + 1, state.tb_zoom_reset_rect.top + 1, state.tb_zoom_reset_rect.right - 1, state.tb_zoom_reset_rect.bottom - 1, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0xC0, 0xC0, 0xC8)
        };
        SetTextColor(hdc, zr_color);
        let pct_str = format!("{}%", state.zoom_pct);
        let w_pct = to_wide_null(&pct_str);
        let pct_x = state.tb_zoom_reset_rect.left + (zoom_reset_w - (pct_str.len() as i32 * s(7))) / 2;
        TextOutW(hdc, pct_x, cy - s(7), w_pct.as_ptr(), (w_pct.len() - 1) as i32);

        // Zoom In Button
        let is_zi_hover = state.hovered_control == Some(ToolbarControl::ZoomIn);
        let zi_color = if is_zi_hover {
            let hb = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.tb_zoom_in_rect.left + 1, state.tb_zoom_in_rect.top + 1, state.tb_zoom_in_rect.right - 1, state.tb_zoom_in_rect.bottom - 1, s(6), s(6));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0xC0, 0xC0, 0xC8)
        };
        draw_vector_plus(hdc, (state.tb_zoom_in_rect.left + state.tb_zoom_in_rect.right) / 2, cy, dpi, zi_color);

        // 3. Search Button: [ 🔍 ]
        let is_search_hovered = state.hovered_control == Some(ToolbarControl::SearchToggle);
        let search_btn_color = if state.search_open {
            let active_brush = CreateSolidBrush(rgb(0x2C, 0x36, 0x4A));
            let active_pen = CreatePen(0, 1, rgb(0x3B, 0x82, 0xF6));
            let old_b = SelectObject(hdc, active_brush as HGDIOBJ);
            let old_p = SelectObject(hdc, active_pen as HGDIOBJ);
            RoundRect(hdc, state.tb_search_rect.left, state.tb_search_rect.top, state.tb_search_rect.right, state.tb_search_rect.bottom, s(6), s(6));
            SelectObject(hdc, old_b);
            SelectObject(hdc, old_p);
            DeleteObject(active_brush as HGDIOBJ);
            DeleteObject(active_pen as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else if is_search_hovered {
            let hover_brush = CreateSolidBrush(rgb(0x23, 0x23, 0x2F));
            let old_b = SelectObject(hdc, hover_brush as HGDIOBJ);
            RoundRect(hdc, state.tb_search_rect.left, state.tb_search_rect.top, state.tb_search_rect.right, state.tb_search_rect.bottom, s(6), s(6));
            SelectObject(hdc, old_b);
            DeleteObject(hover_brush as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0xC0, 0xC0, 0xC8)
        };
        draw_vector_search(hdc, (state.tb_search_rect.left + state.tb_search_rect.right) / 2, cy, dpi, search_btn_color);

        SelectObject(hdc, old_tf);
        DeleteObject(tools_font as HGDIOBJ);

        // ==========================================
        // CENTER ZONE: Hero Document Pill
        // ==========================================
        let pill_h = s(30);
        let pill_y = (tb_height - pill_h) / 2;
        let filename = state.current_file.as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("No Document");
        let total_pages = state.doc.as_ref().map_or(0, |d| d.page_count());
        let page_info = if total_pages > 0 {
            format!("• Page {} of {}", (state.active_page_idx + 1).min(total_pages), total_pages)
        } else {
            String::new()
        };

        let center_x = width / 2;
        let pill_w = (((filename.len() + page_info.len()) as i32 * s(7)) + s(120)).clamp(s(240), s(460));
        let mut pill_left = center_x - pill_w / 2;
        let mut pill_right = center_x + pill_w / 2;

        let min_left = state.tb_redo_rect.right + s(12);
        let max_right = tool_pill_left - s(12);
        if pill_left < min_left {
            pill_left = min_left;
            pill_right = (pill_left + pill_w).min(max_right);
        } else if pill_right > max_right {
            pill_right = max_right;
            pill_left = (pill_right - pill_w).max(min_left);
        }

        if pill_right > pill_left + s(100) {
            let pill_brush = CreateSolidBrush(rgb(0x19, 0x19, 0x22));
            let pill_pen = CreatePen(0, 1, rgb(0x28, 0x28, 0x34));
            let old_b = SelectObject(hdc, pill_brush as HGDIOBJ);
            let old_p = SelectObject(hdc, pill_pen as HGDIOBJ);
            RoundRect(hdc, pill_left, pill_y, pill_right, pill_y + pill_h, s(14), s(14));
            SelectObject(hdc, old_b);
            SelectObject(hdc, old_p);
            DeleteObject(pill_brush as HGDIOBJ);
            DeleteObject(pill_pen as HGDIOBJ);

            // Left vector status indicator dot
            let dot_x = pill_left + s(16);
            let dot_y = pill_y + pill_h / 2;
            draw_vector_status_dot(hdc, dot_x, dot_y, dpi, state.is_dirty, state.save_toast_active);

            let status_text = if state.save_toast_active {
                "Saved"
            } else if state.is_dirty {
                "Edited"
            } else {
                "Saved"
            };
            let status_color = if state.save_toast_active || !state.is_dirty {
                rgb(0x10, 0xB9, 0x81) // Emerald #10B981
            } else {
                rgb(0xF5, 0x9E, 0x0B) // Amber #F59E0B
            };

            let badge_font = create_ui_font(9.5, 600, dpi);
            let old_bf = SelectObject(hdc, badge_font as HGDIOBJ);
            SetTextColor(hdc, status_color);
            let w_badge = to_wide_null(status_text);
            TextOutW(hdc, pill_left + s(26), pill_y + s(6), w_badge.as_ptr(), (w_badge.len() - 1) as i32);
            SelectObject(hdc, old_bf);
            DeleteObject(badge_font as HGDIOBJ);

            // Document info
            let doc_font = create_ui_font(9.5, 400, dpi);
            let old_df = SelectObject(hdc, doc_font as HGDIOBJ);
            SetTextColor(hdc, rgb(0xED, 0xED, 0xF0));
            let full_doc_str = if page_info.is_empty() {
                filename.to_string()
            } else {
                format!("{} {}", filename, page_info)
            };
            let w_doc = to_wide_null(&full_doc_str);
            TextOutW(hdc, pill_left + s(72), pill_y + s(6), w_doc.as_ptr(), (w_doc.len() - 1) as i32);
            SelectObject(hdc, old_df);
            DeleteObject(doc_font as HGDIOBJ);
        }
    }
}

fn paint_search_bar(state: &mut AppState, hdc: HDC) {
    if !state.search_open {
        return;
    }
    state.update_search_controls_layout();
    let dpi = state.dpi;
    let s = |v: i32| scale_dpi(v, dpi);
    let card = state.search_card_rect;
    let card_cy = (card.top + card.bottom) / 2;

    unsafe {
        // Drop shadow behind floating card
        let shadow = RECT {
            left: card.left + s(3),
            top: card.top + s(3),
            right: card.right + s(3),
            bottom: card.bottom + s(3),
        };
        let shadow_brush = CreateSolidBrush(rgb(0x08, 0x08, 0x0B));
        let shadow_pen = CreatePen(0, 1, rgb(0x08, 0x08, 0x0B));
        let old_b = SelectObject(hdc, shadow_brush as HGDIOBJ);
        let old_p = SelectObject(hdc, shadow_pen as HGDIOBJ);
        RoundRect(hdc, shadow.left, shadow.top, shadow.right, shadow.bottom, s(10), s(10));
        SelectObject(hdc, old_b);
        SelectObject(hdc, old_p);
        DeleteObject(shadow_brush as HGDIOBJ);
        DeleteObject(shadow_pen as HGDIOBJ);

        // Container Card (#1A1A24 with #333344 border, radius 10)
        let card_brush = CreateSolidBrush(rgb(0x1A, 0x1A, 0x24));
        let card_pen = CreatePen(0, 1, rgb(0x33, 0x33, 0x44));
        let old_b = SelectObject(hdc, card_brush as HGDIOBJ);
        let old_p = SelectObject(hdc, card_pen as HGDIOBJ);
        RoundRect(hdc, card.left, card.top, card.right, card.bottom, s(10), s(10));
        SelectObject(hdc, old_b);
        SelectObject(hdc, old_p);
        DeleteObject(card_brush as HGDIOBJ);
        DeleteObject(card_pen as HGDIOBJ);

        SetBkMode(hdc, TRANSPARENT);

        // Crisp Vector Magnifying Glass Icon
        draw_vector_search(hdc, card.left + s(18), card_cy, dpi, rgb(0x90, 0x90, 0x9A));

        // Result Counter
        let counter_font = create_ui_font(9.5, 500, dpi);
        let old_cf = SelectObject(hdc, counter_font as HGDIOBJ);
        let counter_text = if state.search_query.trim().is_empty() {
            String::new()
        } else if state.search_matches.is_empty() {
            SetTextColor(hdc, rgb(0xEF, 0x44, 0x44));
            "No matches".to_string()
        } else {
            SetTextColor(hdc, rgb(0x90, 0x90, 0x9A));
            let curr = state.current_search_idx.map_or(0, |i| i + 1);
            format!("{} of {}", curr, state.search_matches.len())
        };
        if !counter_text.is_empty() {
            let wide_counter = to_wide_null(&counter_text);
            TextOutW(hdc, card.left + s(162), card_cy - s(7), wide_counter.as_ptr(), (wide_counter.len() - 1) as i32);
        }
        SelectObject(hdc, old_cf);
        DeleteObject(counter_font as HGDIOBJ);

        // Action Buttons: Prev (Chevron Up), Next (Chevron Down), Close (X)
        // 1. Prev
        let is_prev_hover = state.hovered_control == Some(ToolbarControl::SearchPrev);
        let prev_color = if is_prev_hover {
            let hb = CreateSolidBrush(rgb(0x28, 0x28, 0x38));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.search_prev_rect.left, state.search_prev_rect.top, state.search_prev_rect.right, state.search_prev_rect.bottom, s(4), s(4));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0x90, 0x90, 0x9A)
        };
        draw_vector_chevron(hdc, (state.search_prev_rect.left + state.search_prev_rect.right) / 2, card_cy, true, dpi, prev_color);

        // 2. Next
        let is_next_hover = state.hovered_control == Some(ToolbarControl::SearchNext);
        let next_color = if is_next_hover {
            let hb = CreateSolidBrush(rgb(0x28, 0x28, 0x38));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.search_next_rect.left, state.search_next_rect.top, state.search_next_rect.right, state.search_next_rect.bottom, s(4), s(4));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0x90, 0x90, 0x9A)
        };
        draw_vector_chevron(hdc, (state.search_next_rect.left + state.search_next_rect.right) / 2, card_cy, false, dpi, next_color);

        // 3. Close
        let is_close_hover = state.hovered_control == Some(ToolbarControl::SearchClose);
        let close_color = if is_close_hover {
            let hb = CreateSolidBrush(rgb(0x28, 0x28, 0x38));
            let ob = SelectObject(hdc, hb as HGDIOBJ);
            RoundRect(hdc, state.search_close_rect.left, state.search_close_rect.top, state.search_close_rect.right, state.search_close_rect.bottom, s(4), s(4));
            SelectObject(hdc, ob);
            DeleteObject(hb as HGDIOBJ);
            rgb(0xFF, 0xFF, 0xFF)
        } else {
            rgb(0x90, 0x90, 0x9A)
        };
        draw_vector_close(hdc, (state.search_close_rect.left + state.search_close_rect.right) / 2, card_cy, dpi, close_color);
    }
}

fn paint_status_bar(state: &AppState, hdc: HDC, width: i32, height: i32) {
    let dpi = state.dpi;
    let s = |v: i32| scale_dpi(v, dpi);
    let sb_top = height - state.status_height;
    let sb_rect = RECT { left: 0, top: sb_top, right: width, bottom: height };
    let text_y = sb_top + (state.status_height - s(14)) / 2;

    unsafe {
        // Status bar background (#111114)
        let bar_brush = CreateSolidBrush(rgb(0x11, 0x11, 0x14));
        FillRect(hdc, &sb_rect, bar_brush);
        DeleteObject(bar_brush as HGDIOBJ);

        // Top hairline separator
        let sep_brush = CreateSolidBrush(rgb(0x20, 0x20, 0x26));
        let sep_r = RECT { left: 0, top: sb_top, right: width, bottom: sb_top + 1 };
        FillRect(hdc, &sep_r, sep_brush);
        DeleteObject(sep_brush as HGDIOBJ);

        SetBkMode(hdc, TRANSPARENT);
        let font = create_ui_font(9.5, 400, dpi);
        let old_f = SelectObject(hdc, font as HGDIOBJ);
        SetTextColor(hdc, rgb(0x88, 0x88, 0x8E));

        // Left: Clean document name
        let left_msg = match &state.current_file {
            Some(p) => p.file_name().and_then(|n| n.to_str()).unwrap_or("Document.pdf").to_string(),
            None => "Ready".to_string(),
        };
        let w_left = to_wide_null(&left_msg);
        TextOutW(hdc, s(16), text_y, w_left.as_ptr(), (w_left.len() - 1) as i32);

        // Center: Page indicator
        if let Some(doc) = &state.doc {
            let total = doc.page_count();
            let current = (state.active_page_idx + 1).min(total);
            let center_msg = format!("Page {} of {}", current, total);
            let w_center = to_wide_null(&center_msg);
            let center_x = (width - s(90)) / 2;
            TextOutW(hdc, center_x, text_y, w_center.as_ptr(), (w_center.len() - 1) as i32);
        }

        // Right: Zoom and Tool
        let mode_str = match state.tool_mode {
            ToolMode::Pan => "Hand",
            ToolMode::EditTextObject => "Edit",
            _ => "View",
        };
        let right_msg = format!("{}%  •  {}", state.zoom_pct, mode_str);
        let w_right = to_wide_null(&right_msg);
        let right_x = (width - s(130)).max(width / 2 + s(100));
        TextOutW(hdc, right_x, text_y, w_right.as_ptr(), (w_right.len() - 1) as i32);

        SelectObject(hdc, old_f);
        DeleteObject(font as HGDIOBJ);
    }
}

fn paint_sidebar(state: &mut AppState, hdc: HDC, height: i32) {
    let dpi = state.dpi;
    let s = |v: i32| scale_dpi(v, dpi);
    let sb_rect = RECT {
        left: 0,
        top: state.toolbar_height,
        right: state.sidebar_width,
        bottom: height - state.status_height,
    };

    unsafe {
        // Sidebar background (#141418)
        let sb_brush = CreateSolidBrush(rgb(0x14, 0x14, 0x18));
        FillRect(hdc, &sb_rect, sb_brush);
        DeleteObject(sb_brush as HGDIOBJ);

        // Right hairline border (#23232B)
        let border_brush = CreateSolidBrush(rgb(0x23, 0x23, 0x2B));
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
        let header_font = create_ui_font(9.0, 600, dpi);
        let old_h_f = SelectObject(hdc, header_font as HGDIOBJ);
        SetTextColor(hdc, rgb(0x6E, 0x6E, 0x7C));
        let header_text = to_wide_null("PAGES");
        TextOutW(hdc, s(18), state.toolbar_height + s(8), header_text.as_ptr(), (header_text.len() - 1) as i32);
        SelectObject(hdc, old_h_f);
        DeleteObject(header_font as HGDIOBJ);

        // Page number font
        let font = create_ui_font(9.5, 500, dpi);
        let old_font = SelectObject(hdc, font as HGDIOBJ);

        let mut current_slot_y = s(28);
        let thumb_max_w = s(144);
        let thumb_max_h = s(160);

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
            if th > thumb_max_h {
                th = thumb_max_h;
            }
            if th < s(40) {
                th = s(40);
            }
            let slot_h = th + s(34);

            let visual_y = state.toolbar_height + current_slot_y - state.sidebar_scroll_y;

            if visual_y + slot_h >= state.toolbar_height && visual_y <= height - state.status_height {
                let tx = (state.sidebar_width - tw) / 2;
                let ty = visual_y + s(4);

                // 1. Drop shadow behind thumbnail card
                let shadow_rect = RECT {
                    left: tx + s(2),
                    top: ty + s(2),
                    right: tx + tw + s(2),
                    bottom: ty + th + s(2),
                };
                let shadow_brush = CreateSolidBrush(rgb(0x0A, 0x0A, 0x0E));
                FillRect(hdc, &shadow_rect, shadow_brush);
                DeleteObject(shadow_brush as HGDIOBJ);

                // 2. White paper card base (#FFFFFF)
                let card_rect = RECT { left: tx, top: ty, right: tx + tw, bottom: ty + th };
                let paper_brush = CreateSolidBrush(rgb(0xFF, 0xFF, 0xFF));
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
                            biHeight: -th,
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
                    let ring_brush = CreateSolidBrush(rgb(0x3B, 0x82, 0xF6));
                    FrameRect(hdc, &border_rect, ring_brush);
                    let inner_r = RECT { left: tx + 1, top: ty + 1, right: tx + tw - 1, bottom: ty + th - 1 };
                    FrameRect(hdc, &inner_r, ring_brush);
                    DeleteObject(ring_brush as HGDIOBJ);
                } else {
                    let normal_brush = CreateSolidBrush(rgb(0x2A, 0x2A, 0x34));
                    FrameRect(hdc, &border_rect, normal_brush);
                    DeleteObject(normal_brush as HGDIOBJ);
                }

                // 6. Page number badge
                SetTextColor(hdc, if is_active { rgb(0xFF, 0xFF, 0xFF) } else { rgb(0x82, 0x82, 0x8E) });
                let label = format!("{}", i + 1);
                let wide = to_wide_null(&label);
                let label_x = tx + (tw - s(8)) / 2;
                TextOutW(hdc, label_x, ty + th + s(6), wide.as_ptr(), (wide.len() - 1) as i32);
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

    let mut closest_dist = i32::MAX;
    let mut current_page = state.active_page_idx;

    for layout in &state.page_layouts {
        let page_y = canvas_origin_y + layout.y_offset;

        let page_x = if avail_w > layout.render_w {
            state.sidebar_width + (avail_w - layout.render_w) / 2
        } else {
            state.sidebar_width + 24 - state.scroll_x
        };

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

        // Ambient drop shadow (#08080C)
        let shadow_rect = RECT {
            left: page_x + 4,
            top: page_y + 4,
            right: page_x + page_w + 4,
            bottom: page_y + page_h + 4,
        };
        let shadow_brush = unsafe { CreateSolidBrush(rgb(0x08, 0x08, 0x0C)) };
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

                let form_handle = state.form_env.as_ref().map(|f| f.raw);
                if page.render_into_buffer_ex(&mut buffer, page_w, page_h, stride, rot, 0, form_handle).is_ok() {
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

                // 1px page outline (#282832)
                let outline_rect = RECT {
                    left: page_x,
                    top: page_y,
                    right: page_x + page_w,
                    bottom: page_y + page_h,
                };
                let outline_brush = CreateSolidBrush(rgb(0x28, 0x28, 0x32));
                FrameRect(hdc, &outline_rect, outline_brush);
                DeleteObject(outline_brush as HGDIOBJ);
            }
        }

        // Redaction active drag indicator
        if let Some((drag_page_idx, drag_r)) = &state.redact_drag_rect {
            if *drag_page_idx == layout.index {
                if let Ok(page) = doc.load_page(layout.index) {
                    let rot = page.rotation();
                    let pt1 = page.page_to_device(0, 0, layout.render_w, layout.render_h, rot, drag_r.left as f64, drag_r.top as f64);
                    let pt2 = page.page_to_device(0, 0, layout.render_w, layout.render_h, rot, drag_r.right as f64, drag_r.bottom as f64);
                    if let (Some((x1, y1)), Some((x2, y2))) = (pt1, pt2) {
                        let rx1 = page_x + x1.min(x2);
                        let rx2 = page_x + x1.max(x2);
                        let ry1 = page_y + y1.min(y2);
                        let ry2 = page_y + y1.max(y2);
                        unsafe {
                            let red_pen = CreatePen(0, 2, rgb(0xEF, 0x44, 0x44));
                            let null_brush = GetStockObject(NULL_BRUSH);
                            let old_p = SelectObject(hdc, red_pen as HGDIOBJ);
                            let old_b = SelectObject(hdc, null_brush);
                            Rectangle(hdc, rx1, ry1, rx2, ry2);
                            SelectObject(hdc, old_p);
                            SelectObject(hdc, old_b);
                            DeleteObject(red_pen as HGDIOBJ);
                        }
                    }
                }
            }
        }

        // Active Image Gizmo rendering
        if let Some(gizmo) = &state.active_image_gizmo {
            if gizmo.page_index == layout.index {
                if let Ok(page) = doc.load_page(layout.index) {
                    let rot = page.rotation();
                    let min_x_pt = gizmo.page_x;
                    let max_x_pt = gizmo.page_x + gizmo.width_pt;
                    let min_y_pt = gizmo.page_y;
                    let max_y_pt = gizmo.page_y + gizmo.height_pt;

                    let p_bl = page.page_to_device(0, 0, layout.render_w, layout.render_h, rot, min_x_pt, min_y_pt);
                    let p_tr = page.page_to_device(0, 0, layout.render_w, layout.render_h, rot, max_x_pt, max_y_pt);

                    if let (Some(bl), Some(tr)) = (p_bl, p_tr) {
                        let min_dev_x = page_x + bl.0.min(tr.0);
                        let max_dev_x = page_x + bl.0.max(tr.0);
                        let min_dev_y = page_y + bl.1.min(tr.1);
                        let max_dev_y = page_y + bl.1.max(tr.1);
                        let dev_w = max_dev_x - min_dev_x;
                        let dev_h = max_dev_y - min_dev_y;

                        // 1. Draw image bitmap preview if available
                        if !gizmo.bgra_pixels.is_empty() && gizmo.raw_width_px > 0 && gizmo.raw_height_px > 0 {
                            let bmi = BITMAPINFO {
                                bmiHeader: BITMAPINFOHEADER {
                                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                                    biWidth: gizmo.raw_width_px as i32,
                                    biHeight: -(gizmo.raw_height_px as i32),
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
                                    min_dev_x,
                                    min_dev_y,
                                    dev_w,
                                    dev_h,
                                    0,
                                    0,
                                    gizmo.raw_width_px as i32,
                                    gizmo.raw_height_px as i32,
                                    gizmo.bgra_pixels.as_ptr() as *const c_void,
                                    &bmi,
                                    0,
                                    SRCCOPY,
                                );
                            }
                        }

                        // 2. Draw border
                        unsafe {
                            let gizmo_pen = CreatePen(0, 2, rgb(0x3B, 0x82, 0xF6));
                            let null_b = GetStockObject(NULL_BRUSH);
                            let old_p = SelectObject(hdc, gizmo_pen as HGDIOBJ);
                            let old_b = SelectObject(hdc, null_b);
                            Rectangle(hdc, min_dev_x, min_dev_y, max_dev_x, max_dev_y);
                            SelectObject(hdc, old_p);
                            SelectObject(hdc, old_b);
                            DeleteObject(gizmo_pen as HGDIOBJ);

                            // 3. Draw 8 square handles
                            let hs = scale_dpi(4, state.dpi);
                            let mid_x = (min_dev_x + max_dev_x) / 2;
                            let mid_y = (min_dev_y + max_dev_y) / 2;
                            let handle_pts = [
                                (min_dev_x, min_dev_y),
                                (mid_x, min_dev_y),
                                (max_dev_x, min_dev_y),
                                (min_dev_x, mid_y),
                                (max_dev_x, mid_y),
                                (min_dev_x, max_dev_y),
                                (mid_x, max_dev_y),
                                (max_dev_x, max_dev_y),
                            ];

                            let h_brush = CreateSolidBrush(rgb(0xFF, 0xFF, 0xFF));
                            let h_pen = CreatePen(0, 1, rgb(0x1D, 0x4E, 0xD8));
                            let old_hb = SelectObject(hdc, h_brush as HGDIOBJ);
                            let old_hp = SelectObject(hdc, h_pen as HGDIOBJ);
                            for (hx, hy) in handle_pts {
                                Rectangle(hdc, hx - hs, hy - hs, hx + hs, hy + hs);
                            }
                            SelectObject(hdc, old_hb);
                            SelectObject(hdc, old_hp);
                            DeleteObject(h_brush as HGDIOBJ);
                            DeleteObject(h_pen as HGDIOBJ);
                        }
                    }
                }
            }
        }

        // Render Search Match Highlights on this visible page
        if state.search_open && !state.search_matches.is_empty() {
            if let Ok(page) = doc.load_page(layout.index) {
                let rot = page.rotation();
                for (m_idx, m) in state.search_matches.iter().enumerate() {
                    if m.page_index == layout.index {
                        let pt1 = page.page_to_device(
                            0, 0, layout.render_w, layout.render_h, rot, m.rect.left as f64, m.rect.top as f64,
                        );
                        let pt2 = page.page_to_device(
                            0, 0, layout.render_w, layout.render_h, rot, m.rect.right as f64, m.rect.bottom as f64,
                        );
                        if let (Some((x1, y1)), Some((x2, y2))) = (pt1, pt2) {
                            let rx1 = page_x + x1.min(x2);
                            let rx2 = page_x + x1.max(x2);
                            let ry1 = page_y + y1.min(y2);
                            let ry2 = page_y + y1.max(y2);
                            let match_rc = RECT {
                                left: rx1,
                                top: ry1,
                                right: rx2,
                                bottom: ry2,
                            };
                            let is_active = state.current_search_idx == Some(m_idx);
                            unsafe {
                                if is_active {
                                    InvertRect(hdc, &match_rc);
                                    let active_brush = CreateSolidBrush(rgb(0x3B, 0x82, 0xF6));
                                    FrameRect(hdc, &match_rc, active_brush);
                                    let inner_rc = RECT {
                                        left: rx1 + 1,
                                        top: ry1 + 1,
                                        right: rx2 - 1,
                                        bottom: ry2 - 1,
                                    };
                                    FrameRect(hdc, &inner_rc, active_brush);
                                    DeleteObject(active_brush as HGDIOBJ);
                                } else {
                                    let inactive_brush = CreateSolidBrush(rgb(0xFA, 0xCC, 0x15));
                                    FrameRect(hdc, &match_rc, inactive_brush);
                                    DeleteObject(inactive_brush as HGDIOBJ);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    state.active_page_idx = current_page;

    // Draw active hover highlight for editable text objects
    if let Some(r) = state.hovered_edit_rect {
        unsafe {
            let hover_brush = CreateSolidBrush(rgb(0x3B, 0x82, 0xF6));
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
