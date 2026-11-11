//! Document operations: open/save, edits, and undo/redo.

use super::app::*;
use super::win32::*;
use crate::editor::{self, EditAction};
use crate::pdfium::RectF;
use std::path::Path;
use std::ptr;

const PDF_FILTER: &str = "PDF documents (*.pdf)\0*.pdf\0All files (*.*)\0*.*\0";

// ---------------------------------------------------------------------------
// Open / save
// ---------------------------------------------------------------------------

/// PDFium writes a form field's value only when the field loses focus, so take focus away before
/// saving, snapshotting or closing; otherwise the last typed value is lost.
pub fn commit_form(app: &mut App) {
    if let Some(d) = &mut app.doc {
        if d.form_focus.take().is_some() {
            if let Some(f) = &d.form {
                f.kill_focus();
            }
        }
    }
}

/// Asks to save unsaved changes. Returns false if the user cancelled.
pub fn confirm_discard(app: &mut App) -> bool {
    commit_editor(app);
    commit_gizmo(app);
    commit_form(app);
    let Some(doc) = &app.doc else { return true };
    if !doc.is_dirty() {
        return true;
    }
    let msg = format!("Save changes to \"{}\" before closing it?", doc.name());
    match message_box(app.hwnd, "zPDF", &msg, MB_YESNOCANCEL | MB_ICONWARNING) {
        IDYES => {
            save(app, false);
            !app.doc.as_ref().is_some_and(|d| d.is_dirty())
        }
        IDNO => true,
        _ => false,
    }
}

pub fn prompt_open(app: &mut App) {
    if !confirm_discard(app) {
        return;
    }
    if let Some(path) = file_dialog(app.hwnd, false, "Open PDF", PDF_FILTER, None, None) {
        open_file(app, &path);
    }
}

pub fn open_file(app: &mut App, path: &Path) {
    open_with(app, path, None);
}

fn open_with(app: &mut App, path: &Path, password: Option<&str>) -> bool {
    let pdf = match app.pdfium.open_document(path, password) {
        Ok(d) => d,
        Err(e) if e.contains(crate::pdfium::ERR_PASSWORD) => {
            show_password_prompt(app, path, password.is_some());
            return false;
        }
        Err(e) => {
            message_box(app.hwnd, "zPDF", &format!("Couldn't open \"{}\".\n\n{e}", path.display()), MB_ICONERROR);
            return false;
        }
    };
    remember_position(app);
    cancel_editor(app);
    app.gizmo = None;
    app.selection.clear();
    app.sel_text = None; // holds a page of the old document
    app.drag = Drag::None;
    app.back_stack.clear();
    app.outline_scroll = 0;
    app.doc = None; // close the old document (and its form env) first
    app.doc = Some(Doc::new(pdf, Some(path.to_path_buf())));
    app.scroll_x = 0;
    app.scroll_y = 0;
    app.target_y = 0.0;
    app.sidebar_scroll = 0;
    // Come back to where the reader left off.
    let pos = super::settings::position(path);
    if let Some(p) = pos {
        app.fit = match p.fit {
            1 => Fit::Width,
            2 => Fit::Page,
            _ => Fit::None,
        };
        app.zoom = p.zoom.clamp(super::view::MIN_ZOOM, super::view::MAX_ZOOM);
    }
    app.structure_changed();
    if app.fit != Fit::None {
        app.relayout();
    }
    if let Some(p) = pos {
        let page = p.page.min(app.page_count().saturating_sub(1));
        app.go_to_page(page);
        app.scroll_y = app.target_y.round() as i32;
    }
    app.recent = super::settings::add_recent(path);
    if app.search.open {
        super::search::run(app);
    }
    true
}

/// Records the current page and zoom for the open document (called before closing it).
pub fn remember_position(app: &App) {
    let Some(path) = app.doc.as_ref().and_then(|d| d.path.clone()) else { return };
    let fit = match app.fit {
        Fit::None => 0,
        Fit::Width => 1,
        Fit::Page => 2,
    };
    super::settings::save_position(&path, super::settings::Position { page: app.current_page(), zoom: app.zoom, fit });
}

// ---------------------------------------------------------------------------
// Password prompt
// ---------------------------------------------------------------------------

static mut OLD_PW_PROC: isize = 0;

unsafe extern "system" fn password_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let app = super::app_ptr();
    if !app.is_null() {
        match (msg, wparam as i32) {
            (WM_KEYDOWN, VK_RETURN) => {
                submit_password(&mut *app);
                return 0;
            }
            (WM_KEYDOWN, VK_ESCAPE) => {
                cancel_password(&mut *app);
                return 0;
            }
            (WM_CHAR, 0x0D) | (WM_CHAR, 0x1B) => return 0,
            _ => {}
        }
    }
    CallWindowProcW(OLD_PW_PROC, hwnd, msg, wparam, lparam)
}

fn show_password_prompt(app: &mut App, path: &Path, wrong: bool) {
    if let Some(p) = &mut app.password {
        p.wrong = wrong;
        unsafe {
            SetWindowTextW(p.edit, wide("").as_ptr());
            SetFocus(p.edit);
        }
        app.invalidate();
        return;
    }
    unsafe {
        let class = wide("EDIT");
        let h = CreateWindowExW(0, class.as_ptr(), wide("").as_ptr(), WS_CHILD | WS_VISIBLE | ES_PASSWORD | ES_AUTOHSCROLL, 0, 0, 0, 0, app.hwnd, ID_PASSWORD as HMENU, GetModuleHandleW(ptr::null()), ptr::null_mut());
        if h.is_null() {
            return;
        }
        SendMessageW(h, WM_SETFONT, app.gfx.font(super::gfx::Font::Body) as WPARAM, 1);
        OLD_PW_PROC = SetWindowLongPtrW(h, GWLP_WNDPROC, password_proc as *const () as isize);
        SetFocus(h);
        app.password = Some(PasswordPrompt { path: path.to_path_buf(), edit: h, wrong, card: RECT::default(), ok: RECT::default(), cancel: RECT::default() });
    }
    app.invalidate();
}

pub fn submit_password(app: &mut App) {
    let Some(p) = &app.password else { return };
    let (path, pw) = (p.path.clone(), window_text(p.edit));
    if open_with(app, &path, Some(&pw)) {
        cancel_password(app);
    }
}

pub fn cancel_password(app: &mut App) {
    if let Some(p) = app.password.take() {
        unsafe {
            DestroyWindow(p.edit);
            SetFocus(app.hwnd);
        }
        app.invalidate();
    }
}

pub fn save(app: &mut App, save_as: bool) {
    commit_editor(app);
    commit_gizmo(app);
    commit_form(app);
    let Some(doc) = &app.doc else { return };
    let target = match (&doc.path, save_as) {
        (Some(p), false) => p.clone(),
        _ => {
            let initial = doc.name();
            match file_dialog(app.hwnd, true, "Save PDF as", PDF_FILTER, Some("pdf"), Some(&initial)) {
                Some(p) => p,
                None => return,
            }
        }
    };
    let doc = app.doc.as_mut().unwrap();
    match doc.pdf.save_to_file(&target) {
        Ok(()) => {
            doc.history.mark_saved();
            doc.form_dirty = false;
            doc.path = Some(target.clone());
            app.recent = super::settings::add_recent(&target);
            app.update_title();
            app.toast("Saved", ToastKind::Success);
        }
        Err(e) => {
            message_box(app.hwnd, "zPDF", &format!("Couldn't save \"{}\".\n\n{e}", target.display()), MB_ICONERROR);
        }
    }
}

// ---------------------------------------------------------------------------
// Edit history
// ---------------------------------------------------------------------------

fn record(app: &mut App, action: EditAction) {
    if let Some(doc) = &mut app.doc {
        doc.history.push(action);
    }
}

fn action_page(a: &EditAction) -> Option<usize> {
    match a {
        EditAction::EditTextRun { page_index, .. }
        | EditAction::InsertTextRun { page_index, .. }
        | EditAction::InsertImage { page_index, .. }
        | EditAction::InsertStamp { page_index, .. }
        | EditAction::MoveObject { page_index, .. }
        | EditAction::HighlightAnnot { page_index, .. } => Some(*page_index),
        _ => None,
    }
}

fn stamp(pdf: &crate::pdfium::PdfDocument, page: usize, check: bool, x: f64, y: f64, size: f32) -> Result<usize, String> {
    if check {
        editor::insert_stamp_checkmark(pdf, page, x, y, size)
    } else {
        editor::insert_stamp_cross(pdf, page, x, y, size)
    }
}

/// Applies `a` backwards (undo) or forwards (redo), updating it in place: redo can give
/// re-created objects a new index.
fn apply(app: &mut App, a: &mut EditAction, forward: bool) -> Result<(), String> {
    let doc = app.doc.as_mut().ok_or("no document")?;
    let pdf = &doc.pdf;
    match a {
        EditAction::RotatePage { page_index, old_rotation, new_rotation } => {
            let page = pdf.load_page(*page_index)?;
            page.set_rotation(if forward { *new_rotation } else { *old_rotation });
        }
        EditAction::MoveObject { page_index, object_index, dx, dy } => {
            let s = if forward { 1.0 } else { -1.0 };
            editor::move_page_object(pdf, *page_index, *object_index, *dx * s, *dy * s)?;
        }
        EditAction::EditTextRun { page_index, object_index, old_text, new_text } => {
            editor::apply_text_mutation(pdf, *page_index, *object_index, if forward { new_text } else { old_text })?;
        }
        EditAction::InsertTextRun { page_index, object_index, text, page_x, page_y, font_size } => {
            if forward {
                *object_index = editor::insert_new_text_run(pdf, *page_index, text, *page_x, *page_y, *font_size)?;
            } else {
                editor::remove_page_object(pdf, *page_index, *object_index)?;
            }
        }
        EditAction::InsertStamp { page_index, object_index, checkmark, page_x, page_y, size_pt } => {
            if forward {
                *object_index = stamp(pdf, *page_index, *checkmark, *page_x, *page_y, *size_pt)?;
            } else {
                editor::remove_page_object(pdf, *page_index, *object_index)?;
            }
        }
        EditAction::InsertImage { page_index, object_index, width_px, height_px, bgra_pixels, page_x, page_y, width_pt, height_pt } => {
            if forward {
                *object_index = editor::insert_image_object(pdf, *page_index, *width_px, *height_px, bgra_pixels.clone(), *page_x, *page_y, *width_pt, *height_pt)?;
            } else {
                editor::remove_page_object(pdf, *page_index, *object_index)?;
            }
        }
        EditAction::HighlightAnnot { page_index, annot_index, rects, color } => {
            if forward {
                *annot_index = editor::add_highlight_annot(pdf, *page_index, rects, *color)?;
            } else {
                editor::remove_annot(pdf, *page_index, *annot_index)?;
            }
        }
        EditAction::InsertBlankPage { page_index, width_pt, height_pt } => {
            if forward {
                pdf.new_page(*page_index, *width_pt, *height_pt)?;
            } else if !pdf.delete_page(*page_index) {
                return Err("couldn't remove the page".into());
            }
        }
        EditAction::DocumentSnapshot { before_bytes, after_bytes, .. } => {
            let bytes = if forward { after_bytes.clone() } else { before_bytes.clone() };
            let new_pdf = app.pdfium.open_document_from_bytes(bytes)?;
            app.sel_text = None;
            let doc = app.doc.as_mut().unwrap();
            doc.pages.clear(doc.form.as_ref());
            doc.form = None;
            doc.form = new_pdf.init_form_env();
            doc.outline = new_pdf.outline();
            doc.pdf = new_pdf;
            doc.form_focus = None;
        }
    }
    Ok(())
}

fn after_apply(app: &mut App, a: &EditAction, forward: bool) {
    match (action_page(a), a) {
        (Some(p), _) => app.page_changed(p),
        (None, EditAction::DocumentSnapshot { active_page_before, active_page_after, .. }) => {
            app.structure_changed();
            app.go_to_page(if forward { *active_page_after } else { *active_page_before });
        }
        (None, EditAction::InsertBlankPage { page_index, .. }) => {
            app.structure_changed();
            app.go_to_page((*page_index).min(app.page_count().saturating_sub(1)));
        }
        _ => app.structure_changed(),
    }
}

pub fn undo(app: &mut App) {
    step_history(app, false);
}

pub fn redo(app: &mut App) {
    step_history(app, true);
}

fn step_history(app: &mut App, forward: bool) {
    commit_editor(app);
    // With an image still being placed, undo cancels the placement instead of the last edit.
    if app.gizmo.take().is_some() {
        app.invalidate();
        return;
    }
    commit_form(app);
    let Some(doc) = app.doc.as_mut() else { return };
    let taken = if forward { doc.history.take_redo() } else { doc.history.take_undo() };
    let Some(mut entry) = taken else { return };
    let result = apply(app, &mut entry.action, forward);
    if result.is_ok() {
        after_apply(app, &entry.action, forward);
    }
    let Some(doc) = app.doc.as_mut() else { return };
    match result {
        Ok(()) if forward => doc.history.finish_redo(entry),
        Ok(()) => doc.history.finish_undo(entry),
        Err(e) => {
            doc.history.cancel(entry);
            app.toast(format!("Couldn't {}: {e}", if forward { "redo" } else { "undo" }), ToastKind::Error);
        }
    }
    app.update_title();
    app.invalidate();
}

// ---------------------------------------------------------------------------
// Page operations
// ---------------------------------------------------------------------------

pub fn rotate(app: &mut App, page: usize, quarter_turns: i32) {
    commit_gizmo(app);
    let Some(doc) = &app.doc else { return };
    let Ok(p) = doc.pdf.load_page(page) else { return };
    let old = p.rotation();
    let new = (old + quarter_turns).rem_euclid(4);
    p.set_rotation(new);
    drop(p);
    record(app, EditAction::RotatePage { page_index: page, old_rotation: old, new_rotation: new });
    app.structure_changed();
}

pub fn insert_blank(app: &mut App, at: usize) {
    commit_gizmo(app);
    let Some(doc) = &app.doc else { return };
    let count = doc.page_count();
    let at = at.min(count);
    let (w, h) = doc.pdf.page_size(at.min(count.saturating_sub(1))).map(|(w, h)| (w as f64, h as f64)).unwrap_or((612.0, 792.0));
    if doc.pdf.new_page(at, w, h).is_err() {
        app.toast("Couldn't insert a page", ToastKind::Error);
        return;
    }
    record(app, EditAction::InsertBlankPage { page_index: at, width_pt: w, height_pt: h });
    app.structure_changed();
    app.go_to_page(at);
}

pub fn delete_page(app: &mut App, page: usize) {
    commit_gizmo(app);
    let count = app.page_count();
    if count <= 1 {
        return;
    }
    let msg = format!("Delete page {} of {}?\n\nYou can undo this with Ctrl+Z.", page + 1, count);
    if message_box(app.hwnd, "Delete page", &msg, MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2) != IDYES {
        return;
    }
    let after = page.min(count - 2);
    if snapshot_edit(app, page, after, |pdf| if pdf.delete_page(page) { Ok(()) } else { Err("delete failed".into()) }) {
        app.structure_changed();
        app.go_to_page(after);
    }
}

/// Runs an edit that the engine can't invert precisely, recording before/after document bytes.
fn snapshot_edit(app: &mut App, page_before: usize, page_after: usize, f: impl FnOnce(&crate::pdfium::PdfDocument) -> Result<(), String>) -> bool {
    commit_form(app);
    let Some(doc) = &app.doc else { return false };
    let Ok(before) = doc.pdf.save_to_bytes() else { return false };
    if let Err(e) = f(&doc.pdf) {
        app.toast(e, ToastKind::Error);
        return false;
    }
    let Ok(after) = doc.pdf.save_to_bytes() else { return false };
    record(app, EditAction::DocumentSnapshot { before_bytes: before, after_bytes: after, active_page_before: page_before, active_page_after: page_after });
    true
}

// ---------------------------------------------------------------------------
// Content edits
// ---------------------------------------------------------------------------

pub fn place_stamp(app: &mut App, page: usize, x: f64, y: f64, check: bool) {
    let Some(doc) = &app.doc else { return };
    let size = 28.0;
    match stamp(&doc.pdf, page, check, x, y, size) {
        Ok(idx) => {
            record(app, EditAction::InsertStamp { page_index: page, object_index: idx, checkmark: check, page_x: x, page_y: y, size_pt: size });
            app.page_changed(page);
        }
        Err(e) => app.toast(format!("Couldn't add stamp: {e}"), ToastKind::Error),
    }
}

/// Highlights the current text selection, one annotation per page.
pub fn highlight_selection(app: &mut App) {
    let rects = std::mem::take(&mut app.selection.rects);
    app.selection.text.clear();
    let mut pages: Vec<usize> = rects.iter().map(|(p, _)| *p).collect();
    pages.dedup();
    for page in pages {
        let page_rects: Vec<RectF> = rects.iter().filter(|(p, _)| *p == page).map(|(_, r)| *r).collect();
        let Some(doc) = &app.doc else { return };
        match editor::add_highlight_annot(&doc.pdf, page, &page_rects, editor::HIGHLIGHT_COLOR) {
            Ok(idx) => {
                record(app, EditAction::HighlightAnnot { page_index: page, annot_index: idx, rects: page_rects, color: editor::HIGHLIGHT_COLOR });
                app.page_changed(page);
            }
            Err(e) => app.toast(format!("Couldn't highlight: {e}"), ToastKind::Error),
        }
    }
    app.invalidate();
}

pub fn redact(app: &mut App, page: usize, rect: RectF) {
    let blackline = !app.redact_clear;
    let mut removed = 0;
    let ok = snapshot_edit(app, page, page, |pdf| {
        removed = editor::apply_redaction(pdf, page, rect, blackline)?;
        Ok(())
    });
    if ok {
        app.page_changed(page);
        app.toast(if removed == 0 && !blackline { "Nothing to remove there".to_string() } else { "Redacted".to_string() }, ToastKind::Info);
    }
}

pub fn copy_selection(app: &mut App) {
    if app.selection.text.is_empty() {
        return;
    }
    if crate::wic_decoder::set_clipboard_text(&app.selection.text) {
        app.toast("Copied", ToastKind::Success);
    }
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

pub fn start_image(app: &mut App, w: u32, h: u32, pixels: Vec<u8>) {
    let page = app.current_page();
    let Some(l) = app.layouts.get(page) else { return };
    let (lw, lh) = (l.w_pt, l.h_pt);
    // The gizmo lives in unrotated PDF space; layout sizes already include /Rotate.
    let rot = app.doc.as_mut().and_then(|d| d.page(page)).map_or(0, |p| p.rotation());
    let (pw, ph) = if rot % 2 == 1 { (lh, lw) } else { (lw, lh) };
    let aspect = w as f64 / h.max(1) as f64;
    let mut gw = (pw * 0.4).min(w as f64 * 0.75);
    let mut gh = gw / aspect;
    if gh > ph * 0.4 {
        gh = ph * 0.4;
        gw = gh * aspect;
    }
    app.gizmo = Some(ImageGizmo { page, x: (pw - gw) / 2.0, y: (ph - gh) / 2.0, w: gw, h: gh, px_w: w, px_h: h, pixels });
    app.toast("Drag to position · Enter to place · Esc to cancel", ToastKind::Info);
}

pub fn prompt_insert_image(app: &mut App) {
    let filter = "Images (*.png;*.jpg;*.jpeg;*.bmp;*.gif;*.tif;*.tiff)\0*.png;*.jpg;*.jpeg;*.bmp;*.gif;*.tif;*.tiff\0All files (*.*)\0*.*\0";
    let Some(path) = file_dialog(app.hwnd, false, "Insert image", filter, None, None) else { return };
    match crate::wic_decoder::decode_image_to_bgra(&path) {
        Ok((w, h, px)) => start_image(app, w, h, px),
        Err(e) => app.toast(format!("Couldn't read image: {e}"), ToastKind::Error),
    }
}

pub fn paste_image(app: &mut App) {
    match crate::wic_decoder::get_clipboard_image_bgra() {
        Some((w, h, px)) => start_image(app, w, h, px),
        None => app.toast("The clipboard has no image", ToastKind::Info),
    }
}

pub fn commit_gizmo(app: &mut App) {
    let Some(g) = app.gizmo.take() else { return };
    let Some(doc) = &app.doc else { return };
    match editor::insert_image_object(&doc.pdf, g.page, g.px_w, g.px_h, g.pixels.clone(), g.x, g.y, g.w, g.h) {
        Ok(idx) => {
            record(app, EditAction::InsertImage {
                page_index: g.page,
                object_index: idx,
                width_px: g.px_w,
                height_px: g.px_h,
                bgra_pixels: g.pixels,
                page_x: g.x,
                page_y: g.y,
                width_pt: g.w,
                height_pt: g.h,
            });
            app.page_changed(g.page);
        }
        Err(e) => app.toast(format!("Couldn't insert image: {e}"), ToastKind::Error),
    }
}

// ---------------------------------------------------------------------------
// In-place text editor
// ---------------------------------------------------------------------------

static mut OLD_EDIT_PROC: isize = 0;

unsafe extern "system" fn editor_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let app = super::app_ptr();
    if !app.is_null() {
        let app = &mut *app;
        match msg {
            WM_KEYDOWN => match wparam as i32 {
                VK_RETURN => {
                    commit_editor(app);
                    return 0;
                }
                VK_ESCAPE => {
                    cancel_editor(app);
                    return 0;
                }
                // Let the EDIT control handle its own clipboard / select-all / undo.
                0x41 | 0x43 | 0x56 | 0x58 if key_down(VK_CONTROL) => {}
                0x5A if key_down(VK_CONTROL) && !key_down(VK_SHIFT) && SendMessageW(hwnd, EM_CANUNDO, 0, 0) != 0 => {}
                // Only real app shortcuts leave the editor. Ctrl on its own, Ctrl+arrows,
                // Ctrl+Backspace/Delete/Home/End and AltGr (Ctrl+Alt) characters stay in the EDIT.
                vk if key_down(VK_CONTROL)
                    && !key_down(VK_MENU)
                    && (matches!(vk, 0x30..=0x39 | 0x41..=0x5A) || matches!(vk, VK_OEM_PLUS | VK_OEM_MINUS | VK_ADD | VK_SUBTRACT)) =>
                {
                    commit_editor(app);
                    super::commands::shortcut(app, vk);
                    return 0;
                }
                _ => {}
            },
            WM_CHAR if wparam == 0x0D || wparam == 0x1B || (wparam < 0x20 && key_down(VK_CONTROL) && !matches!(wparam, 0x01 | 0x03 | 0x16 | 0x18 | 0x1A)) => return 0,
            _ => {}
        }
    }
    CallWindowProcW(OLD_EDIT_PROC, hwnd, msg, wparam, lparam)
}

/// Moves a text run by (dx, dy) PDF points (a drag with the Edit tool).
pub fn move_text(app: &mut App, page: usize, object: usize, dx: f64, dy: f64) {
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    let Some(doc) = &app.doc else { return };
    match editor::move_page_object(&doc.pdf, page, object, dx, dy) {
        Ok(()) => {
            record(app, EditAction::MoveObject { page_index: page, object_index: object, dx, dy });
            app.page_changed(page);
        }
        Err(e) => app.toast(format!("Couldn't move text: {e}"), ToastKind::Error),
    }
}

/// Opens the in-place editor over the text run at (x, y), or a new run if there is none.
pub fn begin_text_edit(app: &mut App, x: i32, y: i32) {
    commit_editor(app);
    let Some(page_idx) = app.page_at(x, y) else { return };
    let Some((px, py)) = app.to_pdf(page_idx, x, y) else { return };
    let scale = app.layouts[page_idx].w as f64 / app.layouts[page_idx].w_pt.max(1.0);
    let hit = {
        let Some(page) = app.doc.as_mut().and_then(|d| d.page(page_idx)) else { return };
        editor::hit_test_text_object(page, px as f32, py as f32, 4.0).map(|(idx, text, bounds)| {
            let size = page.get_object(idx).and_then(|o| o.get_font_size()).unwrap_or(12.0);
            (idx, text, bounds, size)
        })
    };
    let (object, text, rect, size) = match hit {
        Some((idx, text, bounds, size)) => {
            let r = app.rect_to_screen(page_idx, bounds).unwrap_or(RECT::xywh(x, y, 160, 24));
            (Some(idx), text, r, size)
        }
        None => {
            let size = 12.0f32;
            let h = (size as f64 * scale * 1.3) as i32;
            (None, String::new(), RECT::xywh(x, y - h / 2, app.s(200), h), size)
        }
    };
    let font_px = ((size as f64 * scale).round() as i32).max(app.s(11));
    let pad = app.s(4);
    let er = RECT::new(rect.left - pad, rect.top - pad, rect.right.max(rect.left + app.s(120)) + pad * 4, rect.bottom.max(rect.top + font_px) + pad);
    unsafe {
        let class = wide("EDIT");
        let tw = wide(&text);
        let h = CreateWindowExW(0, class.as_ptr(), tw.as_ptr(), WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL, er.left, er.top, er.width(), er.height(), app.hwnd, ID_EDITOR as HMENU, GetModuleHandleW(ptr::null()), ptr::null_mut());
        if h.is_null() {
            return;
        }
        let face = wide("Segoe UI");
        let font = CreateFontW(-font_px, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, face.as_ptr());
        SendMessageW(h, WM_SETFONT, font as WPARAM, 1);
        OLD_EDIT_PROC = SetWindowLongPtrW(h, GWLP_WNDPROC, editor_proc as *const () as isize);
        SendMessageW(h, EM_SETSEL, 0, -1);
        SetFocus(h);
        app.editor = Some(TextEditor { hwnd: h, font, page: page_idx, object, original: text, at: (px, py), size, rect: er });
    }
    app.hover_text = None;
    app.invalidate();
}

fn close_editor_window(ed: &TextEditor, hwnd: HWND) {
    unsafe {
        DestroyWindow(ed.hwnd);
        DeleteObject(ed.font);
        SetFocus(hwnd);
    }
}

pub fn cancel_editor(app: &mut App) {
    if let Some(ed) = app.editor.take() {
        close_editor_window(&ed, app.hwnd);
        app.invalidate();
    }
}

pub fn commit_editor(app: &mut App) {
    // Take it out first: destroying the EDIT sends EN_KILLFOCUS, which calls back in here.
    let Some(ed) = app.editor.take() else { return };
    let text = window_text(ed.hwnd);
    close_editor_window(&ed, app.hwnd);
    let Some(doc) = &app.doc else { return };
    let result = match ed.object {
        Some(idx) if text != ed.original => editor::apply_text_mutation(&doc.pdf, ed.page, idx, &text)
            .map(|_| Some(EditAction::EditTextRun { page_index: ed.page, object_index: idx, old_text: ed.original.clone(), new_text: text.clone() })),
        None if !text.trim().is_empty() => editor::insert_new_text_run(&doc.pdf, ed.page, &text, ed.at.0, ed.at.1, ed.size)
            .map(|idx| Some(EditAction::InsertTextRun { page_index: ed.page, object_index: idx, text: text.clone(), page_x: ed.at.0, page_y: ed.at.1, font_size: ed.size })),
        _ => Ok(None),
    };
    match result {
        Ok(Some(a)) => {
            record(app, a);
            app.page_changed(ed.page);
        }
        Ok(None) => app.invalidate(),
        Err(e) => app.toast(format!("Couldn't edit text: {e}"), ToastKind::Error),
    }
}
