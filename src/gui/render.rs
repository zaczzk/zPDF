//! Rendering caches: open page handles, 1024px tiles per zoom level, a low-resolution preview per
//! page (shown stretched while tiles are still rendering) and sidebar thumbnails.
//!
//! Everything here runs on the UI thread; PDFium is not thread-safe.

use crate::pdfium::{PdfDocument, PdfFormEnv, PdfPage};

pub const TILE: i32 = 1024;
const TILE_BUDGET: usize = 192 << 20;
const PREVIEW_BUDGET: usize = 64 << 20;
const THUMB_BUDGET: usize = 32 << 20;
const PREVIEW_MAX_DIM: i32 = 720;
const MAX_OPEN_PAGES: usize = 12;

pub struct Bitmap {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
}

impl Bitmap {
    fn bytes(&self) -> usize {
        self.px.len()
    }
}

/// Renders `page` scaled to `size_w`×`size_h`, cropped to the `w`×`h` window starting at (`x`,`y`).
pub fn render(page: &PdfPage, form: Option<&PdfFormEnv>, x: i32, y: i32, w: i32, h: i32, size_w: i32, size_h: i32) -> Option<Bitmap> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let mut px = vec![0u8; (w * h * 4) as usize];
    page.render_region(&mut px, w, h, w * 4, -x, -y, size_w, size_h, form.map(|f| f.raw)).ok()?;
    Some(Bitmap { w, h, px })
}

struct Entry<K> {
    key: K,
    bmp: Bitmap,
    used: u64,
}

/// Byte-budgeted least-recently-used store. Eviction is a linear scan; entry counts stay small.
struct Lru<K: PartialEq + Copy> {
    entries: Vec<Entry<K>>,
    bytes: usize,
    budget: usize,
}

impl<K: PartialEq + Copy> Lru<K> {
    fn new(budget: usize) -> Self {
        Lru { entries: Vec::new(), bytes: 0, budget }
    }
    fn get(&mut self, key: K, clock: u64) -> Option<&Bitmap> {
        let e = self.entries.iter_mut().find(|e| e.key == key)?;
        e.used = clock;
        Some(&e.bmp)
    }
    fn contains(&self, key: K) -> bool {
        self.entries.iter().any(|e| e.key == key)
    }
    fn insert(&mut self, key: K, bmp: Bitmap, clock: u64) {
        self.remove_where(|k| *k == key);
        // Evict oldest until the new entry fits; an entry larger than the budget still gets stored
        // alone so the current view can be drawn.
        while !self.entries.is_empty() && self.bytes + bmp.bytes() > self.budget {
            let oldest = self.entries.iter().enumerate().min_by_key(|(_, e)| e.used).map(|(i, _)| i).unwrap();
            let e = self.entries.swap_remove(oldest);
            self.bytes -= e.bmp.bytes();
        }
        self.bytes += bmp.bytes();
        self.entries.push(Entry { key, bmp, used: clock });
    }
    fn remove_where(&mut self, f: impl Fn(&K) -> bool) {
        let mut freed = 0;
        self.entries.retain(|e| {
            let drop = f(&e.key);
            if drop {
                freed += e.bmp.bytes();
            }
            !drop
        });
        self.bytes -= freed;
    }
    fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}

/// (page, page render width, tile x, tile y). The render width identifies the zoom level.
type TileKey = (usize, i32, i32, i32);

pub struct RenderCache {
    tiles: Lru<TileKey>,
    previews: Lru<usize>,
    thumbs: Lru<(usize, i32)>,
    clock: u64,
}

impl RenderCache {
    pub fn new() -> Self {
        RenderCache { tiles: Lru::new(TILE_BUDGET), previews: Lru::new(PREVIEW_BUDGET), thumbs: Lru::new(THUMB_BUDGET), clock: 0 }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    pub fn tile(&mut self, key: TileKey) -> Option<&Bitmap> {
        let c = self.tick();
        self.tiles.get(key, c)
    }
    pub fn has_tile(&self, key: TileKey) -> bool {
        self.tiles.contains(key)
    }
    pub fn put_tile(&mut self, key: TileKey, bmp: Bitmap) {
        let c = self.tick();
        self.tiles.insert(key, bmp, c);
    }

    pub fn preview(&mut self, page: usize) -> Option<&Bitmap> {
        let c = self.tick();
        self.previews.get(page, c)
    }
    pub fn has_preview(&self, page: usize) -> bool {
        self.previews.contains(page)
    }
    /// Renders a preview whose longest side is at most `PREVIEW_MAX_DIM` for a page of `w`×`h` px.
    pub fn make_preview(&mut self, idx: usize, page: &PdfPage, form: Option<&PdfFormEnv>, w: i32, h: i32) {
        let scale = (PREVIEW_MAX_DIM as f64 / w.max(h).max(1) as f64).min(1.0);
        let (pw, ph) = (((w as f64) * scale).round().max(1.0) as i32, ((h as f64) * scale).round().max(1.0) as i32);
        if let Some(b) = render(page, form, 0, 0, pw, ph, pw, ph) {
            let c = self.tick();
            self.previews.insert(idx, b, c);
        }
    }

    pub fn thumb(&mut self, page: usize, w: i32) -> Option<&Bitmap> {
        let c = self.tick();
        self.thumbs.get((page, w), c)
    }
    pub fn put_thumb(&mut self, page: usize, w: i32, bmp: Bitmap) {
        let c = self.tick();
        self.thumbs.insert((page, w), bmp, c);
    }

    pub fn invalidate_page(&mut self, page: usize) {
        self.tiles.remove_where(|k| k.0 == page);
        self.previews.remove_where(|k| *k == page);
        self.thumbs.remove_where(|k| k.0 == page);
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
        self.previews.clear();
        self.thumbs.clear();
    }
}

/// Loads a page outside the cache (thumbnails, printing), registered with the form layer for the
/// duration of `f` so form fields draw and no dangling page view is left behind.
pub fn with_temp_page<R>(doc: &PdfDocument, form: Option<&PdfFormEnv>, idx: usize, f: impl FnOnce(&PdfPage) -> R) -> Option<R> {
    let page = doc.load_page(idx).ok()?;
    if let Some(fm) = form {
        fm.on_after_load_page(&page);
    }
    let r = f(&page);
    if let Some(fm) = form {
        fm.on_before_close_page(&page);
    }
    Some(r)
}

/// Keeps recently used pages loaded so we don't re-parse content streams on every paint or mouse
/// move, and so AcroForm fields keep their focus between events (PDFium tracks form state per
/// loaded page).
pub struct PageCache {
    pages: Vec<(usize, PdfPage)>,
}

impl PageCache {
    pub fn new() -> Self {
        PageCache { pages: Vec::new() }
    }

    pub fn get<'a>(&'a mut self, doc: &PdfDocument, form: Option<&PdfFormEnv>, idx: usize) -> Option<&'a PdfPage> {
        if let Some(pos) = self.pages.iter().position(|(i, _)| *i == idx) {
            // Move to the back (most recently used).
            let entry = self.pages.remove(pos);
            self.pages.push(entry);
        } else {
            let page = doc.load_page(idx).ok()?;
            if let Some(f) = form {
                f.on_after_load_page(&page);
            }
            if self.pages.len() >= MAX_OPEN_PAGES {
                let (_, old) = self.pages.remove(0);
                if let Some(f) = form {
                    f.on_before_close_page(&old);
                }
            }
            self.pages.push((idx, page));
        }
        self.pages.last().map(|(_, p)| p)
    }

    /// Drops a page handle after it was modified through another handle, so it gets re-parsed.
    pub fn forget(&mut self, form: Option<&PdfFormEnv>, idx: usize) {
        if let Some(pos) = self.pages.iter().position(|(i, _)| *i == idx) {
            let (_, p) = self.pages.remove(pos);
            if let Some(f) = form {
                f.on_before_close_page(&p);
            }
        }
    }

    pub fn clear(&mut self, form: Option<&PdfFormEnv>) {
        for (_, p) in self.pages.drain(..) {
            if let Some(f) = form {
                f.on_before_close_page(&p);
            }
        }
    }
}
