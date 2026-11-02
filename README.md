<div align="center">

<img src="assets/logo.png" width="128" height="128" alt="zPDF Logo" />

# zPDF

**Ultra-fast, zero-dependency native PDF viewer and editor for Windows 10 & 11.**

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Zero Dependencies](https://img.shields.io/badge/Dependencies-0%20crates-brightgreen.svg)](#strict-zero-dependency-design)
[![Platform](https://img.shields.io/badge/Platform-Windows%20ARM64%20%7C%20x64-crimson.svg)](#building-from-source)
[![Binary Size](https://img.shields.io/badge/Size-~378%20KB-orange.svg)](#features)

</div>

---

## Why zPDF?

Most modern PDF readers are bloated multi-gigabyte Electron shells or web wrappers packed with telemetry, subscriptions, and sluggish startup times. 

**zPDF is built from the ground up to be different:**
* **378 KB Single Standalone Executable** — Launches cold in under 30 milliseconds.
* **Strictly 0 External Crates** (`[dependencies]` = empty) — Zero supply chain vulnerability surface. Engineered purely on Rust `std`, Win32/GDI FFI, WIC COM, and the PDFium C ABI.
* **100% Private & Air-Gapped** — Zero network sockets compiled or linked. Zero analytics. Zero telemetry.
* **No Subscriptions, No Ads, No Nagging** — Asks once quietly to be your default PDF viewer; if dismissed, it never asks again.

---

## Core Capabilities

| Capability | What It Delivers |
| :--- | :--- |
| **In-Place Text Editing** | Directly select and edit existing PDF text runs in-place without flattening, rasterizing, or degrading vector fidelity. |
| **Interactive AcroForms** | Native form filling for interactive text fields, checkboxes, and comboboxes with full keyboard and mouse tab navigation. |
| **Image Insertion & Gizmo** | Drop or paste images (`Ctrl+V`) from clipboard or file (PNG, JPG, BMP, TIFF) via native Windows Imaging Component (WIC COM) with an interactive 8-handle transform gizmo. |
| **One-Click Review Stamps** | Resolution-independent vector stamps: Emerald Checkmark (`✓`, `#10B981`) and Crimson Cross (`✗`, `#EF4444`) with instant hotkeys (`V` / `X`). |
| **True Stream Redaction** | Genuine byte-level stream excision (`FPDFPage_RemoveObject` + `FPDFPage_GenerateContent`) rather than cosmetic overlays. Supports both **Clear Scrubbing** and **Blackline** modes. |
| **Locked 120 FPS Viewport** | Double-buffered cached GDI backbuffer, LRU bounded tile caching, buffer pooling, and sub-pixel precision touchpad scrolling. |
| **High-DPI Crystal Sharp** | Per-Monitor v2 DPI-aware rendering (`dpi / 72.0`) guarantees razor-sharp text and graphics on 4K, 150%, and 200% displays. |
| **Cryptographic DLL Pining** | Built-in pure-Rust SHA-256 engine verifies and pins `pdfium.dll` binary integrity prior to execution. |

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + O` | Open PDF Document |
| `Ctrl + S` | Save In-Place (with subtle confirmation toast) |
| `Ctrl + Shift + S` | Save As New Document |
| `Ctrl + Z` / `Ctrl + Y` | Full Undo / Redo Transaction History |
| `Ctrl + F` | Floating Search Bar with wrap-around navigation (`Enter` / `Shift+Enter`) |
| `Ctrl + V` | Paste Image from Clipboard onto Canvas |
| `Ctrl + Scroll` / `+` / `-` | Smooth Viewport Zoom (clamped 25% – 500%) |
| `Ctrl + 0` | Reset Zoom to 100% |
| `V` / `X` | Place Emerald Checkmark (`✓`) / Crimson Cross (`✗`) Stamp |
| `Enter` / `Esc` | Commit Active Gizmo / Cancel In-Place Text Edit |
| `PageUp` / `PageDown` / `Home` / `End` | Viewport Navigation |

---

## Windows Smart App Control & SmartScreen

When downloading or running freshly compiled native Windows executables:
* **Mark of the Web (SmartScreen):** If downloaded from GitHub, right-click `zpdf.exe` &rarr; **Properties** &rarr; check **Unblock** (or run `Unblock-File .\zpdf.exe` in PowerShell).
* **Smart App Control (SAC):** Windows 11 Smart App Control enforces cloud-reputation checks on newly compiled native binaries. When running locally compiled builds, launch via developer terminal or configure Smart App Control in *Windows Security > App & browser control*.

---

## Building from Source

### Prerequisites
* [Rustup](https://rustup.rs/) with stable toolchain
* MinGW LLVM toolchain: `MartinStorsjo.LLVM-MinGW.UCRT` (`winget install MartinStorsjo.LLVM-MinGW.UCRT`)

### ARM64 (Native Windows on Snapdragon / Surface Pro)
```powershell
cargo build --release --target aarch64-pc-windows-gnullvm
# Output: target/aarch64-pc-windows-gnullvm/release/zpdf.exe (~378 KB)
```

### x64 (Native 64-bit Intel / AMD)
```powershell
cargo build --release --target x86_64-pc-windows-gnullvm
# Output: target/x86_64-pc-windows-gnullvm/release/zpdf.exe (~380 KB)
```

---

## Running zPDF

Ensure `pdfium.dll` is located alongside `zpdf.exe`:

```powershell
# Launch clean landing screen or open file picker
.\zpdf.exe

# Open a specific PDF document directly
.\zpdf.exe sample.pdf

# Run built-in self-tests & cryptographic verification
.\zpdf.exe --self-test
```

---

## Code Signing Policy

**Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).**

Releases are built and signed automatically from this repository via GitHub Actions. Every signing request is cryptographically reviewed and approved by the maintainer.

* **Committers, Reviewers & Approvers:** [@zaczzk](https://github.com/zaczzk)
* **Privacy Policy:** Completely offline and air-gapped. zPDF contains 0 network dependencies and connects to no external servers.

---

## Portability & Uninstalling

zPDF is completely portable with zero installer clutter. To uninstall, simply delete the folder containing `zpdf.exe`.

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
