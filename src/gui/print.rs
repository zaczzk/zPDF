//! Printing through the standard Windows print dialog. Each page is rasterised at up to 300 dpi
//! and scaled to fit the printable area, keeping its aspect ratio.

use super::app::*;
use super::win32::*;
use std::ptr;

pub fn print(app: &mut App) {
    let Some(doc) = app.doc.as_ref() else { return };
    let count = doc.page_count().min(u16::MAX as usize) as u16;
    if count == 0 {
        return;
    }
    let mut pd: PRINTDLGW = unsafe { std::mem::zeroed() };
    pd.lStructSize = std::mem::size_of::<PRINTDLGW>() as u32;
    pd.hwndOwner = app.hwnd;
    pd.Flags = PD_RETURNDC | PD_NOSELECTION | PD_USEDEVMODECOPIESANDCOLLATE;
    pd.nMinPage = 1;
    pd.nMaxPage = count;
    pd.nFromPage = 1;
    pd.nToPage = count;
    pd.nCopies = 1;
    if unsafe { PrintDlgW(&mut pd) } == 0 || pd.hDC.is_null() {
        return;
    }
    let (from, to) = if pd.Flags & PD_PAGENUMS != 0 {
        (pd.nFromPage.max(1) as usize - 1, (pd.nToPage as usize).min(count as usize))
    } else {
        (0, count as usize)
    };
    let hdc = pd.hDC;
    let copies = pd.nCopies.max(1) as usize;
    let result = (0..copies).try_fold(0, |n, _| print_pages(app, hdc, from, to).map(|p| n + p));
    unsafe {
        DeleteDC(hdc);
        if !pd.hDevMode.is_null() {
            GlobalFree(pd.hDevMode);
        }
        if !pd.hDevNames.is_null() {
            GlobalFree(pd.hDevNames);
        }
    }
    match result {
        Ok(n) => app.toast(format!("Sent {n} page{} to the printer", if n == 1 { "" } else { "s" }), ToastKind::Success),
        Err(e) => app.toast(format!("Printing failed: {e}"), ToastKind::Error),
    }
}

fn print_pages(app: &mut App, hdc: HDC, from: usize, to: usize) -> Result<usize, String> {
    let Some(doc) = app.doc.as_ref() else { return Ok(0) };
    let (pw, ph, dpi) = unsafe { (GetDeviceCaps(hdc, HORZRES), GetDeviceCaps(hdc, VERTRES), GetDeviceCaps(hdc, LOGPIXELSX).max(72)) };
    let render_dpi = dpi.min(300) as f64;
    let name = wide(&doc.name());
    let di = DOCINFOW { cbSize: std::mem::size_of::<DOCINFOW>() as i32, lpszDocName: name.as_ptr(), lpszOutput: ptr::null(), lpszDatatype: ptr::null(), fwType: 0 };
    unsafe {
        if StartDocW(hdc, &di) <= 0 {
            return Err("the printer refused the job".into());
        }
    }
    let mut printed = 0;
    for i in from..to {
        let (w_pt, h_pt) = doc.pdf.page_size(i).unwrap_or((612.0, 792.0));
        // Cap the raster at ~64 megapixels so huge pages (posters, plans) still print.
        let px_scale = (64e6 / ((w_pt as f64 * render_dpi / 72.0) * (h_pt as f64 * render_dpi / 72.0)).max(1.0)).sqrt().min(1.0);
        let rw = (w_pt as f64 * render_dpi / 72.0 * px_scale).round().max(1.0) as i32;
        let rh = (h_pt as f64 * render_dpi / 72.0 * px_scale).round().max(1.0) as i32;
        let form = doc.form.as_ref();
        let Some(bmp) = super::render::with_temp_page(&doc.pdf, form, i, |p| super::render::render(p, form, 0, 0, rw, rh, rw, rh)).flatten() else {
            unsafe { AbortDoc(hdc) };
            return Err(format!("couldn't render page {}", i + 1));
        };
        // Fit into the printable area, centred.
        let scale = (pw as f64 / rw as f64).min(ph as f64 / rh as f64);
        let (dw, dh) = ((rw as f64 * scale) as i32, (rh as f64 * scale) as i32);
        let dest = RECT::xywh((pw - dw) / 2, (ph - dh) / 2, dw, dh);
        let bmi = BITMAPINFO::bgra(bmp.w, bmp.h);
        unsafe {
            StartPage(hdc);
            SetStretchBltMode(hdc, HALFTONE);
            SetBrushOrgEx(hdc, 0, 0, ptr::null_mut());
            StretchDIBits(hdc, dest.left, dest.top, dw, dh, 0, 0, bmp.w, bmp.h, bmp.px.as_ptr() as *const _, &bmi, 0, SRCCOPY);
            EndPage(hdc);
        }
        printed += 1;
    }
    unsafe { EndDoc(hdc) };
    Ok(printed)
}
