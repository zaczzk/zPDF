// Generate a 3-page sample PDF for manual & automated testing
#![allow(unused)]

use std::path::PathBuf;

#[path = "../pdfium.rs"]
mod pdfium;
#[path = "../security.rs"]
mod security;

use pdfium::PdfiumLibrary;

fn main() {
    let pdfium_path = PathBuf::from("pdfium.dll");
    let lib = PdfiumLibrary::load(&pdfium_path).expect("load pdfium");
    let doc = lib.create_empty_document().expect("create doc");

    // Page 1: Portrait Letter
    let _p1 = doc.new_page(0, 612.0, 792.0).expect("page 1");
    // Page 2: Landscape
    let _p2 = doc.new_page(1, 792.0, 612.0).expect("page 2");
    // Page 3: Square
    let _p3 = doc.new_page(2, 600.0, 600.0).expect("page 3");

    let bytes = doc.save_to_bytes().expect("save bytes");
    std::fs::write("sample.pdf", &bytes).expect("write sample.pdf");
    println!("Successfully generated sample.pdf with {} pages ({} bytes).", doc.page_count(), bytes.len());
}
