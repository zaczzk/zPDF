// End-to-end tests of the zPDF engine (PDFium wrappers + editor) against a real pdfium.dll.
//
// Skips (prints and passes) when pdfium.dll is not found next to the test
// executable or in CARGO_MANIFEST_DIR. Everything runs inside one #[test]
// because PDFium is not thread-safe and the test harness runs tests in parallel.

use std::path::{Path, PathBuf};

use zpdf::editor::{
    add_highlight_annot, apply_redaction, apply_text_mutation, hit_test_text_object, insert_image_object,
    insert_new_text_run, remove_annot, EditAction, UndoManager, HIGHLIGHT_COLOR,
};
use zpdf::pdfium::{PdfDocument, PdfPage, PdfiumLibrary, RectF, FPDF_PAGEOBJ_IMAGE, FPDF_FILLMODE_WINDING};

const TITLE: &str = "zPDF Document Editor";
const SECOND: &str = "Second line of text";
const IMG_PX: u32 = 192;
const IMG_X: f64 = 300.0;
const IMG_Y: f64 = 300.0;
const IMG_PT: f64 = 200.0;

fn find_pdfium() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let candidates = [
        exe_dir.join("pdfium.dll"),
        exe_dir.parent().map(|p| p.join("pdfium.dll")).unwrap_or_default(),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("pdfium.dll"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zpdf_engine_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Deterministic, incompressible pixel data (xorshift), fully opaque.
fn noise_pixels() -> Vec<u8> {
    let mut s: u32 = 0x9E37_79B9;
    let mut px = Vec::with_capacity((IMG_PX * IMG_PX * 4) as usize);
    for _ in 0..IMG_PX * IMG_PX {
        for _ in 0..3 {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            px.push((s >> 24) as u8);
        }
        px.push(0xFF);
    }
    px
}

/// Two-page letter document: page 0 has two text runs and a noise image,
/// page 1 is blank.
fn build_sample(lib: &PdfiumLibrary) -> PdfDocument {
    let doc = lib.create_empty_document().expect("create doc");
    doc.new_page(0, 612.0, 792.0).expect("page 0");
    doc.new_page(1, 612.0, 792.0).expect("page 1");
    insert_new_text_run(&doc, 0, TITLE, 54.0, 720.0, 28.0).expect("title");
    insert_new_text_run(&doc, 0, SECOND, 54.0, 680.0, 14.0).expect("second line");
    insert_image_object(&doc, 0, IMG_PX, IMG_PX, noise_pixels(), IMG_X, IMG_Y, IMG_PT, IMG_PT).expect("image");
    doc
}

fn page_has_text(doc: &PdfDocument, page: usize, needle: &str) -> bool {
    let page = doc.load_page(page).expect("load page");
    !page.load_text_page().expect("text page").search(needle, true).is_empty()
}

/// Renders at 1px per point (rotate = 0) and returns (buffer, width, height).
fn render(page: &PdfPage) -> (Vec<u8>, i32, i32) {
    let (w, h) = page.dimensions();
    let (w, h) = (w.round() as i32, h.round() as i32);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    page.render_region(&mut buf, w, h, w * 4, 0, 0, w, h, None).expect("render");
    (buf, w, h)
}

/// BGRA pixel at PDF page coordinates (unrotated page, 1px per point).
fn pixel_at(buf: &[u8], w: i32, h: i32, x: i32, y_pdf: i32) -> [u8; 4] {
    let y = h - 1 - y_pdf;
    let i = ((y * w + x) * 4) as usize;
    buf[i..i + 4].try_into().unwrap()
}

#[test]
fn engine_end_to_end() {
    let Some(dll) = find_pdfium() else {
        println!("SKIPPED: pdfium.dll not found next to the test exe or in CARGO_MANIFEST_DIR");
        return;
    };
    let lib = PdfiumLibrary::load(&dll).expect("load pdfium.dll");
    let dir = temp_dir();

    rotation_semantics(&lib);
    open_save_in_place(&lib, &dir);
    highlight_annotations(&lib);
    redaction_overwrites_image_data(&lib, &dir);
    redaction_partial_image_text_and_annots(&lib, &dir);
    navigation_and_passwords(&lib);
    undo_redo_saved_point(&lib);

    let _ = std::fs::remove_dir_all(&dir);
}

/// Documents how PDFium treats /Rotate: sizes already include it, and
/// rendering with rotate = 0 already applies it.
fn rotation_semantics(lib: &PdfiumLibrary) {
    let doc = lib.create_empty_document().expect("doc");
    let page = doc.new_page(0, 612.0, 792.0).expect("page");
    // Black square in the unrotated page's top-left corner.
    let path = doc.create_path(0.0, 692.0).expect("path");
    path.path_line_to(100.0, 692.0);
    path.path_line_to(100.0, 792.0);
    path.path_line_to(0.0, 792.0);
    path.path_close();
    path.path_set_draw_mode(FPDF_FILLMODE_WINDING, false);
    path.set_fill_color(0, 0, 0, 255);
    page.insert_object(&path);
    assert!(page.generate_content());
    page.set_rotation(1);
    drop(page);

    let page = doc.load_page(0).expect("reload");
    let (w, h) = page.dimensions();
    let size = doc.page_size(0).expect("page_size");
    println!("rotation=1: dimensions={:?} page_size={:?} (unrotated 612x792)", (w, h), size);
    assert_eq!((w.round(), h.round()), (792.0, 612.0), "FPDF_GetPageWidthF/HeightF include /Rotate");
    assert_eq!((size.0.round(), size.1.round()), (792.0, 612.0), "FPDF_GetPageSizeByIndexF includes /Rotate");

    // Rendered with rotate=0 into the rotated (792x612) frame, a 90° clockwise
    // turn moves the top-left square to the top-right corner.
    let (bw, bh) = (792, 612);
    let mut buf = vec![0u8; bw * bh * 4];
    page.render_into_buffer(&mut buf, bw as i32, bh as i32, (bw * 4) as i32, 0, 0).expect("render");
    let px = |x: usize, y: usize| buf[(y * bw + x) * 4];
    println!("rotate=0 render: top-left={} top-right={}", px(50, 50), px(bw - 50, 50));
    assert!(px(bw - 50, 50) < 40 && px(50, 50) > 200, "FPDF_RenderPageBitmap(rotate=0) applies the page's /Rotate");

    // Passing the page's own rotation again stacks on top of /Rotate (here
    // 90° + 90° = 180°): the square lands bottom-right of an upright frame.
    let (bw, bh) = (612, 792);
    let mut buf = vec![0u8; bw * bh * 4];
    page.render_into_buffer(&mut buf, bw as i32, bh as i32, (bw * 4) as i32, page.rotation(), 0).expect("render");
    let px = |x: usize, y: usize| buf[(y * bw + x) * 4];
    assert!(px(bw - 50, bh - 50) < 40 && px(50, 50) > 200, "rotate argument is added to /Rotate");
}

fn open_save_in_place(lib: &PdfiumLibrary, dir: &Path) {
    let path = dir.join("sample.pdf");
    build_sample(lib).save_to_file(&path).expect("save sample");

    let doc = lib.open_document(&path, None).expect("open from file");
    assert_eq!(doc.page_count(), 2);
    assert_eq!(doc.page_size(0), Some((612.0, 792.0)));
    assert!(page_has_text(&doc, 0, TITLE));

    // Text hit-test + mutation.
    let page = doc.load_page(0).expect("page 0");
    let (obj_idx, text, bounds) = hit_test_text_object(&page, 60.0, 725.0, 10.0).expect("hit title");
    assert!(text.contains(TITLE), "hit-test returned {:?}", text);
    assert!(bounds.contains(60.0, 725.0));
    drop(page);
    apply_text_mutation(&doc, 0, obj_idx, "Mutated Title").expect("mutate");
    assert!(page_has_text(&doc, 0, "Mutated Title"));

    // Save over the file the document was opened from, keep using the open
    // document, then reopen from disk.
    doc.save_to_file(&path).expect("save in place");
    assert!(page_has_text(&doc, 0, SECOND), "open document still readable after in-place save");
    assert!(!dir.read_dir().unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains("zpdf-tmp")));
    drop(doc);
    let reopened = lib.open_document(&path, None).expect("reopen");
    assert_eq!(reopened.page_count(), 2);
    assert!(page_has_text(&reopened, 0, "Mutated Title"));
    assert!(page_has_text(&reopened, 0, SECOND));
}

fn highlight_annotations(lib: &PdfiumLibrary) {
    let doc = build_sample(lib);
    let rects = [
        RectF { left: 100.0, top: 150.0, right: 300.0, bottom: 120.0 },
        RectF { left: 100.0, top: 110.0, right: 250.0, bottom: 80.0 },
    ];
    let idx = add_highlight_annot(&doc, 1, &rects, HIGHLIGHT_COLOR).expect("add highlight");
    let page = doc.load_page(1).expect("page 1");
    assert_eq!(page.annot_count(), 1);
    let (buf, w, h) = render(&page);
    let inside = pixel_at(&buf, w, h, 200, 135);
    let outside = pixel_at(&buf, w, h, 200, 400);
    println!("highlight pixel inside={:?} outside={:?}", inside, outside);
    assert_eq!(outside, [255, 255, 255, 255]);
    // Yellow: blue channel well below red/green.
    assert!(inside[0] < 120 && inside[1] > 150 && inside[2] > 200, "highlight annotation renders");
    drop(page);

    // Persists through save/reload.
    let reloaded = lib.open_document_from_bytes(doc.save_to_bytes().expect("save")).expect("reload");
    assert_eq!(reloaded.load_page(1).expect("page").annot_count(), 1);

    remove_annot(&doc, 1, idx).expect("remove highlight");
    let page = doc.load_page(1).expect("page 1");
    assert_eq!(page.annot_count(), 0);
    let (buf, w, h) = render(&page);
    assert_eq!(pixel_at(&buf, w, h, 200, 135), [255, 255, 255, 255], "highlight gone after removal");
}

fn image_rect() -> RectF {
    RectF { left: IMG_X as f32, bottom: IMG_Y as f32, right: (IMG_X + IMG_PT) as f32, top: (IMG_Y + IMG_PT) as f32 }
}

/// Redacting the whole image must remove its (incompressible) pixel data from
/// the saved file, not just hide it.
fn redaction_overwrites_image_data(lib: &PdfiumLibrary, dir: &Path) {
    let path = dir.join("redact_full.pdf");
    build_sample(lib).save_to_file(&path).expect("save");
    let before = std::fs::metadata(&path).unwrap().len();
    let image_bytes = (IMG_PX * IMG_PX * 3) as u64;
    assert!(before > image_bytes, "image data present before redaction ({} bytes)", before);

    let doc = lib.open_document(&path, None).expect("open");
    let affected = apply_redaction(&doc, 0, image_rect(), true).expect("redact");
    assert!(affected >= 1);
    doc.save_to_file(&path).expect("save in place");
    drop(doc);
    let after = std::fs::metadata(&path).unwrap().len();
    println!("redaction: file {} -> {} bytes (image data {} bytes)", before, after, image_bytes);
    assert!(after + image_bytes * 9 / 10 < before, "original image bytes are gone from the saved file");

    let doc = lib.open_document(&path, None).expect("reopen");
    assert!(page_has_text(&doc, 0, SECOND), "text outside the redaction survives");
}

fn redaction_partial_image_text_and_annots(lib: &PdfiumLibrary, dir: &Path) {
    let doc = build_sample(lib);
    // Highlight over the second line, then redact the left half of the image
    // and the second line.
    add_highlight_annot(&doc, 0, &[RectF { left: 54.0, top: 695.0, right: 180.0, bottom: 678.0 }], HIGHLIGHT_COLOR).expect("annot");
    let left_half = RectF { right: (IMG_X + IMG_PT / 2.0) as f32, ..image_rect() };
    apply_redaction(&doc, 0, left_half, false).expect("redact image half");
    let affected = apply_redaction(&doc, 0, RectF { left: 50.0, top: 700.0, right: 200.0, bottom: 675.0 }, true).expect("redact line");
    assert!(affected >= 2, "text object and highlight annotation removed (got {})", affected);

    let path = dir.join("redact_partial.pdf");
    doc.save_to_file(&path).expect("save");
    drop(doc);
    let doc = lib.open_document(&path, None).expect("reopen");
    assert!(!page_has_text(&doc, 0, SECOND), "redacted text is gone");
    assert!(page_has_text(&doc, 0, TITLE), "unrelated text survives");
    let page = doc.load_page(0).expect("page");
    assert_eq!(page.annot_count(), 0, "intersecting annotation removed");

    let img = (0..page.count_objects())
        .filter_map(|i| page.get_object(i))
        .find(|o| o.get_type() == FPDF_PAGEOBJ_IMAGE)
        .expect("image object still present");
    let mut bmp = img.get_image_bitmap().expect("image bitmap");
    let (stride, bpp) = (bmp.stride() as usize, bmp.bytes_per_pixel());
    assert!(bpp >= 3, "decoded format {}", bmp.format());
    let pixels = bmp.pixels_mut();
    let original = noise_pixels();
    let row = 96usize;
    let px = |x: usize| &pixels[row * stride + x * bpp..][..3];
    let orig = |x: usize| &original[(row * IMG_PX as usize + x) * 4..][..3];
    assert_eq!(px(10), &[0, 0, 0], "left half blacked out");
    assert_eq!(px(180), orig(180), "right half untouched");
}

fn undo_redo_saved_point(lib: &PdfiumLibrary) {
    let doc = build_sample(lib);
    let page = doc.load_page(0).expect("page");
    let (obj, old_text, _) = hit_test_text_object(&page, 60.0, 725.0, 10.0).expect("hit");
    drop(page);

    let mut undo = UndoManager::new();
    assert!(undo.is_at_saved_point());
    apply_text_mutation(&doc, 0, obj, "Edited").expect("edit");
    undo.push(EditAction::EditTextRun { page_index: 0, object_index: obj, old_text: old_text.clone(), new_text: "Edited".into() });
    assert!(!undo.is_at_saved_point());
    undo.mark_saved();

    match undo.undo() {
        Some(EditAction::EditTextRun { page_index, object_index, old_text, .. }) => {
            apply_text_mutation(&doc, *page_index, *object_index, old_text).expect("undo")
        }
        other => panic!("unexpected undo action {:?}", other),
    }
    assert!(page_has_text(&doc, 0, TITLE));
    assert!(!undo.is_at_saved_point(), "undo past the save is dirty");
    assert!(undo.can_redo());

    match undo.redo() {
        Some(EditAction::EditTextRun { page_index, object_index, new_text, .. }) => {
            apply_text_mutation(&doc, *page_index, *object_index, new_text).expect("redo")
        }
        other => panic!("unexpected redo action {:?}", other),
    }
    assert!(page_has_text(&doc, 0, "Edited"));
    assert!(undo.is_at_saved_point(), "redo back to the save is clean");
}

/// Outline, links, reading-order text ranges and password handling, against hand-written
/// fixtures (PDFium can't author outlines or encryption itself).
fn navigation_and_passwords(lib: &PdfiumLibrary) {
    use zpdf::pdfium::{LinkTarget, ERR_PASSWORD};
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures");

    let doc = lib.open_document(&fixtures.join("links.pdf"), None).expect("open links.pdf");
    let outline = doc.outline();
    let titles: Vec<(&str, usize)> = outline.iter().map(|o| (o.title.as_str(), o.depth)).collect();
    assert_eq!(
        titles,
        [("Chapter 1: Introduction", 0), ("Chapter 2: Middle", 0), ("Chapter 3: End", 0), ("Section 3.1", 1)],
        "outline is flattened depth-first"
    );
    assert_eq!(outline[3].target, Some(LinkTarget::Page { index: 2, y: Some(420.0) }));

    let page = doc.load_page(0).expect("page 1");
    assert_eq!(doc.link_at(&page, 100.0, 645.0), Some(LinkTarget::Page { index: 2, y: Some(420.0) }), "internal link");
    assert_eq!(doc.link_at(&page, 100.0, 605.0), Some(LinkTarget::Uri("https://example.com/".into())), "web link");
    assert_eq!(doc.link_at(&page, 500.0, 100.0), None, "no link in empty space");

    let tp = page.load_text_page().expect("text page");
    let start = tp.get_char_index_at_pos(76.0, 563.0, 5.0, 5.0).expect("char under 'The'");
    assert_eq!(tp.text_range(start, 9), "The quick");
    let rects = tp.range_rects(start, 9);
    assert_eq!(rects.len(), 1, "a single-line range has one rect");
    println!("[nav] outline, links and text ranges OK");

    let locked = fixtures.join("locked.pdf");
    let err = lib.open_document(&locked, None).err().expect("locked.pdf needs a password");
    assert!(err.contains(ERR_PASSWORD), "unexpected error: {err}");
    assert!(lib.open_document(&locked, Some("nope")).is_err(), "wrong password rejected");
    let unlocked = lib.open_document(&locked, Some("test")).expect("right password opens it");
    assert_eq!(unlocked.page_count(), 1);
    println!("[nav] password handling OK");
}
