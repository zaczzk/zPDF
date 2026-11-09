//! Find bar: a floating card with an EDIT control, debounced search and match navigation.

use super::app::*;
use super::gfx::{icon, Canvas, Font};
use super::win32::*;
use std::ptr;

static mut OLD_PROC: isize = 0;

unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let app = super::app_ptr();
    match msg {
        WM_KEYDOWN if !app.is_null() => match wparam as i32 {
            VK_RETURN => {
                if key_down(VK_SHIFT) { prev(&mut *app) } else { next(&mut *app) }
                return 0;
            }
            VK_ESCAPE => {
                close(&mut *app);
                return 0;
            }
            0x46 if key_down(VK_CONTROL) => {
                SendMessageW(hwnd, EM_SETSEL, 0, -1);
                return 0;
            }
            _ => {}
        },
        WM_CHAR if wparam == 0x0D || wparam == 0x1B || wparam == 0x06 => return 0,
        _ => {}
    }
    CallWindowProcW(OLD_PROC, hwnd, msg, wparam, lparam)
}

pub fn open(app: &mut App) {
    if app.doc.is_none() {
        return;
    }
    app.search.open = true;
    unsafe {
        if app.search.edit.is_null() {
            let class = wide("EDIT");
            let text = wide(&app.search.query);
            let h = CreateWindowExW(0, class.as_ptr(), text.as_ptr(), WS_CHILD | ES_AUTOHSCROLL, 0, 0, 0, 0, app.hwnd, ID_SEARCH as HMENU, GetModuleHandleW(ptr::null()), ptr::null_mut());
            if h.is_null() {
                return;
            }
            OLD_PROC = SetWindowLongPtrW(h, GWLP_WNDPROC, edit_proc as *const () as isize);
            app.search.edit = h;
        }
        SendMessageW(app.search.edit, WM_SETFONT, app.gfx.font(Font::Body) as WPARAM, 1);
        layout(app);
        ShowWindow(app.search.edit, SW_SHOW);
        SetFocus(app.search.edit);
        SendMessageW(app.search.edit, EM_SETSEL, 0, -1);
    }
    app.invalidate();
}

pub fn close(app: &mut App) {
    app.search.open = false;
    app.search.pending.clear();
    app.search.restart = false;
    app.search.matches.clear();
    app.search.current = None;
    app.kill_timer(TIMER_SEARCH);
    unsafe {
        if !app.search.edit.is_null() {
            ShowWindow(app.search.edit, SW_HIDE);
        }
        SetFocus(app.hwnd);
    }
    app.invalidate();
}

pub fn layout(app: &mut App) {
    if !app.search.open {
        return;
    }
    let s = app.gfx.scaler();
    let v = app.view_rect();
    let w = s(380).min(v.width() - s(24));
    let h = s(40);
    let card = RECT::xywh(v.right - w - s(16), v.top + s(10), w, h);
    let b = s(28);
    let by = card.top + (h - b) / 2;
    app.search.close = RECT::xywh(card.right - s(6) - b, by, b, b);
    app.search.next = app.search.close.offset(-b - s(2), 0);
    app.search.prev = app.search.next.offset(-b - s(2), 0);
    app.search.card = card;
    let edit_l = card.left + s(36);
    let edit_r = app.search.prev.left - s(84);
    let eh = s(20);
    unsafe {
        if !app.search.edit.is_null() {
            MoveWindow(app.search.edit, edit_l, card.top + (h - eh) / 2, (edit_r - edit_l).max(s(40)), eh, 1);
        }
    }
}

/// EN_CHANGE: wait for typing to pause before searching.
pub fn query_changed(app: &mut App) {
    app.search.query = window_text(app.search.edit);
    app.search.restart = true;
    app.timer(TIMER_SEARCH, 180);
}

/// Starts a fresh search from the current page onwards, wrapping round to the start.
pub fn run(app: &mut App) {
    app.search.restart = false;
    app.search.matches.clear();
    app.search.current = None;
    app.search.jumped = false;
    app.search.pending.clear();
    let q = app.search.query.trim().to_string();
    app.search.running = q.clone();
    if q.is_empty() || app.doc.is_none() {
        app.kill_timer(TIMER_SEARCH);
        app.invalidate();
        return;
    }
    let n = app.page_count();
    let start = app.current_page().min(n.saturating_sub(1));
    // Popped from the back: current page first, then onwards, then wrap.
    app.search.pending = (start..n).chain(0..start).rev().collect();
    step(app);
}

/// Searches pages for a short time slice; reschedules itself until every page is done, so long
/// documents never freeze the window.
pub fn step(app: &mut App) {
    if app.search.restart {
        return run(app);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(30);
    let q = app.search.running.clone();
    while std::time::Instant::now() < deadline {
        let Some(page) = app.search.pending.pop() else { break };
        let Some(doc) = app.doc.as_ref() else { return };
        // Load directly rather than through the page cache so a long search doesn't evict it.
        let Ok(p) = doc.pdf.load_page(page) else { continue };
        let Ok(tp) = p.load_text_page() else { continue };
        let found: Vec<_> = tp.search(&q, false).into_iter().map(|rect| SearchMatch { page, rect }).collect();
        if found.is_empty() {
            continue;
        }
        // Keep matches in page order; shift the current index if we insert before it.
        let at = app.search.matches.partition_point(|m| m.page < page);
        let count = found.len();
        app.search.matches.splice(at..at, found);
        if let Some(c) = app.search.current.as_mut() {
            if at <= *c {
                *c += count;
            }
        }
        if !app.search.jumped {
            app.search.jumped = true;
            select(app, at);
        }
    }
    if app.search.pending.is_empty() {
        app.kill_timer(TIMER_SEARCH);
    } else {
        app.timer(TIMER_SEARCH, 1);
    }
    app.invalidate();
}

pub fn searching(app: &App) -> bool {
    !app.search.pending.is_empty()
}

fn select(app: &mut App, idx: usize) {
    app.search.current = Some(idx);
    let (page, rect) = {
        let m = &app.search.matches[idx];
        (m.page, m.rect)
    };
    // Scroll the match into the middle of the view if it isn't comfortably visible.
    if let Some(r) = app.rect_to_screen(page, rect) {
        let v = app.view_rect();
        let inner = v.inflate(-app.s(40), -app.s(60));
        if !inner.contains(r.left, r.top) || !inner.contains(r.right - 1, r.bottom - 1) {
            let dy = r.top - (v.top + v.height() / 3);
            let dx = if r.left < v.left || r.right > v.right { r.left - (v.left + v.width() / 3) } else { 0 };
            app.scroll_to(app.scroll_x + dx, app.scroll_y + dy, true);
        }
    }
    app.invalidate();
}

pub fn next(app: &mut App) {
    // Enter right after typing: start now instead of waiting for the debounce.
    if app.search.restart {
        run(app);
        return;
    }
    let n = app.search.matches.len();
    if n > 0 {
        select(app, app.search.current.map_or(0, |c| (c + 1) % n));
    }
}

pub fn prev(app: &mut App) {
    let n = app.search.matches.len();
    if n > 0 {
        select(app, app.search.current.map_or(n - 1, |c| (c + n - 1) % n));
    }
}

pub fn paint(app: &mut App, cv: &mut Canvas) {
    if !app.search.open {
        return;
    }
    layout(app);
    let t = cv.t;
    let s = app.search.card;
    cv.shadow(s, cv.s(6));
    cv.round(s, cv.s(8), t.card, Some(t.border));
    cv.icon(RECT::xywh(s.left + cv.s(8), s.top, cv.s(24), s.height()), icon::SEARCH, Font::IconSmall, t.text_dim);
    let counter_r = RECT::new(app.search.prev.left - cv.s(82), s.top, app.search.prev.left - cv.s(6), s.bottom);
    if !app.search.query.trim().is_empty() {
        if app.search.matches.is_empty() {
            if !searching(app) {
                cv.text(counter_r, "No results", Font::Small, t.danger, DT_RIGHT);
            } else {
                cv.text(counter_r, "Searching…", Font::Small, t.text_dim, DT_RIGHT);
            }
        } else {
            let c = app.search.current.map_or(0, |i| i + 1);
            let more = if searching(app) { "…" } else { "" };
            cv.text(counter_r, &format!("{c} of {}{more}", app.search.matches.len()), Font::Small, t.text_dim, DT_RIGHT);
        }
    }
    for (r, g, hot) in [
        (app.search.prev, icon::UP, Hot::SearchPrev),
        (app.search.next, icon::DOWN, Hot::SearchNext),
        (app.search.close, icon::CLOSE, Hot::SearchClose),
    ] {
        if app.hot == Some(hot) {
            cv.round(r, cv.s(4), t.hover, None);
        }
        cv.icon(r, g, Font::IconSmall, t.text);
    }
}
