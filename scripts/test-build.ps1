# A build for the owner's deck to test, faster than a release build (owner's pick 1a, 2026-10-18:
# "Da się jakoś zrobić żeby szybciej się budowało?"; release builds took 10-11 min). Thin LTO, 16
# codegen units and incremental compiling, in a target folder of its own so the release build's
# cache stays as it is. Public releases keep the full settings (Cargo.toml) and are built by CI.
#
#   powershell -ExecutionPolicy Bypass -File scripts/test-build.ps1      # then scripts/win-install.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$env:CARGO_TARGET_DIR = Join-Path $root "target\test-build"
$env:CARGO_PROFILE_RELEASE_LTO = "thin"
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "16"
$env:CARGO_PROFILE_RELEASE_INCREMENTAL = "true"
$started = Get-Date
Push-Location (Join-Path $root "app")
try {
    pnpm tauri build --bundles nsis
    if ($LASTEXITCODE -ne 0) { throw "the build failed" }
} finally {
    Pop-Location
}
"test build done in {0:N1} min" -f ((Get-Date) - $started).TotalMinutes
