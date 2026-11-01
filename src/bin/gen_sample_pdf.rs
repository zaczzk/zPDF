// Generate a 3-page sample PDF with rich text objects for manual & automated testing
#![allow(unused)]

use std::path::PathBuf;

#[path = "../pdfium.rs"]
mod pdfium;
#[path = "../security.rs"]
mod security;

use pdfium::PdfiumLibrary;

fn add_text(doc: &pdfium::PdfDocument, page: &pdfium::PdfPage, font: pdfium::FPDF_FONT, text: &str, size: f32, x: f64, y: f64) {
    if let Some(obj) = doc.create_text_object(font, size) {
        obj.set_text(text);
        obj.transform(1.0, 0.0, 0.0, 1.0, x, y);
        page.insert_object(&obj);
    }
}

fn main() {
    let pdfium_path = PathBuf::from("pdfium.dll");
    let lib = PdfiumLibrary::load(&pdfium_path).expect("load pdfium");
    let doc = lib.create_empty_document().expect("create doc");

    let font_bold = doc.load_standard_font("Helvetica-Bold").unwrap_or_else(|| doc.load_standard_font("Helvetica").expect("font"));
    let font_regular = doc.load_standard_font("Helvetica").expect("font");

    // Page 1: Portrait Letter (612 x 792)
    let p1 = doc.new_page(0, 612.0, 792.0).expect("page 1");
    add_text(&doc, &p1, font_bold, "zPDF Document Editor", 28.0, 54.0, 720.0);
    add_text(&doc, &p1, font_regular, "Fast, zero-dependency native Rust PDF viewer and editor.", 14.0, 54.0, 680.0);
    add_text(&doc, &p1, font_regular, "Click this text with the Edit tool to modify it in-place.", 13.0, 54.0, 640.0);
    add_text(&doc, &p1, font_regular, "Press Enter to commit your edits, or Esc to cancel.", 12.0, 54.0, 610.0);
    add_text(&doc, &p1, font_regular, "Use Ctrl + Scroll to zoom, and drag with Hand tool to pan.", 12.0, 54.0, 580.0);
    p1.generate_content();

    // Page 2: Landscape (792 x 612)
    let p2 = doc.new_page(1, 792.0, 612.0).expect("page 2");
    add_text(&doc, &p2, font_bold, "Page 2: Landscape Document Layout", 26.0, 54.0, 540.0);
    add_text(&doc, &p2, font_regular, "Ultra-fast memory-bounded LRU tile cache with zero heap churn.", 14.0, 54.0, 500.0);
    add_text(&doc, &p2, font_regular, "Click on any text run on this page to edit directly.", 13.0, 54.0, 460.0);
    p2.generate_content();

    // Page 3: Square (600 x 600)
    let p3 = doc.new_page(2, 600.0, 600.0).expect("page 3");
    add_text(&doc, &p3, font_bold, "Page 3: Square Layout & Vector Rendering", 24.0, 54.0, 530.0);
    add_text(&doc, &p3, font_regular, "Tested with comprehensive headless simulation and automated verification.", 13.0, 54.0, 490.0);
    add_text(&doc, &p3, font_regular, "Try editing this line to test multi-page persistence.", 13.0, 54.0, 450.0);
    p3.generate_content();

    let bytes = doc.save_to_bytes().expect("save bytes");
    std::fs::write("sample.pdf", &bytes).expect("write sample.pdf");
    println!("Successfully generated sample.pdf with {} pages and rich text ({} bytes).", doc.page_count(), bytes.len());
}
