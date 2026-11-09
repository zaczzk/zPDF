//! Application state.

use super::gfx::Gfx;
use super::render::{PageCache, RenderCache};
use super::theme::Theme;
use super::toolbar::Button;
use super::win32::*;
use crate::editor::UndoManager;
use crate::pdfium::{PdfDocument, PdfFormEnv, PdfPage, PdfiumLibrary, RectF};
use std::path::PathBuf;
use std::ptr;
use std::sync::Arc;
use std::time::Instant;

pub const TIMER_TOAST: usize = 1;
pub const TIMER_RENDER: usize = 2;
pub const TIMER_SCROLL: usize = 3;
pub const TIMER_SEARCH: usize = 4;
pub const TIMER_TOOLTIP: usize = 5;

pub const ID_EDITOR: usize = 1001;
pub const ID_SEARCH: usize = 1002;
pub const ID_GOTO: usize = 1003;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Pan,
    Select,
    Edit,
    Highlight,
    StampCheck,
    StampCross,
    Redact,
}

impl Tool {
    pub fn hint(self, redact_clear: bool) -> &'static str {
        match self {
            Tool::Pan => "Drag to pan  ·  click form fields to fill them",
            Tool::Select => "Drag across text to select  ·  Ctrl+C copies",
            Tool::Edit => "Click text to edit it, or empty space to add text  ·  Enter commits, Esc cancels",
            Tool::Highlight => "Drag across text to highlight it",
            Tool::StampCheck | Tool::StampCross => "Click to place a stamp",
            Tool::Redact if redact_clear => "Drag a box: content inside is permanently removed (no black bar)",
            Tool::Redact => "Drag a box: content inside is permanently removed and covered in black",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    None,
    Width,
    Page,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Handle {
    Move,
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

pub struct PageLayout {
    /// Size in PDF points, /Rotate already applied.
    pub w_pt: f64,
    pub h_pt: f64,
    /// Size on screen in device pixels at the current zoom.
    pub w: i32,
    pub h: i32,
    /// Top edge inside the scrollable content.
    pub y: i32,
}

/// What the left mouse button is currently dragging.
pub enum Drag {
    None,
    Pan { x: i32, y: i32, sx: i32, sy: i32 },
    VThumb { grab: i32 },
    HThumb { grab: i32 },
    Select { page: usize, from: (f64, f64), to: (f64, f64) },
    Redact { page: usize, from: (f64, f64), to: (f64, f64) },
    Gizmo { handle: Handle, mouse: (i32, i32), orig: (f64, f64, f64, f64) },
    /// A press that PDFium's form layer consumed; the release goes there too.
    Form { page: usize },
}

pub struct ImageGizmo {
    pub page: usize,
    /// Bottom-left corner and size in PDF points.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub px_w: u32,
    pub px_h: u32,
    pub pixels: Vec<u8>,
}

pub struct TextEditor {
    pub hwnd: HWND,
    pub font: HFONT,
    pub page: usize,
    pub object: Option<usize>,
    pub original: String,
    pub at: (f64, f64),
    pub size: f32,
    /// Where the EDIT control sits (client coordinates), for drawing its focus ring.
    pub rect: RECT,
}

#[derive(Default)]
pub struct Selection {
    pub text: String,
    pub rects: Vec<(usize, RectF)>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.text.clear();
        self.rects.clear();
    }
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }
}

pub struct SearchMatch {
    pub page: usize,
    pub rect: RectF,
}

pub struct Search {
    pub open: bool,
    pub edit: HWND,
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub current: Option<usize>,
    pub card: RECT,
    pub prev: RECT,
    pub next: RECT,
    pub close: RECT,
}

impl Default for Search {
    fn default() -> Self {
        Search {
            open: false,
            edit: ptr::null_mut(),
            query: String::new(),
            matches: Vec::new(),
            current: None,
            card: RECT::default(),
            prev: RECT::default(),
            next: RECT::default(),
            close: RECT::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Default)]
pub struct Banner {
    pub card: RECT,
    pub yes: RECT,
    pub never: RECT,
}

/// Anything the pointer can hover that has hover styling or a tooltip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hot {
    Button(usize),
    SearchPrev,
    SearchNext,
    SearchClose,
    BannerYes,
    BannerNever,
    Recent(usize),
    WelcomeOpen,
    VScroll,
    HScroll,
    PageBox,
}

pub struct Doc {
    // Field order matters only for documentation; `Drop` below closes things explicitly.
    pub pages: PageCache,
    pub form: Option<PdfFormEnv>,
    pub pdf: PdfDocument,
    pub path: Option<PathBuf>,
    pub history: UndoManager,
    /// Unsaved AcroForm input (not tracked by the undo history).
    pub form_dirty: bool,
    /// Page whose form field currently has keyboard focus.
    pub form_focus: Option<usize>,
}

impl Doc {
    pub fn new(pdf: PdfDocument, path: Option<PathBuf>) -> Self {
        let form = pdf.init_form_env();
        Doc { pages: PageCache::new(), form, pdf, path, history: UndoManager::new(), form_dirty: false, form_focus: None }
    }

    pub fn page(&mut self, idx: usize) -> Option<&PdfPage> {
        self.pages.get(&self.pdf, self.form.as_ref(), idx)
    }

    pub fn page_and_form(&mut self, idx: usize) -> Option<(&PdfPage, Option<&PdfFormEnv>)> {
        let form = self.form.as_ref();
        let page = self.pages.get(&self.pdf, form, idx)?;
        Some((page, form))
    }

    pub fn page_count(&self) -> usize {
        self.pdf.page_count()
    }

    pub fn is_dirty(&self) -> bool {
        self.form_dirty || !self.history.is_at_saved_point()
    }

    pub fn name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled.pdf".into())
    }
}

impl Drop for Doc {
    fn drop(&mut self) {
        // Pages close before the form environment, which exits before the document closes.
        self.pages.clear(self.form.as_ref());
        self.form = None;
    }
}

pub struct App {
    pub hwnd: HWND,
    pub pdfium: Arc<PdfiumLibrary>,
    pub icon: HICON,
    pub gfx: Gfx,
    pub theme: Theme,

    pub doc: Option<Doc>,
    pub cache: RenderCache,

    // Geometry (client pixels)
    pub width: i32,
    pub height: i32,
    pub toolbar_h: i32,
    pub status_h: i32,
    pub sidebar_w: i32,
    pub sidebar_visible: bool,

    // View
    pub zoom: f64,
    pub fit: Fit,
    pub layouts: Vec<PageLayout>,
    pub content_w: i32,
    pub content_h: i32,
    pub scroll_x: i32,
    pub scroll_y: i32,
    pub target_y: f64,
    pub sidebar_scroll: i32,
    pub thumb_slots: Vec<RECT>,
    /// While set, painting uses stretched previews instead of rendering tiles synchronously.
    pub interacting_until: Option<Instant>,

    // Interaction
    pub tool: Tool,
    pub redact_clear: bool,
    pub drag: Drag,
    pub hot: Option<Hot>,
    pub tooltip: Option<Hot>,
    pub hover_text: Option<RECT>,
    pub selection: Selection,
    pub editor: Option<TextEditor>,
    pub gizmo: Option<ImageGizmo>,
    pub search: Search,
    pub goto_edit: HWND,
    pub edit_brush: *mut std::ffi::c_void,
    pub wheel_accum: i32,
    pub over_form_field: bool,

    // Chrome
    pub buttons: Vec<Button>,
    pub page_box: RECT,
    pub welcome_open: RECT,
    pub recent: Vec<PathBuf>,
    pub recent_rects: Vec<RECT>,
    pub banner: Option<Banner>,
    pub toast: Option<(String, ToastKind)>,

    // Back buffer
    pub bb_dc: HDC,
    pub bb_bmp: HBITMAP,
    pub bb_old: HGDIOBJ,
    pub bb_size: (i32, i32),
}

impl App {
    pub fn new(hwnd: HWND, pdfium: Arc<PdfiumLibrary>, icon: HICON) -> Self {
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        let theme = super::theme::current();
        let mut app = App {
            hwnd,
            pdfium,
            icon,
            gfx: Gfx::new(dpi),
            theme,
            doc: None,
            cache: RenderCache::new(),
            width: 0,
            height: 0,
            toolbar_h: 0,
            status_h: 0,
            sidebar_w: 0,
            sidebar_visible: true,
            zoom: 100.0,
            fit: Fit::None,
            layouts: Vec::new(),
            content_w: 0,
            content_h: 0,
            scroll_x: 0,
            scroll_y: 0,
            target_y: 0.0,
            sidebar_scroll: 0,
            thumb_slots: Vec::new(),
            interacting_until: None,
            tool: Tool::Pan,
            redact_clear: false,
            drag: Drag::None,
            hot: None,
            tooltip: None,
            hover_text: None,
            selection: Selection::default(),
            editor: None,
            gizmo: None,
            search: Search::default(),
            goto_edit: ptr::null_mut(),
            edit_brush: ptr::null_mut(),
            wheel_accum: 0,
            over_form_field: false,
            buttons: Vec::new(),
            page_box: RECT::default(),
            welcome_open: RECT::default(),
            recent: super::settings::recent_files(),
            recent_rects: Vec::new(),
            banner: if crate::default_app::should_prompt_default_app() { Some(Banner::default()) } else { None },
            toast: None,
            bb_dc: ptr::null_mut(),
            bb_bmp: ptr::null_mut(),
            bb_old: ptr::null_mut(),
            bb_size: (0, 0),
        };
        app.apply_metrics();
        app.apply_theme();
        app
    }

    pub fn s(&self, v: i32) -> i32 {
        self.gfx.s(v)
    }

    pub fn apply_metrics(&mut self) {
        self.toolbar_h = self.s(48);
        self.status_h = self.s(28);
        self.sidebar_w = self.s(184);
        self.gfx.warm();
    }

    pub fn apply_theme(&mut self) {
        self.theme = super::theme::current();
        super::theme::apply_to_window(self.hwnd, &self.theme);
        unsafe {
            if !self.edit_brush.is_null() {
                DeleteObject(self.edit_brush);
            }
            self.edit_brush = CreateSolidBrush(self.theme.card);
        }
        self.invalidate();
    }

    pub fn invalidate(&self) {
        unsafe { InvalidateRect(self.hwnd, ptr::null(), 0) };
    }

    pub fn timer(&self, id: usize, ms: u32) {
        unsafe { SetTimer(self.hwnd, id, ms, ptr::null_mut()) };
    }

    pub fn kill_timer(&self, id: usize) {
        unsafe { KillTimer(self.hwnd, id) };
    }

    pub fn toast(&mut self, msg: impl Into<String>, kind: ToastKind) {
        self.toast = Some((msg.into(), kind));
        self.timer(TIMER_TOAST, if kind == ToastKind::Error { 4000 } else { 1800 });
        self.invalidate();
    }

    pub fn update_title(&self) {
        let title = match &self.doc {
            Some(d) => format!("{}{} — zPDF", if d.is_dirty() { "● " } else { "" }, d.name()),
            None => "zPDF".to_string(),
        };
        let w = wide(&title);
        unsafe { SetWindowTextW(self.hwnd, w.as_ptr()) };
    }

    /// Call after any change to page content: drops caches for that page and refreshes chrome.
    pub fn page_changed(&mut self, page: usize) {
        self.cache.invalidate_page(page);
        if let Some(d) = &mut self.doc {
            d.pages.forget(d.form.as_ref(), page);
        }
        self.update_title();
        self.invalidate();
    }

    /// Call after pages were added, removed, rotated or the document was replaced.
    pub fn structure_changed(&mut self) {
        self.cache.clear();
        if let Some(d) = &mut self.doc {
            d.pages.clear(d.form.as_ref());
        }
        self.relayout();
        self.update_title();
        self.invalidate();
    }

    pub fn page_count(&self) -> usize {
        self.doc.as_ref().map_or(0, |d| d.page_count())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            for h in [self.search.edit, self.goto_edit] {
                if !h.is_null() {
                    DestroyWindow(h);
                }
            }
            if let Some(ed) = self.editor.take() {
                DestroyWindow(ed.hwnd);
                DeleteObject(ed.font);
            }
            if !self.edit_brush.is_null() {
                DeleteObject(self.edit_brush);
            }
            if !self.bb_dc.is_null() {
                SelectObject(self.bb_dc, self.bb_old);
                DeleteObject(self.bb_bmp);
                DeleteDC(self.bb_dc);
            }
        }
        // Close the document (and its pages/forms) before the PDFium library handle goes away.
        self.doc = None;
    }
}
