// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Editor, Hit-testing, Vector Annotations & Undo/Redo

#![allow(dead_code)]

use crate::pdfium::{PdfDocument, PdfPage, RectF};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolMode {
    Pan,
    SelectText,
    Highlight,
    FreeText,
    EditTextObject,
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
