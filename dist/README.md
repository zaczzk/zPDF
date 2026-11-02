# dist

Prebuilt zPDF 1.1.2 binaries, built by GitHub Actions on tag `v1.1.2`.

- `zPDF-Setup-arm64.exe` — 4778906 bytes — sha256 `2434273d48e22a134dc65eb663e73c29ab572b8f6bbf0c04a95047f3ff08d4af`
- `zPDF-Setup-x64.exe` — 5044028 bytes — sha256 `3b744e067cd5507978c40d38b7efc1672904bcc13be871148edac0edcccf2c68`
- `zPDF-arm64.zip` — 3817488 bytes — sha256 `c674cecd1f631be0cae73994af8d320863953c32c0fea54281a9afef2be9ad12`
- `zPDF-x64.zip` — 4061885 bytes — sha256 `6be1cf357d04ebfb0ba21339ecd16126e54642b231c5c4e495ef17b43322ed1a`

Two ways to get zPDF:

- **Installer** (`zPDF-Setup-x64.exe` / `zPDF-Setup-arm64.exe`) — installs to your user profile, adds a
  Start-menu entry and an uninstaller. **Running a newer installer upgrades an existing install in place**:
  it reuses the same folder and uninstall entry, replaces the files, and refuses to install a *downgrade*
  over a newer version.
- **Portable** (`zPDF-x64.zip` / `zPDF-arm64.zip`) — no install. Unzip the whole archive and run `zPDF.exe`
  from inside that folder; `pdfium.dll` must stay beside it.

`pdfium.dll` is bundled in every download. Without it the app reports *could not find 'pdfium.dll'*.

Silent upgrade (handy for scripting):

```powershell
.\zPDF-Setup-x64.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
```

These are the same files attached to the [v1.1.2 release](https://github.com/zaczzk/zPDF/releases/tag/v1.1.2).
Verify before you run: `Get-FileHash dist\zPDF-Setup-x64.exe -Algorithm SHA256`.

No network calls, no telemetry.
