//! Every user action, so toolbar buttons, shortcuts and menus share one dispatch path.

use super::app::*;
use super::doc_ops;
use super::win32::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    Open,
    Save,
    SaveAs,
    Print,
    Undo,
    Redo,
    Tool(Tool),
    InsertImage,
    PasteImage,
    Copy,
    ZoomIn,
    ZoomOut,
    ZoomMenu,
    FitWidth,
    FitPage,
    ActualSize,
    Search,
    ToggleSidebar,
    GoToPage,
    RotateCw,
    RotateCcw,
    DeletePage,
}

impl Cmd {
    pub fn enabled(self, app: &App) -> bool {
        let has_doc = app.doc.is_some();
        match self {
            Cmd::Open => true,
            Cmd::Save => app.doc.as_ref().is_some_and(|d| d.is_dirty() || d.path.is_none()),
            Cmd::Undo => app.doc.as_ref().is_some_and(|d| d.history.can_undo()),
            Cmd::Redo => app.doc.as_ref().is_some_and(|d| d.history.can_redo()),
            Cmd::Copy => !app.selection.is_empty(),
            Cmd::DeletePage => app.page_count() > 1,
            _ => has_doc,
        }
    }

    /// Tooltip text: name plus shortcut.
    pub fn tip(self) -> &'static str {
        match self {
            Cmd::Open => "Open  (Ctrl+O)",
            Cmd::Save => "Save  (Ctrl+S)",
            Cmd::SaveAs => "Save as  (Ctrl+Shift+S)",
            Cmd::Print => "Print  (Ctrl+P)",
            Cmd::Undo => "Undo  (Ctrl+Z)",
            Cmd::Redo => "Redo  (Ctrl+Y)",
            Cmd::Tool(Tool::Pan) => "Hand tool  (H)",
            Cmd::Tool(Tool::Select) => "Select text  (S)",
            Cmd::Tool(Tool::Edit) => "Edit text  (T)",
            Cmd::Tool(Tool::Highlight) => "Highlight  (M)",
            Cmd::Tool(Tool::StampCheck) => "Checkmark stamp  (V)",
            Cmd::Tool(Tool::StampCross) => "Cross stamp  (X)",
            Cmd::Tool(Tool::Redact) => "Redact  (right-click for mode)",
            Cmd::InsertImage => "Insert image  (Ctrl+V pastes)",
            Cmd::ZoomIn => "Zoom in  (Ctrl +)",
            Cmd::ZoomOut => "Zoom out  (Ctrl −)",
            Cmd::ZoomMenu => "Zoom presets",
            Cmd::FitWidth => "Fit width  (Ctrl+1)",
            Cmd::FitPage => "Fit page  (Ctrl+2)",
            Cmd::Search => "Find  (Ctrl+F)",
            Cmd::ToggleSidebar => "Page thumbnails  (F9)",
            Cmd::GoToPage => "Go to page  (Ctrl+G)",
            _ => "",
        }
    }
}

pub fn run(app: &mut App, cmd: Cmd) {
    if !cmd.enabled(app) {
        return;
    }
    if !matches!(cmd, Cmd::Copy | Cmd::Tool(_) | Cmd::Search) {
        doc_ops::commit_editor(app);
    }
    let page = app.current_page();
    match cmd {
        Cmd::Open => doc_ops::prompt_open(app),
        Cmd::Save => doc_ops::save(app, false),
        Cmd::SaveAs => doc_ops::save(app, true),
        Cmd::Print => super::print::print(app),
        Cmd::Undo => doc_ops::undo(app),
        Cmd::Redo => doc_ops::redo(app),
        Cmd::Tool(t) => {
            // Clicking the active tool again returns to the hand tool.
            app.tool = if app.tool == t && t != Tool::Pan { Tool::Pan } else { t };
            doc_ops::commit_editor(app);
            if let Some(d) = &mut app.doc {
                if d.form_focus.take().is_some() {
                    if let Some(f) = &d.form {
                        f.kill_focus();
                    }
                }
            }
            app.invalidate();
        }
        Cmd::InsertImage => doc_ops::prompt_insert_image(app),
        Cmd::PasteImage => doc_ops::paste_image(app),
        Cmd::Copy => doc_ops::copy_selection(app),
        Cmd::ZoomIn => app.zoom_step(true, None),
        Cmd::ZoomOut => app.zoom_step(false, None),
        Cmd::ActualSize => {
            app.fit = Fit::None;
            app.set_zoom(100.0, None);
        }
        Cmd::ZoomMenu => zoom_menu(app),
        Cmd::FitWidth => app.set_fit(Fit::Width),
        Cmd::FitPage => app.set_fit(Fit::Page),
        Cmd::Search => super::search::open(app),
        Cmd::ToggleSidebar => {
            app.sidebar_visible = !app.sidebar_visible;
            app.relayout();
            app.invalidate();
        }
        Cmd::GoToPage => super::input::begin_goto(app),
        Cmd::RotateCw => doc_ops::rotate(app, page, 1),
        Cmd::RotateCcw => doc_ops::rotate(app, page, 3),
        Cmd::DeletePage => doc_ops::delete_page(app, page),
    }
}

fn zoom_menu(app: &mut App) {
    let r = app.buttons.iter().find(|b| b.cmd == Cmd::ZoomMenu).map(|b| b.rect).unwrap_or_default();
    let check = |on: bool| if on { MF_CHECKED } else { 0 };
    let z = app.zoom.round() as i32;
    let items = [
        (1, "Fit width\tCtrl+1", check(app.fit == Fit::Width)),
        (2, "Fit page\tCtrl+2", check(app.fit == Fit::Page)),
        (0, "", 0),
        (50, "50%", check(app.fit == Fit::None && z == 50)),
        (75, "75%", check(app.fit == Fit::None && z == 75)),
        (100, "Actual size (100%)\tCtrl+0", check(app.fit == Fit::None && z == 100)),
        (125, "125%", check(app.fit == Fit::None && z == 125)),
        (150, "150%", check(app.fit == Fit::None && z == 150)),
        (200, "200%", check(app.fit == Fit::None && z == 200)),
        (400, "400%", check(app.fit == Fit::None && z == 400)),
    ];
    match popup_menu(app.hwnd, r.left, r.bottom, &items) {
        1 => app.set_fit(Fit::Width),
        2 => app.set_fit(Fit::Page),
        0 => {}
        pct => {
            app.fit = Fit::None;
            app.set_zoom(pct as f64, None);
        }
    }
}

/// Keyboard shortcuts. Returns true when the key was handled.
pub fn shortcut(app: &mut App, vk: i32) -> bool {
    let ctrl = key_down(VK_CONTROL);
    let shift = key_down(VK_SHIFT);
    let cmd = if ctrl {
        match vk {
            0x4F => Cmd::Open,
            0x53 if shift => Cmd::SaveAs,
            0x53 => Cmd::Save,
            0x50 => Cmd::Print,
            0x5A if shift => Cmd::Redo,
            0x5A => Cmd::Undo,
            0x59 => Cmd::Redo,
            0x46 => Cmd::Search,
            0x47 => Cmd::GoToPage,
            0x43 => Cmd::Copy,
            0x56 => Cmd::PasteImage,
            0x42 => Cmd::ToggleSidebar,
            VK_OEM_PLUS | VK_ADD => Cmd::ZoomIn,
            VK_OEM_MINUS | VK_SUBTRACT => Cmd::ZoomOut,
            0x30 | 0x60 => Cmd::ActualSize,
            0x31 | 0x61 => Cmd::FitWidth,
            0x32 | 0x62 => Cmd::FitPage,
            VK_HOME => {
                app.scroll_to(0, 0, true);
                return true;
            }
            VK_END => {
                app.scroll_to(app.scroll_x, i32::MAX / 2, true);
                return true;
            }
            _ => return false,
        }
    } else {
        match vk {
            0x48 => Cmd::Tool(Tool::Pan),
            0x53 => Cmd::Tool(Tool::Select),
            0x54 => Cmd::Tool(Tool::Edit),
            0x4D => Cmd::Tool(Tool::Highlight),
            0x56 => Cmd::Tool(Tool::StampCheck),
            0x58 => Cmd::Tool(Tool::StampCross),
            0x52 if shift => Cmd::RotateCcw,
            0x52 => Cmd::RotateCw,
            VK_DELETE => Cmd::DeletePage,
            VK_F9 => Cmd::ToggleSidebar,
            VK_F3 => {
                if shift { super::search::prev(app) } else { super::search::next(app) }
                return true;
            }
            _ => return navigation_key(app, vk, shift),
        }
    };
    run(app, cmd);
    true
}

fn navigation_key(app: &mut App, vk: i32, shift: bool) -> bool {
    if app.doc.is_none() {
        return false;
    }
    let page = app.view_rect().height() * 9 / 10;
    let line = app.s(48);
    match vk {
        VK_UP => app.scroll_by(0, -line, true),
        VK_DOWN => app.scroll_by(0, line, true),
        VK_LEFT => app.scroll_by(-line, 0, false),
        VK_RIGHT => app.scroll_by(line, 0, false),
        VK_PRIOR => app.scroll_by(0, -page, true),
        VK_NEXT => app.scroll_by(0, page, true),
        VK_SPACE => app.scroll_by(0, if shift { -page } else { page }, true),
        VK_HOME => app.scroll_to(0, 0, true),
        VK_END => app.scroll_to(app.scroll_x, i32::MAX / 2, true),
        _ => return false,
    }
    true
}
