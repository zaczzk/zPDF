// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Main Entrypoint & Self-Test

#![windows_subsystem = "windows"]

use zpdf::{default_app, editor, pdfium, wic_decoder};

mod gui;
mod security;

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use pdfium::PdfiumLibrary;
use security::harden_process_dll_search_order;

#[link(name = "kernel32")]
extern "system" {
    fn AttachConsole(process_id: u32) -> i32;
}

const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
const MB_ICONERROR: u32 = 0x0000_0010;
const MB_YESNO_QUESTION: u32 = 0x0000_0024;
const IDYES: i32 = 6;

/// pdfium.dll is loaded only from the directory that holds zpdf.exe. The current directory and
/// any well-known folder are never searched, so a planted DLL next to a PDF cannot be loaded.
fn find_pdfium_dll() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let candidate = exe.parent()?.join("pdfium.dll");
    candidate.is_file().then_some(candidate)
}

fn run_self_tests() -> Result<(), String> {
    println!("zPDF {} self-test", env!("CARGO_PKG_VERSION"));

    println!("[1/3] DLL search-order hardening");
    harden_process_dll_search_order();

    println!("[2/3] Loading pdfium.dll from the exe directory");
    let pdfium_path = find_pdfium_dll().ok_or("pdfium.dll not found next to zpdf.exe")?;
    println!("  -> {}", pdfium_path.display());
    let lib = PdfiumLibrary::load(&pdfium_path)?;

    println!("[3/3] Create, render and save a document");
    let doc = lib.create_empty_document()?;
    let page = doc.new_page(0, 612.0, 792.0)?;
    if doc.page_count() != 1 {
        return Err(format!("expected 1 page, got {}", doc.page_count()));
    }
    let (w, h) = page.dimensions();
    if (w, h) != (612.0, 792.0) {
        return Err(format!("unexpected page size {w} x {h}"));
    }
    let (render_w, render_h) = (300, 400);
    let stride = render_w * 4;
    let mut buffer = vec![0u8; (stride * render_h) as usize];
    page.render_into_buffer(&mut buffer, render_w, render_h, stride, 0, 0)?;
    let bytes = doc.save_to_bytes()?;
    if bytes.len() < 100 || !bytes.starts_with(b"%PDF") {
        return Err(format!("saved document is not a PDF ({} bytes)", bytes.len()));
    }
    println!("  -> OK ({} bytes)", bytes.len());

    println!("ALL SELF-TESTS PASSED");
    Ok(())
}

fn error_box(title: &str, text: &str) {
    gui::win32::message_box(std::ptr::null_mut(), title, text, MB_ICONERROR);
}

fn main() {
    // Lock down the DLL search order before anything else can load a library.
    harden_process_dll_search_order();

    let args: Vec<String> = env::args().collect();
    let has = |names: &[&str]| args.iter().skip(1).any(|a| names.contains(&a.as_str()));

    if has(&["--self-test", "-t"]) {
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        if let Err(e) = run_self_tests() {
            eprintln!("Self-test failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    // --install / --uninstall are kept as aliases for older docs and scripts.
    if has(&["--register", "--install", "/install", "-i"]) {
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        match default_app::register_in_place() {
            Ok(()) => {
                println!("zPDF registered as a PDF handler.");
                let choice = gui::win32::message_box(
                    std::ptr::null_mut(),
                    "zPDF",
                    "zPDF is now registered as a PDF handler for your account.\n\n\
                     Open Windows Default Apps settings to make it your default PDF viewer?",
                    MB_YESNO_QUESTION,
                );
                if choice == IDYES {
                    default_app::open_default_apps_settings();
                }
            }
            Err(e) => {
                eprintln!("Registration failed: {e}");
                error_box("zPDF — Registration Failed", &format!("Could not register zPDF:\n\n{e}"));
                std::process::exit(1);
            }
        }
        return;
    }

    // Silent: the Inno Setup uninstaller runs this. Only registry values are removed; files
    // belong to the installer.
    if has(&["--unregister", "--uninstall", "/uninstall", "-u"]) {
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        if let Err(e) = default_app::unregister() {
            eprintln!("Unregistration incomplete: {e}");
            std::process::exit(1);
        }
        println!("zPDF file associations removed.");
        return;
    }

    let pdfium_path = match find_pdfium_dll() {
        Some(p) => p,
        None => {
            error_box(
                "zPDF — Missing Dependency",
                "Could not find 'pdfium.dll'.\n\nIt must be in the same folder as zpdf.exe. \
                 Reinstall zPDF or extract the whole release archive.",
            );
            std::process::exit(1);
        }
    };

    let pdfium_lib = match PdfiumLibrary::load(&pdfium_path) {
        Ok(lib) => Arc::new(lib),
        Err(e) => {
            error_box("zPDF — Initialization Error", &format!("Failed to initialize PDFium library:\n\n{e}"));
            std::process::exit(1);
        }
    };

    let initial_pdf = args.get(1).filter(|a| !a.starts_with('-')).map(PathBuf::from);

    if let Err(e) = gui::run(pdfium_lib, initial_pdf) {
        eprintln!("Application Error: {e}");
        std::process::exit(1);
    }
}
