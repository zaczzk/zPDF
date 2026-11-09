# dist

Binaries are no longer committed to this repository. Download them from
[GitHub Releases](https://github.com/zaczzk/zPDF/releases), where every release is built by GitHub
Actions from its `vX.Y.Z` tag and ships with a `SHA256SUMS.txt` listing the checksum of each file.

Each release has:

- **Installer** (`zPDF-Setup-x64.exe` / `zPDF-Setup-arm64.exe`) — installs to your user profile, adds a
  Start-menu entry and an uninstaller. Running a newer installer on a machine that already has zPDF
  **upgrades it in place**: same folder, same uninstall entry, no duplicate installs. If the machine
  already has a *newer* version than the installer, it asks before going backwards (never blocks it).
- **Portable** (`zPDF-x64.zip` / `zPDF-arm64.zip`) — no install. Unzip the whole archive and run
  `zPDF.exe` from inside that folder; `pdfium.dll` must stay beside it (zPDF loads it only from its own
  folder).

Verify a download against the release's `SHA256SUMS.txt`:

```powershell
Get-FileHash .\zPDF-Setup-x64.exe -Algorithm SHA256
```

Silent upgrade (handy for scripting) — prompts are auto-answered with "yes", so it never waits:

```powershell
.\zPDF-Setup-x64.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
```

No network calls, no telemetry.
