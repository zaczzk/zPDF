//! Colour tokens. Follows the Windows app light/dark setting and honours High Contrast.

use super::win32::*;
use std::ffi::c_void;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub dark: bool,
    pub high_contrast: bool,
    pub canvas: COLORREF,
    pub chrome: COLORREF,
    pub sidebar: COLORREF,
    pub border: COLORREF,
    pub text: COLORREF,
    pub text_dim: COLORREF,
    pub text_faint: COLORREF,
    pub hover: COLORREF,
    pub pressed: COLORREF,
    pub accent: COLORREF,
    pub accent_soft: COLORREF,
    pub on_accent: COLORREF,
    pub danger: COLORREF,
    pub success: COLORREF,
    pub card: COLORREF,
    pub shadow: COLORREF,
    pub page_outline: COLORREF,
    pub selection: COLORREF,
    pub search_hit: COLORREF,
    pub scrollbar: COLORREF,
}

pub const DARK: Theme = Theme {
    dark: true,
    high_contrast: false,
    canvas: rgb(0x17, 0x17, 0x1B),
    chrome: rgb(0x20, 0x20, 0x25),
    sidebar: rgb(0x1C, 0x1C, 0x21),
    border: rgb(0x30, 0x30, 0x38),
    text: rgb(0xEC, 0xEC, 0xF0),
    text_dim: rgb(0xA4, 0xA4, 0xB0),
    text_faint: rgb(0x70, 0x70, 0x7C),
    hover: rgb(0x2D, 0x2D, 0x35),
    pressed: rgb(0x37, 0x37, 0x40),
    accent: rgb(0x4C, 0x8D, 0xF6),
    accent_soft: rgb(0x24, 0x33, 0x4E),
    on_accent: rgb(0xFF, 0xFF, 0xFF),
    danger: rgb(0xF0, 0x52, 0x52),
    success: rgb(0x34, 0xC7, 0x6F),
    card: rgb(0x28, 0x28, 0x2F),
    shadow: rgb(0x0B, 0x0B, 0x0E),
    page_outline: rgb(0x2A, 0x2A, 0x31),
    selection: rgb(0x3B, 0x82, 0xF6),
    search_hit: rgb(0xFA, 0xCC, 0x15),
    scrollbar: rgb(0x4A, 0x4A, 0x55),
};

pub const LIGHT: Theme = Theme {
    dark: false,
    high_contrast: false,
    canvas: rgb(0xE6, 0xE7, 0xEB),
    chrome: rgb(0xF9, 0xF9, 0xFB),
    sidebar: rgb(0xF2, 0xF2, 0xF5),
    border: rgb(0xD9, 0xD9, 0xE0),
    text: rgb(0x1A, 0x1A, 0x20),
    text_dim: rgb(0x55, 0x55, 0x62),
    text_faint: rgb(0x8C, 0x8C, 0x98),
    hover: rgb(0xE9, 0xE9, 0xEF),
    pressed: rgb(0xDD, 0xDD, 0xE5),
    accent: rgb(0x25, 0x63, 0xEB),
    accent_soft: rgb(0xDB, 0xE6, 0xFD),
    on_accent: rgb(0xFF, 0xFF, 0xFF),
    danger: rgb(0xDC, 0x26, 0x26),
    success: rgb(0x16, 0xA3, 0x4A),
    card: rgb(0xFF, 0xFF, 0xFF),
    shadow: rgb(0xC4, 0xC5, 0xCC),
    page_outline: rgb(0xCC, 0xCD, 0xD4),
    selection: rgb(0x25, 0x63, 0xEB),
    search_hit: rgb(0xF5, 0xB7, 0x00),
    scrollbar: rgb(0xB4, 0xB4, 0xBE),
};

#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(
        hkey: isize,
        sub_key: *const u16,
        value: *const u16,
        flags: u32,
        ty: *mut u32,
        data: *mut c_void,
        cb_data: *mut u32,
    ) -> i32;
}

fn system_prefers_dark() -> bool {
    const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
    const RRF_RT_REG_DWORD: u32 = 0x10;
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let val = wide("AppsUseLightTheme");
    let mut data: u32 = 1;
    let mut cb = 4u32;
    let rc = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), val.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), &mut data as *mut u32 as *mut c_void, &mut cb)
    };
    rc == 0 && data == 0
}

fn high_contrast_on() -> bool {
    let mut hc = HIGHCONTRASTW { cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32, dwFlags: 0, lpszDefaultScheme: std::ptr::null_mut() };
    unsafe {
        SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, &mut hc as *mut _ as *mut c_void, 0) != 0 && hc.dwFlags & HCF_HIGHCONTRASTON != 0
    }
}

fn high_contrast_theme() -> Theme {
    let sys = |i| unsafe { GetSysColor(i) };
    let bg = sys(COLOR_WINDOW);
    let fg = sys(COLOR_WINDOWTEXT);
    let hl = sys(COLOR_HIGHLIGHT);
    let dark = (bg & 0xFF) + ((bg >> 8) & 0xFF) + ((bg >> 16) & 0xFF) < 384;
    Theme {
        dark,
        high_contrast: true,
        canvas: bg,
        chrome: sys(COLOR_BTNFACE),
        sidebar: bg,
        border: fg,
        text: fg,
        text_dim: fg,
        text_faint: sys(COLOR_GRAYTEXT),
        hover: hl,
        pressed: hl,
        accent: hl,
        accent_soft: hl,
        on_accent: sys(COLOR_HIGHLIGHTTEXT),
        danger: fg,
        success: fg,
        card: bg,
        shadow: bg,
        page_outline: fg,
        selection: hl,
        search_hit: sys(COLOR_HOTLIGHT),
        scrollbar: fg,
    }
}

pub fn current() -> Theme {
    let forced = std::env::var("ZPDF_THEME").unwrap_or_default();
    if forced.eq_ignore_ascii_case("light") {
        LIGHT
    } else if forced.eq_ignore_ascii_case("dark") {
        DARK
    } else if high_contrast_on() {
        high_contrast_theme()
    } else if system_prefers_dark() {
        DARK
    } else {
        LIGHT
    }
}

/// Matches the title bar to the theme and asks for rounded corners on Windows 11.
pub fn apply_to_window(hwnd: HWND, theme: &Theme) {
    let dark: i32 = theme.dark as i32;
    let round: i32 = 2; // DWMWCP_ROUND
    unsafe {
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const i32 as *const c_void, 4);
        DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &round as *const i32 as *const c_void, 4);
    }
}
