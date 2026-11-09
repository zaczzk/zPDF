//! Painting. Everything draws into a cached back buffer that is blitted once per WM_PAINT.

use super::app::*;
use super::gfx::{icon, Canvas, Font, Gfx};
use super::render::{self, TILE};
use super::win32::*;
use std::ptr;
use std::time::{Duration, Instant};

const SYNC_RENDER_BUDGET: Duration = Duration::from_millis(28);
const TIMER_RENDER_BUDGET: Duration = Duration::from_millis(40);

pub fn paint(app: &mut App, hdc: HDC) {
    let (w, h) = (app.width, app.height);
    if w <= 0 || h <= 0 {
        return;
    }
    unsafe {
        if app.bb_dc.is_null() || app.bb_size != (w, h) {
            if !app.bb_dc.is_null() {
                SelectObject(app.bb_dc, app.bb_old);
                DeleteObject(app.bb_bmp);
                DeleteDC(app.bb_dc);
            }
            app.bb_dc = CreateCompatibleDC(hdc);
            app.bb_bmp = CreateCompatibleBitmap(hdc, w, h);
            app.bb_old = SelectObject(app.bb_dc, app.bb_bmp);
            app.bb_size = (w, h);
        }
    }
    let mut gfx = Gfx::take(&mut app.gfx);
    {
        let mut cv = Canvas::new(app.bb_dc, &mut gfx, app.theme);
        cv.fill(RECT::new(0, 0, w, h), cv.t.canvas);
        if app.doc.is_some() {
            paint_pages(app, &mut cv);
            paint_sidebar(app, &mut cv);
        } else {
            paint_welcome(app, &mut cv);
        }
        app.layout_toolbar(&mut cv);
        app.paint_toolbar(&mut cv);
        super::search::paint(app, &mut cv);
        paint_banner(app, &mut cv);
        paint_password(app, &mut cv);
        paint_status(app, &mut cv);
        paint_toast(app, &mut cv);
        paint_tooltip(app, &mut cv);
    }
    app.gfx = gfx;
    unsafe { BitBlt(hdc, 0, 0, w, h, app.bb_dc, 0, 0, SRCCOPY) };
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

fn tile_range(page: RECT, view: RECT) -> Option<(std::ops::Range<i32>, std::ops::Range<i32>)> {
    let l = view.left.max(page.left) - page.left;
    let r = view.right.min(page.right) - page.left;
    let t = view.top.max(page.top) - page.top;
    let b = view.bottom.min(page.bottom) - page.top;
    if r <= l || b <= t {
        return None;
    }
    Some((l / TILE..(r - 1) / TILE + 1, t / TILE..(b - 1) / TILE + 1))
}

fn render_tile(app: &mut App, page: usize, tx: i32, ty: i32) {
    let (w, h) = (app.layouts[page].w, app.layouts[page].h);
    let Some(doc) = app.doc.as_mut() else { return };
    let Some((p, form)) = doc.page_and_form(page) else { return };
    let (x, y) = (tx * TILE, ty * TILE);
    if let Some(b) = render::render(p, form, x, y, TILE.min(w - x), TILE.min(h - y), w, h) {
        app.cache.put_tile((page, w, tx, ty), b);
    }
}

fn ensure_preview(app: &mut App, page: usize) {
    if app.cache.has_preview(page) {
        return;
    }
    let (w, h) = (app.layouts[page].w, app.layouts[page].h);
    let Some(doc) = app.doc.as_mut() else { return };
    if let Some((p, form)) = doc.page_and_form(page) {
        app.cache.make_preview(page, p, form, w, h);
    }
}

/// Renders missing tiles for the visible pages until `deadline`. Returns true if any remain.
pub fn render_pending(app: &mut App, deadline: Instant) -> bool {
    let view = app.view_rect();
    let mut remaining = false;
    for page in app.visible_pages() {
        let Some(r) = app.page_rect(page) else { continue };
        let Some((xs, ys)) = tile_range(r, view) else { continue };
        let w = app.layouts[page].w;
        for ty in ys.clone() {
            for tx in xs.clone() {
                if app.cache.has_tile((page, w, tx, ty)) {
                    continue;
                }
                if Instant::now() >= deadline {
                    remaining = true;
                    continue;
                }
                render_tile(app, page, tx, ty);
            }
        }
    }
    remaining
}

/// WM_TIMER(TIMER_RENDER): sharpen what's on screen in small slices so input stays responsive.
pub fn on_render_timer(app: &mut App) {
    if app.is_interacting() {
        return; // still zooming; the timer fires again
    }
    app.kill_timer(TIMER_RENDER);
    app.interacting_until = None;
    if render_pending(app, Instant::now() + TIMER_RENDER_BUDGET) {
        app.timer(TIMER_RENDER, 15);
    }
    app.invalidate();
}

fn paint_pages(app: &mut App, cv: &mut Canvas) {
    let t = cv.t;
    let view = app.view_rect();
    let pages = app.visible_pages();
    let interacting = app.is_interacting();
    if !interacting {
        render_pending(app, Instant::now() + SYNC_RENDER_BUDGET);
    }
    let mut missing = false;
    for page in pages.clone() {
        let Some(r) = app.page_rect(page) else { continue };
        cv.shadow(r, cv.s(4));
        cv.fill(r, rgb(255, 255, 255));
        let w = app.layouts[page].w;
        let Some((xs, ys)) = tile_range(r, view) else { continue };
        let complete = ys.clone().all(|ty| xs.clone().all(|tx| app.cache.has_tile((page, w, tx, ty))));
        if !complete {
            missing = true;
            ensure_preview(app, page);
            if let Some(p) = app.cache.preview(page) {
                cv.blit(&p.px, p.w, p.h, r);
            }
        }
        for ty in ys {
            for tx in xs.clone() {
                if let Some(b) = app.cache.tile((page, w, tx, ty)) {
                    cv.blit(&b.px, b.w, b.h, RECT::xywh(r.left + tx * TILE, r.top + ty * TILE, b.w, b.h));
                }
            }
        }
        cv.frame(r, t.page_outline, 1);
        paint_page_overlays(app, cv, page);
    }
    if missing {
        app.timer(TIMER_RENDER, if interacting { 170 } else { 15 });
    }

    if let Some(r) = app.hover_text {
        cv.blend(r, t.accent, 28);
        cv.frame(r, t.accent, 1);
    }
    if let Some(ed) = &app.editor {
        cv.frame(ed.rect.inflate(2, 2), t.accent, cv.s(2));
    }
    for (bar, hot) in [(app.vbar(), Hot::VScroll), (app.hbar(), Hot::HScroll)] {
        if let Some((_, thumb)) = bar {
            let on = app.hot == Some(hot) || matches!(app.drag, Drag::VThumb { .. } | Drag::HThumb { .. });
            cv.round(thumb, thumb.width().min(thumb.height()) / 2, if on { t.text_dim } else { t.scrollbar }, None);
        }
    }
}

fn paint_page_overlays(app: &mut App, cv: &mut Canvas, page: usize) {
    let t = cv.t;
    // Search hits
    if app.search.open {
        let hits: Vec<(usize, crate::pdfium::RectF)> = app.search.matches.iter().enumerate().filter(|(_, m)| m.page == page).map(|(i, m)| (i, m.rect)).collect();
        for (i, rc) in hits {
            if let Some(r) = app.rect_to_screen(page, rc) {
                let r = r.inflate(1, 1);
                if app.search.current == Some(i) {
                    cv.blend(r, t.accent, 110);
                    cv.frame(r.inflate(1, 1), t.accent, cv.s(2));
                } else {
                    cv.blend(r, t.search_hit, 110);
                }
            }
        }
    }
    // Text selection
    let sel: Vec<_> = app.selection.rects.iter().filter(|(p, _)| *p == page).map(|(_, r)| *r).collect();
    for rc in sel {
        if let Some(r) = app.rect_to_screen(page, rc) {
            cv.blend(r, t.selection, 80);
        }
    }
    // Drag rectangles
    let drag = match app.drag {
        Drag::Select { page: p, from, to, anchor: None } if p == page => Some((from, to, false)),
        Drag::Redact { page: p, from, to } if p == page => Some((from, to, true)),
        _ => None,
    };
    if let Some((a, b, redact)) = drag {
        let (Some(p1), Some(p2)) = (app.to_screen(page, a.0, a.1), app.to_screen(page, b.0, b.1)) else { return };
        let r = RECT::new(p1.0.min(p2.0), p1.1.min(p2.1), p1.0.max(p2.0), p1.1.max(p2.1));
        if redact {
            cv.blend(r, if app.redact_clear { t.danger } else { rgb(0, 0, 0) }, 70);
            cv.frame(r, t.danger, cv.s(2));
        } else if app.selection.is_empty() {
            cv.outline(r, t.accent, 1, true);
        }
    }
    // Image placement gizmo
    if let Some(g) = &app.gizmo {
        if g.page == page {
            let (x, y, w, h, gw, gh) = (g.x, g.y, g.w, g.h, g.px_w as i32, g.px_h as i32);
            let (Some(a), Some(b)) = (app.to_screen(page, x, y + h), app.to_screen(page, x + w, y)) else { return };
            let r = RECT::new(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
            if let Some(g) = &app.gizmo {
                cv.blit(&g.pixels, gw, gh, r);
            }
            cv.frame(r, t.accent, cv.s(2));
            let hs = cv.s(4);
            for (hx, hy) in gizmo_handles(r) {
                let hr = RECT::new(hx - hs, hy - hs, hx + hs, hy + hs);
                cv.fill(hr, rgb(255, 255, 255));
                cv.frame(hr, t.accent, 1);
            }
        }
    }
}

pub fn gizmo_handles(r: RECT) -> [(i32, i32); 8] {
    let (cx, cy) = r.center();
    [(r.left, r.top), (cx, r.top), (r.right, r.top), (r.left, cy), (r.right, cy), (r.left, r.bottom), (cx, r.bottom), (r.right, r.bottom)]
}

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

fn paint_sidebar(app: &mut App, cv: &mut Canvas) {
    let sw = app.sidebar_width();
    if sw == 0 {
        return;
    }
    let t = cv.t;
    let top = app.toolbar_h;
    let bottom = app.height - app.status_h;
    cv.fill(RECT::new(0, top, sw, bottom), t.sidebar);
    cv.fill(RECT::new(sw - 1, top, sw, bottom), t.border);

    let has_outline = app.doc.as_ref().is_some_and(|d| !d.outline.is_empty());
    if !has_outline {
        app.sidebar_tab = SidebarTab::Pages;
    }
    if app.sidebar_tab == SidebarTab::Outline {
        paint_outline(app, cv);
    } else {
        paint_thumbs(app, cv);
    }

    // Header drawn last so the list scrolls underneath it.
    let header = RECT::new(0, top, sw - 1, top + cv.s(36));
    cv.fill(header, t.sidebar);
    if has_outline {
        let half = (sw - cv.s(24)) / 2;
        app.tab_pages = RECT::xywh(cv.s(12), top + cv.s(6), half, cv.s(26));
        app.tab_outline = RECT::xywh(cv.s(12) + half, top + cv.s(6), half, cv.s(26));
        for (r, label, tab, hot) in [
            (app.tab_pages, "Pages", SidebarTab::Pages, Hot::TabPages),
            (app.tab_outline, "Outline", SidebarTab::Outline, Hot::TabOutline),
        ] {
            let on = app.sidebar_tab == tab;
            if on {
                cv.round(r, cv.s(5), t.accent_soft, None);
            } else if app.hot == Some(hot) {
                cv.round(r, cv.s(5), t.hover, None);
            }
            cv.text(r, label, if on { Font::BodyBold } else { Font::Body }, if on { t.accent } else { t.text_dim }, DT_CENTER);
        }
    } else {
        app.tab_pages = RECT::default();
        app.tab_outline = RECT::default();
        let n = app.page_count();
        cv.text(header.offset(cv.s(16), 0), &format!("{n} page{}", if n == 1 { "" } else { "s" }), Font::Small, t.text_faint, DT_LEFT);
    }
}

fn paint_outline(app: &mut App, cv: &mut Canvas) {
    let t = cv.t;
    let sw = app.sidebar_width();
    let top = app.toolbar_h + cv.s(40);
    let bottom = app.height - app.status_h;
    let row_h = cv.s(30);
    let current = app.current_page();
    // The section the reader is in: the last outline entry pointing at or before this page.
    let items: Vec<(String, usize, Option<usize>)> = app
        .doc
        .as_ref()
        .map(|d| {
            d.outline
                .iter()
                .map(|o| {
                    let page = match &o.target {
                        Some(crate::pdfium::LinkTarget::Page { index, .. }) => Some(*index),
                        _ => None,
                    };
                    (o.title.clone(), o.depth, page)
                })
                .collect()
        })
        .unwrap_or_default();
    let active = items.iter().rposition(|(_, _, p)| p.is_some_and(|p| p <= current));
    app.outline_rows.clear();
    for (i, (title, depth, page)) in items.iter().enumerate() {
        let y = top + i as i32 * row_h - app.outline_scroll;
        let row = RECT::xywh(cv.s(8), y, sw - cv.s(16), row_h - cv.s(2));
        app.outline_rows.push(row);
        if row.bottom < top || row.top > bottom {
            continue;
        }
        let on = active == Some(i);
        if on {
            cv.round(row, cv.s(5), t.accent_soft, None);
        } else if app.hot == Some(Hot::OutlineRow(i)) {
            cv.round(row, cv.s(5), t.hover, None);
        }
        let indent = cv.s(10) + (*depth as i32).min(6) * cv.s(12);
        let num_w = cv.s(30);
        let fg = if on { t.accent } else if *depth == 0 { t.text } else { t.text_dim };
        cv.text(RECT::new(row.left + indent, row.top, row.right - num_w, row.bottom), if title.is_empty() { "(untitled)" } else { title }, if *depth == 0 { Font::BodyBold } else { Font::Body }, fg, DT_LEFT);
        if let Some(p) = page {
            cv.text(RECT::new(row.right - num_w, row.top, row.right - cv.s(6), row.bottom), &(p + 1).to_string(), Font::Small, t.text_faint, DT_RIGHT);
        }
    }
}

fn paint_thumbs(app: &mut App, cv: &mut Canvas) {
    let t = cv.t;
    let sw = app.sidebar_width();
    let top = app.toolbar_h;
    let bottom = app.height - app.status_h;
    let current = app.current_page();
    let tw_req = app.thumb_width();
    let deadline = Instant::now() + Duration::from_millis(12);
    let mut pending = false;
    for i in 0..app.thumb_slots.len() {
        let Some(r) = app.thumb_rect(i) else { continue };
        if r.bottom + cv.s(24) < top || r.top > bottom {
            continue;
        }
        let active = i == current;
        if active {
            cv.round(r.inflate(cv.s(6), cv.s(6)).offset(0, 0), cv.s(6), t.accent_soft, None);
        }
        cv.fill(r, rgb(255, 255, 255));
        if app.cache.thumb(i, tw_req).is_none() {
            if Instant::now() < deadline {
                // Load outside the page cache so scrolling thumbnails never evicts the pages on
                // screen (or the one holding form focus).
                let Some(doc) = app.doc.as_ref() else { break };
                let (w, h) = (r.width(), r.height());
                if let Some(Some(b)) = render::with_temp_page(&doc.pdf, doc.form.as_ref(), i, |p| render::render(p, doc.form.as_ref(), 0, 0, w, h, w, h)) {
                    app.cache.put_thumb(i, tw_req, b);
                }
            } else {
                pending = true;
            }
        }
        if let Some(b) = app.cache.thumb(i, tw_req) {
            cv.blit(&b.px, b.w, b.h, r);
        }
        cv.frame(r, if active { t.accent } else { t.page_outline }, if active { cv.s(2) } else { 1 });
        let label = RECT::new(0, r.bottom + cv.s(4), sw, r.bottom + cv.s(22));
        cv.text(label, &(i + 1).to_string(), if active { Font::BodyBold } else { Font::Small }, if active { t.accent } else { t.text_dim }, DT_CENTER);
    }
    if pending {
        app.timer(TIMER_RENDER, 30);
    }
}

// ---------------------------------------------------------------------------
// Chrome overlays
// ---------------------------------------------------------------------------

fn paint_status(app: &mut App, cv: &mut Canvas) {
    let t = cv.t;
    let r = RECT::new(0, app.height - app.status_h, app.width, app.height);
    cv.fill(r, t.chrome);
    cv.fill(RECT::new(0, r.top, app.width, r.top + 1), t.border);
    let pad = cv.s(14);
    let inner = RECT::new(r.left + pad, r.top, r.right - pad, r.bottom);
    if app.doc.is_none() {
        cv.text(inner, "Drop a PDF anywhere to open it", Font::Small, t.text_faint, DT_LEFT);
        return;
    }
    let fit = match app.fit {
        Fit::Width => "Fit width · ",
        Fit::Page => "Fit page · ",
        Fit::None => "",
    };
    let right = format!("{fit}{}%", app.zoom.round() as i32);
    let rw = cv.measure(&right, Font::Small) + cv.s(8);
    cv.text(inner, &right, Font::Small, t.text_dim, DT_RIGHT);
    let link;
    let hint = if let Some(l) = &app.hover_link {
        link = super::nav::describe(l);
        link.as_str()
    } else if app.gizmo.is_some() {
        "Drag the image or its handles · Enter places it · Esc cancels"
    } else if !app.selection.is_empty() {
        "Ctrl+C copies · right-click to highlight"
    } else {
        app.tool.hint(app.redact_clear)
    };
    let color = if app.hover_link.is_some() { t.accent } else { t.text_dim };
    cv.text(RECT::new(inner.left, r.top, inner.right - rw - cv.s(16), r.bottom), hint, Font::Small, color, DT_LEFT);
}

fn paint_toast(app: &mut App, cv: &mut Canvas) {
    let Some((msg, kind)) = app.toast.clone() else { return };
    let t = cv.t;
    let w = cv.measure(&msg, Font::Body) + cv.s(48);
    let h = cv.s(36);
    // Sit above the default-app banner when it is showing.
    let lift = if app.banner.is_some() && app.doc.is_some() { cv.s(56) } else { 0 };
    let r = RECT::xywh((app.width + app.sidebar_width() - w) / 2, app.height - app.status_h - h - cv.s(18) - lift, w, h);
    let (bg, fg, glyph) = match kind {
        ToastKind::Success => (t.card, t.success, icon::CHECK),
        ToastKind::Error => (t.card, t.danger, icon::CROSS),
        ToastKind::Info => (t.card, t.accent, '\u{E946}'),
    };
    cv.shadow(r, cv.s(6));
    cv.round(r, h / 2, bg, Some(t.border));
    cv.icon(RECT::xywh(r.left + cv.s(10), r.top, cv.s(20), h), glyph, Font::IconSmall, fg);
    cv.text(RECT::new(r.left + cv.s(34), r.top, r.right - cv.s(12), r.bottom), &msg, Font::Body, t.text, DT_LEFT);
}

fn paint_tooltip(app: &mut App, cv: &mut Canvas) {
    let Some(hot) = app.tooltip else { return };
    let (text, anchor) = match hot {
        Hot::Button(i) => match app.buttons.get(i) {
            Some(b) => (b.cmd.tip(), b.rect),
            None => return,
        },
        Hot::PageBox => ("Go to page  (Ctrl+G)", app.page_box),
        Hot::SearchPrev => ("Previous match  (Shift+Enter)", app.search.prev),
        Hot::SearchNext => ("Next match  (Enter)", app.search.next),
        Hot::SearchClose => ("Close  (Esc)", app.search.close),
        _ => return,
    };
    if text.is_empty() {
        return;
    }
    let t = cv.t;
    let w = cv.measure(text, Font::Small) + cv.s(20);
    let h = cv.s(26);
    let x = (anchor.center().0 - w / 2).clamp(cv.s(4), app.width - w - cv.s(4));
    let r = RECT::xywh(x, anchor.bottom + cv.s(6), w, h);
    cv.shadow(r, cv.s(3));
    cv.round(r, cv.s(4), t.card, Some(t.border));
    cv.text(r, text, Font::Small, t.text, DT_CENTER);
}

fn paint_banner(app: &mut App, cv: &mut Canvas) {
    if app.banner.is_none() || app.doc.is_none() {
        return;
    }
    let t = cv.t;
    let s = cv.g.scaler();
    let v = app.view_rect();
    let msg = "Make zPDF your default PDF app?";
    let yes_w = cv.measure("Set as default", Font::BodyBold) + s(24);
    let no_w = cv.measure("No thanks", Font::Body) + s(20);
    let w = (cv.measure(msg, Font::Body) + yes_w + no_w + s(56)).min(v.width() - s(24));
    let h = s(44);
    let card = RECT::xywh(v.left + (v.width() - w) / 2, v.bottom - h - s(18), w, h);
    let bh = s(28);
    let yes = RECT::xywh(card.right - s(8) - no_w - s(6) - yes_w, card.top + (h - bh) / 2, yes_w, bh);
    let never = RECT::xywh(card.right - s(8) - no_w, yes.top, no_w, bh);
    cv.shadow(card, s(6));
    cv.round(card, s(8), t.card, Some(t.border));
    cv.text(RECT::new(card.left + s(16), card.top, yes.left - s(8), card.bottom), msg, Font::Body, t.text, DT_LEFT);
    cv.round(yes, s(5), if app.hot == Some(Hot::BannerYes) { t.selection } else { t.accent }, None);
    cv.text(yes, "Set as default", Font::BodyBold, t.on_accent, DT_CENTER);
    if app.hot == Some(Hot::BannerNever) {
        cv.round(never, s(5), t.hover, None);
    }
    cv.text(never, "No thanks", Font::Body, t.text_dim, DT_CENTER);
    if let Some(b) = &mut app.banner {
        b.card = card;
        b.yes = yes;
        b.never = never;
    }
}

fn paint_password(app: &mut App, cv: &mut Canvas) {
    let Some(p) = &app.password else { return };
    let t = cv.t;
    let s = cv.g.scaler();
    let (name, wrong, edit) = (p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), p.wrong, p.edit);
    // Dim everything behind the card.
    cv.blend(RECT::new(0, app.toolbar_h, app.width, app.height), rgb(0, 0, 0), if t.dark { 110 } else { 70 });
    let w = s(400).min(app.width - s(32));
    let h = s(200);
    let card = RECT::xywh((app.width - w) / 2, app.toolbar_h + ((app.height - app.toolbar_h - h) / 2).max(s(12)), w, h);
    cv.shadow(card, s(8));
    cv.round(card, s(10), t.card, Some(t.border));
    cv.icon(RECT::xywh(card.left + s(20), card.top + s(18), s(24), s(28)), '\u{E72E}', Font::Icon, t.accent);
    cv.text(RECT::new(card.left + s(50), card.top + s(18), card.right - s(20), card.top + s(46)), "This PDF is password protected", Font::BodyBold, t.text, DT_LEFT);
    cv.text(RECT::new(card.left + s(50), card.top + s(44), card.right - s(20), card.top + s(66)), &name, Font::Small, t.text_dim, DT_LEFT);
    let field = RECT::new(card.left + s(20), card.top + s(80), card.right - s(20), card.top + s(112));
    cv.round(field, s(5), t.card, Some(if wrong { t.danger } else { t.accent }));
    unsafe { MoveWindow(edit, field.left + s(10), field.top + s(7), field.width() - s(20), field.height() - s(14), 1) };
    if wrong {
        cv.text(RECT::new(field.left, field.bottom + s(4), field.right, field.bottom + s(24)), "That password didn't work. Try again.", Font::Small, t.danger, DT_LEFT);
    }
    let bw = s(96);
    let ok = RECT::xywh(card.right - s(20) - bw, card.bottom - s(50), bw, s(32));
    let cancel = ok.offset(-bw - s(8), 0);
    cv.round(ok, s(5), if app.hot == Some(Hot::PasswordOk) { t.selection } else { t.accent }, None);
    cv.text(ok, "Open", Font::BodyBold, t.on_accent, DT_CENTER);
    cv.round(cancel, s(5), if app.hot == Some(Hot::PasswordCancel) { t.hover } else { t.card }, Some(t.border));
    cv.text(cancel, "Cancel", Font::Body, t.text, DT_CENTER);
    if let Some(p) = &mut app.password {
        p.card = card;
        p.ok = ok;
        p.cancel = cancel;
    }
}

fn paint_welcome(app: &mut App, cv: &mut Canvas) {
    let t = cv.t;
    let s = cv.g.scaler();
    let area = RECT::new(0, app.toolbar_h, app.width, app.height - app.status_h);
    let recent: Vec<_> = app.recent.iter().take(6).cloned().collect();
    let card_w = s(440).min(area.width() - s(32));
    let list_h = if recent.is_empty() { 0 } else { s(36) + recent.len() as i32 * s(44) };
    let card_h = s(236) + list_h;
    let card = RECT::xywh((app.width - card_w) / 2, area.top + ((area.height() - card_h) / 2).max(s(16)), card_w, card_h);
    cv.shadow(card, s(8));
    cv.round(card, s(12), t.card, Some(t.border));

    let isz = s(48);
    let icon_r = RECT::xywh(card.left + (card_w - isz) / 2, card.top + s(28), isz, isz);
    if app.icon.is_null() {
        cv.icon(icon_r, icon::DOCUMENT, Font::IconLarge, t.accent);
    } else {
        unsafe { DrawIconEx(cv.hdc, icon_r.left, icon_r.top, app.icon, isz, isz, 0, ptr::null_mut(), 3) };
    }
    cv.text(RECT::xywh(card.left, card.top + s(86), card_w, s(32)), "Open a PDF to get started", Font::Title, t.text, DT_CENTER);
    cv.text(RECT::xywh(card.left, card.top + s(118), card_w, s(22)), "Drop a file anywhere in this window, or", Font::Body, t.text_dim, DT_CENTER);

    let bw = s(148);
    let btn = RECT::xywh(card.left + (card_w - bw) / 2, card.top + s(152), bw, s(36));
    app.welcome_open = btn;
    let hot = app.hot == Some(Hot::WelcomeOpen);
    cv.round(btn, s(6), if hot { t.selection } else { t.accent }, None);
    cv.icon(RECT::xywh(btn.left + s(14), btn.top, s(20), btn.height()), icon::OPEN, Font::IconSmall, t.on_accent);
    cv.text(RECT::new(btn.left + s(38), btn.top, btn.right - s(10), btn.bottom), "Open file…", Font::BodyBold, t.on_accent, DT_LEFT);
    cv.text(RECT::xywh(card.left, btn.bottom + s(6), card_w, s(20)), "Ctrl+O", Font::Small, t.text_faint, DT_CENTER);

    app.recent_rects.clear();
    if !recent.is_empty() {
        let mut y = card.top + s(236);
        cv.fill(RECT::new(card.left + s(20), y - s(12), card.right - s(20), y - s(11)), t.border);
        cv.text(RECT::xywh(card.left + s(24), y, card_w, s(24)), "Recent", Font::BodyBold, t.text_dim, DT_LEFT);
        y += s(32);
        for (i, path) in recent.iter().enumerate() {
            let row = RECT::xywh(card.left + s(12), y, card_w - s(24), s(42));
            if app.hot == Some(Hot::Recent(i)) {
                cv.round(row, s(6), t.hover, None);
            }
            cv.icon(RECT::xywh(row.left + s(8), row.top, s(24), row.height()), icon::DOCUMENT, Font::Icon, t.accent);
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let dir = path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            cv.text(RECT::new(row.left + s(42), row.top + s(3), row.right - s(8), row.top + s(22)), &name, Font::Body, t.text, DT_LEFT);
            cv.text(RECT::new(row.left + s(42), row.top + s(21), row.right - s(8), row.bottom - s(3)), &dir, Font::Small, t.text_faint, DT_LEFT);
            app.recent_rects.push(row);
            y += s(44);
        }
    }
}
