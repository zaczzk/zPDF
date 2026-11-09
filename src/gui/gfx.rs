//! Thin drawing layer over GDI: DPI scaling, cached fonts, DC brush/pen fills, ellipsized text,
//! icon glyphs (Segoe Fluent Icons / MDL2 Assets) and translucent fills.

use super::theme::Theme;
use super::win32::*;
use std::ffi::c_void;
use std::ptr;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Font {
    Small,
    Body,
    BodyBold,
    Brand,
    Title,
    Icon,
    IconSmall,
    IconLarge,
}

impl Font {
    fn spec(self) -> (f32, i32, bool) {
        match self {
            Font::Small => (8.5, 400, false),
            Font::Body => (9.5, 400, false),
            Font::BodyBold => (9.5, 600, false),
            Font::Brand => (11.0, 700, false),
            Font::Title => (17.0, 600, false),
            Font::Icon => (11.0, 400, true),
            Font::IconSmall => (9.0, 400, true),
            Font::IconLarge => (22.0, 400, true),
        }
    }
}

const ALL_FONTS: [Font; 8] = [Font::Small, Font::Body, Font::BodyBold, Font::Brand, Font::Title, Font::Icon, Font::IconSmall, Font::IconLarge];

/// Icon glyph code points shared by Segoe Fluent Icons (Win11) and Segoe MDL2 Assets (Win10).
pub mod icon {
    pub const OPEN: char = '\u{E838}';
    pub const SAVE: char = '\u{E74E}';
    pub const UNDO: char = '\u{E7A7}';
    pub const REDO: char = '\u{E7A6}';
    pub const HAND: char = '\u{E7C9}';
    pub const SELECT: char = '\u{E8D3}';
    pub const EDIT: char = '\u{E70F}';
    pub const HIGHLIGHT: char = '\u{E7E6}';
    pub const CHECK: char = '\u{E73E}';
    pub const CROSS: char = '\u{E711}';
    pub const IMAGE: char = '\u{EB9F}';
    pub const REDACT: char = '\u{ED62}';
    pub const ZOOM_IN: char = '\u{E710}';
    pub const ZOOM_OUT: char = '\u{E738}';
    pub const SEARCH: char = '\u{E721}';
    pub const UP: char = '\u{E70E}';
    pub const DOWN: char = '\u{E70D}';
    pub const CLOSE: char = '\u{E711}';
    pub const FIT_WIDTH: char = '\u{E799}';
    pub const FIT_PAGE: char = '\u{E9A6}';
    pub const PRINT: char = '\u{E749}';
    pub const SIDEBAR: char = '\u{E8A0}';
    pub const DOCUMENT: char = '\u{E8A5}';
}

pub struct Gfx {
    pub dpi: u32,
    fonts: Vec<(Font, HFONT)>,
    icon_face: Option<&'static str>,
    alpha_dc: HDC,
    alpha_bmp: HBITMAP,
    alpha_old: HGDIOBJ,
    alpha_bits: *mut u32,
}

impl Gfx {
    pub fn new(dpi: u32) -> Self {
        let mut g = Gfx {
            dpi: dpi.max(96),
            fonts: Vec::new(),
            icon_face: None,
            alpha_dc: ptr::null_mut(),
            alpha_bmp: ptr::null_mut(),
            alpha_old: ptr::null_mut(),
            alpha_bits: ptr::null_mut(),
        };
        unsafe {
            g.alpha_dc = CreateCompatibleDC(ptr::null_mut());
            let bmi = BITMAPINFO::bgra(1, 1);
            let mut bits: *mut c_void = ptr::null_mut();
            g.alpha_bmp = CreateDIBSection(g.alpha_dc, &bmi, 0, &mut bits, ptr::null_mut(), 0);
            g.alpha_bits = bits as *mut u32;
            g.alpha_old = SelectObject(g.alpha_dc, g.alpha_bmp);
        }
        g
    }

    /// Swaps the real cache out for an empty stand-in (same DPI) so a `Canvas` can borrow it while
    /// the rest of the app state stays mutable. Put it back with `std::mem::replace`.
    pub fn take(slot: &mut Gfx) -> Gfx {
        let stand_in = Gfx {
            dpi: slot.dpi,
            fonts: Vec::new(),
            icon_face: slot.icon_face,
            alpha_dc: ptr::null_mut(),
            alpha_bmp: ptr::null_mut(),
            alpha_old: ptr::null_mut(),
            alpha_bits: ptr::null_mut(),
        };
        std::mem::replace(slot, stand_in)
    }

    pub fn set_dpi(&mut self, dpi: u32) {
        let dpi = dpi.max(96);
        if dpi != self.dpi {
            self.dpi = dpi;
            self.clear_fonts();
        }
    }

    #[inline]
    pub fn s(&self, v: i32) -> i32 {
        (v * self.dpi as i32 + 48) / 96
    }

    /// A copyable DPI scaler that doesn't borrow `self`.
    pub fn scaler(&self) -> impl Fn(i32) -> i32 + Copy {
        let d = self.dpi as i32;
        move |v| (v * d + 48) / 96
    }

    fn clear_fonts(&mut self) {
        for (_, f) in self.fonts.drain(..) {
            unsafe { DeleteObject(f) };
        }
    }

    fn create_font(&mut self, face: &str, pt: f32, weight: i32) -> HFONT {
        let height = -((pt * self.dpi as f32 / 72.0).round() as i32);
        let face_w = wide(face);
        unsafe { CreateFontW(height, 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, 5, 0, face_w.as_ptr()) }
    }

    fn resolve_icon_face(&mut self) -> &'static str {
        if let Some(f) = self.icon_face {
            return f;
        }
        // Prefer Fluent Icons (Windows 11); fall back to MDL2 Assets (Windows 10).
        let mut chosen = "Segoe MDL2 Assets";
        let probe = self.create_font("Segoe Fluent Icons", 10.0, 400);
        unsafe {
            let dc = CreateCompatibleDC(ptr::null_mut());
            let old = SelectObject(dc, probe);
            let mut buf = [0u16; 64];
            let n = GetTextFaceW(dc, buf.len() as i32, buf.as_mut_ptr());
            if String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize]).eq_ignore_ascii_case("Segoe Fluent Icons") {
                chosen = "Segoe Fluent Icons";
            }
            SelectObject(dc, old);
            DeleteDC(dc);
            DeleteObject(probe);
        }
        self.icon_face = Some(chosen);
        chosen
    }

    pub fn font(&mut self, f: Font) -> HFONT {
        if let Some((_, h)) = self.fonts.iter().find(|(k, _)| *k == f) {
            return *h;
        }
        let (pt, weight, is_icon) = f.spec();
        let face = if is_icon { self.resolve_icon_face() } else { "Segoe UI Variable Text" };
        let mut h = self.create_font(face, pt, weight);
        if !is_icon {
            // "Segoe UI Variable" only exists on Windows 11; GDI silently substitutes otherwise.
            unsafe {
                let dc = CreateCompatibleDC(ptr::null_mut());
                let old = SelectObject(dc, h);
                let mut buf = [0u16; 64];
                let n = GetTextFaceW(dc, buf.len() as i32, buf.as_mut_ptr());
                let got = String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize]);
                SelectObject(dc, old);
                DeleteDC(dc);
                if !got.starts_with("Segoe UI Variable") {
                    DeleteObject(h);
                    h = self.create_font("Segoe UI", pt, weight);
                }
            }
        }
        self.fonts.push((f, h));
        h
    }

    /// Prebuilds every font so painting never creates GDI objects.
    pub fn warm(&mut self) {
        for f in ALL_FONTS {
            self.font(f);
        }
    }
}

impl Drop for Gfx {
    fn drop(&mut self) {
        self.clear_fonts();
        unsafe {
            if !self.alpha_dc.is_null() {
                SelectObject(self.alpha_dc, self.alpha_old);
                DeleteObject(self.alpha_bmp);
                DeleteDC(self.alpha_dc);
            }
        }
    }
}

/// A drawing surface: an HDC plus the shared font cache and current theme.
pub struct Canvas<'a> {
    pub hdc: HDC,
    pub g: &'a mut Gfx,
    pub t: Theme,
}

impl<'a> Canvas<'a> {
    pub fn new(hdc: HDC, g: &'a mut Gfx, t: Theme) -> Self {
        unsafe { SetBkMode(hdc, TRANSPARENT) };
        Canvas { hdc, g, t }
    }

    #[inline]
    pub fn s(&self, v: i32) -> i32 {
        self.g.s(v)
    }

    pub fn fill(&self, r: RECT, color: COLORREF) {
        unsafe {
            SetDCBrushColor(self.hdc, color);
            FillRect(self.hdc, &r, GetStockObject(DC_BRUSH));
        }
    }

    pub fn frame(&self, r: RECT, color: COLORREF, width: i32) {
        let w = width.max(1);
        self.fill(RECT::new(r.left, r.top, r.right, r.top + w), color);
        self.fill(RECT::new(r.left, r.bottom - w, r.right, r.bottom), color);
        self.fill(RECT::new(r.left, r.top, r.left + w, r.bottom), color);
        self.fill(RECT::new(r.right - w, r.top, r.right, r.bottom), color);
    }

    /// Rounded rectangle with an optional 1px border.
    pub fn round(&self, r: RECT, radius: i32, fill: COLORREF, border: Option<COLORREF>) {
        unsafe {
            SetDCBrushColor(self.hdc, fill);
            SetDCPenColor(self.hdc, border.unwrap_or(fill));
            let ob = SelectObject(self.hdc, GetStockObject(DC_BRUSH));
            let op = SelectObject(self.hdc, GetStockObject(DC_PEN));
            RoundRect(self.hdc, r.left, r.top, r.right, r.bottom, radius * 2, radius * 2);
            SelectObject(self.hdc, ob);
            SelectObject(self.hdc, op);
        }
    }

    /// Unfilled rectangle outline using a (possibly dotted) pen.
    pub fn outline(&self, r: RECT, color: COLORREF, width: i32, dotted: bool) {
        unsafe {
            let pen = CreatePen(if dotted { PS_DOT } else { PS_SOLID }, if dotted { 1 } else { width }, color);
            let op = SelectObject(self.hdc, pen);
            let ob = SelectObject(self.hdc, GetStockObject(NULL_BRUSH));
            Rectangle(self.hdc, r.left, r.top, r.right, r.bottom);
            SelectObject(self.hdc, op);
            SelectObject(self.hdc, ob);
            DeleteObject(pen);
        }
    }

    pub fn ellipse(&self, r: RECT, color: COLORREF) {
        unsafe {
            SetDCBrushColor(self.hdc, color);
            SetDCPenColor(self.hdc, color);
            let ob = SelectObject(self.hdc, GetStockObject(DC_BRUSH));
            let op = SelectObject(self.hdc, GetStockObject(DC_PEN));
            Ellipse(self.hdc, r.left, r.top, r.right, r.bottom);
            SelectObject(self.hdc, ob);
            SelectObject(self.hdc, op);
        }
    }

    /// Translucent fill (alpha 0–255).
    pub fn blend(&self, r: RECT, color: COLORREF, alpha: u8) {
        if r.is_empty() {
            return;
        }
        unsafe {
            // BGRA pixel; COLORREF is 0x00BBGGRR.
            let (rr, gg, bb) = (color & 0xFF, (color >> 8) & 0xFF, (color >> 16) & 0xFF);
            *self.g.alpha_bits = 0xFF00_0000 | (rr << 16) | (gg << 8) | bb;
            let bf = BLENDFUNCTION { BlendOp: 0, BlendFlags: 0, SourceConstantAlpha: alpha, AlphaFormat: 0 };
            GdiAlphaBlend(self.hdc, r.left, r.top, r.width(), r.height(), self.g.alpha_dc, 0, 0, 1, 1, bf);
        }
    }

    /// Single-line text in `r`; `align` is DT_LEFT / DT_CENTER / DT_RIGHT. Ellipsizes when too long.
    pub fn text(&mut self, r: RECT, s: &str, font: Font, color: COLORREF, align: u32) {
        let w: Vec<u16> = s.encode_utf16().collect();
        let f = self.g.font(font);
        unsafe {
            let of = SelectObject(self.hdc, f);
            SetTextColor(self.hdc, color);
            let mut rc = r;
            DrawTextW(self.hdc, w.as_ptr(), w.len() as i32, &mut rc, align | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS);
            SelectObject(self.hdc, of);
        }
    }

    pub fn measure(&mut self, s: &str, font: Font) -> i32 {
        let w: Vec<u16> = s.encode_utf16().collect();
        let f = self.g.font(font);
        let mut sz = SIZE::default();
        unsafe {
            let of = SelectObject(self.hdc, f);
            GetTextExtentPoint32W(self.hdc, w.as_ptr(), w.len() as i32, &mut sz);
            SelectObject(self.hdc, of);
        }
        sz.cx
    }

    pub fn icon(&mut self, r: RECT, glyph: char, font: Font, color: COLORREF) {
        let mut buf = [0u16; 2];
        let s = glyph.encode_utf16(&mut buf);
        let f = self.g.font(font);
        unsafe {
            let of = SelectObject(self.hdc, f);
            SetTextColor(self.hdc, color);
            let mut rc = r;
            DrawTextW(self.hdc, s.as_ptr(), s.len() as i32, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
            SelectObject(self.hdc, of);
        }
    }

    /// Blits a top-down BGRA buffer into `dest`, smoothing when it has to scale.
    pub fn blit(&self, pixels: &[u8], w: i32, h: i32, dest: RECT) {
        self.blit_src(pixels, w, h, RECT::xywh(0, 0, w, h), dest);
    }

    pub fn blit_src(&self, pixels: &[u8], w: i32, h: i32, src: RECT, dest: RECT) {
        if w <= 0 || h <= 0 || pixels.len() < (w * h * 4) as usize {
            return;
        }
        let bmi = BITMAPINFO::bgra(w, h);
        unsafe {
            let scaled = src.width() != dest.width() || src.height() != dest.height();
            SetStretchBltMode(self.hdc, if scaled { HALFTONE } else { COLORONCOLOR });
            SetBrushOrgEx(self.hdc, 0, 0, ptr::null_mut());
            // StretchDIBits measures source Y from the bottom for top-down DIBs as well: flip it.
            let src_y = h - src.bottom;
            StretchDIBits(
                self.hdc,
                dest.left,
                dest.top,
                dest.width(),
                dest.height(),
                src.left,
                src_y,
                src.width(),
                src.height(),
                pixels.as_ptr() as *const c_void,
                &bmi,
                0,
                SRCCOPY,
            );
        }
    }

    /// Soft drop shadow: a few stacked translucent frames.
    pub fn shadow(&self, r: RECT, spread: i32) {
        let c = self.t.shadow;
        for i in 1..=spread.max(1) {
            let a = (60 / i).max(8) as u8;
            self.blend(RECT::new(r.left - i + 1, r.bottom + i - 1, r.right + i - 1, r.bottom + i), c, a);
            self.blend(RECT::new(r.right + i - 1, r.top - i + 2, r.right + i, r.bottom + i), c, a);
            self.blend(RECT::new(r.left - i, r.top - i + 2, r.left - i + 1, r.bottom + i), c, a / 2);
        }
    }
}
