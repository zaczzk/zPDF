//! Page layout, coordinate transforms, scrolling and zoom.

use super::app::*;
use super::win32::RECT;
use crate::pdfium::RectF;
use std::time::{Duration, Instant};

pub const MIN_ZOOM: f64 = 10.0;
pub const MAX_ZOOM: f64 = 800.0;
const ZOOM_STEPS: [f64; 19] = [10.0, 25.0, 33.0, 50.0, 67.0, 75.0, 90.0, 100.0, 110.0, 125.0, 150.0, 175.0, 200.0, 250.0, 300.0, 400.0, 500.0, 600.0, 800.0];

impl App {
    pub fn margin(&self) -> i32 {
        self.s(20)
    }

    pub fn page_gap(&self) -> i32 {
        self.s(14)
    }

    pub fn sidebar_width(&self) -> i32 {
        if self.sidebar_visible && self.doc.is_some() {
            self.sidebar_w
        } else {
            0
        }
    }

    /// The scrollable document area in client coordinates.
    pub fn view_rect(&self) -> RECT {
        RECT::new(self.sidebar_width(), self.toolbar_h, self.width, self.height - self.status_h)
    }

    fn px_per_pt(&self) -> f64 {
        self.zoom / 100.0 * self.gfx.dpi as f64 / 72.0
    }

    pub fn relayout(&mut self) {
        if self.fit != Fit::None && !self.layouts.is_empty() {
            self.zoom = self.fit_zoom(self.fit);
        }
        self.layouts.clear();
        let Some(doc) = &self.doc else {
            self.content_w = 0;
            self.content_h = 0;
            self.scroll_x = 0;
            self.scroll_y = 0;
            return;
        };
        let scale = self.px_per_pt();
        let (margin, gap) = (self.margin(), self.page_gap());
        let mut y = margin;
        let mut max_w = 0;
        for i in 0..doc.page_count() {
            let (w_pt, h_pt) = doc.pdf.page_size(i).map(|(w, h)| (w as f64, h as f64)).unwrap_or((612.0, 792.0));
            let w = (w_pt * scale).round().max(1.0) as i32;
            let h = (h_pt * scale).round().max(1.0) as i32;
            self.layouts.push(PageLayout { w_pt, h_pt, w, h, y });
            y += h + gap;
            max_w = max_w.max(w);
        }
        self.content_h = y - gap + margin;
        self.content_w = max_w + 2 * margin;
        self.layout_thumbs();
        self.clamp_scroll();
    }

    fn fit_zoom(&self, fit: Fit) -> f64 {
        let Some(doc) = &self.doc else { return self.zoom };
        let page = self.current_page().min(doc.page_count().saturating_sub(1));
        let (w_pt, h_pt) = doc.pdf.page_size(page).map(|(w, h)| (w as f64, h as f64)).unwrap_or((612.0, 792.0));
        let v = self.view_rect();
        let m = 2.0 * self.margin() as f64;
        let base = self.gfx.dpi as f64 / 72.0 / 100.0;
        let zw = (v.width() as f64 - m).max(50.0) / (w_pt * base);
        let zh = (v.height() as f64 - m).max(50.0) / (h_pt * base);
        let z = if fit == Fit::Width { zw } else { zw.min(zh) };
        z.clamp(MIN_ZOOM, MAX_ZOOM)
    }

    pub fn max_scroll(&self) -> (i32, i32) {
        let v = self.view_rect();
        ((self.content_w - v.width()).max(0), (self.content_h - v.height()).max(0))
    }

    pub fn clamp_scroll(&mut self) {
        let (mx, my) = self.max_scroll();
        self.scroll_x = self.scroll_x.clamp(0, mx);
        self.scroll_y = self.scroll_y.clamp(0, my);
        self.target_y = self.target_y.clamp(0.0, my as f64);
        let max_sb = (self.thumb_slots.last().map_or(0, |r| r.bottom + self.s(12)) - (self.height - self.status_h - self.toolbar_h)).max(0);
        self.sidebar_scroll = self.sidebar_scroll.clamp(0, max_sb);
        let rows = self.doc.as_ref().map_or(0, |d| d.outline.len()) as i32;
        let max_ol = (rows * self.s(30) + self.s(52) - (self.height - self.status_h - self.toolbar_h)).max(0);
        self.outline_scroll = self.outline_scroll.clamp(0, max_ol);
    }

    /// On-screen rectangle of a page.
    pub fn page_rect(&self, idx: usize) -> Option<RECT> {
        let l = self.layouts.get(idx)?;
        let v = self.view_rect();
        let content_left = if self.content_w < v.width() { (v.width() - self.content_w) / 2 } else { 0 };
        let x = v.left + content_left + (self.content_w - l.w) / 2 - self.scroll_x;
        let y = v.top + l.y - self.scroll_y;
        Some(RECT::xywh(x, y, l.w, l.h))
    }

    /// Indices of pages intersecting the view.
    pub fn visible_pages(&self) -> std::ops::Range<usize> {
        let v = self.view_rect();
        let top = self.scroll_y;
        let bottom = self.scroll_y + v.height();
        let first = self.layouts.partition_point(|l| l.y + l.h < top);
        let last = self.layouts.partition_point(|l| l.y <= bottom);
        first..last.max(first)
    }

    /// The page occupying the upper third of the view: what the user is "on".
    pub fn current_page(&self) -> usize {
        if self.layouts.is_empty() {
            return 0;
        }
        let probe = self.scroll_y + self.view_rect().height() / 3;
        let idx = self.layouts.partition_point(|l| l.y + l.h + self.page_gap() < probe);
        idx.min(self.layouts.len() - 1)
    }

    pub fn page_at(&self, x: i32, y: i32) -> Option<usize> {
        if !self.view_rect().contains(x, y) {
            return None;
        }
        self.visible_pages().find(|&i| self.page_rect(i).is_some_and(|r| r.contains(x, y)))
    }

    /// Client pixel → PDF page coordinates.
    pub fn to_pdf(&mut self, page: usize, x: i32, y: i32) -> Option<(f64, f64)> {
        let r = self.page_rect(page)?;
        let p = self.doc.as_mut()?.page(page)?;
        p.device_to_page(r.left, r.top, r.width(), r.height(), 0, x, y)
    }

    /// PDF page coordinates → client pixel.
    pub fn to_screen(&mut self, page: usize, x: f64, y: f64) -> Option<(i32, i32)> {
        let r = self.page_rect(page)?;
        let p = self.doc.as_mut()?.page(page)?;
        p.page_to_device(r.left, r.top, r.width(), r.height(), 0, x, y)
    }

    pub fn rect_to_screen(&mut self, page: usize, rc: RectF) -> Option<RECT> {
        let (x1, y1) = self.to_screen(page, rc.left as f64, rc.top as f64)?;
        let (x2, y2) = self.to_screen(page, rc.right as f64, rc.bottom as f64)?;
        Some(RECT::new(x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)))
    }

    /// Scroll so that a page's top sits at the top of the view.
    pub fn go_to_page(&mut self, idx: usize) {
        if let Some(l) = self.layouts.get(idx) {
            let y = l.y - self.margin() / 2;
            self.scroll_to(self.scroll_x, y, true);
        }
    }

    /// Smoothly (or instantly) scrolls the document.
    pub fn scroll_to(&mut self, x: i32, y: i32, smooth: bool) {
        let (mx, my) = self.max_scroll();
        self.scroll_x = x.clamp(0, mx);
        self.target_y = (y as f64).clamp(0.0, my as f64);
        if smooth {
            self.timer(TIMER_SCROLL, 10);
        } else {
            self.scroll_y = self.target_y.round() as i32;
            self.kill_timer(TIMER_SCROLL);
        }
        self.view_changed();
    }

    pub fn scroll_by(&mut self, dx: i32, dy: i32, smooth: bool) {
        // Accumulate onto the in-flight animation target so fast wheel spins don't stall.
        let base = if smooth { self.target_y.round() as i32 } else { self.scroll_y };
        self.scroll_to(self.scroll_x + dx, base + dy, smooth);
    }

    /// One animation step towards `target_y`; returns false when settled.
    pub fn animate_scroll(&mut self) -> bool {
        let diff = self.target_y - self.scroll_y as f64;
        if diff.abs() < 1.0 {
            self.scroll_y = self.target_y.round() as i32;
            self.invalidate();
            return false;
        }
        let step = diff * 0.32;
        self.scroll_y += if step.abs() < 1.0 { diff.signum() as i32 } else { step.round() as i32 };
        self.invalidate();
        true
    }

    pub fn set_zoom(&mut self, zoom: f64, anchor: Option<(i32, i32)>) {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if (zoom - self.zoom).abs() < 0.01 || self.doc.is_none() {
            return;
        }
        let v = self.view_rect();
        let (ax, ay) = anchor.unwrap_or(v.center());
        // Keep the document point under the anchor fixed.
        let fx = (self.scroll_x + ax - v.left) as f64 / self.content_w.max(1) as f64;
        let fy = (self.scroll_y + ay - v.top) as f64 / self.content_h.max(1) as f64;
        self.zoom = zoom;
        self.relayout();
        self.scroll_x = (fx * self.content_w as f64).round() as i32 - (ax - v.left);
        self.scroll_y = (fy * self.content_h as f64).round() as i32 - (ay - v.top);
        self.clamp_scroll();
        self.target_y = self.scroll_y as f64;
        self.interacting_until = Some(Instant::now() + Duration::from_millis(160));
        self.timer(TIMER_RENDER, 170);
        self.view_changed();
    }

    pub fn zoom_step(&mut self, up: bool, anchor: Option<(i32, i32)>) {
        self.fit = Fit::None;
        let z = self.zoom;
        let next = if up {
            ZOOM_STEPS.iter().copied().find(|&s| s > z + 0.5).unwrap_or(MAX_ZOOM)
        } else {
            ZOOM_STEPS.iter().rev().copied().find(|&s| s < z - 0.5).unwrap_or(MIN_ZOOM)
        };
        self.set_zoom(next, anchor);
    }

    pub fn set_fit(&mut self, fit: Fit) {
        let page = self.current_page();
        self.fit = fit;
        if fit != Fit::None {
            let z = self.fit_zoom(fit);
            self.set_zoom(z, None);
            self.go_to_page(page);
            self.scroll_y = self.target_y.round() as i32;
        }
        self.invalidate();
    }

    /// Anything moved under the overlays: commit inline editing and refresh.
    pub fn view_changed(&mut self) {
        super::doc_ops::commit_editor(self);
        self.hover_text = None;
        self.invalidate();
    }

    pub fn is_interacting(&self) -> bool {
        self.interacting_until.is_some_and(|t| Instant::now() < t)
    }

    // ---- Scrollbars (overlay style) ----

    fn bar_thickness(&self, hot: bool) -> i32 {
        if hot { self.s(10) } else { self.s(6) }
    }

    /// (track, thumb) of the vertical scrollbar, if content overflows.
    pub fn vbar(&self) -> Option<(RECT, RECT)> {
        let v = self.view_rect();
        let (_, my) = self.max_scroll();
        if my == 0 {
            return None;
        }
        let t = self.bar_thickness(matches!(self.hot, Some(Hot::VScroll)) || matches!(self.drag, Drag::VThumb { .. }));
        let track = RECT::new(v.right - t - self.s(2), v.top + self.s(2), v.right - self.s(2), v.bottom - self.s(2));
        let len = ((track.height() as f64 * v.height() as f64 / self.content_h as f64) as i32).max(self.s(32)).min(track.height());
        let pos = ((track.height() - len) as f64 * self.scroll_y as f64 / my as f64).round() as i32;
        Some((track, RECT::xywh(track.left, track.top + pos, t, len)))
    }

    pub fn hbar(&self) -> Option<(RECT, RECT)> {
        let v = self.view_rect();
        let (mx, _) = self.max_scroll();
        if mx == 0 {
            return None;
        }
        let t = self.bar_thickness(matches!(self.hot, Some(Hot::HScroll)) || matches!(self.drag, Drag::HThumb { .. }));
        let track = RECT::new(v.left + self.s(2), v.bottom - t - self.s(2), v.right - self.s(14), v.bottom - self.s(2));
        let len = ((track.width() as f64 * v.width() as f64 / self.content_w as f64) as i32).max(self.s(32)).min(track.width());
        let pos = ((track.width() - len) as f64 * self.scroll_x as f64 / mx as f64).round() as i32;
        Some((track, RECT::xywh(track.left + pos, track.top, len, t)))
    }

    /// Hit area for the vertical bar is wider than what's drawn.
    pub fn in_vbar(&self, x: i32, y: i32) -> bool {
        self.vbar().is_some_and(|(track, _)| x >= track.right - self.s(14) && y >= track.top && y < track.bottom)
    }

    pub fn in_hbar(&self, x: i32, y: i32) -> bool {
        self.hbar().is_some_and(|(track, _)| y >= track.bottom - self.s(14) && x >= track.left && x < track.right)
    }

    /// Positions the scroll so the thumb's leading edge is at `pos` along the track.
    pub fn drag_vthumb(&mut self, y: i32, grab: i32) {
        if let Some((track, thumb)) = self.vbar() {
            let (_, my) = self.max_scroll();
            let span = (track.height() - thumb.height()).max(1);
            let f = (y - grab - track.top) as f64 / span as f64;
            self.scroll_to(self.scroll_x, (f * my as f64).round() as i32, false);
        }
    }

    pub fn drag_hthumb(&mut self, x: i32, grab: i32) {
        if let Some((track, thumb)) = self.hbar() {
            let (mx, _) = self.max_scroll();
            let span = (track.width() - thumb.width()).max(1);
            let f = (x - grab - track.left) as f64 / span as f64;
            self.scroll_to((f * mx as f64).round() as i32, self.scroll_y, false);
        }
    }

    // ---- Sidebar thumbnails ----

    pub fn thumb_width(&self) -> i32 {
        self.sidebar_w - self.s(64)
    }

    fn layout_thumbs(&mut self) {
        self.thumb_slots.clear();
        let tw = self.thumb_width();
        let max_h = (tw as f64 * 1.5) as i32;
        let mut y = self.s(40);
        for l in &self.layouts {
            let th = ((tw as f64 * l.h_pt / l.w_pt.max(1.0)).round() as i32).clamp(self.s(24), max_h);
            let tw2 = if (tw as f64 * l.h_pt / l.w_pt.max(1.0)) > max_h as f64 { (max_h as f64 * l.w_pt / l.h_pt.max(1.0)) as i32 } else { tw };
            let x = (self.sidebar_w - tw2) / 2;
            self.thumb_slots.push(RECT::xywh(x, y, tw2, th));
            y += th + self.s(30);
        }
    }

    /// Thumbnail rect on screen (sidebar coordinates translated into the client area).
    pub fn thumb_rect(&self, idx: usize) -> Option<RECT> {
        self.thumb_slots.get(idx).map(|r| r.offset(0, self.toolbar_h - self.sidebar_scroll))
    }

    pub fn thumb_at(&self, x: i32, y: i32) -> Option<usize> {
        if self.sidebar_tab != SidebarTab::Pages || x >= self.sidebar_width() || y < self.toolbar_h + self.s(36) || y >= self.height - self.status_h {
            return None;
        }
        (0..self.thumb_slots.len()).find(|&i| self.thumb_rect(i).is_some_and(|r| r.inflate(self.s(12), self.s(6)).offset(0, self.s(8)).contains(x, y)))
    }
}
