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

/// Where the reader left a document: page index, zoom percent, fit mode (0 none, 1 width, 2 page).
#[derive(Clone, Copy)]
pub struct Position {
    pub page: usize,
    pub zoom: f64,
    pub fit: u8,
}

/// Recent entries are stored as `path<TAB>page<TAB>zoom<TAB>fit`, so positions are pruned along
/// with the recent list. Plain paths (older format) still load.
fn entries() -> Vec<(PathBuf, Option<Position>)> {
    let Some(raw) = read("Recent", RRF_RT_REG_MULTI_SZ) else { return Vec::new() };
    let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    units
        .split(|&c| c == 0)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let line = String::from_utf16_lossy(s);
            let mut parts = line.split('\t');
            let path = PathBuf::from(parts.next().unwrap_or_default());
            let page = parts.next().and_then(|v| v.parse().ok());
            let zoom = parts.next().and_then(|v| v.parse().ok());
            let fit = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let pos = page.zip(zoom).map(|(page, zoom)| Position { page, zoom, fit });
            (path, pos)
        })
        .filter(|(p, _)| p.is_file())
        .take(MAX_RECENT)
        .collect()
}

fn write_entries(list: &[(PathBuf, Option<Position>)]) {
    let mut units: Vec<u16> = Vec::new();
    for (p, pos) in list {
        let mut line = p.to_string_lossy().into_owned();
        if let Some(pos) = pos {
            line.push_str(&format!("\t{}\t{:.1}\t{}", pos.page, pos.zoom, pos.fit));
        }
        units.extend(line.encode_utf16());
        units.push(0);
    }
    units.push(0);
    let bytes: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();
    write("Recent", REG_MULTI_SZ, &bytes);
}

fn same(a: &Path, b: &Path) -> bool {
    a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
}

pub fn recent_files() -> Vec<PathBuf> {
    entries().into_iter().map(|(p, _)| p).collect()
}

pub fn position(path: &Path) -> Option<Position> {
    entries().into_iter().find(|(p, _)| same(p, path)).and_then(|(_, pos)| pos)
}

/// Moves `path` to the front of the recent list (keeping its saved position) and returns the list.
pub fn add_recent(path: &Path) -> Vec<PathBuf> {
    let mut list = entries();
    let pos = list.iter().find(|(p, _)| same(p, path)).and_then(|(_, pos)| *pos);
    list.retain(|(p, _)| !same(p, path));
    list.insert(0, (path.to_path_buf(), pos));
    list.truncate(MAX_RECENT);
    write_entries(&list);
    list.into_iter().map(|(p, _)| p).collect()
}

pub fn save_position(path: &Path, pos: Position) {
    let mut list = entries();
    match list.iter_mut().find(|(p, _)| same(p, path)) {
        Some(entry) => entry.1 = Some(pos),
        None => {
            list.insert(0, (path.to_path_buf(), Some(pos)));
            list.truncate(MAX_RECENT);
        }
    }
    write_entries(&list);
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
