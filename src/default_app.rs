// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: .pdf association registration (per-user, in place) and the "ask once" default-app prompt.
//
// zPDF never copies itself anywhere and never writes an Uninstall entry: the Inno Setup installer
// owns install/uninstall. This module only registers the *current* exe as a PDF handler
// (ProgID + Default Programs Capabilities) so it appears in Settings > Default apps, and removes
// those registrations again. It deliberately does not write the `.pdf` default value: Windows
// protects the real choice (UserChoice), and the user makes it in Settings.
//
// The registry layout below must stay identical to the [Registry] section of installer.iss.

use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

const HKEY_CURRENT_USER: usize = 0x8000_0001;
const KEY_QUERY_VALUE: u32 = 0x0001;
const REG_SZ: u32 = 1;
const REG_DWORD: u32 = 4;
const RRF_RT_REG_SZ: u32 = 0x0000_0002;
const RRF_RT_REG_DWORD: u32 = 0x0000_0010;
const ERROR_SUCCESS: i32 = 0;
const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_MORE_DATA: i32 = 234;

const ASSOCF_NONE: u32 = 0;
const ASSOCSTR_COMMAND: u32 = 1;
const SHCNE_ASSOCCHANGED: u32 = 0x0800_0000;
const SHCNF_IDLIST: u32 = 0x0000;
const SW_SHOWNORMAL: i32 = 1;

const APP_NAME: &str = "zPDF";
const PROG_ID: &str = "zPDF.Document";
const APP_EXE: &str = "zpdf.exe";
const APP_DESCRIPTION: &str = "Fast, zero-overhead PDF viewer and editor.";
const NEVER_ASK_VALUE: &str = "NeverAskDefault";
/// Every key below lives under `HKCU\<root>`; production uses `Software`, tests a scratch key.
const PROD_ROOT: &str = "Software";

#[link(name = "shlwapi")]
extern "system" {
    fn AssocQueryStringW(
        flags: u32,
        str_type: u32,
        assoc: *const u16,
        extra: *const u16,
        out: *mut u16,
        out_len: *mut u32,
    ) -> i32;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(
        hkey: usize,
        sub_key: *const u16,
        value: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut c_void,
        data_len: *mut u32,
    ) -> i32;
    fn RegSetKeyValueW(
        hkey: usize,
        sub_key: *const u16,
        value_name: *const u16,
        kind: u32,
        data: *const c_void,
        data_len: u32,
    ) -> i32;
    fn RegDeleteKeyValueW(hkey: usize, sub_key: *const u16, value_name: *const u16) -> i32;
    fn RegDeleteTreeW(hkey: usize, sub_key: *const u16) -> i32;
    fn RegDeleteKeyW(hkey: usize, sub_key: *const u16) -> i32;
    fn RegOpenKeyExW(hkey: usize, sub_key: *const u16, options: u32, sam: u32, out: *mut usize) -> i32;
    fn RegQueryInfoKeyW(
        hkey: usize,
        class: *mut u16,
        class_len: *mut u32,
        reserved: *mut u32,
        sub_keys: *mut u32,
        max_sub_key_len: *mut u32,
        max_class_len: *mut u32,
        values: *mut u32,
        max_value_name_len: *mut u32,
        max_value_len: *mut u32,
        security_descriptor_len: *mut u32,
        last_write_time: *mut c_void,
    ) -> i32;
    fn RegCloseKey(hkey: usize) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut c_void,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show_cmd: i32,
    ) -> *mut c_void;
    fn SHChangeNotify(event_id: u32, flags: u32, item1: *const c_void, item2: *const c_void);
}

// --- Helpers ---

/// UTF-16, null-terminated, for every W-suffixed Win32 call in this module.
fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// `None` is the key's default ("(Default)") value, passed to Win32 as a null name.
fn wide_opt(s: Option<&str>) -> Option<Vec<u16>> {
    s.map(wide)
}

fn ptr_or_null(v: &Option<Vec<u16>>) -> *const u16 {
    v.as_ref().map_or(std::ptr::null(), |w| w.as_ptr())
}

fn reg_get_string(subkey: &str, value: Option<&str>) -> Option<String> {
    let key = wide(subkey);
    let name = wide_opt(value);
    let mut buf = vec![0u16; 512];
    loop {
        let mut cb = (buf.len() * 2) as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                ptr_or_null(&name),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut c_void,
                &mut cb,
            )
        };
        match status {
            ERROR_SUCCESS => {
                let len = (cb as usize / 2).saturating_sub(1); // drop the terminator
                return Some(String::from_utf16_lossy(&buf[..len]));
            }
            ERROR_MORE_DATA => buf.resize(cb as usize / 2 + 1, 0),
            _ => return None,
        }
    }
}

fn reg_get_dword(subkey: &str, value: &str) -> Option<u32> {
    let key = wide(subkey);
    let name = wide(value);
    let mut data: u32 = 0;
    let mut cb = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut c_void,
            &mut cb,
        )
    };
    (status == ERROR_SUCCESS).then_some(data)
}

fn check(status: i32, what: &str, subkey: &str) -> Result<(), String> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("{what} HKCU\\{subkey} failed (Win32 error {status})"))
    }
}

/// Creates the key if needed and writes a REG_SZ value.
fn reg_set_string(subkey: &str, value: Option<&str>, data: &str) -> Result<(), String> {
    let key = wide(subkey);
    let name = wide_opt(value);
    let bytes = wide(data);
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            ptr_or_null(&name),
            REG_SZ,
            bytes.as_ptr() as *const c_void,
            (bytes.len() * 2) as u32,
        )
    };
    check(status, "writing", subkey)
}

fn reg_set_dword(subkey: &str, value: &str, data: u32) -> Result<(), String> {
    let key = wide(subkey);
    let name = wide(value);
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            REG_DWORD,
            &data as *const u32 as *const c_void,
            std::mem::size_of::<u32>() as u32,
        )
    };
    check(status, "writing", subkey)
}

/// Missing keys/values count as already deleted.
fn not_found_ok(status: i32) -> i32 {
    if status == ERROR_FILE_NOT_FOUND {
        ERROR_SUCCESS
    } else {
        status
    }
}

fn reg_delete_value(subkey: &str, value: Option<&str>) -> Result<(), String> {
    let key = wide(subkey);
    let name = wide_opt(value);
    let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), ptr_or_null(&name)) };
    check(not_found_ok(status), "deleting a value under", subkey)
}

fn reg_delete_tree(subkey: &str) -> Result<(), String> {
    let key = wide(subkey);
    let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
    if not_found_ok(status) != ERROR_SUCCESS {
        return check(status, "deleting", subkey);
    }
    // RegDeleteTreeW empties the key but leaves the key itself.
    let status = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, key.as_ptr()) };
    check(not_found_ok(status), "deleting", subkey)
}

/// Removes a key only when it has no values and no subkeys left, so values written by other
/// programs (e.g. another viewer's OpenWithProgids entry) are never collateral damage.
fn reg_delete_key_if_empty(subkey: &str) -> Result<(), String> {
    let key = wide(subkey);
    let mut hkey: usize = 0;
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) } != ERROR_SUCCESS {
        return Ok(()); // already gone
    }
    let (mut sub_keys, mut values) = (0u32, 0u32);
    let n = std::ptr::null_mut::<u32>;
    let status = unsafe {
        RegQueryInfoKeyW(hkey, std::ptr::null_mut(), n(), n(), &mut sub_keys, n(), n(), &mut values, n(), n(), n(), std::ptr::null_mut())
    };
    unsafe { RegCloseKey(hkey) };
    if status == ERROR_SUCCESS && sub_keys == 0 && values == 0 {
        let status = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, key.as_ptr()) };
        return check(not_found_ok(status), "deleting", subkey);
    }
    Ok(())
}

fn notify_shell_assoc_changed() {
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
}

/// The command Windows actually runs for `ext`, honouring the user's UserChoice.
fn query_association_command(ext: &str) -> Option<String> {
    let ext = wide(ext);
    let mut buf = [0u16; 2048];
    let mut cch = buf.len() as u32;
    let hr = unsafe {
        AssocQueryStringW(ASSOCF_NONE, ASSOCSTR_COMMAND, ext.as_ptr(), std::ptr::null(), buf.as_mut_ptr(), &mut cch)
    };
    if hr != 0 || cch == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..cch as usize - 1]))
}

/// The executable at the start of a shell command line (`"C:\x\a.exe" "%1"` or `C:\x\a.exe %1`).
fn command_executable(cmd: &str) -> Option<&str> {
    let cmd = cmd.trim_start();
    if let Some(rest) = cmd.strip_prefix('"') {
        return rest.split('"').next();
    }
    // ASCII lowercasing keeps byte offsets, so the index is valid in `cmd` too.
    match cmd.to_ascii_lowercase().find(".exe") {
        Some(i) => Some(&cmd[..i + 4]),
        None => cmd.split_whitespace().next(),
    }
}

fn normalize_path(p: &str) -> String {
    let p = p.strip_prefix(r"\\?\").unwrap_or(p);
    p.replace('/', "\\").to_lowercase()
}

/// True only when the command launches exactly `exe` (case-insensitive path comparison).
fn command_targets_exe(cmd: &str, exe: &Path) -> bool {
    match command_executable(cmd) {
        Some(target) => normalize_path(target) == normalize_path(&exe.to_string_lossy()),
        None => false,
    }
}

// --- Registration at an arbitrary HKCU root (production: `Software`) ---

fn classes(root: &str) -> String {
    format!("{root}\\Classes")
}

fn capabilities_key(root: &str) -> String {
    format!("{root}\\{APP_NAME}\\Capabilities")
}

fn register_at(root: &str, exe: &Path) -> Result<(), String> {
    let classes = classes(root);
    let open_command = format!("\"{}\" \"%1\"", exe.display());
    let icon = format!("\"{}\",0", exe.display());

    let prog_id = format!("{classes}\\{PROG_ID}");
    reg_set_string(&prog_id, None, "zPDF Document")?;
    reg_set_string(&prog_id, Some("FriendlyTypeName"), "zPDF Document")?;
    reg_set_string(&format!("{prog_id}\\DefaultIcon"), None, &icon)?;
    reg_set_string(&format!("{prog_id}\\shell\\open\\command"), None, &open_command)?;

    let app = format!("{classes}\\Applications\\{APP_EXE}");
    // The name "Open with" shows; without it Windows falls back to the exe's description.
    reg_set_string(&app, Some("FriendlyAppName"), APP_NAME)?;
    reg_set_string(&format!("{app}\\DefaultIcon"), None, &icon)?;
    reg_set_string(&format!("{app}\\shell\\open\\command"), None, &open_command)?;
    reg_set_string(&format!("{app}\\SupportedTypes"), Some(".pdf"), "")?;

    reg_set_string(&format!("{classes}\\.pdf\\OpenWithProgids"), Some(PROG_ID), "")?;

    let caps = capabilities_key(root);
    reg_set_string(&caps, Some("ApplicationName"), APP_NAME)?;
    reg_set_string(&caps, Some("ApplicationDescription"), APP_DESCRIPTION)?;
    reg_set_string(&format!("{caps}\\FileAssociations"), Some(".pdf"), PROG_ID)?;
    reg_set_string(&format!("{root}\\RegisteredApplications"), Some(APP_NAME), &caps)?;
    Ok(())
}

fn unregister_at(root: &str) -> Result<(), String> {
    let classes = classes(root);
    let pdf = format!("{classes}\\.pdf");
    let mut errors = Vec::new();
    let mut run = |r: Result<(), String>| {
        if let Err(e) = r {
            errors.push(e);
        }
    };

    run(reg_delete_tree(&format!("{classes}\\{PROG_ID}")));
    run(reg_delete_tree(&format!("{classes}\\Applications\\{APP_EXE}")));
    run(reg_delete_value(&format!("{pdf}\\OpenWithProgids"), Some(PROG_ID)));
    run(reg_delete_value(&format!("{root}\\RegisteredApplications"), Some(APP_NAME)));
    run(reg_delete_tree(&capabilities_key(root)));

    // zPDF <= 1.1.5 also claimed the `.pdf` default and its icon. Remove them only while they
    // still point at zPDF; anything another program has written since is left alone.
    if reg_get_string(&pdf, None).as_deref() == Some(PROG_ID) {
        run(reg_delete_value(&pdf, None));
        for (name, ours) in [("Content Type", "application/pdf"), ("PerceivedType", "document")] {
            if reg_get_string(&pdf, Some(name)).as_deref() == Some(ours) {
                run(reg_delete_value(&pdf, Some(name)));
            }
        }
    }
    let pdf_icon = format!("{pdf}\\DefaultIcon");
    if reg_get_string(&pdf_icon, None).is_some_and(|v| v.to_lowercase().contains(APP_EXE)) {
        run(reg_delete_value(&pdf_icon, None));
    }

    for key in [pdf_icon, format!("{pdf}\\OpenWithProgids"), pdf, format!("{root}\\{APP_NAME}")] {
        run(reg_delete_key_if_empty(&key));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn never_ask_at(root: &str) -> bool {
    reg_get_dword(&format!("{root}\\{APP_NAME}"), NEVER_ASK_VALUE) == Some(1)
}

fn set_never_ask_at(root: &str, never: bool) -> Result<(), String> {
    reg_set_dword(&format!("{root}\\{APP_NAME}"), NEVER_ASK_VALUE, never as u32)
}

// --- Public API ---

/// True when Windows opens `.pdf` files with exactly this running executable.
pub fn is_default_pdf_viewer() -> bool {
    match (query_association_command(".pdf"), std::env::current_exe()) {
        (Some(cmd), Ok(exe)) => command_targets_exe(&cmd, &exe),
        _ => false,
    }
}

/// Whether the "make zPDF your default PDF viewer" banner should be shown: zPDF is not the
/// default and the user has not chosen "don't ask again" (`HKCU\Software\zPDF\NeverAskDefault`).
pub fn should_prompt_default_app() -> bool {
    !is_default_pdf_viewer() && !never_ask_at(PROD_ROOT)
}

/// Persists the "don't ask again" choice read by [`should_prompt_default_app`].
pub fn set_never_ask(never: bool) -> Result<(), String> {
    set_never_ask_at(PROD_ROOT, never)
}

/// Registers the running exe (where it is, no copying) as a PDF handler for the current user:
/// ProgID `zPDF.Document`, `.pdf\OpenWithProgids`, Capabilities and RegisteredApplications.
/// Idempotent; re-running it re-points everything at the current exe path.
pub fn register_in_place() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot determine zpdf.exe path: {e}"))?;
    let result = register_at(PROD_ROOT, &exe);
    notify_shell_assoc_changed();
    result
}

/// Removes everything [`register_in_place`] (or zPDF <= 1.1.5) wrote for the current user.
/// Values that no longer point at zPDF are left untouched. Never deletes files.
pub fn unregister() -> Result<(), String> {
    let result = unregister_at(PROD_ROOT);
    notify_shell_assoc_changed();
    result
}

/// Opens Settings > Default apps, on zPDF's own page when Windows supports it.
pub fn open_default_apps_settings() {
    let open = wide("open");
    for uri in ["ms-settings:defaultapps?registeredAppUser=zPDF", "ms-settings:defaultapps"] {
        let target = wide(uri);
        let h = unsafe {
            ShellExecuteW(std::ptr::null_mut(), open.as_ptr(), target.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL)
        };
        if h as usize > 32 {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A private `HKCU\Software\zPDF-test-<pid>-<tag>` root, deleted on drop (even on panic).
    /// Registry tests never touch the developer's real association keys.
    struct ScratchRoot(String);

    impl ScratchRoot {
        fn new(tag: &str) -> Self {
            let root = format!("Software\\zPDF-test-{}-{tag}", std::process::id());
            let _ = reg_delete_tree(&root);
            ScratchRoot(root)
        }
    }

    impl Drop for ScratchRoot {
        fn drop(&mut self) {
            let _ = reg_delete_tree(&self.0);
        }
    }

    fn key_exists(subkey: &str) -> bool {
        let key = wide(subkey);
        let mut hkey = 0usize;
        let ok = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) } == ERROR_SUCCESS;
        if ok {
            unsafe { RegCloseKey(hkey) };
        }
        ok
    }

    #[test]
    fn command_must_name_the_exact_exe() {
        let exe = Path::new(r"C:\Program Files\zPDF\zpdf.exe");
        assert!(command_targets_exe(r#""C:\Program Files\zPDF\zpdf.exe" "%1""#, exe));
        assert!(command_targets_exe(r#""c:\program files\zpdf\ZPDF.EXE" "%1""#, exe));
        assert!(command_targets_exe(r"C:\Program Files\zPDF\zpdf.exe %1", exe));
        assert!(command_targets_exe(r#""C:/Program Files/zPDF/zpdf.exe" "%1""#, Path::new(r"\\?\C:\Program Files\zPDF\zpdf.exe")));

        // Another copy of zPDF, or anything that merely mentions zpdf, is not this exe.
        assert!(!command_targets_exe(r#""C:\Users\me\Downloads\zPDF\zpdf.exe" "%1""#, exe));
        assert!(!command_targets_exe(r#""C:\Tools\viewer.exe" --from-zpdf "%1""#, exe));
        assert!(!command_targets_exe(r#""C:\Program Files\zPDF\zpdf.exe.bak" "%1""#, exe));
        assert!(!command_targets_exe(
            r#""C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --single-argument %1"#,
            exe
        ));
        assert!(!command_targets_exe("", exe));
    }

    #[test]
    fn register_is_idempotent_and_matches_installer_layout() {
        let scratch = ScratchRoot::new("register");
        let root = scratch.0.as_str();
        let exe = Path::new(r"C:\Program Files\zPDF\zpdf.exe");
        register_at(root, exe).unwrap();
        register_at(root, exe).unwrap();

        let c = format!("{root}\\Classes");
        assert_eq!(
            reg_get_string(&format!("{c}\\zPDF.Document\\shell\\open\\command"), None).as_deref(),
            Some(r#""C:\Program Files\zPDF\zpdf.exe" "%1""#)
        );
        assert_eq!(
            reg_get_string(&format!("{c}\\zPDF.Document\\DefaultIcon"), None).as_deref(),
            Some(r#""C:\Program Files\zPDF\zpdf.exe",0"#)
        );
        assert_eq!(reg_get_string(&format!("{c}\\.pdf\\OpenWithProgids"), Some("zPDF.Document")).as_deref(), Some(""));
        assert_eq!(reg_get_string(&format!("{c}\\.pdf"), None), None, "the .pdf default must not be claimed");
        assert_eq!(
            reg_get_string(&format!("{root}\\RegisteredApplications"), Some("zPDF")),
            Some(format!("{root}\\zPDF\\Capabilities"))
        );
        assert_eq!(
            reg_get_string(&format!("{root}\\zPDF\\Capabilities\\FileAssociations"), Some(".pdf")).as_deref(),
            Some("zPDF.Document")
        );

        // Re-registering from another location re-points the ProgID at that exe.
        register_at(root, Path::new(r"D:\portable\zpdf.exe")).unwrap();
        assert_eq!(
            reg_get_string(&format!("{c}\\zPDF.Document\\shell\\open\\command"), None).as_deref(),
            Some(r#""D:\portable\zpdf.exe" "%1""#)
        );
    }

    #[test]
    fn unregister_removes_only_what_points_at_zpdf() {
        let scratch = ScratchRoot::new("unregister");
        let root = scratch.0.as_str();
        let pdf = format!("{root}\\Classes\\.pdf");
        register_at(root, Path::new(r"C:\zPDF\zpdf.exe")).unwrap();
        set_never_ask_at(root, true).unwrap();
        // Legacy 1.1.5 values, plus another viewer's entry that must survive.
        reg_set_string(&pdf, None, "zPDF.Document").unwrap();
        reg_set_string(&pdf, Some("Content Type"), "application/pdf").unwrap();
        reg_set_string(&format!("{pdf}\\DefaultIcon"), None, r#""C:\zPDF\zpdf.exe",0"#).unwrap();
        reg_set_string(&format!("{pdf}\\OpenWithProgids"), Some("AcroExch.Document"), "").unwrap();

        unregister_at(root).unwrap();

        assert!(!key_exists(&format!("{root}\\Classes\\zPDF.Document")));
        assert!(!key_exists(&format!("{root}\\Classes\\Applications\\zpdf.exe")));
        assert!(!key_exists(&format!("{root}\\zPDF\\Capabilities")));
        assert_eq!(reg_get_string(&format!("{root}\\RegisteredApplications"), Some("zPDF")), None);
        assert_eq!(reg_get_string(&pdf, None), None);
        assert_eq!(reg_get_string(&pdf, Some("Content Type")), None);
        assert!(!key_exists(&format!("{pdf}\\DefaultIcon")));
        assert_eq!(reg_get_string(&format!("{pdf}\\OpenWithProgids"), Some("zPDF.Document")), None);
        assert_eq!(reg_get_string(&format!("{pdf}\\OpenWithProgids"), Some("AcroExch.Document")).as_deref(), Some(""));
        assert!(never_ask_at(root), "the user's settings outlive unregistering");

        // A .pdf default that another app owns is never removed.
        reg_set_string(&pdf, None, "AcroExch.Document").unwrap();
        unregister_at(root).unwrap();
        assert_eq!(reg_get_string(&pdf, None).as_deref(), Some("AcroExch.Document"));
    }

    #[test]
    fn never_ask_roundtrip() {
        let scratch = ScratchRoot::new("never-ask");
        let root = scratch.0.as_str();
        assert!(!never_ask_at(root));
        set_never_ask_at(root, true).unwrap();
        assert!(never_ask_at(root));
        set_never_ask_at(root, false).unwrap();
        assert!(!never_ask_at(root));
    }
}
