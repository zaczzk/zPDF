//! Window procedure and input handling.

use super::app::*;
use super::commands::{self, Cmd};
use super::doc_ops;
use super::paint::gizmo_handles;
use super::nav;
use super::search;
use super::win32::*;
use crate::pdfium::RectF;
use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr;

const FORM_SHIFT: i32 = 1;
const FORM_CTRL: i32 = 2;
const FORM_ALT: i32 = 4;

fn form_mods() -> i32 {
    (if key_down(VK_SHIFT) { FORM_SHIFT } else { 0 }) | (if key_down(VK_CONTROL) { FORM_CTRL } else { 0 }) | (if key_down(VK_MENU) { FORM_ALT } else { 0 })
}

pub unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let p = super::app_ptr();
    if p.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let app = &mut *p;
    match msg {
        WM_SIZE => {
            let (w, h) = ((lparam & 0xFFFF) as i32, ((lparam >> 16) & 0xFFFF) as i32);
            // Minimizing reports 0x0; laying out for that would clamp the scroll position to 0.
            if wparam == 1 || w == 0 || h == 0 {
                return 0;
            }
            app.width = w;
            app.height = h;
            app.relayout();
            search::layout(app);
            app.invalidate();
            0
        }
        WM_GETMINMAXINFO => {
            // MINMAXINFO: ptReserved, ptMaxSize, ptMaxPosition, ptMinTrackSize, ...
            let pts = lparam as *mut POINT;
            *pts.add(3) = POINT { x: app.s(560), y: app.s(400) };
            0
        }
        WM_DPICHANGED => {
            // The in-place editor's font is sized for the old DPI; commit rather than resize it.
            doc_ops::commit_editor(app);
            app.gfx.set_dpi((wparam & 0xFFFF) as u32);
            app.apply_metrics();
            app.cache.clear();
            app.relayout();
            let pw = app.password.as_ref().map_or(ptr::null_mut(), |p| p.edit);
            for h in [app.search.edit, app.goto_edit, pw] {
                if !h.is_null() {
                    SendMessageW(h, WM_SETFONT, app.gfx.font(super::gfx::Font::Body) as WPARAM, 1);
                }
            }
            let r = *(lparam as *const RECT);
            MoveWindow(hwnd, r.left, r.top, r.width(), r.height(), 1);
            0
        }
        WM_ACTIVATE | WM_SETFOCUS => {
            // Returning to the window: keep typing in the password box while it is open.
            let r = DefWindowProcW(hwnd, msg, wparam, lparam);
            if let Some(p) = &app.password {
                if msg == WM_SETFOCUS || (wparam & 0xFFFF) != 0 {
                    SetFocus(p.edit);
                }
            }
            r
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED => {
            app.apply_theme();
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            if doc_ops::confirm_discard(app) {
                doc_ops::remember_position(app);
                let mut wp = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
                if GetWindowPlacement(hwnd, &mut wp) != 0 {
                    super::settings::save_window_placement(&wp);
                }
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            super::paint::paint(app, hdc);
            EndPaint(hwnd, &ps);
            0
        }
        WM_TIMER => {
            match wparam {
                TIMER_TOAST => {
                    app.kill_timer(TIMER_TOAST);
                    app.toast = None;
                    app.invalidate();
                }
                TIMER_RENDER => super::paint::on_render_timer(app),
                TIMER_SCROLL => {
                    if !app.animate_scroll() {
                        app.kill_timer(TIMER_SCROLL);
                    }
                }
                TIMER_SEARCH => search::step(app),
                TIMER_TOOLTIP => {
                    app.kill_timer(TIMER_TOOLTIP);
                    if app.drag_is_none() {
                        app.tooltip = app.hot;
                        app.invalidate();
                    }
                }
                _ => {}
            }
            0
        }
        // Only for our own client area: child EDIT controls keep their I-beam.
        WM_SETCURSOR if (lparam & 0xFFFF) as i16 == 1 && wparam as HWND == hwnd => {
            let mut pt = POINT::default();
            GetCursorPos(&mut pt);
            ScreenToClient(hwnd, &mut pt);
            set_cursor(cursor_for(app, pt.x, pt.y));
            1
        }
        WM_DROPFILES => {
            on_drop(app, wparam as *mut c_void);
            0
        }
        WM_CTLCOLOREDIT => {
            let hdc = wparam as HDC;
            SetTextColor(hdc, app.theme.text);
            SetBkColor(hdc, app.theme.card);
            app.edit_brush as LRESULT
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            let code = ((wparam >> 16) & 0xFFFF) as u16;
            match (id, code) {
                (ID_EDITOR, EN_KILLFOCUS) => doc_ops::commit_editor(app),
                (ID_SEARCH, EN_CHANGE) => search::query_changed(app),
                (ID_GOTO, EN_KILLFOCUS) => end_goto(app, true),
                _ => {}
            }
            0
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            let (x, y) = lparam_point(lparam);
            on_left_down(app, x, y);
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = lparam_point(lparam);
            on_mouse_move(app, x, y);
            0
        }
        WM_LBUTTONUP => {
            let (x, y) = lparam_point(lparam);
            on_left_up(app, x, y);
            0
        }
        WM_RBUTTONUP => {
            let (x, y) = lparam_point(lparam);
            on_right_up(app, x, y);
            0
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let delta = ((wparam >> 16) & 0xFFFF) as i16 as i32;
            let mut pt = POINT { x: lparam_point(lparam).0, y: lparam_point(lparam).1 };
            ScreenToClient(hwnd, &mut pt);
            on_wheel(app, delta, msg == WM_MOUSEHWHEEL, wparam & 0x08 != 0, wparam & 0x04 != 0, pt.x, pt.y);
            0
        }
        WM_MOUSELEAVE => {
            app.hot = None;
            app.tooltip = None;
            app.hover_text = None;
            app.invalidate();
            0
        }
        WM_CAPTURECHANGED => {
            // Capture lost (alt-tab, a menu opened…): never leave a drag stuck.
            if GetCapture() != hwnd && !app.drag_is_none() {
                app.drag = Drag::None;
                app.pending_link = None;
                nav::end(app);
                app.invalidate();
            }
            0
        }
        WM_SYSKEYDOWN if wparam as i32 == VK_LEFT && nav::go_back(app) => 0,
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if msg == WM_KEYDOWN && on_key_down(app, wparam as i32) {
                return 0;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_KEYUP => {
            if let Some(page) = app.doc.as_ref().and_then(|d| d.form_focus) {
                with_form_page(app, page, |f, p| f.on_key_up(p, wparam as i32, form_mods()));
            }
            0
        }
        WM_CHAR => {
            if let Some(page) = app.doc.as_ref().and_then(|d| d.form_focus) {
                if wparam >= 0x20 || wparam == 0x08 {
                    if with_form_page(app, page, |f, p| f.on_char(p, wparam as i32, form_mods())) {
                        form_edited(app, page);
                    }
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

impl App {
    pub fn drag_is_none(&self) -> bool {
        matches!(self.drag, Drag::None)
    }
}

// ---------------------------------------------------------------------------
// AcroForms
// ---------------------------------------------------------------------------

fn with_form_page(app: &mut App, page: usize, f: impl FnOnce(&crate::pdfium::PdfFormEnv, &crate::pdfium::PdfPage) -> bool) -> bool {
    let Some(doc) = app.doc.as_mut() else { return false };
    let Some((p, Some(form))) = doc.page_and_form(page) else { return false };
    f(form, p)
}

/// Form input changes the page appearance but keeps the page handle (and field focus) alive.
fn form_edited(app: &mut App, page: usize) {
    if let Some(d) = &mut app.doc {
        d.form_dirty = true;
    }
    app.cache.invalidate_page(page);
    app.update_title();
    app.invalidate();
}

fn kill_form_focus(app: &mut App) {
    if let Some(doc) = &mut app.doc {
        if doc.form_focus.take().is_some() {
            if let Some(f) = &doc.form {
                f.kill_focus();
            }
            app.invalidate();
        }
    }
}

// ---------------------------------------------------------------------------
// Keyboard
// ---------------------------------------------------------------------------

fn on_key_down(app: &mut App, vk: i32) -> bool {
    // The password prompt is modal: never act on the (previous) document behind it.
    if let Some(p) = &app.password {
        unsafe { SetFocus(p.edit) };
        return true;
    }
    let ctrl = key_down(VK_CONTROL);
    // A focused form field gets keys first; plain letters never reach the tool shortcuts.
    if let Some(page) = app.doc.as_ref().and_then(|d| d.form_focus) {
        if vk == VK_ESCAPE {
            kill_form_focus(app);
            return true;
        }
        let editing_key = !ctrl || matches!(vk, 0x41 | 0x43 | 0x56 | 0x58 | 0x5A | 0x59);
        if editing_key {
            if with_form_page(app, page, |f, p| f.on_key_down(p, vk, form_mods())) {
                form_edited(app, page);
            }
            return true;
        }
    }
    match vk {
        VK_ESCAPE => {
            if app.gizmo.take().is_some() {
            } else if app.search.open {
                search::close(app);
            } else if !app.selection.is_empty() {
                app.selection.clear();
            } else if app.tool != Tool::Pan {
                app.tool = Tool::Pan;
            }
            app.invalidate();
            true
        }
        VK_RETURN if app.gizmo.is_some() => {
            doc_ops::commit_gizmo(app);
            true
        }
        _ => commands::shortcut(app, vk),
    }
}

// ---------------------------------------------------------------------------
// Mouse
// ---------------------------------------------------------------------------

fn hot_at(app: &App, x: i32, y: i32) -> Option<Hot> {
    if let Some(p) = &app.password {
        if p.ok.contains(x, y) {
            return Some(Hot::PasswordOk);
        }
        if p.cancel.contains(x, y) {
            return Some(Hot::PasswordCancel);
        }
    }
    if y < app.toolbar_h {
        if app.page_box.contains(x, y) {
            return Some(Hot::PageBox);
        }
        return app.button_at(x, y).map(Hot::Button);
    }
    if let Some(b) = &app.banner {
        if app.doc.is_some() {
            if b.yes.contains(x, y) {
                return Some(Hot::BannerYes);
            }
            if b.never.contains(x, y) {
                return Some(Hot::BannerNever);
            }
        }
    }
    if app.search.open {
        for (r, h) in [(app.search.prev, Hot::SearchPrev), (app.search.next, Hot::SearchNext), (app.search.close, Hot::SearchClose)] {
            if r.contains(x, y) {
                return Some(h);
            }
        }
    }
    if app.doc.is_none() {
        if app.welcome_open.contains(x, y) {
            return Some(Hot::WelcomeOpen);
        }
        return app.recent_rects.iter().position(|r| r.contains(x, y)).map(Hot::Recent);
    }
    if x < app.sidebar_width() {
        if app.tab_pages.contains(x, y) {
            return Some(Hot::TabPages);
        }
        if app.tab_outline.contains(x, y) {
            return Some(Hot::TabOutline);
        }
        if app.sidebar_tab == SidebarTab::Outline {
            if y < app.toolbar_h + app.s(36) {
                return None;
            }
            return app.outline_rows.iter().position(|r| r.contains(x, y)).map(Hot::OutlineRow);
        }
        return None;
    }
    if app.in_vbar(x, y) {
        return Some(Hot::VScroll);
    }
    if app.in_hbar(x, y) {
        return Some(Hot::HScroll);
    }
    None
}

/// Counts rapid clicks at the same spot (1 = single, 2 = double, 3 = triple…).
fn click_count(app: &mut App, x: i32, y: i32) -> u32 {
    let (t, lx, ly, n) = app.last_click;
    let slop = app.s(4);
    let quick = t.elapsed().as_millis() <= unsafe { GetDoubleClickTime() } as u128;
    let count = if quick && (x - lx).abs() <= slop && (y - ly).abs() <= slop { n + 1 } else { 1 };
    app.last_click = (std::time::Instant::now(), x, y, count);
    count
}

fn on_left_down(app: &mut App, x: i32, y: i32) {
    if !app.goto_edit.is_null() {
        end_goto(app, true);
    }
    app.tooltip = None;
    let clicks = click_count(app, x, y);
    match hot_at(app, x, y) {
        Some(Hot::PasswordOk) => return doc_ops::submit_password(app),
        Some(Hot::PasswordCancel) => return doc_ops::cancel_password(app),
        _ if app.password.is_some() => return, // modal: ignore clicks outside the card
        Some(Hot::TabPages) => {
            app.sidebar_tab = SidebarTab::Pages;
            app.invalidate();
            return;
        }
        Some(Hot::TabOutline) => {
            app.sidebar_tab = SidebarTab::Outline;
            app.invalidate();
            return;
        }
        Some(Hot::OutlineRow(i)) => {
            let target = app.doc.as_ref().and_then(|d| d.outline.get(i)).and_then(|o| o.target.clone());
            if let Some(t) = target {
                nav::follow(app, &t);
            }
            return;
        }
        Some(Hot::Button(i)) => {
            let cmd = app.buttons[i].cmd;
            commands::run(app, cmd);
            return;
        }
        Some(Hot::PageBox) => return begin_goto(app),
        Some(Hot::BannerYes) => {
            let _ = crate::default_app::register_in_place();
            crate::default_app::open_default_apps_settings();
            let _ = crate::default_app::set_never_ask(true);
            app.banner = None;
            app.invalidate();
            return;
        }
        Some(Hot::BannerNever) => {
            let _ = crate::default_app::set_never_ask(true);
            app.banner = None;
            app.invalidate();
            return;
        }
        Some(Hot::SearchPrev) => return search::prev(app),
        Some(Hot::SearchNext) => return search::next(app),
        Some(Hot::SearchClose) => return search::close(app),
        Some(Hot::WelcomeOpen) => return doc_ops::prompt_open(app),
        Some(Hot::Recent(i)) => {
            if let Some(p) = app.recent.get(i).cloned() {
                doc_ops::open_file(app, &p);
            }
            return;
        }
        Some(Hot::VScroll) => {
            let Some((_, thumb)) = app.vbar() else { return };
            if y >= thumb.top && y < thumb.bottom {
                app.drag = Drag::VThumb { grab: y - thumb.top };
                capture(app);
            } else {
                let page = app.view_rect().height() * 9 / 10;
                app.scroll_by(0, if y < thumb.top { -page } else { page }, true);
            }
            app.invalidate();
            return;
        }
        Some(Hot::HScroll) => {
            let Some((_, thumb)) = app.hbar() else { return };
            if x >= thumb.left && x < thumb.right {
                app.drag = Drag::HThumb { grab: x - thumb.left };
                capture(app);
            } else {
                let step = app.view_rect().width() * 9 / 10;
                app.scroll_by(if x < thumb.left { -step } else { step }, 0, false);
            }
            return;
        }
        None => {}
    }
    if (app.search.open && app.search.card.contains(x, y)) || app.banner.as_ref().is_some_and(|b| b.card.contains(x, y)) || app.doc.is_none() {
        return;
    }
    // Clicking the document takes keyboard focus back from the find, go-to or other boxes, so
    // shortcuts, arrows and form typing reach the document.
    if unsafe { GetFocus() } != app.hwnd {
        unsafe { SetFocus(app.hwnd) };
    }
    if let Some(i) = app.thumb_at(x, y) {
        doc_ops::commit_editor(app);
        app.go_to_page(i);
        return;
    }
    if x < app.sidebar_width() || y >= app.height - app.status_h {
        return;
    }

    // Image gizmo first: handles and body move/resize it, clicking elsewhere places it.
    if app.gizmo.is_some() {
        if let Some(handle) = gizmo_hit(app, x, y) {
            let g = app.gizmo.as_ref().unwrap();
            app.drag = Drag::Gizmo { handle, mouse: (x, y), orig: (g.x, g.y, g.w, g.h) };
            capture(app);
            return;
        }
        doc_ops::commit_gizmo(app);
    }

    let page = app.page_at(x, y);
    let pt = page.and_then(|p| app.to_pdf(p, x, y).map(|pt| (p, pt)));

    // Double-click selects a word, triple-click a line (in the hand, select and highlight tools).
    if clicks >= 2 && matches!(app.tool, Tool::Pan | Tool::Select | Tool::Highlight) && !app.over_form_field {
        if let Some((p, pt)) = pt {
            if nav::select_unit(app, p, pt, clicks >= 3) {
                nav::end(app);
                if app.tool == Tool::Highlight {
                    doc_ops::highlight_selection(app);
                }
                return;
            }
        }
    }

    match app.tool {
        Tool::Pan => {
            if let Some((p, (px, py))) = pt {
                let handled = with_form_page(app, p, |f, pg| f.on_lbutton_down(pg, form_mods(), px, py));
                if handled {
                    if let Some(d) = &mut app.doc {
                        d.form_focus = Some(p);
                    }
                    app.drag = Drag::Form { page: p };
                    capture(app);
                    form_edited_light(app, p);
                    return;
                }
                app.pending_link = nav::link_at(app, p, px, py);
            }
            kill_form_focus(app);
            app.selection.clear();
            app.drag = Drag::Pan { x, y, sx: app.scroll_x, sy: app.scroll_y };
            capture(app);
        }
        Tool::Select | Tool::Highlight => {
            app.selection.clear();
            if let Some((p, pt)) = pt {
                let anchor = nav::begin(app, p, pt);
                app.drag = Drag::Select { page: p, from: pt, to: pt, anchor };
                capture(app);
            }
            app.invalidate();
        }
        Tool::Edit => {
            doc_ops::commit_editor(app);
            // A press on an existing run is either a click (edit it) or a drag (move it); the
            // release decides. Anywhere else starts a new run straight away.
            let hit = pt.and_then(|(p, (px, py))| {
                let page = app.doc.as_mut().and_then(|d| d.page(p))?;
                crate::editor::hit_test_text_object(page, px as f32, py as f32, 4.0).map(|(obj, _, b)| (p, obj, b, (px, py)))
            });
            match hit {
                Some((p, object, bounds, from)) => {
                    app.drag = Drag::MoveText { page: p, object, bounds, from, to: from, down: (x, y), moved: false };
                    capture(app);
                }
                None => doc_ops::begin_text_edit(app, x, y),
            }
        }
        Tool::StampCheck | Tool::StampCross => {
            if let Some((p, (px, py))) = pt {
                doc_ops::place_stamp(app, p, px, py, app.tool == Tool::StampCheck);
            }
        }
        Tool::Redact => {
            if let Some((p, pt)) = pt {
                app.drag = Drag::Redact { page: p, from: pt, to: pt };
                capture(app);
            }
        }
    }
}

/// Repaint after a click on a form field without dropping the page (which would lose focus).
fn form_edited_light(app: &mut App, page: usize) {
    app.cache.invalidate_page(page);
    app.invalidate();
}

fn capture(app: &App) {
    unsafe { SetCapture(app.hwnd) };
}

fn on_mouse_move(app: &mut App, x: i32, y: i32) {
    let mut tme = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: app.hwnd, dwHoverTime: 0 };
    unsafe { TrackMouseEvent(&mut tme) };

    match app.drag {
        Drag::Pan { x: x0, y: y0, sx, sy } => {
            // A press on a link only becomes a pan once the pointer really moves.
            if (x - x0).abs().max((y - y0).abs()) > app.s(4) {
                app.pending_link = None;
            }
            if app.pending_link.is_none() {
                app.scroll_to(sx - (x - x0), sy - (y - y0), false);
            }
            return;
        }
        Drag::VThumb { grab } => return app.drag_vthumb(y, grab),
        Drag::HThumb { grab } => return app.drag_hthumb(x, grab),
        Drag::Select { page, from, anchor, .. } => {
            if let Some(to) = app.to_pdf_clamped(page, x, y) {
                app.drag = Drag::Select { page, from, to, anchor };
                nav::update(app, page, anchor, from, to);
            }
            return;
        }
        Drag::Redact { page, from, .. } => {
            if let Some(to) = app.to_pdf_clamped(page, x, y) {
                app.drag = Drag::Redact { page, from, to };
                app.invalidate();
            }
            return;
        }
        Drag::Gizmo { handle, mouse, orig } => return drag_gizmo(app, handle, mouse, orig, x, y),
        Drag::MoveText { page, object, bounds, from, to, down, moved } => {
            // A press only becomes a move once the pointer really travels.
            let moved = moved || (x - down.0).abs().max((y - down.1).abs()) > app.s(4);
            let to = if moved { app.to_pdf_clamped(page, x, y).unwrap_or(to) } else { to };
            app.drag = Drag::MoveText { page, object, bounds, from, to, down, moved };
            if moved {
                app.invalidate();
            }
            return;
        }
        Drag::Form { page } => {
            if let Some((px, py)) = app.to_pdf(page, x, y) {
                with_form_page(app, page, |f, p| f.on_mouse_move(p, form_mods(), px, py));
            }
            return;
        }
        Drag::None => {}
    }

    let hot = hot_at(app, x, y);
    if hot != app.hot {
        app.hot = hot;
        app.tooltip = None;
        if hot.is_some() {
            app.timer(TIMER_TOOLTIP, 550);
        }
        app.invalidate();
    }

    // Hover feedback over the document.
    let page = app.page_at(x, y);
    let mut hover = None;
    let mut link = None;
    app.over_form_field = false;
    if let Some(p) = page {
        if let Some((px, py)) = app.to_pdf(p, x, y) {
            match app.tool {
                Tool::Edit if app.editor.is_none() => {
                    let bounds = app.doc.as_mut().and_then(|d| d.page(p)).and_then(|pg| crate::editor::hit_test_text_object(pg, px as f32, py as f32, 4.0)).map(|(_, _, b)| b);
                    hover = bounds.and_then(|b| app.rect_to_screen(p, b)).map(|r| r.inflate(2, 2));
                }
                Tool::Pan => {
                    app.over_form_field = with_form_page(app, p, |f, pg| {
                        f.on_mouse_move(pg, form_mods(), px, py);
                        f.field_at(pg, px, py)
                    });
                    if !app.over_form_field {
                        link = nav::link_at(app, p, px, py);
                    }
                }
                _ => {}
            }
        }
    }
    if hover != app.hover_text || link != app.hover_link {
        app.hover_text = hover;
        app.hover_link = link;
        app.invalidate();
    }
}

impl App {
    /// Like `to_pdf` but clamps the point into the page so drags can leave it.
    fn to_pdf_clamped(&mut self, page: usize, x: i32, y: i32) -> Option<(f64, f64)> {
        let r = self.page_rect(page)?;
        self.to_pdf(page, x.clamp(r.left, r.right - 1), y.clamp(r.top, r.bottom - 1))
    }
}

fn norm(a: (f64, f64), b: (f64, f64)) -> RectF {
    RectF { left: a.0.min(b.0) as f32, right: a.0.max(b.0) as f32, bottom: a.1.min(b.1) as f32, top: a.1.max(b.1) as f32 }
}

fn on_left_up(app: &mut App, x: i32, y: i32) {
    let drag = std::mem::replace(&mut app.drag, Drag::None);
    unsafe { ReleaseCapture() };
    match drag {
        Drag::Pan { .. } => {
            if let Some(target) = app.pending_link.take() {
                nav::follow(app, &target);
            }
        }
        Drag::Select { .. } => {
            nav::end(app);
            if app.tool == Tool::Highlight && !app.selection.is_empty() {
                doc_ops::highlight_selection(app);
            }
        }
        Drag::Redact { page, from, to } => {
            let r = norm(from, to);
            if r.right - r.left > 3.0 && r.top - r.bottom > 3.0 {
                doc_ops::redact(app, page, r);
            }
        }
        Drag::MoveText { page, object, from, to, down, moved, .. } => {
            if moved {
                doc_ops::move_text(app, page, object, to.0 - from.0, to.1 - from.1);
            } else {
                doc_ops::begin_text_edit(app, down.0, down.1);
            }
        }
        Drag::Form { page } => {
            if let Some((px, py)) = app.to_pdf_clamped(page, x, y) {
                if with_form_page(app, page, |f, p| f.on_lbutton_up(p, form_mods(), px, py)) {
                    form_edited_light(app, page);
                }
            }
        }
        _ => {}
    }
    app.invalidate();
}

fn on_right_up(app: &mut App, x: i32, y: i32) {
    if let Some(Hot::Button(i)) = hot_at(app, x, y) {
        match app.buttons[i].cmd {
            Cmd::Tool(Tool::Redact) => {
                let items = [
                    (1, "Blackout — remove and cover with black", if app.redact_clear { 0 } else { MF_CHECKED }),
                    (2, "Erase — remove without a black bar", if app.redact_clear { MF_CHECKED } else { 0 }),
                ];
                match popup_menu(app.hwnd, x, y, &items) {
                    1 => app.redact_clear = false,
                    2 => app.redact_clear = true,
                    _ => return,
                }
                app.tool = Tool::Redact;
                app.invalidate();
            }
            Cmd::ZoomMenu | Cmd::ZoomIn | Cmd::ZoomOut => commands::run(app, Cmd::ZoomMenu),
            _ => {}
        }
        return;
    }
    if app.doc.is_none() {
        return;
    }
    let page = app.thumb_at(x, y).or_else(|| app.page_at(x, y));
    let Some(page) = page else { return };
    if !app.selection.is_empty() && app.page_at(x, y).is_some() {
        match popup_menu(app.hwnd, x, y, &[(1, "Copy\tCtrl+C", 0), (2, "Highlight", 0)]) {
            1 => doc_ops::copy_selection(app),
            2 => doc_ops::highlight_selection(app),
            _ => {}
        }
        return;
    }
    let single = if app.page_count() <= 1 { MF_GRAYED } else { 0 };
    let items = [
        (1, "Rotate clockwise\tR", 0),
        (2, "Rotate counter-clockwise\tShift+R", 0),
        (0, "", 0),
        (3, "Insert blank page before", 0),
        (4, "Insert blank page after", 0),
        (0, "", 0),
        (5, "Delete page…\tDel", single),
    ];
    let cmd = popup_menu(app.hwnd, x, y, &items);
    match cmd {
        1 => doc_ops::rotate(app, page, 1),
        2 => doc_ops::rotate(app, page, 3),
        3 => doc_ops::insert_blank(app, page),
        4 => doc_ops::insert_blank(app, page + 1),
        5 => doc_ops::delete_page(app, page),
        _ => {}
    }
}

fn on_wheel(app: &mut App, delta: i32, horizontal: bool, ctrl: bool, shift: bool, x: i32, y: i32) {
    if app.password.is_some() {
        return;
    }
    if app.doc.is_none() {
        return;
    }
    if ctrl && !horizontal {
        // Touchpads send small deltas: accumulate to whole notches.
        app.wheel_accum += delta;
        while app.wheel_accum.abs() >= 120 {
            let up = app.wheel_accum > 0;
            app.wheel_accum -= 120 * app.wheel_accum.signum();
            app.zoom_step(up, Some((x, y)));
        }
        return;
    }
    if x < app.sidebar_width() && y > app.toolbar_h {
        if app.sidebar_tab == SidebarTab::Outline {
            app.outline_scroll -= delta * app.s(100) / 120;
        } else {
            app.sidebar_scroll -= delta * app.s(100) / 120;
        }
        app.clamp_scroll();
        app.invalidate();
        return;
    }
    let mut lines: u32 = 3;
    unsafe { SystemParametersInfoW(SPI_GETWHEELSCROLLLINES, 0, &mut lines as *mut u32 as *mut c_void, 0) };
    let step = delta * lines.clamp(1, 10) as i32 * app.s(20) / 120;
    if horizontal {
        app.scroll_by(step, 0, false);
    } else if shift {
        app.scroll_by(-step, 0, false);
    } else {
        app.scroll_by(0, -step, true);
    }
}

fn cursor_for(app: &mut App, x: i32, y: i32) -> usize {
    if app.doc.is_none() || y < app.toolbar_h || y >= app.height - app.status_h || x < app.sidebar_width() || hot_at(app, x, y).is_some() {
        return IDC_ARROW;
    }
    if app.search.open && app.search.card.contains(x, y) {
        return IDC_ARROW;
    }
    if let Some(h) = gizmo_hit(app, x, y) {
        return match h {
            Handle::NW | Handle::SE => IDC_SIZENWSE,
            Handle::NE | Handle::SW => IDC_SIZENESW,
            Handle::N | Handle::S => IDC_SIZENS,
            Handle::E | Handle::W => IDC_SIZEWE,
            Handle::Move => IDC_SIZEALL,
        };
    }
    if matches!(app.drag, Drag::Pan { .. } | Drag::MoveText { moved: true, .. }) {
        return IDC_SIZEALL;
    }
    if app.page_at(x, y).is_none() {
        return IDC_ARROW;
    }
    match app.tool {
        Tool::Pan if app.over_form_field => IDC_ARROW,
        Tool::Pan if app.hover_link.is_some() => IDC_HAND,
        Tool::Pan => IDC_ARROW,
        Tool::Select | Tool::Highlight | Tool::Edit => IDC_IBEAM,
        Tool::StampCheck | Tool::StampCross | Tool::Redact => IDC_CROSS,
    }
}

fn on_drop(app: &mut App, hdrop: *mut c_void) {
    let mut buf = [0u16; 1024];
    let n = unsafe { DragQueryFileW(hdrop, 0, buf.as_mut_ptr(), buf.len() as u32) };
    unsafe { DragFinish(hdrop) };
    if n == 0 {
        return;
    }
    let path = PathBuf::from(String::from_utf16_lossy(&buf[..n as usize]));
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let is_image = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "gif" | "tif" | "tiff");
    if is_image && app.doc.is_some() {
        match crate::wic_decoder::decode_image_to_bgra(&path) {
            Ok((w, h, px)) => {
                doc_ops::commit_gizmo(app);
                doc_ops::start_image(app, w, h, px);
            }
            Err(e) => app.toast(format!("Couldn't read image: {e}"), ToastKind::Error),
        }
    } else if doc_ops::confirm_discard(app) {
        doc_ops::open_file(app, &path);
    }
}

// ---------------------------------------------------------------------------
// Image gizmo
// ---------------------------------------------------------------------------

fn gizmo_rect(app: &mut App) -> Option<RECT> {
    let (page, x, y, w, h) = app.gizmo.as_ref().map(|g| (g.page, g.x, g.y, g.w, g.h))?;
    let a = app.to_screen(page, x, y + h)?;
    let b = app.to_screen(page, x + w, y)?;
    Some(RECT::new(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)))
}

fn gizmo_hit(app: &mut App, x: i32, y: i32) -> Option<Handle> {
    let r = gizmo_rect(app)?;
    let grab = app.s(7);
    const ORDER: [Handle; 8] = [Handle::NW, Handle::N, Handle::NE, Handle::W, Handle::E, Handle::SW, Handle::S, Handle::SE];
    for (i, (hx, hy)) in gizmo_handles(r).into_iter().enumerate() {
        if (x - hx).abs() <= grab && (y - hy).abs() <= grab {
            return Some(ORDER[i]);
        }
    }
    r.contains(x, y).then_some(Handle::Move)
}

/// A handle grabbed on screen, expressed as the edge(s) it moves in unrotated PDF space.
/// /Rotate turns the page clockwise, so screen east is PDF north at 90°, west at 180°, south at 270°.
fn screen_to_pdf_handle(h: Handle, rot: i32) -> Handle {
    // Directions as N=0, E=1, S=2, W=3.
    let (v, hz) = match h {
        Handle::Move => return Handle::Move,
        Handle::N => (Some(0), None),
        Handle::S => (Some(2), None),
        Handle::E => (None, Some(1)),
        Handle::W => (None, Some(3)),
        Handle::NE => (Some(0), Some(1)),
        Handle::NW => (Some(0), Some(3)),
        Handle::SE => (Some(2), Some(1)),
        Handle::SW => (Some(2), Some(3)),
    };
    let turn = |d: i32| (d - rot).rem_euclid(4);
    let dirs: Vec<i32> = v.into_iter().chain(hz).map(turn).collect();
    let has = |d: i32| dirs.contains(&d);
    match (has(0), has(2), has(1), has(3)) {
        (true, _, true, _) => Handle::NE,
        (true, _, _, true) => Handle::NW,
        (_, true, true, _) => Handle::SE,
        (_, true, _, true) => Handle::SW,
        (true, ..) => Handle::N,
        (_, true, ..) => Handle::S,
        (_, _, true, _) => Handle::E,
        _ => Handle::W,
    }
}

fn drag_gizmo(app: &mut App, handle: Handle, mouse: (i32, i32), orig: (f64, f64, f64, f64), x: i32, y: i32) {
    let Some(page) = app.gizmo.as_ref().map(|g| g.page) else { return };
    // Measure the drag in PDF space, so it is right on rotated pages too.
    let (Some(a), Some(b)) = (app.to_pdf(page, mouse.0, mouse.1), app.to_pdf(page, x, y)) else { return };
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let rot = app.doc.as_mut().and_then(|d| d.page(page)).map_or(0, |p| p.rotation());
    let handle = screen_to_pdf_handle(handle, rot);
    let (ox, oy, ow, oh) = orig;
    let min = 8.0;
    let (mut nx, mut ny, mut nw, mut nh) = (ox, oy, ow, oh);
    let keep_aspect = key_down(VK_SHIFT) == false && !matches!(handle, Handle::N | Handle::S | Handle::E | Handle::W | Handle::Move);
    match handle {
        Handle::Move => {
            nx = ox + dx;
            ny = oy + dy;
        }
        _ => {
            if matches!(handle, Handle::E | Handle::NE | Handle::SE) {
                nw = (ow + dx).max(min);
            }
            if matches!(handle, Handle::W | Handle::NW | Handle::SW) {
                nw = (ow - dx).max(min);
                nx = ox + ow - nw;
            }
            if matches!(handle, Handle::N | Handle::NE | Handle::NW) {
                nh = (oh + dy).max(min);
            }
            if matches!(handle, Handle::S | Handle::SE | Handle::SW) {
                nh = (oh - dy).max(min);
                ny = oy + oh - nh;
            }
            if keep_aspect {
                // Corner drags keep the image's proportions unless Shift is held.
                let aspect = ow / oh.max(0.01);
                nh = nw / aspect;
                if matches!(handle, Handle::SE | Handle::SW) {
                    ny = oy + oh - nh;
                }
            }
        }
    }
    if let Some(g) = app.gizmo.as_mut() {
        (g.x, g.y, g.w, g.h) = (nx, ny, nw, nh);
    }
    app.invalidate();
}

// ---------------------------------------------------------------------------
// Go to page
// ---------------------------------------------------------------------------

static mut OLD_GOTO_PROC: isize = 0;

unsafe extern "system" fn goto_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let app = super::app_ptr();
    if !app.is_null() {
        match (msg, wparam as i32) {
            (WM_KEYDOWN, VK_RETURN) => {
                end_goto(&mut *app, true);
                return 0;
            }
            (WM_KEYDOWN, VK_ESCAPE) => {
                end_goto(&mut *app, false);
                return 0;
            }
            (WM_CHAR, 0x0D) | (WM_CHAR, 0x1B) => return 0,
            _ => {}
        }
    }
    CallWindowProcW(OLD_GOTO_PROC, hwnd, msg, wparam, lparam)
}

pub fn begin_goto(app: &mut App) {
    if app.doc.is_none() || !app.goto_edit.is_null() || app.page_box.is_empty() {
        return;
    }
    let r = app.page_box.inflate(-app.s(6), -app.s(7));
    let cur = wide(&(app.current_page() + 1).to_string());
    unsafe {
        let class = wide("EDIT");
        let h = CreateWindowExW(0, class.as_ptr(), cur.as_ptr(), WS_CHILD | WS_VISIBLE | ES_NUMBER | ES_CENTER, r.left, r.top, r.width(), r.height(), app.hwnd, ID_GOTO as HMENU, GetModuleHandleW(ptr::null()), ptr::null_mut());
        if h.is_null() {
            return;
        }
        SendMessageW(h, WM_SETFONT, app.gfx.font(super::gfx::Font::Body) as WPARAM, 1);
        OLD_GOTO_PROC = SetWindowLongPtrW(h, GWLP_WNDPROC, goto_proc as *const () as isize);
        SendMessageW(h, EM_SETSEL, 0, -1);
        app.goto_edit = h;
        SetFocus(h);
    }
    app.invalidate();
}

pub fn end_goto(app: &mut App, accept: bool) {
    let h = std::mem::replace(&mut app.goto_edit, ptr::null_mut());
    if h.is_null() {
        return;
    }
    let text = window_text(h);
    unsafe {
        DestroyWindow(h);
        SetFocus(app.hwnd);
    }
    if accept {
        if let Ok(n) = text.trim().parse::<usize>() {
            let n = n.clamp(1, app.page_count().max(1));
            app.go_to_page(n - 1);
        }
    }
    app.invalidate();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_handles_map_to_pdf_edges() {
        // Unrotated: identity.
        for h in [Handle::N, Handle::E, Handle::S, Handle::W, Handle::NE, Handle::SW, Handle::Move] {
            assert_eq!(screen_to_pdf_handle(h, 0), h);
        }
        // /Rotate 90 (clockwise): the PDF's top edge is on the screen's right.
        assert_eq!(screen_to_pdf_handle(Handle::E, 1), Handle::N);
        assert_eq!(screen_to_pdf_handle(Handle::S, 1), Handle::E);
        assert_eq!(screen_to_pdf_handle(Handle::N, 1), Handle::W);
        assert_eq!(screen_to_pdf_handle(Handle::SE, 1), Handle::NE);
        // 180: everything flips.
        assert_eq!(screen_to_pdf_handle(Handle::NE, 2), Handle::SW);
        // 270: the PDF's top edge is on the screen's left.
        assert_eq!(screen_to_pdf_handle(Handle::W, 3), Handle::N);
        assert_eq!(screen_to_pdf_handle(Handle::NW, 3), Handle::NE);
    }
}
