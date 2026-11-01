<div align="center">

<img src="assets/logo.png" width="128" height="128" alt="zPDF Logo" />

# zPDF

**Fast, tiny, zero-dependency native Rust PDF viewer & editor for Windows 10 & 11.**

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Zero Dependencies](https://img.shields.io/badge/Dependencies-0%20crates-brightgreen.svg)](#strict-zero-dependency-design)
[![Platform](https://img.shields.io/badge/Platform-Windows%20ARM64%20%7C%20x64-crimson.svg)](#building)

</div>

---

## Highlights
- **~320 KB Standalone Native Binary** — Instant cold start (< 50 ms).
- **0 Crates ([dependencies] = empty)** — Zero external crate supply-chain risk. Pure Rust std + direct Win32/C ABI.
- **0 Telemetry / 0 Network** — No network sockets linked or compiled. Completely air-gapped and private.
- **Pure-Rust SHA-256 Pinned Security** — FIPS 180-4 cryptographic engine pins and validates pdfium.dll before loading.
- **120 FPS Viewport** — Double-buffered GDI rendering, LRU tile caching, memory-pool reuse, and virtualized thumbnail rail.
- **In-Place PDF Editing** — Directly edit existing PDF text runs without rasterization or layout damage, plus annotations, page rotation, and reordering.
- **Windows 11 Fluent Dark UI** — Native dark theme with title bar integration, embedded application icon, and DPI awareness (Per-Monitor v2).

---

## Windows 11 Smart App Control & SmartScreen

When downloading or running freshly built native Windows executables:
- **Mark of the Web (SmartScreen):** If downloaded from the web or GitHub, right-click zpdf.exe -> **Properties** -> check **Unblock** (or run Unblock-File .\zpdf.exe in PowerShell).
- **Smart App Control (SAC):** Windows 11 Smart App Control enforces cloud-reputation checks on newly compiled native binaries. If SAC is in strict **On** mode on your machine, launch via developer shell or set Smart App Control to **Evaluation** / **Off** in *Windows Security > App & browser control > Smart App Control*.

---

## Building from Source

### Prerequisites
- [Rustup](https://rustup.rs/) with stable toolchain
- MinGW LLVM toolchain: MartinStorsjo.LLVM-MinGW.UCRT (installed via winget install MartinStorsjo.LLVM-MinGW.UCRT)

### ARM64 (Native Windows on Snapdragon / Surface Pro)
`powershell
cargo build --release --target aarch64-pc-windows-gnullvm
# Output: target/aarch64-pc-windows-gnullvm/release/zpdf.exe (~328 KB)
`

### x64 (Native 64-bit Intel / AMD)
`powershell
cargo build --release --target x86_64-pc-windows-gnullvm
# Output: target/x86_64-pc-windows-gnullvm/release/zpdf.exe (~380 KB)
`

---

## Running zPDF

Ensure pdfium.dll is in the same folder as zpdf.exe:

`powershell
# Launch empty landing screen (or open file picker)
.\zpdf.exe

# Open a specific PDF document
.\zpdf.exe document.pdf

# Run built-in self-tests & integrity checks
.\zpdf.exe --self-test
`

---

## License
Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.