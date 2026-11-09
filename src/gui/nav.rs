//! Links, outline navigation, back navigation and reading-order text selection.

use super::app::*;
use super::win32::*;
use crate::pdfium::{LinkTarget, RectF};
use std::ptr;

/// Content-space y (pixels) of a point `y_pt` (PDF points from the bottom) on page `idx`.
fn content_y(app: &mut App, idx: usize, y_pt: Option<f32>) -> Option<i32> {
    let l = app.layouts.get(idx)?;
    let top = l.y;
    let Some(y) = y_pt else { return Some(top - app.margin() / 2) };
    let (_, sy) = app.to_screen(idx, 0.0, y as f64)?;
    Some(sy - app.view_rect().top + app.scroll_y - app.s(12))
}

pub fn follow(app: &mut App, target: &LinkTarget) {
    match target {
        LinkTarget::Page { index, y } => {
            if *index >= app.page_count() {
                return;
            }
            app.back_stack.push((app.scroll_x, app.scroll_y));
            if app.back_stack.len() > 50 {
                app.back_stack.remove(0);
            }
            let y = content_y(app, *index, *y).unwrap_or(0);
            app.scroll_to(app.scroll_x, y, true);
            app.toast("Alt+← goes back", ToastKind::Info);
        }
        LinkTarget::Uri(uri) => open_uri(app, uri),
    }
}

/// External links come from the document, so they are confirmed and limited to web/mail schemes.
fn open_uri(app: &mut App, uri: &str) {
    let lower = uri.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:")) {
        app.toast("This link type isn't opened for safety", ToastKind::Info);
        return;
    }
    let shown: String = uri.chars().take(300).collect();
    let msg = format!("Open this link in your browser?\n\n{shown}");
    if message_box(app.hwnd, "Open link", &msg, MB_YESNO | MB_ICONWARNING) == IDYES {
        let (op, target) = (wide("open"), wide(uri));
        unsafe { ShellExecuteW(app.hwnd, op.as_ptr(), target.as_ptr(), ptr::null(), ptr::null(), SW_SHOW) };
    }
}

pub fn go_back(app: &mut App) -> bool {
    match app.back_stack.pop() {
        Some((x, y)) => {
            app.scroll_to(x, y, true);
            true
        }
        None => false,
    }
}

pub fn link_at(app: &mut App, page: usize, x: f64, y: f64) -> Option<LinkTarget> {
    let doc = app.doc.as_mut()?;
    let p = doc.pages.get(&doc.pdf, doc.form.as_ref(), page)?;
    doc.pdf.link_at(p, x, y)
}

pub fn describe(target: &LinkTarget) -> String {
    match target {
        LinkTarget::Page { index, .. } => format!("Go to page {}", index + 1),
        LinkTarget::Uri(u) => u.clone(),
    }
}

// ---------------------------------------------------------------------------
// Text selection in reading order
// ---------------------------------------------------------------------------

fn load_sel(app: &mut App, page: usize) -> bool {
    if app.sel_text.as_ref().is_some_and(|s| s.index == page) {
        return true;
    }
    app.sel_text = None;
    let Some(doc) = app.doc.as_ref() else { return false };
    let Ok(p) = doc.pdf.load_page(page) else { return false };
    let Ok(text) = p.load_text_page() else { return false };
    app.sel_text = Some(SelText { text, _page: p, index: page });
    true
}

/// Character nearest to (x, y), searching a few tolerances so slightly-off clicks still land.
fn char_near(s: &SelText, x: f64, y: f64) -> Option<usize> {
    [2.0, 6.0, 14.0].iter().find_map(|&tol| s.text.get_char_index_at_pos(x, y, tol, tol))
}

/// Starts a selection gesture; returns the anchor character if the press landed on text.
pub fn begin(app: &mut App, page: usize, pt: (f64, f64)) -> Option<usize> {
    if !load_sel(app, page) {
        return None;
    }
    char_near(app.sel_text.as_ref()?, pt.0, pt.1)
}

fn set_range(app: &mut App, page: usize, a: usize, b: usize) {
    let Some(s) = app.sel_text.as_ref() else { return };
    let (start, end) = (a.min(b), a.max(b));
    let count = end - start + 1;
    app.selection.text = s.text.text_range(start, count).replace("\r\n", "\n").trim_end().to_string();
    app.selection.rects = s.text.range_rects(start, count).into_iter().filter(|r| r.width() > 0.0).map(|r| (page, r)).collect();
    app.invalidate();
}

/// Extends the selection from `anchor` to the character under `to`, or falls back to the box
/// between the two points when the drag started off-text.
pub fn update(app: &mut App, page: usize, anchor: Option<usize>, from: (f64, f64), to: (f64, f64)) {
    if !load_sel(app, page) {
        return;
    }
    match anchor {
        Some(a) => {
            let Some(b) = app.sel_text.as_ref().and_then(|s| char_near(s, to.0, to.1)) else { return };
            set_range(app, page, a, b);
        }
        None => {
            let rect = RectF {
                left: from.0.min(to.0) as f32,
                right: from.0.max(to.0) as f32,
                bottom: from.1.min(to.1) as f32,
                top: from.1.max(to.1) as f32,
            };
            let Some(s) = app.sel_text.as_ref() else { return };
            let (text, rects) = s.text.select_text_in_rect(rect);
            app.selection.text = text;
            app.selection.rects = rects.into_iter().map(|r| (page, r)).collect();
            app.invalidate();
        }
    }
}

/// Double-click selects a word, triple-click the line.
pub fn select_unit(app: &mut App, page: usize, pt: (f64, f64), line: bool) -> bool {
    if !load_sel(app, page) {
        return false;
    }
    let Some(s) = app.sel_text.as_ref() else { return false };
    let Some(i) = char_near(s, pt.0, pt.1) else { return false };
    let n = s.text.count_chars();
    let in_unit = |c: char| if line { c != '\r' && c != '\n' } else { c.is_alphanumeric() || c == '_' || c == '\'' || c == '-' };
    if !s.text.get_char_unicode(i).is_some_and(in_unit) {
        return false;
    }
    let mut a = i;
    while a > 0 && s.text.get_char_unicode(a - 1).is_some_and(in_unit) {
        a -= 1;
    }
    let mut b = i;
    while b + 1 < n && s.text.get_char_unicode(b + 1).is_some_and(in_unit) {
        b += 1;
    }
    set_range(app, page, a, b);
    true
}

pub fn end(app: &mut App) {
    app.sel_text = None;
}
