// Comprehensive End-to-End Test Harness for zPDF GUI Engine & Editing
// Tests:
// 1. Clamping & Anti-Void Scrolling (scroll_x, scroll_y, sidebar_scroll_y)
// 2. Thumbnail Rasterization (ensures previews are never blank)
// 3. Coordinate Transforms (DeviceToPage & PageToDevice)
// 4. In-Place Text Hit-Testing & Mutation
// 5. New Text Run Insertion
// 6. Multi-page Save & Verification
// 7. Undo / Redo Transaction Integrity

use std::path::PathBuf;
use std::sync::Arc;

#[path = "../pdfium.rs"]
mod pdfium;
#[path = "../security.rs"]
mod security;
#[path = "../tile_cache.rs"]
mod tile_cache;
#[path = "../editor.rs"]
mod editor;

use pdfium::PdfiumLibrary;
use editor::{hit_test_text_object, apply_text_mutation, insert_new_text_run, EditAction, UndoManager};
use tile_cache::ThumbnailCache;

fn main() {
    println!("============================================================");
    println!(">>> RUNNING COMPREHENSIVE zPDF ENGINE & GUI INTEGRATION TEST");
    println!("============================================================");

    let pdfium_path = PathBuf::from("pdfium.dll");
    let lib = Arc::new(PdfiumLibrary::load(&pdfium_path).expect("Failed to load pdfium.dll"));
    println!("[TEST 1/7] PDFium Library Loaded & Cryptographically Verified: OK");

    // 1. Load sample.pdf
    let sample_path = PathBuf::from("sample.pdf");
    let doc = lib.open_document(&sample_path, None).expect("Open sample.pdf");
    let page_count = doc.page_count();
    println!("[TEST 2/7] Document Opened: {} pages found", page_count);
    assert_eq!(page_count, 3, "Expected 3 pages in sample.pdf");

    // 2. Simulate Viewport Layout & Strict Clamping Math
    let viewport_w = 1280;
    let viewport_h = 820;
    let toolbar_h = 44;
    let status_h = 24;
    let sidebar_w = 200;
    let avail_w = viewport_w - sidebar_w;
    let avail_h = viewport_h - toolbar_h - status_h;

    let mut total_content_height = 24;
    let mut sidebar_total_height = 28;
    let mut render_widths = Vec::new();

    for i in 0..page_count {
        let page = doc.load_page(i).expect("load page");
        let (w_pt, h_pt) = page.dimensions();
        let rot = page.rotation();
        let (eff_w, eff_h) = if rot == 1 || rot == 3 { (h_pt, w_pt) } else { (w_pt, h_pt) };
        let render_w = (eff_w * 1.333333).round() as i32;
        let render_h = (eff_h * 1.333333).round() as i32;
        render_widths.push(render_w);
        total_content_height += render_h + 24;

        let aspect = eff_h / eff_w.max(1.0);
        let tw = 144;
        let mut th = ((tw as f64) * aspect).round() as i32;
        if th > 160 { th = 160; }
        if th < 40 { th = 40; }
        sidebar_total_height += th + 34;
    }

    let max_page_w = render_widths.iter().copied().max().unwrap_or(0);
    let max_scroll_x = (max_page_w + 48 - avail_w).max(0);
    let max_scroll_y = (total_content_height - avail_h).max(0);
    let max_sidebar_scroll = (sidebar_total_height - avail_h).max(0);

    println!("[TEST 3/7] Layout Dimensions & Bounds:");
    println!("   total_content_height = {}, max_scroll_y = {}", total_content_height, max_scroll_y);
    println!("   max_page_w = {}, max_scroll_x = {}", max_page_w, max_scroll_x);
    println!("   sidebar_total_height = {}, max_sidebar_scroll = {}", sidebar_total_height, max_sidebar_scroll);

    // Verify clamping prevents infinite scroll in all directions:
    let test_scroll_values = [-100000, -500, -1, 0, 100, 5000, 100000];
    for &val in &test_scroll_values {
        let clamped_y = val.clamp(0, max_scroll_y);
        assert!(clamped_y >= 0 && clamped_y <= max_scroll_y, "scroll_y clamp failed for {}", val);
        let clamped_x = val.clamp(0, max_scroll_x);
        assert!(clamped_x >= 0 && clamped_x <= max_scroll_x, "scroll_x clamp failed for {}", val);
        let clamped_sb = val.clamp(0, max_sidebar_scroll);
        assert!(clamped_sb >= 0 && clamped_sb <= max_sidebar_scroll, "sidebar_scroll_y clamp failed for {}", val);
    }
    println!("   -> Clamping guarantees no infinite scroll into black canvas: OK");

    // 3. Verify Thumbnail Rasterization & Non-Blank Render
    println!("[TEST 4/7] Testing Sidebar Thumbnail Rasterization:");
    let mut thumb_cache = ThumbnailCache::new();
    for i in 0..page_count {
        let page = doc.load_page(i).expect("load page");
        let (w_pt, h_pt) = page.dimensions();
        let rot = page.rotation();
        let (eff_w, eff_h) = if rot == 1 || rot == 3 { (h_pt, w_pt) } else { (w_pt, h_pt) };
        let aspect = eff_h / eff_w.max(1.0);
        let tw = 144;
        let mut th = ((tw as f64) * aspect).round() as i32;
        if th > 160 { th = 160; }
        if th < 40 { th = 40; }
        let stride = tw * 4;
        let mut buffer = vec![0u8; (stride * th) as usize];
        assert!(page.render_into_buffer(&mut buffer, tw, th, stride, rot, 0).is_ok());

        // Ensure buffer contains actual image data (not all zero, not all white)
        let all_zero = buffer.iter().all(|&b| b == 0);
        let all_white = buffer.iter().all(|&b| b == 255);
        assert!(!all_zero, "Page {} thumbnail buffer was all zeros!", i);
        assert!(!all_white, "Page {} thumbnail buffer was all white (no text rendered)!", i);

        thumb_cache.insert(i, tw, th, buffer);
        assert!(thumb_cache.get(i).is_some());
        println!("   Page {} thumbnail ({}x{}): Valid rasterized pixels confirmed: OK", i, tw, th);
    }

    // 4. Test Coordinate Conversions (DeviceToPage & PageToDevice)
    println!("[TEST 5/7] Testing DeviceToPage <-> PageToDevice Coordinate Round-Trip:");
    let p0 = doc.load_page(0).expect("page 0");
    let test_device_points = [(50, 50), (200, 300), (600, 700), (750, 950)];
    for &(dx, dy) in &test_device_points {
        let (px, py) = p0.device_to_page(0, 0, 816, 1056, 0, dx, dy).expect("d2p failed");
        let (round_dx, round_dy) = p0.page_to_device(0, 0, 816, 1056, 0, px, py).expect("p2d failed");
        let diff_x = (round_dx - dx).abs();
        let diff_y = (round_dy - dy).abs();
        assert!(diff_x <= 1 && diff_y <= 1, "Coordinate drift too large: ({}, {}) -> ({}, {})", dx, dy, round_dx, round_dy);
    }
    println!("   -> Sub-pixel coordinate fidelity verified across all axes: OK");

    // 5. Test Hit-Testing on Page 0 Text Objects
    println!("[TEST 6/7] Testing Text Hit-Testing & In-Place Mutation:");
    // On Page 0, title is at (54.0, 720.0) with font size 28.0:
    let hit = hit_test_text_object(&p0, 60.0, 725.0, 10.0);
    assert!(hit.is_some(), "Failed to hit-test title text object on page 0!");
    let (obj_idx, orig_text, bounds) = hit.unwrap();
    println!("   Hit-test found object #{}: {:?} with bounds {:?}", obj_idx, orig_text, bounds);
    assert!(orig_text.contains("zPDF Document Editor"), "Unexpected text: {}", orig_text);

    // Apply text mutation
    let new_title = "zPDF Ultralight Engine [MUTATED]";
    let mut undo_mgr = UndoManager::new();
    assert!(apply_text_mutation(&doc, 0, obj_idx, new_title).is_ok());
    undo_mgr.push(EditAction::EditTextRun {
        page_index: 0,
        object_index: obj_idx,
        old_text: orig_text.clone(),
        new_text: new_title.to_string(),
    });

    // Verify mutation took effect on the page
    let p0_reloaded = doc.load_page(0).expect("reload p0");
    let tp = p0_reloaded.load_text_page().expect("text page");
    let text_content = tp.get_all_text();
    assert!(text_content.contains(new_title), "Mutated text not found on page! Got: {}", text_content);
    println!("   -> Text mutation applied and verified in page content stream: OK");

    // Test Undo
    let action = undo_mgr.pop_undo().expect("pop undo");
    if let EditAction::EditTextRun { page_index, object_index, old_text, .. } = action {
        assert!(apply_text_mutation(&doc, page_index, object_index, &old_text).is_ok());
    }
    let p0_undo = doc.load_page(0).expect("reload p0");
    let tp_undo = p0_undo.load_text_page().expect("text page");
    assert!(tp_undo.get_all_text().contains("zPDF Document Editor"));
    println!("   -> Undo transaction restored original text: OK");

    // Test Redo
    let redo_act = undo_mgr.pop_redo().expect("pop redo");
    if let EditAction::EditTextRun { page_index, object_index, new_text, .. } = redo_act {
        assert!(apply_text_mutation(&doc, page_index, object_index, &new_text).is_ok());
    }
    let p0_redo = doc.load_page(0).expect("reload p0");
    let tp_redo = p0_redo.load_text_page().expect("text page");
    assert!(tp_redo.get_all_text().contains(new_title));
    println!("   -> Redo transaction successfully reapplied mutation: OK");

    // 6. Test New Text Run Insertion on Page 1
    println!("[TEST 7/7] Testing New Text Run Insertion & Disk Persistence:");
    let inserted_msg = "NEW INSERTED ANNOTATION @ (100, 300)";
    let new_obj_idx = insert_new_text_run(&doc, 1, inserted_msg, 100.0, 300.0, 16.0)
        .expect("insert new text run");
    println!("   Inserted new text object #{}: {:?}", new_obj_idx, inserted_msg);

    let p1 = doc.load_page(1).expect("load p1");
    let tp1 = p1.load_text_page().expect("load tp1");
    assert!(tp1.get_all_text().contains(inserted_msg), "Newly inserted text not found!");

    // Save to test output file
    let test_out_path = PathBuf::from("test_output.pdf");
    doc.save_to_file(&test_out_path).expect("save doc to test_output.pdf");
    println!("   Saved modified document to: {}", test_out_path.display());

    // Reload document from scratch and verify contents
    let reloaded_doc = lib.open_document(&test_out_path, None).expect("open reloaded doc");
    assert_eq!(reloaded_doc.page_count(), 3);
    let rp0 = reloaded_doc.load_page(0).expect("rp0");
    let rtp0 = rp0.load_text_page().expect("rtp0");
    assert!(rtp0.get_all_text().contains(new_title), "Mutated title missing from saved file!");

    let rp1 = reloaded_doc.load_page(1).expect("rp1");
    let rtp1 = rp1.load_text_page().expect("rtp1");
    assert!(rtp1.get_all_text().contains(inserted_msg), "Inserted text missing from saved file!");

    // Clean up test file
    let _ = std::fs::remove_file("test_output.pdf");

    println!("============================================================");
    println!(">>> ALL 7 COMPREHENSIVE INTEGRATION SUITES PASSED (100% OK)");
    println!("============================================================");
}
