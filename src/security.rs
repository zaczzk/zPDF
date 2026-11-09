// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Security (DLL search-order lockdown)
//
// pdfium.dll integrity is checked in CI against pinned SHA-256 values before it is packaged
// (.github/workflows/release.yml), not by hashing it on every launch.

const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x0000_0800;

#[link(name = "kernel32")]
extern "system" {
    fn SetDefaultDllDirectories(directory_flags: u32) -> i32;
    fn SetDllDirectoryW(path_name: *const u16) -> i32;
}

/// Hardens the current process against DLL hijacking and search-order attacks: implicit loads
/// come only from System32, and the current directory is removed from the search path.
/// pdfium.dll is then loaded by absolute path from the exe's own directory.
pub fn harden_process_dll_search_order() {
    unsafe {
        let _ = SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32);
        let empty: [u16; 1] = [0];
        let _ = SetDllDirectoryW(empty.as_ptr());
    }
}
