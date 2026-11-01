// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Windows Default PDF App Association & "Ask Once" Detection Logic
// Strictly zero external crates: pure Rust std + Win32 FFI

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::c_void;
use std::path::Path;

// --- Win32 Constants & Types ---

pub const ASSOCF_NONE: u32 = 0x00000000;
pub const ASSOCSTR_COMMAND: u32 = 1;
pub const ASSOCSTR_EXECUTABLE: u32 = 2;

pub const HKEY_CURRENT_USER: usize = 0x80000001;
pub const KEY_QUERY_VALUE: u32 = 0x0001;
pub const KEY_SET_VALUE: u32 = 0x0002;
pub const KEY_CREATE_SUB_KEY: u32 = 0x0004;

pub const REG_SZ: u32 = 1;
pub const REG_DWORD: u32 = 4;
pub const ERROR_SUCCESS: i32 = 0;

pub const SHCNE_ASSOCCHANGED: u32 = 0x08000000;
pub const SHCNF_IDLIST: u32 = 0x0000;
pub const SW_SHOWNORMAL: i32 = 1;

// --- Win32 FFI Imports ---

#[link(name = "shlwapi")]
extern "system" {
    fn AssocQueryStringW(
        flags: u32,
        str_type: u32,
        pszAssoc: *const u16,
        pszExtra: *const u16,
        pszOut: *mut u16,
        pcchOut: *mut u32,
    ) -> i32;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        hKey: usize,
        lpSubKey: *const u16,
        ulOptions: u32,
        samDesired: u32,
        phkResult: *mut usize,
    ) -> i32;

    fn RegCreateKeyExW(
        hKey: usize,
        lpSubKey: *const u16,
        Reserved: u32,
        lpClass: *mut u16,
        dwOptions: u32,
        samDesired: u32,
        lpSecurityAttributes: *mut c_void,
        phkResult: *mut usize,
        lpdwDisposition: *mut u32,
    ) -> i32;

    fn RegQueryValueExW(
        hKey: usize,
        lpValueName: *const u16,
        lpReserved: *mut u32,
        lpType: *mut u32,
        lpData: *mut u8,
        lpcbData: *mut u32,
    ) -> i32;

    fn RegSetValueExW(
        hKey: usize,
        lpValueName: *const u16,
        Reserved: u32,
        dwType: u32,
        lpData: *const u8,
        cbData: u32,
    ) -> i32;

    fn RegCloseKey(hKey: usize) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut c_void,
        lpOperation: *const u16,
        lpFile: *const u16,
        lpParameters: *const u16,
        lpDirectory: *const u16,
        nShowCmd: i32,
    ) -> *mut c_void;

    fn SHChangeNotify(
        wEventId: u32,
        uFlags: u32,
        dwItem1: *const c_void,
        dwItem2: *const c_void,
    );
}

// --- Internal Registry & Helper Functions ---

/// Reads the command line associated with the given file extension (e.g. ".pdf").
pub fn query_file_association_command(ext: &str) -> Option<String> {
    let mut ext_u16: Vec<u16> = ext.encode_utf16().collect();
    ext_u16.push(0);

    let mut buf = [0u16; 1024];
    let mut cch: u32 = buf.len() as u32;

    let hr = unsafe {
        AssocQueryStringW(
            ASSOCF_NONE,
            ASSOCSTR_COMMAND,
            ext_u16.as_ptr(),
            std::ptr::null(),
            buf.as_mut_ptr(),
            &mut cch,
        )
    };

    if hr != ERROR_SUCCESS || cch == 0 {
        return None;
    }

    let len = (cch.saturating_sub(1)) as usize;
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// Determines whether an association command line refers to the target executable.
pub fn is_command_for_app(command_line: &str, exe_path: &Path) -> bool {
    let cmd_lower = command_line.to_lowercase();
    let exe_str = exe_path.to_string_lossy().to_lowercase();
    if cmd_lower.contains(&exe_str) {
        return true;
    }

    if let Some(name) = exe_path.file_name() {
        let name_str = name.to_string_lossy().to_lowercase();
        if cmd_lower.contains(&name_str) {
            return true;
        }
    }

    false
}

/// Reads a DWORD registry value under `HKCU\<subkey>`.
fn reg_get_dword(subkey: &str, value_name: &str) -> Option<u32> {
    let subkey_u16: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let val_u16: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: usize = 0;

    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey_u16.as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut hkey,
        )
    };
    if status != ERROR_SUCCESS || hkey == 0 {
        return None;
    }

    let mut val_type: u32 = 0;
    let mut data: u32 = 0;
    let mut cb_data: u32 = std::mem::size_of::<u32>() as u32;

    let query_status = unsafe {
        RegQueryValueExW(
            hkey,
            val_u16.as_ptr(),
            std::ptr::null_mut(),
            &mut val_type,
            &mut data as *mut u32 as *mut u8,
            &mut cb_data,
        )
    };

    unsafe {
        RegCloseKey(hkey);
    }

    if query_status == ERROR_SUCCESS && val_type == REG_DWORD {
        Some(data)
    } else {
        None
    }
}

/// Writes a DWORD registry value under `HKCU\<subkey>`.
fn reg_set_dword(subkey: &str, value_name: &str, value: u32) -> bool {
    let subkey_u16: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let val_u16: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: usize = 0;
    let mut disp: u32 = 0;

    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey_u16.as_ptr(),
            0,
            std::ptr::null_mut(),
            0,
            KEY_SET_VALUE | KEY_CREATE_SUB_KEY,
            std::ptr::null_mut(),
            &mut hkey,
            &mut disp,
        )
    };
    if status != ERROR_SUCCESS || hkey == 0 {
        return false;
    }

    let set_status = unsafe {
        RegSetValueExW(
            hkey,
            val_u16.as_ptr(),
            0,
            REG_DWORD,
            &value as *const u32 as *const u8,
            std::mem::size_of::<u32>() as u32,
        )
    };

    unsafe {
        RegCloseKey(hkey);
    }

    set_status == ERROR_SUCCESS
}

/// Writes a string registry value under `HKCU\<subkey>`.
fn reg_set_string(subkey: &str, value_name: Option<&str>, data: &str) -> bool {
    let subkey_u16: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: usize = 0;
    let mut disp: u32 = 0;

    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey_u16.as_ptr(),
            0,
            std::ptr::null_mut(),
            0,
            KEY_SET_VALUE | KEY_CREATE_SUB_KEY,
            std::ptr::null_mut(),
            &mut hkey,
            &mut disp,
        )
    };
    if status != ERROR_SUCCESS || hkey == 0 {
        return false;
    }

    let val_u16: Vec<u16> = match value_name {
        Some(v) => v.encode_utf16().chain(std::iter::once(0)).collect(),
        None => vec![0], // Default value
    };

    let data_u16: Vec<u16> = data.encode_utf16().chain(std::iter::once(0)).collect();
    let data_bytes = data_u16.len() * std::mem::size_of::<u16>();

    let set_status = unsafe {
        RegSetValueExW(
            hkey,
            val_u16.as_ptr(),
            0,
            REG_SZ,
            data_u16.as_ptr() as *const u8,
            data_bytes as u32,
        )
    };

    unsafe {
        RegCloseKey(hkey);
    }

    set_status == ERROR_SUCCESS
}

// --- Public API Functions ---

/// Checks if the current application is configured as the Windows default PDF viewer.
/// Uses `AssocQueryStringW(ASSOCF_NONE, ASSOCSTR_COMMAND, L".pdf", ...)` via `shlwapi.dll`.
pub fn is_default_pdf_viewer() -> bool {
    let cmd = match query_file_association_command(".pdf") {
        Some(c) => c,
        None => return false,
    };

    if let Ok(current_exe) = std::env::current_exe() {
        if is_command_for_app(&cmd, &current_exe) {
            return true;
        }
    }

    let cmd_lower = cmd.to_lowercase();
    cmd_lower.contains("zpdf.exe") || cmd_lower.contains("zpdf")
}

/// Checks whether the user should be prompted to set zPDF as default.
/// Returns true if `is_default_pdf_viewer()` is false AND `NeverAskDefault` in `HKCU\Software\zPDF` is NOT `1`.
pub fn should_prompt_default_app() -> bool {
    if is_default_pdf_viewer() {
        return false;
    }

    match reg_get_dword("Software\\zPDF", "NeverAskDefault") {
        Some(1) => false,
        _ => true,
    }
}

/// Sets the `NeverAskDefault` flag in `HKCU\Software\zPDF`.
/// If `never` is true, writes `NeverAskDefault = 1` into `HKCU\Software\zPDF` via `advapi32.dll`.
pub fn set_never_ask_default(never: bool) {
    let val = if never { 1u32 } else { 0u32 };
    reg_set_dword("Software\\zPDF", "NeverAskDefault", val);
}

/// Registers the `zPDF.Document` ProgID in `HKCU\Software\Classes` pointing to the current executable with `%1`.
pub fn register_prog_id() {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };

    let exe_str = exe_path.to_string_lossy();
    let open_command = format!("\"{}\" \"%1\"", exe_str);
    let icon_str = format!("{},0", exe_str);

    // 1. zPDF.Document ProgID description
    reg_set_string("Software\\Classes\\zPDF.Document", None, "zPDF Document");

    // 2. DefaultIcon
    reg_set_string(
        "Software\\Classes\\zPDF.Document\\DefaultIcon",
        None,
        &icon_str,
    );

    // 3. shell\open\command
    reg_set_string(
        "Software\\Classes\\zPDF.Document\\shell\\open\\command",
        None,
        &open_command,
    );

    // 4. Applications\zpdf.exe registration
    reg_set_string(
        "Software\\Classes\\Applications\\zpdf.exe\\shell\\open\\command",
        None,
        &open_command,
    );
    reg_set_string(
        "Software\\Classes\\Applications\\zpdf.exe\\SupportedTypes",
        Some(".pdf"),
        "",
    );

    // 5. OpenWithProgids under .pdf
    reg_set_string(
        "Software\\Classes\\.pdf\\OpenWithProgids",
        Some("zPDF.Document"),
        "",
    );

    // Notify the Windows Shell of association change
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

/// Launches the Windows Default Apps settings page via `ShellExecuteW(0, L"open", L"ms-settings:defaultapps", ...)`.
pub fn launch_default_apps_settings() {
    let op: Vec<u16> = "open\0".encode_utf16().collect();
    let target: Vec<u16> = "ms-settings:defaultapps\0".encode_utf16().collect();

    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            op.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

// --- Unit Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_command_for_app_matching() {
        let exe = Path::new("C:\\Program Files\\zPDF\\zpdf.exe");

        assert!(is_command_for_app(
            "\"C:\\Program Files\\zPDF\\zpdf.exe\" \"%1\"",
            exe
        ));
        assert!(is_command_for_app("C:\\Program Files\\zPDF\\zpdf.exe %1", exe));
        assert!(is_command_for_app("\"c:\\program files\\zpdf\\zpdf.exe\"", exe));

        // Edge or Acrobat should NOT match
        assert!(!is_command_for_app(
            "\"C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe\" --single-argument %1",
            exe
        ));
        assert!(!is_command_for_app(
            "\"C:\\Program Files\\Adobe\\Acrobat DC\\Acrobat\\Acrobat.exe\" \"%1\"",
            exe
        ));
    }

    #[test]
    fn test_query_file_association_command_runs() {
        // Querying .pdf might return a string or None depending on the OS setup, but should never panic
        let _ = query_file_association_command(".pdf");
    }

    #[test]
    fn test_is_default_pdf_viewer_runs() {
        // Must return a bool without panicking
        let _is_default = is_default_pdf_viewer();
    }

    extern "system" {
        fn CreateMutexW(lpMutexAttributes: *mut c_void, bInitialOwner: i32, lpName: *const u16) -> *mut c_void;
        fn WaitForSingleObject(hHandle: *mut c_void, dwMilliseconds: u32) -> u32;
        fn ReleaseMutex(hMutex: *mut c_void) -> i32;
        fn CloseHandle(hObject: *mut c_void) -> i32;
    }

    struct NamedRegLock {
        handle: *mut c_void,
    }

    impl NamedRegLock {
        fn acquire() -> Self {
            let name: Vec<u16> = "Local\\zPDF_Test_Reg_Lock\0".encode_utf16().collect();
            let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
            if !handle.is_null() {
                unsafe { WaitForSingleObject(handle, 10000) };
            }
            Self { handle }
        }
    }

    impl Drop for NamedRegLock {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    ReleaseMutex(self.handle);
                    CloseHandle(self.handle);
                }
            }
        }
    }

    #[test]
    fn test_never_ask_default_roundtrip() {
        let _lock = NamedRegLock::acquire();
        // Save previous value if any
        let prev = reg_get_dword("Software\\zPDF", "NeverAskDefault");

        set_never_ask_default(true);
        assert_eq!(
            reg_get_dword("Software\\zPDF", "NeverAskDefault"),
            Some(1)
        );

        set_never_ask_default(false);
        assert_eq!(
            reg_get_dword("Software\\zPDF", "NeverAskDefault"),
            Some(0)
        );

        // Restore original value
        if let Some(orig) = prev {
            set_never_ask_default(orig == 1);
        }
    }

    #[test]
    fn test_should_prompt_default_app_logic() {
        let _lock = NamedRegLock::acquire();
        let prev = reg_get_dword("Software\\zPDF", "NeverAskDefault");

        // When NeverAskDefault is 1, should_prompt_default_app() MUST be false
        set_never_ask_default(true);
        assert!(!should_prompt_default_app());

        // Restore original
        if let Some(orig) = prev {
            set_never_ask_default(orig == 1);
        } else {
            set_never_ask_default(false);
        }
    }

    #[test]
    fn test_register_prog_id_runs() {
        // Should execute and set the HKCU\Software\Classes keys without panic
        register_prog_id();

        // Verify the ProgID description was written
        let subkey: Vec<u16> = "Software\\Classes\\zPDF.Document\0".encode_utf16().collect();
        let mut hkey: usize = 0;
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut hkey,
            )
        };
        assert_eq!(status, ERROR_SUCCESS, "zPDF.Document registry key should be created");
        if hkey != 0 {
            unsafe {
                RegCloseKey(hkey);
            }
        }
    }
}
