# Single source of version truth. Both workflows call this file so the release build and the
# every-push check cannot drift apart in strictness.
#
#   powershell -File scripts/check-versions.ps1
#
# Fails when Cargo.toml, resources.rc (FILEVERSION, PRODUCTVERSION, FileVersion string,
# ProductVersion string) or installer.iss disagree. ProductVersion was 1.0.0.0 for three
# releases because nothing compared it.
$ErrorActionPreference = 'Stop'

# Work from the repo root however this is called: ./scripts/check-versions.ps1 from root,
# an absolute -File path from anywhere, or a CI step.
if (Test-Path (Join-Path (Get-Location) 'resources.rc')) {
  $root = (Get-Location).Path
} else {
  $root = Split-Path -Parent $PSScriptRoot
  Set-Location $root
}

function Norm([string]$v) {
  $p = $v.Split('.')
  while ($p.Count -lt 4) { $p += '0' }
  return ($p[0..3] -join '.')
}

# Exactly one match, so a duplicated or reordered line can never be silently compared instead.
function Field([string]$file, [string]$pattern) {
  # Count the MatchInfo objects, not .Matches on a possibly-scalar result: with one matching line
  # PowerShell hands back a single MatchInfo, and property enumeration on that is not an array.
  $hits = @(Select-String -CaseSensitive -Path $file -Pattern $pattern)
  if ($hits.Count -eq 0) { throw "$file`: no line matches $pattern" }
  if ($hits.Count -ne 1) { throw "$file`: $($hits.Count) lines match $pattern, expected exactly one" }
  return $hits[0].Matches[0].Groups[1].Value
}

# cargo metadata, not a regex over Cargo.toml: a dependency or workspace version line
# before the [package] section would make a text search compare the wrong version.
$cargo = (Get-Command cargo -ErrorAction SilentlyContinue)
if ($cargo) {
    $meta = & cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    $cargoVersion = $meta.packages | Where-Object { $_.name -eq 'zpdf' } | Select-Object -First 1
    if (-not $cargoVersion) { $cargoVersion = $meta.packages[0] }
    $cargoVersion = Norm $cargoVersion.version
} else {
    # Local convenience only: Cargo.toml is NOT compared on this path, so say so loudly. Both CI
    # workflows install a toolchain, so a released artifact always goes through the cargo branch.
    $cargoVersion = Norm ((Field 'resources.rc' '^FILEVERSION (.+)') -replace ',', '.')
    Write-Warning 'cargo not on PATH: Cargo.toml was NOT compared, only resources.rc and installer.iss'
}

# cargo metadata reads Cargo.toml only, so a stale lockfile would slip through. Cargo.lock carries
# the same version for the root package; check it when it exists.
if (Test-Path 'Cargo.lock') {
  $lock = @(Select-String -CaseSensitive -Path Cargo.lock -Pattern '^name = "zpdf"\s*$' -Context 0,1)
  if ($lock.Count -eq 0) { throw "Cargo.lock has no entry named zpdf - cannot confirm the lock matches" }
  if ($lock.Count -gt 1) { throw "Cargo.lock has $($lock.Count) entries named zpdf - cannot tell which one is the package" }
  if (-not $lock[0].Context.PostContext -or $lock[0].Context.PostContext.Count -eq 0) { throw "Cargo.lock: no version line follows the name = zpdf entry" }
  $lockVersion = [regex]::Match($lock[0].Context.PostContext[0], '(\d+(\.\d+)+)').Groups[1].Value
  if (-not $lockVersion) { throw 'Cargo.lock: could not read the version after the name = zpdf entry' }
  if ((Norm $lockVersion) -ne $cargoVersion) {
    throw "Cargo.lock says $lockVersion but Cargo.toml says $cargoVersion; run cargo update -p zpdf or bump the lock"
  }
}

$sources = [ordered]@{
  'resources.rc FILEVERSION'          = Norm ((Field 'resources.rc' '^FILEVERSION (.+)') -replace ',', '.')
  'resources.rc PRODUCTVERSION'       = Norm ((Field 'resources.rc' '^PRODUCTVERSION (.+)') -replace ',', '.')
  'resources.rc FileVersion string'   = Norm (Field 'resources.rc' '^\s*VALUE "FileVersion", "([\d.]+)')
  'resources.rc ProductVersion string'= Norm (Field 'resources.rc' '^\s*VALUE "ProductVersion", "([\d.]+)')
  'installer.iss MyAppVersion'        = Norm (Field 'installer.iss' '^#define MyAppVersion "([\d.]+)"')
}

$bad = @()
foreach ($name in $sources.Keys) {
  if ($sources[$name] -ne $cargoVersion) { $bad += "$name = $($sources[$name])" }
}

if ($bad.Count -gt 0) {
  throw "version drift: expected $cargoVersion, found $($bad -join ', ')"
}

Write-Host "all version locations agree on $cargoVersion"
