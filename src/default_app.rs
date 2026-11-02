// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Windows Default PDF App Association & "Ask Once" Detection Logic
// Strictly zero external crates: pure Rust std + Win32 FFI

#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

// --- Win32 Constants & Types ---

pub type HRESULT = i32;
pub type ULONG = u32;

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

pub const CLSCTX_INPROC_SERVER: u32 = 1;
pub const COINIT_APARTMENTTHREADED: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GUID {
    pub Data1: u32,
    pub Data2: u16,
    pub Data3: u16,
    pub Data4: [u8; 8],
}

pub const CLSID_ShellLink: GUID = GUID {
    Data1: 0x00021401,
    Data2: 0x0000,
    Data3: 0x0000,
    Data4: [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

pub const IID_IShellLinkW: GUID = GUID {
    Data1: 0x000214f9,
    Data2: 0x0000,
    Data3: 0x0000,
    Data4: [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

pub const IID_IPersistFile: GUID = GUID {
    Data1: 0x0000010b,
    Data2: 0x0000,
    Data3: 0x0000,
    Data4: [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

#[repr(C)]
pub struct IShellLinkWVtbl {
    pub QueryInterface: unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    pub GetPath: unsafe extern "system" fn(this: *mut c_void, pszFile: *mut u16, cch: i32, pfd: *mut c_void, fFlags: u32) -> HRESULT,
    pub GetIDList: unsafe extern "system" fn(this: *mut c_void, ppidl: *mut c_void) -> HRESULT,
    pub SetIDList: unsafe extern "system" fn(this: *mut c_void, pidl: *const c_void) -> HRESULT,
    pub GetDescription: unsafe extern "system" fn(this: *mut c_void, pszName: *mut u16, cch: i32) -> HRESULT,
    pub SetDescription: unsafe extern "system" fn(this: *mut c_void, pszName: *const u16) -> HRESULT,
    pub GetWorkingDirectory: unsafe extern "system" fn(this: *mut c_void, pszDir: *mut u16, cch: i32) -> HRESULT,
    pub SetWorkingDirectory: unsafe extern "system" fn(this: *mut c_void, pszDir: *const u16) -> HRESULT,
    pub GetArguments: unsafe extern "system" fn(this: *mut c_void, pszArgs: *mut u16, cch: i32) -> HRESULT,
    pub SetArguments: unsafe extern "system" fn(this: *mut c_void, pszArgs: *const u16) -> HRESULT,
    pub GetHotkey: unsafe extern "system" fn(this: *mut c_void, pwHotkey: *mut u16) -> HRESULT,
    pub SetHotkey: unsafe extern "system" fn(this: *mut c_void, wHotkey: u16) -> HRESULT,
    pub GetShowCmd: unsafe extern "system" fn(this: *mut c_void, piShowCmd: *mut i32) -> HRESULT,
    pub SetShowCmd: unsafe extern "system" fn(this: *mut c_void, iShowCmd: i32) -> HRESULT,
    pub GetIconLocation: unsafe extern "system" fn(this: *mut c_void, pszIconPath: *mut u16, cch: i32, piIcon: *mut i32) -> HRESULT,
    pub SetIconLocation: unsafe extern "system" fn(this: *mut c_void, pszIconPath: *const u16, iIcon: i32) -> HRESULT,
    pub SetRelativePath: unsafe extern "system" fn(this: *mut c_void, pszPathRel: *const u16, dwReserved: u32) -> HRESULT,
    pub Resolve: unsafe extern "system" fn(this: *mut c_void, hwnd: *mut c_void, fFlags: u32) -> HRESULT,
    pub SetPath: unsafe extern "system" fn(this: *mut c_void, pszFile: *const u16) -> HRESULT,
}

#[repr(C)]
pub struct IPersistFileVtbl {
    pub QueryInterface: unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    pub GetClassID: unsafe extern "system" fn(this: *mut c_void, pClassID: *mut GUID) -> HRESULT,
    pub IsDirty: unsafe extern "system" fn(this: *mut c_void) -> HRESULT,
    pub Load: unsafe extern "system" fn(this: *mut c_void, pszFileName: *const u16, dwMode: u32) -> HRESULT,
    pub Save: unsafe extern "system" fn(this: *mut c_void, pszFileName: *const u16, fRemember: i32) -> HRESULT,
    pub SaveCompleted: unsafe extern "system" fn(this: *mut c_void, pszFileName: *const u16) -> HRESULT,
    pub GetCurFile: unsafe extern "system" fn(this: *mut c_void, ppszFileName: *mut *mut u16) -> HRESULT,
}

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

    fn RegDeleteTreeW(hKey: usize, lpSubKey: *const u16) -> i32;

    fn RegDeleteValueW(hKey: usize, lpValueName: *const u16) -> i32;

    fn RegCloseKey(hKey: usize) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> HRESULT;
    fn CoUninitialize();
    fn CoCreateInstance(
        rclsid: *const GUID,
        pUnkOuter: *mut c_void,
        dwClsContext: u32,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT;
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

/// Deletes an entire registry subkey tree under `HKCU\<subkey>`.
pub fn reg_delete_tree(subkey: &str) -> bool {
    let subkey_u16: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, subkey_u16.as_ptr()) };
    status == ERROR_SUCCESS || status == 2 // 2 = ERROR_FILE_NOT_FOUND
}

/// Deletes a specific registry value under `HKCU\<subkey>`.
pub fn reg_delete_value(subkey: &str, value_name: &str) -> bool {
    let subkey_u16: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let val_u16: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: usize = 0;

    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey_u16.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
    };
    if status != ERROR_SUCCESS || hkey == 0 {
        return false;
    }

    let del_status = unsafe { RegDeleteValueW(hkey, val_u16.as_ptr()) };
    unsafe {
        RegCloseKey(hkey);
    }

    del_status == ERROR_SUCCESS || del_status == 2
}

/// Creates a Windows `.lnk` shortcut using pure Win32 COM `IShellLinkW` and `IPersistFile`.
pub fn create_shortcut(
    target_path: &Path,
    shortcut_path: &Path,
    description: &str,
    icon_path: Option<&Path>,
) -> Result<(), String> {
    unsafe {
        let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
        let uninit = hr == 0 || hr == 1; // S_OK or S_FALSE

        let mut shell_link_ptr: *mut c_void = std::ptr::null_mut();
        let hr_inst = CoCreateInstance(
            &CLSID_ShellLink,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IShellLinkW,
            &mut shell_link_ptr,
        );
        if hr_inst != 0 || shell_link_ptr.is_null() {
            if uninit {
                CoUninitialize();
            }
            return Err(format!("CoCreateInstance(IShellLinkW) failed (0x{:08X})", hr_inst));
        }

        let link = *(shell_link_ptr as *mut *mut IShellLinkWVtbl);

        let target_u16: Vec<u16> = target_path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        ((*link).SetPath)(shell_link_ptr, target_u16.as_ptr());

        if let Some(dir) = target_path.parent() {
            let dir_u16: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            ((*link).SetWorkingDirectory)(shell_link_ptr, dir_u16.as_ptr());
        }

        let desc_u16: Vec<u16> = description.encode_utf16().chain(std::iter::once(0)).collect();
        ((*link).SetDescription)(shell_link_ptr, desc_u16.as_ptr());

        if let Some(icon) = icon_path {
            let icon_u16: Vec<u16> = icon.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            ((*link).SetIconLocation)(shell_link_ptr, icon_u16.as_ptr(), 0);
        }

        let mut persist_file_ptr: *mut c_void = std::ptr::null_mut();
        let hr_qi = ((*link).QueryInterface)(
            shell_link_ptr,
            &IID_IPersistFile,
            &mut persist_file_ptr,
        );

        let save_res = if hr_qi == 0 && !persist_file_ptr.is_null() {
            let pf = *(persist_file_ptr as *mut *mut IPersistFileVtbl);
            if let Some(parent) = shortcut_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let save_u16: Vec<u16> = shortcut_path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            let hr_save = ((*pf).Save)(persist_file_ptr, save_u16.as_ptr(), 1);
            ((*pf).Release)(persist_file_ptr);
            if hr_save == 0 {
                Ok(())
            } else {
                Err(format!("IPersistFile::Save failed (0x{:08X})", hr_save))
            }
        } else {
            Err(format!("QueryInterface(IPersistFile) failed (0x{:08X})", hr_qi))
        };

        ((*link).Release)(shell_link_ptr);
        if uninit {
            CoUninitialize();
        }

        save_res
    }
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

/// Registers the `zPDF.Document` ProgID and Default Programs capabilities for the given executable.
pub fn register_prog_id_for_path(exe_path: &Path) {
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

    // 6. RegisteredApplications & Capabilities for Default Apps
    reg_set_string(
        "Software\\RegisteredApplications",
        Some("zPDF"),
        "Software\\zPDF\\Capabilities",
    );
    reg_set_string(
        "Software\\zPDF\\Capabilities",
        Some("ApplicationDescription"),
        "Fast, zero-overhead PDF viewer and editor.",
    );
    reg_set_string(
        "Software\\zPDF\\Capabilities",
        Some("ApplicationName"),
        "zPDF",
    );
    reg_set_string(
        "Software\\zPDF\\Capabilities\\FileAssociations",
        Some(".pdf"),
        "zPDF.Document",
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

/// Registers the `zPDF.Document` ProgID in `HKCU\Software\Classes` pointing to the current executable with `%1`.
pub fn register_prog_id() {
    if let Ok(exe_path) = std::env::current_exe() {
        register_prog_id_for_path(&exe_path);
    }
}

/// Installs zPDF to `%LOCALAPPDATA%\Programs\zPDF`, creates Start Menu shortcut,
/// and registers ProgID and Default App capabilities.
pub fn install_app() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;

    let local_app_data = std::env::var("LOCALAPPDATA")
        .or_else(|_| {
            std::env::var("USERPROFILE").map(|u| format!("{}\\AppData\\Local", u))
        })
        .map_err(|_| "Could not determine %LOCALAPPDATA% directory".to_string())?;

    let install_dir = Path::new(&local_app_data).join("Programs").join("zPDF");
    std::fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Failed to create directory {}: {}", install_dir.display(), e))?;

    let target_exe = install_dir.join("zpdf.exe");

    // Copy executable if not already running from target
    if current_exe != target_exe {
        std::fs::copy(&current_exe, &target_exe)
            .map_err(|e| format!("Failed to copy zpdf.exe: {}", e))?;
    }

    // Copy companion files (pdfium.dll, libunwind.dll)
    if let Some(parent) = current_exe.parent() {
        let pdfium_src = parent.join("pdfium.dll");
        if pdfium_src.exists() && pdfium_src != install_dir.join("pdfium.dll") {
            let _ = std::fs::copy(&pdfium_src, install_dir.join("pdfium.dll"));
        }
        let unwind_src = parent.join("libunwind.dll");
        if unwind_src.exists() && unwind_src != install_dir.join("libunwind.dll") {
            let _ = std::fs::copy(&unwind_src, install_dir.join("libunwind.dll"));
        }
    }
    // Also check current working directory for pdfium.dll
    if let Ok(cwd) = std::env::current_dir() {
        let pdfium_cwd = cwd.join("pdfium.dll");
        if pdfium_cwd.exists() && !install_dir.join("pdfium.dll").exists() {
            let _ = std::fs::copy(&pdfium_cwd, install_dir.join("pdfium.dll"));
        }
    }

    // Create Start Menu shortcut
    if let Ok(app_data) = std::env::var("APPDATA") {
        let start_menu_programs = Path::new(&app_data)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        let shortcut_path = start_menu_programs.join("zPDF.lnk");
        let _ = create_shortcut(
            &target_exe,
            &shortcut_path,
            "zPDF - Lightweight PDF Viewer & Editor",
            Some(&target_exe),
        );
    }

    // Register ProgID and Default App capabilities pointing to target_exe
    register_prog_id_for_path(&target_exe);

    // Register in Add/Remove Programs (Installed apps)
    let uninstall_key = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\zPDF";
    reg_set_string(uninstall_key, Some("DisplayName"), "zPDF");
    reg_set_string(uninstall_key, Some("DisplayVersion"), env!("CARGO_PKG_VERSION"));
    reg_set_string(uninstall_key, Some("Publisher"), "zPDF");
    let icon_str = format!("{},0", target_exe.display());
    reg_set_string(uninstall_key, Some("DisplayIcon"), &icon_str);
    reg_set_string(uninstall_key, Some("InstallLocation"), &install_dir.to_string_lossy());
    let uninstall_cmd = format!("\"{}\" --uninstall", target_exe.display());
    reg_set_string(uninstall_key, Some("UninstallString"), &uninstall_cmd);
    reg_set_dword(uninstall_key, "NoModify", 1);
    reg_set_dword(uninstall_key, "NoRepair", 1);

    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }

    Ok(install_dir)
}

/// Uninstalls zPDF: removes shortcuts, unregisters ProgID and Default Apps registry keys,
/// and cleans up the installation directory.
pub fn uninstall_app() -> Result<(), String> {
    // 1. Remove Start Menu shortcut
    if let Ok(app_data) = std::env::var("APPDATA") {
        let shortcut_path = Path::new(&app_data)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("zPDF.lnk");
        let _ = std::fs::remove_file(shortcut_path);
    }

    // 2. Remove Desktop shortcut if present
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let desktop_shortcut = Path::new(&profile).join("Desktop").join("zPDF.lnk");
        let _ = std::fs::remove_file(desktop_shortcut);
    }

    // 3. Remove Registry keys
    reg_delete_tree("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\zPDF");
    reg_delete_tree("Software\\Classes\\zPDF.Document");
    reg_delete_tree("Software\\Classes\\Applications\\zpdf.exe");
    reg_delete_value("Software\\Classes\\.pdf\\OpenWithProgids", "zPDF.Document");
    reg_delete_value("Software\\RegisteredApplications", "zPDF");
    reg_delete_tree("Software\\zPDF");

    // 4. Notify Windows Shell
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }

    // 5. Clean up installed directory if running from it
    if let Ok(current_exe) = std::env::current_exe() {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let install_dir = Path::new(&local_app_data).join("Programs").join("zPDF");
            if current_exe.starts_with(&install_dir) {
                let dir_str = install_dir.to_string_lossy();
                let cmd = format!("timeout /t 1 /nobreak > nul & rmdir /s /q \"{}\"", dir_str);
                let _ = std::process::Command::new("cmd")
                    .args(&["/C", &cmd])
                    .spawn();
            }
        }
    }

    Ok(())
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

    #[test]
    fn test_create_shortcut() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("zpdf_test_target.exe");
        let _ = std::fs::write(&target, b"MZ"); // mock exe
        let shortcut = temp_dir.join("zpdf_test_shortcut.lnk");
        if shortcut.exists() {
            let _ = std::fs::remove_file(&shortcut);
        }

        let res = create_shortcut(&target, &shortcut, "Test Shortcut", Some(&target));
        assert!(res.is_ok(), "Shortcut creation should succeed: {:?}", res.err());
        assert!(shortcut.exists(), "Created shortcut file should exist on disk");

        // Clean up
        let _ = std::fs::remove_file(&shortcut);
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn test_reg_delete_helpers() {
        let test_key = "Software\\zPDF_Delete_Test";
        reg_set_string(test_key, Some("TestVal"), "Hello");
        assert!(reg_delete_value(test_key, "TestVal"));
        assert!(reg_delete_tree(test_key));
    }
}
