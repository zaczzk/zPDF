//! Data-driven toolbar: one list of buttons, laid out responsively, painted and hit-tested generically.

use super::app::*;
use super::commands::Cmd;
use super::gfx::{icon, Canvas, Font};
use super::win32::*;

pub struct Button {
    pub cmd: Cmd,
    pub rect: RECT,
    pub glyph: char,
    pub label: Option<&'static str>,
}

enum Item {
    Btn(Cmd, char, Option<&'static str>, u8),
    Gap,
    Zoom,
}

/// Lower `priority` numbers survive narrower windows.
fn items() -> Vec<Item> {
    use Item::*;
    vec![
        Btn(Cmd::Open, icon::OPEN, Some("Open"), 0),
        Btn(Cmd::Save, icon::SAVE, Some("Save"), 0),
        Btn(Cmd::Print, icon::PRINT, None, 3),
        Gap,
        Btn(Cmd::Undo, icon::UNDO, None, 0),
        Btn(Cmd::Redo, icon::REDO, None, 0),
    ]
}

fn tool_items() -> Vec<Item> {
    use Item::*;
    vec![
        Btn(Cmd::Tool(Tool::Pan), icon::HAND, None, 0),
        Btn(Cmd::Tool(Tool::Select), icon::SELECT, None, 0),
        Btn(Cmd::Tool(Tool::Edit), icon::EDIT, None, 0),
        Gap,
        Btn(Cmd::Tool(Tool::Highlight), icon::HIGHLIGHT, None, 1),
        Btn(Cmd::Tool(Tool::StampCheck), icon::CHECK, None, 1),
        Btn(Cmd::Tool(Tool::StampCross), icon::CROSS, None, 1),
        Btn(Cmd::InsertImage, icon::IMAGE, None, 2),
        Btn(Cmd::Tool(Tool::Redact), icon::REDACT, None, 1),
    ]
}

fn view_items() -> Vec<Item> {
    use Item::*;
    vec![
        Btn(Cmd::ZoomOut, icon::ZOOM_OUT, None, 2),
        Zoom,
        Btn(Cmd::ZoomIn, icon::ZOOM_IN, None, 2),
        Btn(Cmd::FitWidth, icon::FIT_WIDTH, None, 2),
        Btn(Cmd::FitPage, icon::FIT_PAGE, None, 2),
        Gap,
        Btn(Cmd::Search, icon::SEARCH, None, 0),
        Btn(Cmd::ToggleSidebar, icon::SIDEBAR, None, 3),
    ]
}

impl App {
    /// Recomputes button rectangles for the current width.
    pub fn layout_toolbar(&mut self, cv: &mut Canvas) {
        self.buttons.clear();
        let s = cv.g.scaler();
        let h = self.toolbar_h;
        let bh = s(34);
        let top = (h - bh) / 2;
        let icon_w = s(36);
        let gap = s(10);
        let has_doc = self.doc.is_some();

        // Decide how much fits: level 0 = everything, 1 = no labels, 2..4 = drop low-priority buttons.
        fn keep_at(level: u8, p: u8) -> bool {
            match level {
                0 | 1 => true,
                2 => p < 3,
                3 => p < 2,
                _ => p < 1,
            }
        }
        let measure = |level: u8, cv: &mut Canvas| -> i32 {
            let mut w = s(112) + s(130); // brand + page box
            for set in [items(), tool_items(), view_items()] {
                for it in set {
                    w += match it {
                        Item::Btn(_, _, label, p) if keep_at(level, p) => match (label, level) {
                            (Some(l), 0) => icon_w + cv.measure(l, Font::Body) + s(8),
                            _ => icon_w + s(2),
                        },
                        Item::Btn(..) => 0,
                        Item::Gap => gap,
                        Item::Zoom => s(58),
                    };
                }
            }
            w
        };
        let mut level = 0u8;
        while level < 4 && measure(level, cv) > self.width {
            level += 1;
        }
        let keep = |p: u8| keep_at(level, p);

        let push = |btns: &mut Vec<Button>, x: &mut i32, it: &Item, cv: &mut Canvas| match *it {
            Item::Btn(cmd, glyph, label, p) if keep(p) => {
                let label = if level == 0 { label } else { None };
                let w = label.map_or(icon_w, |l| icon_w + cv.measure(l, Font::Body) + s(6));
                btns.push(Button { cmd, rect: RECT::xywh(*x, top, w, bh), glyph, label });
                *x += w + s(2);
            }
            Item::Btn(..) => {}
            Item::Gap => *x += gap,
            Item::Zoom => {
                btns.push(Button { cmd: Cmd::ZoomMenu, rect: RECT::xywh(*x, top, s(56), bh), glyph: ' ', label: None });
                *x += s(58);
            }
        };

        // Left: brand + file + history.
        let mut x = s(112);
        for it in items() {
            push(&mut self.buttons, &mut x, &it, cv);
        }
        if !has_doc {
            self.page_box = RECT::default();
            return;
        }
        // Right: view controls, laid out from the right edge.
        let mut right: Vec<Button> = Vec::new();
        let mut rx = 0;
        for it in view_items() {
            push(&mut right, &mut rx, &it, cv);
        }
        let shift = self.width - s(10) - rx;
        for mut b in right {
            b.rect = b.rect.offset(shift, 0);
            self.buttons.push(b);
        }
        // Middle: tools, then the page box centred in the remaining space.
        let mut tools: Vec<Button> = Vec::new();
        let mut tx = 0;
        for it in tool_items() {
            push(&mut tools, &mut tx, &it, cv);
        }
        let box_w = s(108);
        let free_l = x + gap;
        let free_r = shift - gap;
        let total = box_w + gap * 2 + tx;
        let start = (free_l + (free_r - free_l - total) / 2).max(free_l);
        self.page_box = RECT::xywh(start, top, box_w, bh);
        let tool_x = start + box_w + gap * 2;
        for mut b in tools {
            b.rect = b.rect.offset(tool_x, 0);
            self.buttons.push(b);
        }
    }

    pub fn button_at(&self, x: i32, y: i32) -> Option<usize> {
        self.buttons.iter().position(|b| b.rect.contains(x, y))
    }

    pub fn paint_toolbar(&mut self, cv: &mut Canvas) {
        let t = cv.t;
        let bar = RECT::new(0, 0, self.width, self.toolbar_h);
        cv.fill(bar, t.chrome);
        cv.fill(RECT::new(0, bar.bottom - 1, self.width, bar.bottom), t.border);

        // Brand
        let isz = cv.s(20);
        if self.icon.is_null() {
            cv.icon(RECT::xywh(cv.s(12), 0, isz + cv.s(4), self.toolbar_h), icon::DOCUMENT, Font::Icon, t.accent);
        } else {
            unsafe { DrawIconEx(cv.hdc, cv.s(14), (self.toolbar_h - isz) / 2, self.icon, isz, isz, 0, std::ptr::null_mut(), 3) };
        }
        cv.text(RECT::new(cv.s(42), 0, cv.s(100), self.toolbar_h), "zPDF", Font::Brand, t.text, DT_LEFT);
        cv.fill(RECT::new(cv.s(102), cv.s(14), cv.s(103), self.toolbar_h - cv.s(14)), t.border);

        let n = self.buttons.len();
        for i in 0..n {
            let (cmd, r, glyph, label) = {
                let b = &self.buttons[i];
                (b.cmd, b.rect, b.glyph, b.label)
            };
            let enabled = cmd.enabled(self);
            let active = match cmd {
                Cmd::Tool(tl) => self.tool == tl,
                Cmd::Search => self.search.open,
                Cmd::ToggleSidebar => self.sidebar_visible,
                Cmd::FitWidth => self.fit == Fit::Width,
                Cmd::FitPage => self.fit == Fit::Page,
                _ => false,
            };
            let hot = self.hot == Some(Hot::Button(i)) && enabled;
            let radius = cv.s(5);
            if active {
                cv.round(r, radius, t.accent_soft, None);
            } else if hot {
                cv.round(r, radius, t.hover, None);
            }
            let fg = if !enabled {
                t.text_faint
            } else if active {
                t.accent
            } else {
                t.text
            };
            if cmd == Cmd::ZoomMenu {
                let z = format!("{}%", self.zoom.round() as i32);
                cv.text(r, &z, Font::Body, fg, DT_CENTER);
                continue;
            }
            let ir = RECT::xywh(r.left, r.top, cv.s(36), r.height());
            cv.icon(ir, glyph, Font::Icon, fg);
            if let Some(l) = label {
                cv.text(RECT::new(ir.right - cv.s(4), r.top, r.right, r.bottom), l, Font::Body, fg, DT_LEFT);
            }
            // Unsaved-changes dot on Save.
            if cmd == Cmd::Save && self.doc.as_ref().is_some_and(|d| d.is_dirty()) {
                let d = cv.s(6);
                cv.ellipse(RECT::xywh(ir.right - cv.s(11), r.top + cv.s(7), d, d), t.accent);
            }
        }

        // Page box: "3 / 12", click to type a page number.
        if let Some(doc) = &self.doc {
            let r = self.page_box;
            let total = doc.page_count();
            let hot = self.hot == Some(Hot::PageBox);
            cv.round(r, cv.s(5), if hot { t.hover } else { t.canvas }, Some(t.border));
            if self.goto_edit.is_null() {
                let cur = if total == 0 { 0 } else { self.current_page() + 1 };
                cv.text(r, &format!("{cur} / {total}"), Font::Body, t.text, DT_CENTER);
            }
        }
    }
}
