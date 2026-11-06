// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Editor, Hit-testing, Vector Annotations & Undo/Redo

#![allow(dead_code)]

use std::ffi::c_void;
use crate::pdfium::{
    PdfDocument, PdfPage, RectF, FPDFBitmap_BGRA, FPDF_FILLMODE_WINDING,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolMode {
    Pan,
    SelectText,
    Highlight,
    FreeText,
    EditTextObject,
    StampCheckmark,
    StampCross,
    RedactClear,
    RedactBlackline,
    InsertImage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GizmoHandle {
    Move,
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Clone, Debug)]
pub struct ActiveImageGizmo {
    pub page_index: usize,
    pub object_index: Option<usize>,
    pub page_x: f64,
    pub page_y: f64,
    pub width_pt: f64,
    pub height_pt: f64,
    pub raw_width_px: u32,
    pub raw_height_px: u32,
    pub bgra_pixels: Vec<u8>,
    pub dragging_handle: Option<GizmoHandle>,
    pub drag_start_mouse: (i32, i32),
    pub original_bounds: (f64, f64, f64, f64),
}

#[derive(Clone, Debug)]
pub enum Annotation {
    Highlight {
        rect: RectF,
        color: (u8, u8, u8, u8), // RGBA
    },
    FreeText {
        rect: RectF,
        text: String,
        font_size: f32,
        color: (u8, u8, u8),
    },
}

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
    InsertHighlight {
        page_index: usize,
        object_index: usize,
        rect: RectF,
    },
    InsertBlankPage {
        page_index: usize,
        width_pt: f64,
        height_pt: f64,
    },
    DocumentSnapshot {
        before_bytes: Vec<u8>,
        after_bytes: Vec<u8>,
        active_page_before: usize,
        active_page_after: usize,
    },
    RemoveObject {
        page_index: usize,
        object_index: usize,
    },
    RedactRegion {
        page_index: usize,
        rect: RectF,
        blackline: bool,
    },
    AddAnnotation {
        page_index: usize,
        annotation: Annotation,
    },
    DeleteAnnotation {
        page_index: usize,
        annotation_index: usize,
        annotation: Annotation,
    },
    RotatePage {
        page_index: usize,
        old_rotation: i32,
        new_rotation: i32,
    },
}

pub struct UndoManager {
    undo_stack: Vec<EditAction>,
    redo_stack: Vec<EditAction>,
}

impl UndoManager {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn push(&mut self, action: EditAction) {
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn pop_undo(&mut self) -> Option<EditAction> {
        let action = self.undo_stack.pop()?;
        self.redo_stack.push(action.clone());
        Some(action)
    }

    pub fn pop_redo(&mut self) -> Option<EditAction> {
        let action = self.redo_stack.pop()?;
        self.undo_stack.push(action.clone());
        Some(action)
    }
}

pub struct ActiveTextEditor {
    pub page_index: usize,
    pub object_index: Option<usize>, // None for newly inserted text
    pub original_text: String,
    pub current_text: String,
    pub bounds: RectF,
    pub insert_pos: (f64, f64),      // (pdf_x, pdf_y) for new text
    pub font_size: f32,
}

/// Spatially tests for text page objects on a PDF page at (pdf_x, pdf_y).
/// Returns (object_index, text, bounds) if a text run was clicked.
pub fn hit_test_text_object(
    page: &PdfPage,
    pdf_x: f32,
    pdf_y: f32,
    padding: f32,
) -> Option<(usize, String, RectF)> {
    let count = page.count_objects();
    let text_page = page.load_text_page().ok()?;

    // 1. Direct object bounds test
    for i in (0..count).rev() {
        if let Some(obj) = page.get_object(i) {
            if obj.is_text() {
                if let Some(mut bounds) = obj.get_bounds() {
                    let hit_box = RectF {
                        left: bounds.left.min(bounds.right) - padding,
                        right: bounds.left.max(bounds.right) + padding,
                        bottom: bounds.bottom.min(bounds.top) - padding,
                        top: bounds.bottom.max(bounds.top) + padding,
                    };

                    if hit_box.contains(pdf_x, pdf_y) {
                        let text = obj.get_text(&text_page).unwrap_or_default();
                        bounds.left = hit_box.left + padding;
                        bounds.right = hit_box.right - padding;
                        bounds.bottom = hit_box.bottom + padding;
                        bounds.top = hit_box.top - padding;
                        return Some((i, text, bounds));
                    }
                }
            }
        }
    }

    // 2. Character-level proximity fallback
    if let Some(char_idx) = text_page.get_char_index_at_pos(pdf_x as f64, pdf_y as f64, padding as f64 * 2.0, padding as f64 * 2.0) {
        if let Some(char_box) = text_page.get_char_box(char_idx) {
            // Find object corresponding to this character
            for i in (0..count).rev() {
                if let Some(obj) = page.get_object(i) {
                    if obj.is_text() {
                        if let Some(bounds) = obj.get_bounds() {
                            if bounds.contains(char_box.left, char_box.bottom) || bounds.contains(char_box.right, char_box.top) {
                                let text = obj.get_text(&text_page).unwrap_or_default();
                                return Some((i, text, bounds));
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// Mutates an existing text object's content and regenerates page content stream.
pub fn apply_text_mutation(
    doc: &PdfDocument,
    page_index: usize,
    object_index: usize,
    new_text: &str,
) -> Result<(), String> {
    let page = doc.load_page(page_index)?;
    if let Some(obj) = page.get_object(object_index) {
        if obj.is_text() {
            if obj.set_text(new_text) {
                if page.generate_content() {
                    return Ok(());
                } else {
                    return Err("Failed to generate page content stream".to_string());
                }
            } else {
                return Err("Failed to set text on PDF text object".to_string());
            }
        }
    }
    Err("Text object not found on page".to_string())
}

/// Inserts a new text object into a page and regenerates page content stream.
pub fn insert_new_text_run(
    doc: &PdfDocument,
    page_index: usize,
    text: &str,
    page_x: f64,
    page_y: f64,
    font_size: f32,
) -> Result<usize, String> {
    let page = doc.load_page(page_index)?;
    let font = doc.load_standard_font("Helvetica")
        .or_else(|| doc.load_standard_font("Arial"))
        .ok_or_else(|| "Failed to load standard font".to_string())?;

    let obj = doc.create_text_object(font, font_size)
        .ok_or_else(|| "Failed to create text object".to_string())?;

    if !obj.set_text(text) {
        return Err("Failed to set text on new object".to_string());
    }

    obj.transform(1.0, 0.0, 0.0, 1.0, page_x, page_y);

    if !page.insert_object(&obj) {
        return Err("Failed to insert object into page".to_string());
    }

    if !page.generate_content() {
        return Err("Failed to generate page content stream".to_string());
    }

    let new_idx = page.count_objects().saturating_sub(1);
    Ok(new_idx)
}

/// Inserts an image (BGRA 32bpp) onto a page with dimensions (width_pt, height_pt) at (page_x, page_y).
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
    let page = doc.load_page(page_index)?;
    let img_obj = doc
        .create_image_object()
        .ok_or_else(|| "Failed to create image object".to_string())?;

    let stride = (width_px * 4) as i32;
    unsafe {
        let bitmap = (doc.api.FPDFBitmap_CreateEx)(
            width_px as i32,
            height_px as i32,
            FPDFBitmap_BGRA,
            bgra_pixels.as_mut_ptr() as *mut c_void,
            stride,
        );
        if bitmap.is_null() {
            return Err("Failed to create PDFium bitmap for image".to_string());
        }

        let mut pages = [page.raw];
        let set_ok = img_obj.set_image_bitmap(&mut pages, bitmap);
        (doc.api.FPDFBitmap_Destroy)(bitmap);

        if !set_ok {
            return Err("Failed to set image bitmap on object".to_string());
        }
    }

    img_obj.set_image_matrix(width_pt, 0.0, 0.0, height_pt, page_x, page_y);

    if !page.insert_object(&img_obj) {
        return Err("Failed to insert image object into page".to_string());
    }

    if !page.generate_content() {
        return Err("Failed to generate page content after image insertion".to_string());
    }

    let new_idx = page.count_objects().saturating_sub(1);
    Ok(new_idx)
}

/// Inserts an Emerald Checkmark (`#10B981`) vector path stamp at (page_x, page_y) with size_pt.
pub fn insert_stamp_checkmark(
    doc: &PdfDocument,
    page_index: usize,
    page_x: f64,
    page_y: f64,
    size_pt: f32,
) -> Result<usize, String> {
    let page = doc.load_page(page_index)?;
    let half = size_pt * 0.5;
    let x0 = (page_x as f32) - half * 0.8;
    let y0 = (page_y as f32) - half * 0.2;
    let x1 = (page_x as f32) - half * 0.2;
    let y1 = (page_y as f32) - half * 0.8;
    let x2 = (page_x as f32) + half * 0.9;
    let y2 = (page_y as f32) + half * 0.8;

    let path_obj = doc
        .create_path(x0, y0)
        .ok_or_else(|| "Failed to create checkmark path object".to_string())?;

    path_obj.path_line_to(x1, y1);
    path_obj.path_line_to(x2, y2);
    path_obj.path_set_draw_mode(0, true); // Stroke mode
    path_obj.set_stroke_color(0x10, 0xB9, 0x81, 0xFF); // Emerald #10B981
    path_obj.set_stroke_width(size_pt * 0.12);

    if !page.insert_object(&path_obj) {
        return Err("Failed to insert checkmark path into page".to_string());
    }

    if !page.generate_content() {
        return Err("Failed to regenerate page content stream".to_string());
    }

    let new_idx = page.count_objects().saturating_sub(1);
    Ok(new_idx)
}

/// Inserts a Crimson Cross (`#EF4444`) vector path stamp at (page_x, page_y) with size_pt.
pub fn insert_stamp_cross(
    doc: &PdfDocument,
    page_index: usize,
    page_x: f64,
    page_y: f64,
    size_pt: f32,
) -> Result<usize, String> {
    let page = doc.load_page(page_index)?;
    let half = size_pt * 0.45;
    let px = page_x as f32;
    let py = page_y as f32;

    let path_obj = doc
        .create_path(px - half, py - half)
        .ok_or_else(|| "Failed to create cross path object".to_string())?;

    path_obj.path_line_to(px + half, py + half);
    path_obj.path_move_to(px - half, py + half);
    path_obj.path_line_to(px + half, py - half);
    path_obj.path_set_draw_mode(0, true); // Stroke mode
    path_obj.set_stroke_color(0xEF, 0x44, 0x44, 0xFF); // Crimson #EF4444
    path_obj.set_stroke_width(size_pt * 0.12);

    if !page.insert_object(&path_obj) {
        return Err("Failed to insert cross path into page".to_string());
    }

    if !page.generate_content() {
        return Err("Failed to regenerate page content stream".to_string());
    }

    let new_idx = page.count_objects().saturating_sub(1);
    Ok(new_idx)
}

/// Inserts a translucent Amber/Yellow (`#FACC15`, alpha `0x58`) highlight rectangle on a page.
pub fn insert_highlight_rect(
    doc: &PdfDocument,
    page_index: usize,
    rect: RectF,
) -> Result<usize, String> {
    let page = doc.load_page(page_index)?;
    let r_min_x = rect.left.min(rect.right);
    let r_max_x = rect.left.max(rect.right);
    let r_min_y = rect.bottom.min(rect.top);
    let r_max_y = rect.bottom.max(rect.top);

    let box_obj = doc
        .create_path(r_min_x, r_min_y)
        .ok_or_else(|| "Failed to create highlight path object".to_string())?;

    box_obj.path_line_to(r_max_x, r_min_y);
    box_obj.path_line_to(r_max_x, r_max_y);
    box_obj.path_line_to(r_min_x, r_max_y);
    box_obj.path_close();
    box_obj.path_set_draw_mode(FPDF_FILLMODE_WINDING, false);
    box_obj.set_fill_color(0xFA, 0xCC, 0x15, 0x58); // Translucent warm yellow

    if !page.insert_object(&box_obj) {
        return Err("Failed to insert highlight path into page".to_string());
    }

    if !page.generate_content() {
        return Err("Failed to regenerate page content stream after highlight".to_string());
    }

    let new_idx = page.count_objects().saturating_sub(1);
    Ok(new_idx)
}

/// True Stream Redaction:
/// Permanently expunges text and path objects from the PDF content stream that fall within `rect`.
/// If `blackline` is true, replaces the redacted content with an opaque black vector rectangle.
pub fn apply_redaction(
    doc: &PdfDocument,
    page_index: usize,
    rect: RectF,
    blackline: bool,
) -> Result<usize, String> {
    let page = doc.load_page(page_index)?;
    let count = page.count_objects();
    let mut removed_count = 0;

    let r_min_x = rect.left.min(rect.right);
    let r_max_x = rect.left.max(rect.right);
    let r_min_y = rect.bottom.min(rect.top);
    let r_max_y = rect.bottom.max(rect.top);

    // 1. Spatially identify and remove overlapping page objects
    for i in (0..count).rev() {
        if let Some(obj) = page.get_object(i) {
            if let Some(bounds) = obj.get_bounds() {
                let b_min_x = bounds.left.min(bounds.right);
                let b_max_x = bounds.left.max(bounds.right);
                let b_min_y = bounds.bottom.min(bounds.top);
                let b_max_y = bounds.bottom.max(bounds.top);

                let intersects = r_min_x <= b_max_x
                    && r_max_x >= b_min_x
                    && r_min_y <= b_max_y
                    && r_max_y >= b_min_y;

                if intersects {
                    if page.remove_object(&obj) {
                        obj.destroy();
                        removed_count += 1;
                    }
                }
            }
        }
    }

    // 2. If blackline redaction, insert a solid black filled vector rectangle
    if blackline {
        if let Some(box_obj) = doc.create_path(r_min_x, r_min_y) {
            box_obj.path_line_to(r_max_x, r_min_y);
            box_obj.path_line_to(r_max_x, r_max_y);
            box_obj.path_line_to(r_min_x, r_max_y);
            box_obj.path_close();
            box_obj.path_set_draw_mode(FPDF_FILLMODE_WINDING, false);
            box_obj.set_fill_color(0, 0, 0, 255);
            page.insert_object(&box_obj);
        }
    }

    if !page.generate_content() {
        return Err("Failed to generate page content after redaction".to_string());
    }

    Ok(removed_count)
}

/// Removes a page object by index and regenerates the content stream.
pub fn remove_page_object(
    doc: &PdfDocument,
    page_index: usize,
    object_index: usize,
) -> Result<(), String> {
    let page = doc.load_page(page_index)?;
    if let Some(obj) = page.get_object(object_index) {
        if page.remove_object(&obj) {
            obj.destroy();
            if page.generate_content() {
                return Ok(());
            } else {
                return Err("Failed to regenerate page content stream".to_string());
            }
        }
    }
    Err("Object not found on page".to_string())
}
