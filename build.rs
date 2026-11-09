// Compiles the Windows resources (icon, VERSIONINFO, manifest) into the zpdf binary.
//
// resources.rc and zpdf.manifest are templates; the version comes from Cargo.toml, so there is
// nothing to keep in sync by hand. Tools, by target:
//   *-msvc        rc.exe (PATH, else the newest Windows 10/11 SDK)   -> .res
//   *-gnu/gnullvm llvm-windres / <triple>-windres / windres / llvm-rc -> .o / .res
// With no tool the build continues without resources and prints a warning, unless
// ZPDF_REQUIRE_RESOURCES=1 (set by the release workflow), which turns it into an error.
// No crates: std only.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources.rc");
    println!("cargo:rerun-if-changed=zpdf.manifest");
    println!("cargo:rerun-if-changed=assets/logo.ico");
    println!("cargo:rerun-if-env-changed=ZPDF_REQUIRE_RESOURCES");
    println!("cargo:rerun-if-env-changed=PATH");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let require = env::var("ZPDF_REQUIRE_RESOURCES").as_deref() == Ok("1");

    match compile_resources() {
        Ok(file) => println!("cargo:rustc-link-arg-bin=zpdf={}", file.display()),
        Err(e) if require => panic!("ZPDF_REQUIRE_RESOURCES=1 but resources could not be built: {e}"),
        Err(e) => println!("cargo:warning=zpdf.exe built WITHOUT icon/version/manifest: {e}"),
    }
}

fn compile_resources() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let version = format!(
        "{}.{}.{}.0",
        env::var("CARGO_PKG_VERSION_MAJOR").unwrap(),
        env::var("CARGO_PKG_VERSION_MINOR").unwrap(),
        env::var("CARGO_PKG_VERSION_PATCH").unwrap()
    );

    let manifest = read(&manifest_dir.join("zpdf.manifest"))?.replace("@VERSION@", &version);
    let manifest_out = out_dir.join("zpdf.manifest");
    write(&manifest_out, &manifest)?;

    // Forward slashes: valid for rc.exe and windres, and no escaping inside RC string literals.
    let slashes = |p: &Path| p.display().to_string().replace('\\', "/");
    let rc: String = read(&manifest_dir.join("resources.rc"))?
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("@ICON@", &slashes(&manifest_dir.join("assets").join("logo.ico")))
        .replace("@MANIFEST@", &slashes(&manifest_out))
        .replace("@VERSION_COMMAS@", &version.replace('.', ","))
        .replace("@VERSION@", &version)
        .replace("@PKG_VERSION@", &env::var("CARGO_PKG_VERSION").unwrap());
    let rc_out = out_dir.join("resources.rc");
    write(&rc_out, &(rc + "\n"))?;

    let target = env::var("TARGET").unwrap();
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let rc_exe = find_rc_exe().ok_or("rc.exe not found on PATH or in any Windows SDK")?;
        let res = out_dir.join("resources.res");
        run(Command::new(&rc_exe).args(["/nologo", "/fo"]).arg(&res).arg(&rc_out))?;
        return Ok(res);
    }

    // GNU-style targets: windres emits a COFF object the MinGW linkers take directly.
    let obj = out_dir.join("resources.o");
    let windres_target = if target.starts_with("aarch64") { "pe-aarch64-little" } else { "pe-x86-64" };
    let mut errors = Vec::new();
    let triple_windres = format!("{}-w64-mingw32-windres", target.split('-').next().unwrap());
    for tool in ["llvm-windres", triple_windres.as_str(), "windres"] {
        let Some(path) = find_on_path(tool) else { continue };
        let mut cmd = Command::new(&path);
        if tool == "llvm-windres" {
            cmd.args(["--target", windres_target]);
        }
        cmd.args(["-O", "coff", "-i"]).arg(&rc_out).arg("-o").arg(&obj);
        match run(&mut cmd) {
            Ok(()) => return Ok(obj),
            Err(e) => errors.push(e),
        }
    }
    if let Some(llvm_rc) = find_on_path("llvm-rc") {
        let res = out_dir.join("resources.res");
        match run(Command::new(llvm_rc).args(["/no-preprocess", "/fo"]).arg(&res).arg(&rc_out)) {
            Ok(()) => return Ok(res),
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        Err("no resource compiler (llvm-windres, windres or llvm-rc) on PATH".into())
    } else {
        Err(errors.join("; "))
    }
}

fn read(p: &Path) -> Result<String, String> {
    fs::read_to_string(p).map_err(|e| format!("reading {}: {e}", p.display()))
}

fn write(p: &Path, s: &str) -> Result<(), String> {
    fs::write(p, s).map_err(|e| format!("writing {}: {e}", p.display()))
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("{:?}: {e}", cmd.get_program()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{:?} failed ({}): {}{}",
            cmd.get_program(),
            out.status,
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn find_on_path(tool: &str) -> Option<PathBuf> {
    let exe = format!("{tool}.exe");
    env::split_paths(&env::var_os("PATH")?)
        .map(|dir| dir.join(&exe))
        .find(|p| p.is_file())
}

/// rc.exe from PATH (a VS developer prompt), else the newest SDK under Windows Kits\10\bin.
fn find_rc_exe() -> Option<PathBuf> {
    if let Some(p) = find_on_path("rc") {
        return Some(p);
    }
    let program_files = env::var_os("ProgramFiles(x86)").unwrap_or_else(|| r"C:\Program Files (x86)".into());
    let bin = Path::new(&program_files).join("Windows Kits").join("10").join("bin");
    let host_dirs: &[&str] = if env::var("HOST").unwrap_or_default().starts_with("aarch64") {
        &["arm64", "x64", "x86"]
    } else {
        &["x64", "x86"]
    };
    let mut sdks: Vec<(Vec<u32>, PathBuf)> = fs::read_dir(&bin)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let ver: Vec<u32> = name.split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
            Some((ver, e.path()))
        })
        .collect();
    sdks.sort();
    sdks.iter()
        .rev()
        .flat_map(|(_, dir)| host_dirs.iter().map(move |h| dir.join(h).join("rc.exe")))
        .find(|p| p.is_file())
}
