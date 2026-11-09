# Prints the zpdf package version. Cargo.toml is the only place a version is written:
# build.rs stamps it into the exe (VERSIONINFO + manifest) and CI passes it to Inno Setup.
#
#   ./scripts/check-versions.ps1                 # prints e.g. 1.1.5
#   ./scripts/check-versions.ps1 -Tag v1.1.5     # also fails unless the tag is v<version>
#
# --locked makes cargo fail when Cargo.lock no longer matches Cargo.toml (a bumped version
# without a refreshed lock).
param([string]$Tag = '')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

$meta = cargo metadata --format-version 1 --no-deps --locked | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata --locked failed: is Cargo.lock out of date?' }
$version = ($meta.packages | Where-Object { $_.name -eq 'zpdf' }).version
if (-not $version) { throw 'zpdf package not found in cargo metadata' }

if ($Tag -and $Tag -ne "v$version") {
  throw "tag $Tag does not match Cargo.toml version $version (expected v$version)"
}
$version
