# dist

Prebuilt zPDF 1.1.0 binaries, built by GitHub Actions on tag `v1.1.0`.

- `zPDF-Setup-arm64.exe` — 2204336 bytes — sha256 `84eb33d92f69664e404df81092aaa644406c671e8e956015130c1a88faa0de05`
- `zPDF-Setup-x64.exe` — 2226102 bytes — sha256 `1d572127da5ccbe71ccd5b732888f86f33f3b09d09eb5fc686fae5fda3c4b15d`
- `zPDF-arm64.zip` — 209960 bytes — sha256 `8a1013f50b094e6399958f8908ed53bec99b2b79c7c23646f791e258f9dc2264`
- `zPDF-x64.zip` — 227657 bytes — sha256 `be6dc4b544bc2bc71828c3b14e08e6029192b1b1d4638b1ce3f9684242374e0d`

Two ways to get zPDF:

- **Installer** (Windows, adds a Start-menu entry and an uninstaller): `zPDF-Setup-x64.exe` or `zPDF-Setup-arm64.exe`.
- **Portable** (no install, just unzip and run): `zPDF-x64.zip` or `zPDF-arm64.zip`.

These are the same files attached to the [v1.1.0 release](https://github.com/zaczzk/zPDF/releases/tag/v1.1.0).
Older versions stay on the releases page. `pdfium.dll` ships inside each zip and stays next to `zPDF.exe`.

Verify before you run:

```powershell
Get-FileHash dist\zPDF-Setup-x64.exe -Algorithm SHA256
```

The app is portable: no installer required, no network calls, no telemetry.
