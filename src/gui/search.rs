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

/// EN_CHANGE: wait for typing to pause before searching every page.
pub fn query_changed(app: &mut App) {
    app.search.query = window_text(app.search.edit);
    app.timer(TIMER_SEARCH, 180);
}

pub fn run(app: &mut App) {
    app.kill_timer(TIMER_SEARCH);
    app.search.matches.clear();
    app.search.current = None;
    let q = app.search.query.trim().to_string();
    if q.is_empty() {
        app.invalidate();
        return;
    }
    let start_page = app.current_page();
    let Some(doc) = app.doc.as_ref() else { return };
    for i in 0..doc.page_count() {
        // Load directly rather than through the page cache so a long search doesn't evict it.
        let Ok(page) = doc.pdf.load_page(i) else { continue };
        if let Ok(tp) = page.load_text_page() {
            for rect in tp.search(&q, false) {
                app.search.matches.push(SearchMatch { page: i, rect });
            }
        }
    }
    if !app.search.matches.is_empty() {
        let first = app.search.matches.iter().position(|m| m.page >= start_page).unwrap_or(0);
        select(app, first);
    }
    app.invalidate();
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
    // Enter right after typing: flush the pending debounce first.
    if !app.search.query.trim().is_empty() && app.search.matches.is_empty() {
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
            cv.text(counter_r, "No results", Font::Small, t.danger, DT_RIGHT);
        } else {
            let c = app.search.current.map_or(0, |i| i + 1);
            cv.text(counter_r, &format!("{c} of {}", app.search.matches.len()), Font::Small, t.text_dim, DT_RIGHT);
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
