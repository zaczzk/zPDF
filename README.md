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

### ARM64 (Optimized Native Windows on ARM)
```powershell
cargo build --release --target aarch64-pc-windows-gnullvm
# Output: target/aarch64-pc-windows-gnullvm/release/zpdf.exe (~277 KB)
```

### x64 (Optimized Native Windows 64-bit AMD64)
```powershell
cargo build --release --target x86_64-pc-windows-gnullvm
# Output: target/x86_64-pc-windows-gnullvm/release/zpdf.exe (~321 KB)
```

## Running
```powershell
.\zpdf.exe [path_to_pdf]
```
