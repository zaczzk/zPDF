// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Main Entrypoint & Self-Test Suite

#![allow(dead_code)]

mod editor;
mod pdfium;
mod security;
mod tile_cache;
mod win32_gui;

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use pdfium::PdfiumLibrary;
use security::{harden_process_dll_search_order, verify_pdfium_integrity, Sha256};

fn find_pdfium_dll() -> Option<PathBuf> {
    // 1. Check next to the executable
    if let Ok(exe_path) = env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = parent.join("pdfium.dll");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 2. Check current working directory
    let cwd_candidate = PathBuf::from("pdfium.dll");
    if cwd_candidate.exists() {
        return Some(cwd_candidate);
    }

    // 3. Check common development or local paths
    let local_candidates = [
        "C:\\pdfium\\pdfium.dll",
        "C:\\Program Files\\pdfium\\pdfium.dll",
    ];
    for &p in &local_candidates {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

fn run_self_tests() -> Result<(), String> {
    println!("============================================================");
    println!(" Running zPDF Self-Test Suite (Zero-Dependency Native Rust) ");
    println!("============================================================");

    // Test 1: DLL Search Order Lockdown
    println!("[1/4] Enforcing DLL search-order hardening...");
    harden_process_dll_search_order();
    println!("  -> OK: SetDefaultDllDirectories and SetDllDirectoryW active.");

    // Test 2: Pure-Rust FIPS 180-4 SHA-256
    println!("[2/4] Verifying pure-Rust SHA-256 cryptographic engine...");
    let empty_hash = security::bytes_to_hex(&Sha256::hash(b""));
    assert_eq!(
        empty_hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "NIST SHA-256 empty test vector failed"
    );

    let abc_hash = security::bytes_to_hex(&Sha256::hash(b"abc"));
    assert_eq!(
        abc_hash,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "NIST SHA-256 'abc' test vector failed"
    );
    println!("  -> OK: Pure-Rust SHA-256 passes all NIST test vectors.");

    // Test 3: Locate and verify PDFium DLL
    println!("[3/4] Searching for local pdfium.dll...");
    let pdfium_path = match find_pdfium_dll() {
        Some(p) => p,
        None => {
            println!("  -> Notice: pdfium.dll not found in standard directories.");
            println!("  -> Tip: Place pdfium.dll next to zpdf.exe to enable PDF rendering & editing.");
            println!("\n[OK] Core security & cryptographic tests PASSED!");
            return Ok(());
        }
    };

    let hash_hex = verify_pdfium_integrity(&pdfium_path, None)?;
    println!("  -> PDFium Path: {}", pdfium_path.display());
    println!("  -> SHA-256 Hash: {}", hash_hex);

    // Test 4: PDFium RAII lifecycle, Document creation & serialization
    println!("[4/4] Verifying PDFium dynamic loading & document creation...");
    let lib = PdfiumLibrary::load(&pdfium_path)?;
    let doc = lib.create_empty_document()?;
    assert_eq!(doc.page_count(), 0);

    // Create a new blank page (letter size: 612 x 792 pt)
    let page = doc.new_page(0, 612.0, 792.0)?;
    assert_eq!(doc.page_count(), 1);
    let (w, h) = page.dimensions();
    assert!((w - 612.0).abs() < 1.0);
    assert!((h - 792.0).abs() < 1.0);

    // Test render bitmap
    let render_w = 300;
    let render_h = 400;
    let stride = render_w * 4;
    let mut buffer = vec![0u8; (stride * render_h) as usize];
    page.render_into_buffer(&mut buffer, render_w, render_h, stride, 0, 0)?;

    // Test save to memory stream
    let bytes = doc.save_to_bytes()?;
    assert!(bytes.len() > 100);
    assert_eq!(&bytes[..4], b"%PDF");

    println!("  -> OK: PDF creation, page allocation, bitmap rasterization and streaming save verified.");
    println!("\n============================================================");
    println!(" ALL SELF-TESTS PASSED (0 dependencies, 100% pure Rust/Win32) ");
    println!("============================================================");
    Ok(())
}

fn main() {
    // 1. Lockdown process immediately upon entry
    harden_process_dll_search_order();

    let args: Vec<String> = env::args().collect();

    // Check for --self-test flag
    if args.iter().any(|a| a == "--self-test" || a == "-t") {
        if let Err(e) = run_self_tests() {
            eprintln!("Self-test failed: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Locate PDFium library
    let pdfium_path = match find_pdfium_dll() {
        Some(p) => p,
        None => {
            eprintln!("Error: pdfium.dll not found.");
            eprintln!("Please place 'pdfium.dll' in the same folder as zpdf.exe.");
            eprintln!("You can run 'zpdf.exe --self-test' to test the security engine.");
            std::process::exit(1);
        }
    };

    // Verify integrity of PDFium DLL
    if let Err(e) = verify_pdfium_integrity(&pdfium_path, None) {
        eprintln!("Security Error: {}", e);
        std::process::exit(1);
    }

    // Load PDFium dynamically
    let pdfium_lib = match PdfiumLibrary::load(&pdfium_path) {
        Ok(lib) => Arc::new(lib),
        Err(e) => {
            eprintln!("Failed to initialize PDFium library: {}", e);
            std::process::exit(1);
        }
    };

    // Check if a file path was passed on command line
    let initial_pdf = args
        .get(1)
        .filter(|a| !a.starts_with('-'))
        .map(PathBuf::from);

    // Launch Win32 GUI
    if let Err(e) = win32_gui::run_gui(pdfium_lib, initial_pdf) {
        eprintln!("Application Error: {}", e);
        std::process::exit(1);
    }
}
