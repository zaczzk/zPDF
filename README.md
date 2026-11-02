<div align="center">

<img src="assets/logo.png" width="128" height="128" alt="zPDF Logo" />

# zPDF

**Ultra-fast, zero-dependency native PDF viewer & editor for Windows 10 & 11.**

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Zero Dependencies](https://img.shields.io/badge/Dependencies-0%20crates-brightgreen.svg)](#what-it-never-does)
[![Platform](https://img.shields.io/badge/Platform-Windows%20ARM64%20%7C%20x64-crimson.svg)](#building-from-source)
[![Binary Size](https://img.shields.io/badge/Size-~378%20KB-orange.svg)](#what-it-never-does)

</div>

---

## What it does

* **Opens PDFs instantly** — Cold start in < 30 ms with a locked 120 FPS viewport.
* **Edits text in-place** — Click any text run to edit directly without rasterization or layout damage.
* **Fills forms** — Interactive AcroForms support with mouse, keyboard, and `Tab` field navigation.
* **Inserts images** — Paste from clipboard (`Ctrl+V`) or import PNG, JPG, BMP, TIFF with an interactive 8-handle transform gizmo.
* **Stamps reviews** — One-key vector checkmarks (`✓`) and crosses (`✗`) via `V` and `X`.
* **Redacts securely** — True byte-level stream removal (Clear or Blackline), not cosmetic visual overlays.
* **Searches with live highlights** — Floating search card (`Ctrl+F`) with wrap-around match navigation.
* **Manages pages** — Rotate clockwise/counter-clockwise or delete pages via sidebar right-click menu.
* **Crystal sharp on 4K** — Per-Monitor v2 DPI-aware scaling (`dpi / 72.0`) for high-density displays.
* **Guarantees cryptographic integrity** — Built-in pure-Rust SHA-256 engine pins and verifies `pdfium.dll`.

---

## What it never does

* **No subscriptions.**
* **No ads.**
* **No telemetry, analytics, or network calls** — Completely offline and air-gapped.
* **No external crate dependencies** — Strictly 0 dependencies in `Cargo.toml`.
* **No nag screens** — Asks once quietly to be your default PDF app, then never again.
* **No installer clutter** — Single standalone 378 KB portable `.exe`.

---

## Shortcuts

| Key | Action |
| :--- | :--- |
| `Ctrl + O` | Open PDF Document |
| `Ctrl + S` | Save In-Place |
| `Ctrl + Shift + S` | Save As New Document |
| `Ctrl + Z` / `Ctrl + Y` | Full Undo / Redo |
| `Ctrl + F` | Floating Search Bar (`Enter` next, `Shift+Enter` prev) |
| `Ctrl + V` | Paste Clipboard Image onto Canvas |
| `V` / `X` | Insert Emerald Checkmark (`✓`) / Crimson Cross (`✗`) Stamp |
| `Enter` | Commit Active Image Gizmo to PDF |
| `Esc` | Cancel Active Gizmo or In-Place Text Edit |
| `Ctrl + Scroll` / `+` / `-` | Viewport Zoom (25% – 500%) |
| `Ctrl + 0` | Reset Zoom to 100% |
| `PageUp` / `PageDown` / `Home` / `End` | Viewport Navigation |

---

## Building from Source

### Prerequisites
* [Rustup](https://rustup.rs/) (stable toolchain)
* MinGW LLVM toolchain: `MartinStorsjo.LLVM-MinGW.UCRT` (`winget install MartinStorsjo.LLVM-MinGW.UCRT`)

### ARM64 (Surface Pro / Windows on Snapdragon)
```powershell
cargo build --release --target aarch64-pc-windows-gnullvm
# Output: target/aarch64-pc-windows-gnullvm/release/zpdf.exe (~378 KB)
```

### x64 (Intel / AMD)
```powershell
cargo build --release --target x86_64-pc-windows-gnullvm
# Output: target/x86_64-pc-windows-gnullvm/release/zpdf.exe (~380 KB)
```

---

## Running zPDF

Ensure `pdfium.dll` is in the same folder as `zpdf.exe`:

```powershell
# Launch empty landing screen or open file picker
.\zpdf.exe

# Open a specific PDF document
.\zpdf.exe sample.pdf

# Run built-in self-tests & integrity checks
.\zpdf.exe --self-test
```

---

## Windows Smart App Control & SmartScreen

* **Mark of the Web (SmartScreen):** If downloaded via a browser, right-click `zpdf.exe` &rarr; **Properties** &rarr; check **Unblock** (or run `Unblock-File .\zpdf.exe`).
* **Smart App Control (SAC):** If Windows 11 Smart App Control blocks newly compiled local binaries, launch from your developer shell or configure SAC under *Windows Security > App & browser control*.

---

## Code Signing Policy

**Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).**

Releases are built and signed automatically from this repository via GitHub Actions. Every signing request is cryptographically reviewed and approved by the maintainer.

* **Committers, Reviewers & Approvers:** [@zaczzk](https://github.com/zaczzk)
* **Privacy Policy:** Completely offline and air-gapped. zPDF contains 0 network dependencies and connects to no external servers.

---

## Uninstalling

zPDF is completely portable. Delete the folder containing `zpdf.exe`.

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
