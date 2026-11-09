//! Per-user settings in HKCU\Software\zPDF: recent files and window placement.

use super::win32::{wide, WINDOWPLACEMENT};
use std::ffi::c_void;
use std::path::{Path, PathBuf};

const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
const KEY: &str = r"Software\zPDF";
const REG_BINARY: u32 = 3;
const REG_MULTI_SZ: u32 = 7;
const RRF_RT_REG_BINARY: u32 = 0x08;
const RRF_RT_REG_MULTI_SZ: u32 = 0x20;
const MAX_RECENT: usize = 8;

#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(hkey: isize, sub: *const u16, val: *const u16, flags: u32, ty: *mut u32, data: *mut c_void, cb: *mut u32) -> i32;
    fn RegSetKeyValueW(hkey: isize, sub: *const u16, val: *const u16, ty: u32, data: *const c_void, cb: u32) -> i32;
}

fn read(name: &str, flags: u32) -> Option<Vec<u8>> {
    let (k, v) = (wide(KEY), wide(name));
    let mut cb = 0u32;
    unsafe {
        if RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr(), flags, std::ptr::null_mut(), std::ptr::null_mut(), &mut cb) != 0 {
            return None;
        }
        let mut buf = vec![0u8; cb as usize];
        if RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr(), flags, std::ptr::null_mut(), buf.as_mut_ptr() as *mut c_void, &mut cb) != 0 {
            return None;
        }
        buf.truncate(cb as usize);
        Some(buf)
    }
}

fn write(name: &str, ty: u32, data: &[u8]) {
    let (k, v) = (wide(KEY), wide(name));
    unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr(), ty, data.as_ptr() as *const c_void, data.len() as u32);
    }
}

pub fn recent_files() -> Vec<PathBuf> {
    let Some(raw) = read("Recent", RRF_RT_REG_MULTI_SZ) else { return Vec::new() };
    let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    units
        .split(|&c| c == 0)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(String::from_utf16_lossy(s)))
        .filter(|p| p.is_file())
        .take(MAX_RECENT)
        .collect()
}

/// Moves `path` to the front of the recent list and returns the new list.
pub fn add_recent(path: &Path) -> Vec<PathBuf> {
    let mut list = recent_files();
    let canon = path.to_path_buf();
    list.retain(|p| !p.as_os_str().eq_ignore_ascii_case(canon.as_os_str()));
    list.insert(0, canon);
    list.truncate(MAX_RECENT);
    let mut units: Vec<u16> = Vec::new();
    for p in &list {
        units.extend(p.to_string_lossy().encode_utf16());
        units.push(0);
    }
    units.push(0);
    let bytes: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();
    write("Recent", REG_MULTI_SZ, &bytes);
    list
}

pub fn window_placement() -> Option<WINDOWPLACEMENT> {
    let raw = read("Window", RRF_RT_REG_BINARY)?;
    if raw.len() != std::mem::size_of::<WINDOWPLACEMENT>() {
        return None;
    }
    let mut wp = WINDOWPLACEMENT::default();
    unsafe { std::ptr::copy_nonoverlapping(raw.as_ptr(), &mut wp as *mut _ as *mut u8, raw.len()) };
    (wp.rcNormalPosition.right > wp.rcNormalPosition.left).then_some(wp)
}

pub fn save_window_placement(wp: &WINDOWPLACEMENT) {
    let bytes = unsafe { std::slice::from_raw_parts(wp as *const _ as *const u8, std::mem::size_of::<WINDOWPLACEMENT>()) };
    write("Window", REG_BINARY, bytes);
}
