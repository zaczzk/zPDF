//! Native Win32/GDI user interface.
//!
//! - `app`: state;  `view`: layout, scrolling, zoom;  `render`: tile/preview/thumbnail caches
//! - `paint`, `toolbar`, `search`: drawing;  `input`: window procedure and mouse/keyboard
//! - `commands`: every action and shortcut;  `doc_ops`: document edits and undo/redo
//! - `gfx`, `theme`, `win32`: drawing helpers, colours, raw bindings;  `settings`, `print`

mod app;
mod commands;
mod doc_ops;
mod gfx;
mod input;
mod paint;
mod print;
mod render;
mod search;
mod settings;
mod theme;
mod toolbar;
mod view;
pub mod win32;

use app::App;
use std::path::PathBuf;
use std::ptr;
use std::sync::Arc;
use win32::*;

/// The single window's state. Set once the window exists and cleared before teardown; window
/// procedures and subclassed controls reach the app through it.
static mut APP: *mut App = ptr::null_mut();

pub(crate) fn app_ptr() -> *mut App {
    unsafe { APP }
}

pub fn run(pdfium: Arc<crate::pdfium::PdfiumLibrary>, initial: Option<PathBuf>) -> Result<(), String> {
    unsafe {
        SetProcessDpiAwarenessContext(-4); // per-monitor v2
        let instance = GetModuleHandleW(ptr::null());
        let class = wide("zPDF_Main");
        let icon = LoadIconW(instance, 1 as *const u16);
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: input::window_proc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: icon,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW as *const u16),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: icon,
        };
        if RegisterClassExW(&wc) == 0 {
            return Err("Failed to register the window class".into());
        }
        let title = wide("zPDF");
        // Created hidden: state, theme and placement are applied before the first paint.
        let hwnd = CreateWindowExW(
            WS_EX_ACCEPTFILES,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );
        if hwnd.is_null() {
            return Err("Failed to create the main window".into());
        }
        DragAcceptFiles(hwnd, 1);

        APP = Box::into_raw(Box::new(App::new(hwnd, pdfium, icon)));
        let app = &mut *APP;

        let mut show = SW_SHOW;
        if let Some(mut wp) = settings::window_placement() {
            wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
            if wp.showCmd == SW_SHOWMAXIMIZED {
                show = SW_SHOWMAXIMIZED as i32;
            }
            wp.showCmd = 0; // SW_HIDE: just position it
            SetWindowPlacement(hwnd, &wp);
        } else {
            let (w, h) = (app.s(1200), app.s(820));
            MoveWindow(hwnd, app.s(80), app.s(60), w, h, 0);
        }
        let mut rc = RECT::default();
        GetClientRect(hwnd, &mut rc);
        app.width = rc.width();
        app.height = rc.height();
        if let Some(path) = initial {
            doc_ops::open_file(app, &path);
        }
        app.update_title();
        ShowWindow(hwnd, show);
        UpdateWindow(hwnd);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let app = APP;
        APP = ptr::null_mut();
        drop(Box::from_raw(app));
    }
    Ok(())
}
