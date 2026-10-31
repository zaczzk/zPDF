# zPDF

Fast, tiny, zero-dependency native Rust PDF viewer & editor for Windows.

## TLDR
- **~150 KB `.exe`** — Instant cold start (< 50 ms).
- **0 Crates** — `Cargo.toml` has `[dependencies]` = empty. Zero supply-chain risk.
- **0 Telemetry / 0 Network** — No network sockets linked. Can't phone home.
- **SHA-256 Pinned** — Verifies `pdfium.dll` hash in pure Rust before loading.
- **120 FPS Viewport** — LRU bitmap cache + reusable pixel buffer pool + virtualized thumbnails.
- **Direct PDF Editing** — Edit text runs in-place, highlight, annotate, rotate, delete, insert pages, undo/redo, and save.

## Features
- **High-Performance Native GUI:** Hardware-accelerated GDI double-buffered viewport with Windows 11 dark mode.
- **Direct Text Run In-Place Mutation:** Edit existing PDF text runs directly without rasterization or layout damage.
- **Rich Vector Annotations:** Add text callouts, yellow highlights, and notes with transactional Undo/Redo (`Ctrl+Z` / `Ctrl+Y`).
- **Page Management:** Continuous vertical scroll, thumbnail rail, rotate pages (`R`), delete pages (`Del`), and reorder.
- **Strict Supply-Chain Isolation:** Written in pure `std` Rust with zero external third-party crates and strict DLL search-order lockdown.

## Building
```powershell
cargo build --release
```

## Running
```powershell
.\target\release\zpdf.exe [path_to_pdf]
```
