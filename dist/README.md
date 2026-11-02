# dist

Prebuilt zPDF 1.1.1 binaries, built by GitHub Actions on tag `v1.1.1`.

- `zPDF-Setup-arm64.exe` — 4776622 bytes — sha256 `521dc198f098407379632a31899a5199e0e3dcb1682e00cb25680464751b197e`
- `zPDF-Setup-x64.exe` — 5044062 bytes — sha256 `d96e4c54805d80095ed9866859e92449c4c5fed9886793263eb8e0e93d09a73f`
- `zPDF-arm64.zip` — 3817493 bytes — sha256 `fc6844e01410f565c9256af0bb615f04f12a04ddfe3e85d5e4629434cb046512`
- `zPDF-x64.zip` — 4061888 bytes — sha256 `5c7500b0b02cc7aa5bea97982c74f67a2efc5702d3f79af92c979e10145cef20`

Two ways to get zPDF:

- **Installer** (Windows, adds a Start-menu entry and an uninstaller): `zPDF-Setup-x64.exe` or `zPDF-Setup-arm64.exe`.
- **Portable** (no install, just unzip and run): `zPDF-x64.zip` or `zPDF-arm64.zip`.

`pdfium.dll` is bundled in every download — it must stay in the same folder as `zPDF.exe`, otherwise the app
opens with a "could not find 'pdfium.dll'" error. The installer places it correctly; with the portable zip,
unzip the whole archive and run the exe from inside that folder.

These are the same files attached to the [v1.1.1 release](https://github.com/zaczzk/zPDF/releases/tag/v1.1.1).
Verify before you run:

```powershell
Get-FileHash dist\zPDF-Setup-x64.exe -Algorithm SHA256
```

The app is portable-friendly: no network calls, no telemetry.
