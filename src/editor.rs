// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Editor, Hit-testing, Vector Stamps, Annotations, Redaction & Undo/Redo

use std::collections::VecDeque;
use std::ffi::c_void;

use crate::pdfium::{
    PdfBitmap, PdfDocument, PdfPage, PdfPageObject, RectF, FPDFBitmap_BGRA, FPDF_FILLMODE_NONE,
    FPDF_FILLMODE_WINDING, FPDF_FONT, FPDF_PAGEOBJ_IMAGE,
};

// --- Undo / Redo ---

#[derive(Clone, Debug)]
pub enum EditAction {
    EditTextRun {
        page_index: usize,
        object_index: usize,
        old_text: String,
        new_text: String,
    },
    InsertTextRun {
        page_index: usize,
        object_index: usize,
        text: String,
        page_x: f64,
        page_y: f64,
        font_size: f32,
    },
    InsertImage {
        page_index: usize,
        object_index: usize,
        width_px: u32,
        height_px: u32,
        bgra_pixels: Vec<u8>,
        page_x: f64,
        page_y: f64,
        width_pt: f64,
        height_pt: f64,
    },
    InsertStamp {
        page_index: usize,
        object_index: usize,
        checkmark: bool,
        page_x: f64,
        page_y: f64,
        size_pt: f32,
    },
    /// Highlight annotation created by `add_highlight_annot`; undo with `remove_annot`.
    HighlightAnnot {
        page_index: usize,
        annot_index: usize,
        rects: Vec<RectF>,
        color: (u8, u8, u8, u8),
    },
    InsertBlankPage {
        page_index: usize,
        width_pt: f64,
        height_pt: f64,
    },
    /// A page object translated by (dx, dy) PDF points; undo moves it back.
    MoveObject {
        page_index: usize,
        object_index: usize,
        dx: f64,
        dy: f64,
    },
    DocumentSnapshot {
        before_bytes: Vec<u8>,
        after_bytes: Vec<u8>,
        active_page_before: usize,
        active_page_after: usize,
    },
    RotatePage {
        page_index: usize,
        old_rotation: i32,
        new_rotation: i32,
    },
}

impl EditAction {
    /// Approximate heap + inline size, used for the undo memory budget.
    fn approx_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                EditAction::EditTextRun { old_text, new_text, .. } => old_text.len() + new_text.len(),
                EditAction::InsertTextRun { text, .. } => text.len(),
                EditAction::InsertImage { bgra_pixels, .. } => bgra_pixels.len(),
                EditAction::HighlightAnnot { rects, .. } => rects.len() * std::mem::size_of::<RectF>(),
                EditAction::DocumentSnapshot { before_bytes, after_bytes, .. } => before_bytes.len() + after_bytes.len(),
                _ => 0,
            }
    }
}

/// An action temporarily taken out of the history while it is being applied.
pub struct HistoryEntry {
    id: u64,
    pub action: EditAction,
    from_undo: bool,
}

/// Default undo history budget.
const UNDO_MEMORY_LIMIT: usize = 256 * 1024 * 1024;

/// Undo/redo stacks with a memory cap and saved-point tracking.
///
/// Every pushed action gets a unique id; the document state is identified by
/// the id of the action on top of the undo stack (or `base_id` when it is
/// empty). `mark_saved` records the current state id, and `is_at_saved_point`
/// compares against it, so dirty state stays correct across undo/redo.
pub struct UndoManager {
    undo_stack: VecDeque<(u64, EditAction)>,
    redo_stack: Vec<(u64, EditAction)>,
    next_id: u64,
    /// State id when the undo stack is empty (advances when old entries are dropped).
    base_id: u64,
    saved_id: Option<u64>,
    bytes: usize,
    max_bytes: usize,
}

impl UndoManager {
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            next_id: 1,
            base_id: 0,
            saved_id: Some(0),
            bytes: 0,
            max_bytes: UNDO_MEMORY_LIMIT,
        }
    }

    /// Drops all history and treats the current document state as saved
    /// (call after opening or reloading a document).
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.bytes = 0;
        self.base_id = self.next_id;
        self.next_id += 1;
        self.saved_id = Some(self.base_id);
    }

    /// Records a new action. Clears the redo stack, then drops the oldest undo
    /// entries while the history exceeds the memory budget (the newest entry is
    /// always kept, even if it alone is over budget).
    pub fn push(&mut self, action: EditAction) {
        for (_, a) in self.redo_stack.drain(..) {
            self.bytes -= a.approx_bytes();
        }
        self.bytes += action.approx_bytes();
        self.undo_stack.push_back((self.next_id, action));
        self.next_id += 1;
        while self.bytes > self.max_bytes && self.undo_stack.len() > 1 {
            if let Some((id, old)) = self.undo_stack.pop_front() {
                self.bytes -= old.approx_bytes();
                self.base_id = id;
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Moves the newest action to the redo stack (no copy) and returns it so
    /// the caller can apply its inverse.
    pub fn undo(&mut self) -> Option<&EditAction> {
        let entry = self.undo_stack.pop_back()?;
        self.redo_stack.push(entry);
        self.redo_stack.last().map(|(_, a)| a)
    }

    /// Moves the next redo action back to the undo stack (no copy) and returns
    /// it so the caller can re-apply it.
    pub fn redo(&mut self) -> Option<&EditAction> {
        let entry = self.redo_stack.pop()?;
        self.undo_stack.push_back(entry);
        self.undo_stack.back().map(|(_, a)| a)
    }

    /// Takes the newest action off the undo stack so the caller can apply its inverse, then hand
    /// it back with `finish_undo` (success) or `cancel` (failure, state unchanged).
    pub fn take_undo(&mut self) -> Option<HistoryEntry> {
        self.undo_stack.pop_back().map(|(id, action)| HistoryEntry { id, action, from_undo: true })
    }

    /// Takes the next redo action; hand it back with `finish_redo` or `cancel`.
    /// The caller may update the action (e.g. new object indices after re-insertion).
    pub fn take_redo(&mut self) -> Option<HistoryEntry> {
        self.redo_stack.pop().map(|(id, action)| HistoryEntry { id, action, from_undo: false })
    }

    pub fn finish_undo(&mut self, e: HistoryEntry) {
        self.redo_stack.push((e.id, e.action));
    }

    pub fn finish_redo(&mut self, e: HistoryEntry) {
        self.undo_stack.push_back((e.id, e.action));
    }

    /// Puts an entry back where it came from after it failed to apply.
    pub fn cancel(&mut self, e: HistoryEntry) {
        if e.from_undo {
            self.undo_stack.push_back((e.id, e.action));
        } else {
            self.redo_stack.push((e.id, e.action));
        }
    }

    fn current_id(&self) -> u64 {
        self.undo_stack.back().map_or(self.base_id, |(id, _)| *id)
    }

    /// Records the current state as the one written to disk.
    pub fn mark_saved(&mut self) {
        self.saved_id = Some(self.current_id());
    }

    /// True when the document matches the last saved (or opened) state.
    pub fn is_at_saved_point(&self) -> bool {
        self.saved_id == Some(self.current_id())
    }
}

// --- Hit-testing & text ---

/// Spatially tests for text page objects on a PDF page at (pdf_x, pdf_y).
/// Returns (object_index, text, bounds) if a text run was clicked.
pub fn hit_test_text_object(page: &PdfPage, pdf_x: f32, pdf_y: f32, padding: f32) -> Option<(usize, String, RectF)> {
    let count = page.count_objects();
    let text_page = page.load_text_page().ok()?;
    let text_objects = || {
        (0..count)
            .rev()
            .filter_map(|i| page.get_object(i).map(|o| (i, o)))
            .filter(|(_, o)| o.is_text())
            .filter_map(|(i, o)| o.get_bounds().map(|b| (i, o, b.normalized())))
    };

    // 1. Direct object bounds test (padded).
    for (i, obj, b) in text_objects() {
        let hit_box = RectF { left: b.left - padding, right: b.right + padding, bottom: b.bottom - padding, top: b.top + padding };
        if hit_box.contains(pdf_x, pdf_y) {
            return Some((i, obj.get_text(&text_page).unwrap_or_default(), b));
        }
    }

    // 2. Character-level proximity fallback.
    let tol = padding as f64 * 2.0;
    let char_idx = text_page.get_char_index_at_pos(pdf_x as f64, pdf_y as f64, tol, tol)?;
    let cb = text_page.get_char_box(char_idx)?;
    for (i, obj, b) in text_objects() {
        if b.contains(cb.left, cb.bottom) || b.contains(cb.right, cb.top) {
            return Some((i, obj.get_text(&text_page).unwrap_or_default(), b));
        }
    }
    None
}

/// Generates the page content stream and returns the index of the last object.
fn commit(page: &PdfPage) -> Result<usize, String> {
    if !page.generate_content() {
        return Err("Failed to generate page content stream".to_string());
    }
    Ok(page.count_objects().saturating_sub(1))
}

/// Inserts a detached object into `page`, destroying it if insertion fails.
fn insert_owned(page: &PdfPage, obj: PdfPageObject) -> Result<(), String> {
    if page.insert_object(&obj) {
        Ok(())
    } else {
        obj.destroy();
        Err("Failed to insert object into page".to_string())
    }
}

/// Mutates an existing text object's content and regenerates page content stream.
pub fn apply_text_mutation(doc: &PdfDocument, page_index: usize, object_index: usize, new_text: &str) -> Result<(), String> {
    let page = doc.load_page(page_index)?;
    let obj = page
        .get_object(object_index)
        .filter(|o| o.is_text())
        .ok_or_else(|| "Text object not found on page".to_string())?;
    if !obj.set_text(new_text) {
        return Err("Failed to set text on PDF text object".to_string());
    }
    commit(&page).map(|_| ())
}

/// Translates a page object by (dx, dy) PDF points and regenerates the content stream.
pub fn move_page_object(doc: &PdfDocument, page_index: usize, object_index: usize, dx: f64, dy: f64) -> Result<(), String> {
    let page = doc.load_page(page_index)?;
    let obj = page.get_object(object_index).ok_or_else(|| "Object not found on page".to_string())?;
    obj.transform(1.0, 0.0, 0.0, 1.0, dx, dy);
    commit(&page).map(|_| ())
}

/// Closes a standard font handle when dropped.
struct FontGuard<'a> {
    doc: &'a PdfDocument,
    font: FPDF_FONT,
}

impl Drop for FontGuard<'_> {
    fn drop(&mut self) {
        self.doc.close_font(self.font);
    }
}

/// Inserts a new Helvetica text object into a page and regenerates the content stream.
pub fn insert_new_text_run(doc: &PdfDocument, page_index: usize, text: &str, page_x: f64, page_y: f64, font_size: f32) -> Result<usize, String> {
    let page = doc.load_page(page_index)?;
    let font = doc.load_standard_font("Helvetica").ok_or_else(|| "Failed to load standard font".to_string())?;
    // The text object keeps its own reference to the font, so the handle can
    // be released as soon as the object exists.
    let font = FontGuard { doc, font };
    let obj = doc
        .create_text_object(font.font, font_size)
        .ok_or_else(|| "Failed to create text object".to_string())?;
    if !obj.set_text(text) {
        obj.destroy();
        return Err("Failed to set text on new object".to_string());
    }
    obj.transform(1.0, 0.0, 0.0, 1.0, page_x, page_y);
    insert_owned(&page, obj)?;
    commit(&page)
}

/// Inserts an image (BGRA 32bpp) with size (width_pt, height_pt) at (page_x, page_y).
pub fn insert_image_object(
    doc: &PdfDocument,
    page_index: usize,
    width_px: u32,
    height_px: u32,
    mut bgra_pixels: Vec<u8>,
    page_x: f64,
    page_y: f64,
    width_pt: f64,
    height_pt: f64,
) -> Result<usize, String> {
    let stride = width_px as usize * 4;
    if width_px == 0 || height_px == 0 || bgra_pixels.len() < stride * height_px as usize {
        return Err("Image pixel buffer does not match its dimensions".to_string());
    }
    let page = doc.load_page(page_index)?;
    let img_obj = doc.create_image_object().ok_or_else(|| "Failed to create image object".to_string())?;

    let set_ok = unsafe {
        let bitmap = (doc.api.FPDFBitmap_CreateEx)(
            width_px as i32,
            height_px as i32,
            FPDFBitmap_BGRA,
            bgra_pixels.as_mut_ptr() as *mut c_void,
            stride as i32,
        );
        if bitmap.is_null() {
            false
        } else {
            let ok = img_obj.set_image_bitmap(&mut [page.raw], bitmap);
            (doc.api.FPDFBitmap_Destroy)(bitmap);
            ok
        }
    };
    if !set_ok {
        img_obj.destroy();
        return Err("Failed to set image bitmap on object".to_string());
    }
    img_obj.set_image_matrix(width_pt, 0.0, 0.0, height_pt, page_x, page_y);
    insert_owned(&page, img_obj)?;
    commit(&page)
}

// --- Vector paths (stamps, legacy highlight, redaction box) ---

enum PathStyle {
    Stroke { rgba: (u8, u8, u8, u8), width: f32 },
    Fill { rgba: (u8, u8, u8, u8) },
}

/// Builds a path from polylines (each starts with a move-to) and inserts it.
/// `closed` closes the last subpath.
fn add_path(doc: &PdfDocument, page: &PdfPage, polylines: &[&[(f32, f32)]], closed: bool, style: PathStyle) -> Result<(), String> {
    let start = polylines.first().and_then(|p| p.first()).ok_or_else(|| "Empty path".to_string())?;
    let path = doc.create_path(start.0, start.1).ok_or_else(|| "Failed to create path object".to_string())?;
    for (n, line) in polylines.iter().enumerate() {
        for (k, &(x, y)) in line.iter().enumerate() {
            match (n, k) {
                (0, 0) => {}
                (_, 0) => {
                    path.path_move_to(x, y);
                }
                _ => {
                    path.path_line_to(x, y);
                }
            }
        }
    }
    if closed {
        path.path_close();
    }
    match style {
        PathStyle::Stroke { rgba: (r, g, b, a), width } => {
            path.path_set_draw_mode(FPDF_FILLMODE_NONE, true);
            path.set_stroke_color(r, g, b, a);
            path.set_stroke_width(width);
        }
        PathStyle::Fill { rgba: (r, g, b, a) } => {
            path.path_set_draw_mode(FPDF_FILLMODE_WINDING, false);
            path.set_fill_color(r, g, b, a);
        }
    }
    insert_owned(page, path)
}

fn rect_polyline(rect: RectF) -> [(f32, f32); 4] {
    let r = rect.normalized();
    [(r.left, r.bottom), (r.right, r.bottom), (r.right, r.top), (r.left, r.top)]
}

/// Inserts an emerald (`#10B981`) checkmark stamp centered at (page_x, page_y).
pub fn insert_stamp_checkmark(doc: &PdfDocument, page_index: usize, page_x: f64, page_y: f64, size_pt: f32) -> Result<usize, String> {
    let page = doc.load_page(page_index)?;
    let (px, py, h) = (page_x as f32, page_y as f32, size_pt * 0.5);
    let pts = [(px - h * 0.8, py - h * 0.2), (px - h * 0.2, py - h * 0.8), (px + h * 0.9, py + h * 0.8)];
    add_path(doc, &page, &[&pts], false, PathStyle::Stroke { rgba: (0x10, 0xB9, 0x81, 0xFF), width: size_pt * 0.12 })?;
    commit(&page)
}

/// Inserts a crimson (`#EF4444`) cross stamp centered at (page_x, page_y).
pub fn insert_stamp_cross(doc: &PdfDocument, page_index: usize, page_x: f64, page_y: f64, size_pt: f32) -> Result<usize, String> {
    let page = doc.load_page(page_index)?;
    let (px, py, h) = (page_x as f32, page_y as f32, size_pt * 0.45);
    let a = [(px - h, py - h), (px + h, py + h)];
    let b = [(px - h, py + h), (px + h, py - h)];
    add_path(doc, &page, &[&a, &b], false, PathStyle::Stroke { rgba: (0xEF, 0x44, 0x44, 0xFF), width: size_pt * 0.12 })?;
    commit(&page)
}

// --- Annotations ---

/// Default highlight color (warm yellow, as RGBA).
pub const HIGHLIGHT_COLOR: (u8, u8, u8, u8) = (0xFA, 0xCC, 0x15, 0xFF);

/// Adds a /Highlight annotation covering `rects` (one quad per rect, e.g. the
/// per-line rects from `PdfTextPage::select_text_in_rect`). Returns the new
/// annotation's index for `remove_annot`.
pub fn add_highlight_annot(doc: &PdfDocument, page_index: usize, rects: &[RectF], color: (u8, u8, u8, u8)) -> Result<usize, String> {
    doc.load_page(page_index)?.create_highlight_annot(rects, color)
}

/// Removes the annotation at `annot_index` from the page.
pub fn remove_annot(doc: &PdfDocument, page_index: usize, annot_index: usize) -> Result<(), String> {
    if doc.load_page(page_index)?.remove_annot(annot_index) {
        Ok(())
    } else {
        Err(format!("Annotation {} not found on page {}", annot_index, page_index))
    }
}

// --- Redaction ---

/// Blacks out the pixels of an image object that fall inside `rect` and writes
/// the result back, overwriting the image stream's original data. If the image
/// cannot be decoded, its data is replaced by a single black pixel.
fn redact_image(doc: &PdfDocument, page: &PdfPage, obj: &PdfPageObject, rect: &RectF) -> bool {
    let decoded = obj.get_image_bitmap().filter(|b| b.bytes_per_pixel() > 0 && b.width() > 0 && b.height() > 0);
    let bitmap = match (decoded, obj.get_matrix()) {
        (Some(mut bmp), Some([a, b, c, d, e, f])) => {
            let (w, h, stride, bpp) = (bmp.width() as usize, bmp.height() as usize, bmp.stride() as usize, bmp.bytes_per_pixel());
            let has_alpha = bmp.format() == FPDFBitmap_BGRA;
            let pixels = bmp.pixels_mut();
            for y in 0..h {
                // Image space: the unit square, row 0 at the top (v = 1).
                let v = 1.0 - (y as f32 + 0.5) / h as f32;
                for x in 0..w {
                    let u = (x as f32 + 0.5) / w as f32;
                    if rect.contains(a * u + c * v + e, b * u + d * v + f) {
                        let px = &mut pixels[y * stride + x * bpp..][..bpp];
                        px.fill(0);
                        if has_alpha {
                            px[3] = 0xFF;
                        }
                    }
                }
            }
            bmp
        }
        _ => match PdfBitmap::new(doc, 1, 1, false) {
            Some(mut bmp) => {
                bmp.pixels_mut()[..3].fill(0);
                bmp
            }
            None => return false,
        },
    };
    obj.set_image_bitmap(&mut [page.raw], bitmap.raw)
}

/// Redacts `rect` on a page. Returns the number of objects/annotations affected.
///
/// - Image objects: the pixels inside `rect` are blacked out and the image
///   stream is rewritten, so the original pixel data is not kept in the saved
///   file. (A soft mask is dropped when the decoded image has no alpha.)
/// - Text objects intersecting `rect` are removed *entirely* — PDFium cannot
///   split a text run, so characters outside `rect` in the same run go too.
/// - Path and other objects (shadings, form XObjects) intersecting `rect` are
///   removed entirely. Data inside a removed form XObject's stream may remain
///   in the file.
/// - Annotations whose rect intersects `rect` are removed.
///
/// If `blackline` is true, an opaque black rectangle is drawn over `rect`.
pub fn apply_redaction(doc: &PdfDocument, page_index: usize, rect: RectF, blackline: bool) -> Result<usize, String> {
    let page = doc.load_page(page_index)?;
    let rect = rect.normalized();
    let mut affected = 0;

    for i in (0..page.count_objects()).rev() {
        let Some(obj) = page.get_object(i) else { continue };
        if !obj.get_bounds().is_some_and(|b| b.intersects(&rect)) {
            continue;
        }
        if obj.get_type() == FPDF_PAGEOBJ_IMAGE && redact_image(doc, &page, &obj, &rect) {
            affected += 1;
        } else if page.remove_object(&obj) {
            obj.destroy();
            affected += 1;
        }
    }

    for i in (0..page.annot_count()).rev() {
        if page.annot_rect(i).is_some_and(|r| r.intersects(&rect)) && page.remove_annot(i) {
            affected += 1;
        }
    }

    if blackline {
        add_path(doc, &page, &[&rect_polyline(rect)], true, PathStyle::Fill { rgba: (0, 0, 0, 0xFF) })?;
    }
    commit(&page)?;
    Ok(affected)
}

/// Removes a page object by index and regenerates the content stream.
pub fn remove_page_object(doc: &PdfDocument, page_index: usize, object_index: usize) -> Result<(), String> {
    let page = doc.load_page(page_index)?;
    let obj = page.get_object(object_index).ok_or_else(|| "Object not found on page".to_string())?;
    if !page.remove_object(&obj) {
        return Err("Failed to remove object from page".to_string());
    }
    obj.destroy();
    commit(&page).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_action(n: usize) -> EditAction {
        EditAction::InsertTextRun { page_index: 0, object_index: n, text: String::new(), page_x: 0.0, page_y: 0.0, font_size: 12.0 }
    }

    #[test]
    fn saved_point_tracks_undo_redo() {
        let mut m = UndoManager::new();
        assert!(m.is_at_saved_point());
        m.push(text_action(0));
        assert!(!m.is_at_saved_point());
        m.mark_saved();
        m.push(text_action(1));
        assert!(!m.is_at_saved_point());
        assert!(m.undo().is_some());
        assert!(m.is_at_saved_point());
        assert!(m.undo().is_some());
        assert!(!m.is_at_saved_point());
        assert!(m.redo().is_some());
        assert!(m.is_at_saved_point());
        // A new edit after undoing past the saved point makes it unreachable.
        m.undo();
        m.push(text_action(2));
        assert!(!m.is_at_saved_point());
        m.undo();
        assert!(!m.is_at_saved_point());
        m.clear();
        assert!(m.is_at_saved_point() && !m.can_undo() && !m.can_redo());
    }

    #[test]
    fn memory_cap_drops_oldest() {
        let mut m = UndoManager::new();
        m.max_bytes = 3 * 1024;
        let snap = || EditAction::DocumentSnapshot { before_bytes: vec![0; 512], after_bytes: vec![0; 512], active_page_before: 0, active_page_after: 0 };
        for _ in 0..10 {
            m.push(snap());
        }
        assert!(m.bytes <= m.max_bytes);
        assert!(m.undo_stack.len() < 10);
        // Undoing everything that is left does not reach the (dropped) saved state.
        while m.undo().is_some() {}
        assert!(!m.is_at_saved_point());
        // An entry larger than the whole budget is still kept.
        m.push(EditAction::DocumentSnapshot { before_bytes: vec![0; 8192], after_bytes: Vec::new(), active_page_before: 0, active_page_after: 0 });
        assert_eq!(m.undo_stack.len(), 1);
    }
}
