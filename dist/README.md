# dist

Prebuilt zPDF 1.1.4 binaries, built by GitHub Actions on tag `v1.1.4`.

- `zPDF-Setup-arm64.exe` — 4779585 bytes — sha256 `f7e76605f34d6b5ad51df7ac83ff758503f46cb8fba2c7b95406f842cdf5428b`
- `zPDF-Setup-x64.exe` — 5044844 bytes — sha256 `ba2b5bd523aa98fc93769a45c2c3248292e7c23e15590169ec93341d83f9cc44`
- `zPDF-arm64.zip` — 3817583 bytes — sha256 `7af64ed1b6daaae60020ee6ca0e2afbc71d745f25e4dbb120fa95e53c047b19a`
- `zPDF-x64.zip` — 4061979 bytes — sha256 `ce8e806baa3e36d9269825938b08237ccb67c3caa2fc2a577016df36fded6fbe`

Two ways to get zPDF:

- **Installer** (`zPDF-Setup-x64.exe` / `zPDF-Setup-arm64.exe`) — installs to your user profile, adds a
  Start-menu entry and an uninstaller. Running a newer installer on a machine that already has zPDF
  **upgrades it in place**: same folder, same uninstall entry, no duplicate installs, no uninstall step
  needed. If the machine already has a *newer* version than the installer, it asks before going backwards
  (never blocks it).
- **Portable** (`zPDF-x64.zip` / `zPDF-arm64.zip`) — no install. Unzip the whole archive and run `zPDF.exe`
  from inside that folder; `pdfium.dll` must stay beside it.

`pdfium.dll` is bundled in every download. Without it the app reports *could not find 'pdfium.dll'*.

Silent upgrade (handy for scripting) — prompts are auto-answered with "yes", so it never waits:

```powershell
.\zPDF-Setup-x64.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
```

These are the same files attached to the [v1.1.4 release](https://github.com/zaczzk/zPDF/releases/tag/v1.1.4).
Verify before you run: `Get-FileHash dist\zPDF-Setup-x64.exe -Algorithm SHA256`.

No network calls, no telemetry.
