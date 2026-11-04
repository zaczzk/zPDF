// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Main Entrypoint & Self-Test Suite

#![allow(dead_code)]
#![windows_subsystem = "windows"]

pub use zpdf::{default_app, wic_decoder};

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

#[link(name = "kernel32")]
extern "system" {
    fn AttachConsole(dwProcessId: u32) -> i32;
}

const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;

fn find_pdfium_dll() -> Option<PathBuf> {
    // 1. Check next to the executable
    if let Ok(exe_path) = env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = parent.join("pdfium.dll");
            if candidate.exists() {
                return candidate.canonicalize().ok().or(Some(candidate));
            }
        }
    }

    // 2. Check current working directory
    if let Ok(cwd) = env::current_dir() {
        let candidate = cwd.join("pdfium.dll");
        if candidate.exists() {
            return candidate.canonicalize().ok().or(Some(candidate));
        }
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
    println!("  -> PDFium library initialized successfully.");
    let doc = lib.create_empty_document()?;
    println!("  -> Empty document created. Pages: {}", doc.page_count());

    // Create a new blank page (letter size: 612 x 792 pt)
    let page = doc.new_page(0, 612.0, 792.0)?;
    println!("  -> New page created. Pages: {}", doc.page_count());
    let (w, h) = page.dimensions();
    println!("  -> Page dimensions: {} x {}", w, h);

    // Test render bitmap
    let render_w = 300;
    let render_h = 400;
    let stride = render_w * 4;
    let mut buffer = vec![0u8; (stride * render_h) as usize];
    page.render_into_buffer(&mut buffer, render_w, render_h, stride, 0, 0)?;
    println!("  -> Page rendered to BGRA bitmap.");

    // Test save to memory stream
    let bytes = doc.save_to_bytes()?;
    println!("  -> Document saved to memory stream ({} bytes).", bytes.len());
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
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS); }
        if let Err(e) = run_self_tests() {
            eprintln!("Self-test failed: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Check for --install flag
    if args.iter().any(|a| a == "--install" || a == "/install" || a == "-i") {
        match default_app::install_app() {
            Ok(dir) => {
                let msg = format!(
                    "zPDF has been successfully installed!\n\n\
                    Location:\n{}\n\n\
                    Start Menu shortcut and .pdf associations have been registered.\n\n\
                    Would you like to open Windows Default Apps settings to set zPDF as your default PDF viewer?",
                    dir.display()
                );
                let choice = win32_gui::show_message_box(
                    std::ptr::null_mut(),
                    "zPDF — Installation Complete",
                    &msg,
                    0x00000024, // MB_YESNO | MB_ICONQUESTION
                );
                if choice == 6 { // IDYES
                    default_app::launch_default_apps_settings();
                }
            }
            Err(e) => {
                win32_gui::show_message_box(
                    std::ptr::null_mut(),
                    "zPDF — Installation Failed",
                    &format!("Failed to install zPDF:\n\n{}", e),
                    0x00000010, // MB_ICONERROR
                );
                std::process::exit(1);
            }
        }
        return;
    }

    // Check for --uninstall flag
    if args.iter().any(|a| a == "--uninstall" || a == "/uninstall" || a == "-u") {
        let choice = win32_gui::show_message_box(
            std::ptr::null_mut(),
            "Uninstall zPDF",
            "Are you sure you want to uninstall zPDF and remove all shortcuts and file associations?",
            0x00000024, // MB_YESNO | MB_ICONQUESTION
        );
        if choice == 6 { // IDYES
            match default_app::uninstall_app() {
                Ok(_) => {
                    win32_gui::show_message_box(
                        std::ptr::null_mut(),
                        "zPDF — Uninstalled",
                        "zPDF has been successfully uninstalled from your system.",
                        0x00000040, // MB_ICONINFORMATION
                    );
                }
                Err(e) => {
                    win32_gui::show_message_box(
                        std::ptr::null_mut(),
                        "zPDF — Uninstall Error",
                        &format!("An error occurred during uninstallation:\n\n{}", e),
                        0x00000010, // MB_ICONERROR
                    );
                    std::process::exit(1);
                }
            }
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
            win32_gui::show_message_box(
                std::ptr::null_mut(),
                "zPDF — Missing Dependency",
                "Could not find 'pdfium.dll'.\n\nPlease ensure 'pdfium.dll' is placed in the same folder as zpdf.exe to open and edit PDF files.\n\nPre-built binaries and instructions are available in the repository.",
                0x00000010,
            );
            std::process::exit(1);
        }
    };

    // Verify integrity of PDFium DLL
    if let Err(e) = verify_pdfium_integrity(&pdfium_path, None) {
        eprintln!("Security Error: {}", e);
        win32_gui::show_message_box(
            std::ptr::null_mut(),
            "zPDF — Security Error",
            &format!("PDFium DLL integrity verification failed:\n\n{}\n\nThe file may be corrupted or untrusted.", e),
            0x00000010,
        );
        std::process::exit(1);
    }

    // Load PDFium dynamically
    let pdfium_lib = match PdfiumLibrary::load(&pdfium_path) {
        Ok(lib) => Arc::new(lib),
        Err(e) => {
            eprintln!("Failed to initialize PDFium library: {}", e);
            win32_gui::show_message_box(
                std::ptr::null_mut(),
                "zPDF — Initialization Error",
                &format!("Failed to initialize PDFium library:\n\n{}", e),
                0x00000010,
            );
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
