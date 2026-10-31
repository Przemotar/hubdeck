# A build for the owner's deck to test, faster than a release build (owner's pick 1a, 2026-10-18:
# "Da się jakoś zrobić żeby szybciej się budowało?"; release builds took 10-11 min). Thin LTO, 16
# codegen units and incremental compiling, in a target folder of its own so the release build's
# cache stays as it is. Public releases keep the full settings (Cargo.toml) and are built by CI.
#
#   powershell -ExecutionPolicy Bypass -File scripts/test-build.ps1      # then scripts/win-install.ps1
$ErrorActionPreference = "Stop"
# below normal, so the owner's own programs keep the processor while it builds (owner, 2026-10-26:
# "coś mi mega zamula"); cargo and rustc started from here inherit it
(Get-Process -Id $PID).PriorityClass = "Idle"
$root = Split-Path -Parent $PSScriptRoot
$env:CARGO_TARGET_DIR = Join-Path $root "target\test-build"
$env:CARGO_PROFILE_RELEASE_LTO = "thin"
# at most half the processor's cores, the compile jobs and the final optimisation's threads alike
# (owner's "1 a", 2026-10-26: the PC stays smooth, a build takes ~7-8 min instead of ~5)
$half = [Math]::Max(2, [Math]::Floor([Environment]::ProcessorCount / 2))
$env:CARGO_BUILD_JOBS = "$half"
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "$half"
$env:CARGO_PROFILE_RELEASE_INCREMENTAL = "true"
# marks the app as a test build: only a test build reads shaders from the folder named in
# `shader-dir.txt` (canvas.rs `shader_dir`); release builds (CI) never do
$env:HUBDECK_TEST_BUILD = "1"
# every shader compiles before the build (2026-10-26: two layers never ran on the deck, the app's log
# only said "Validation Error"); needs the `wgpu` package in .venv (docs/SETUP.md)
$py = Join-Path $root ".venv\Scripts\python.exe"
if (Test-Path $py) {
    & $py -I (Join-Path $root "scripts\wgsl-check.py") | Where-Object { $_ -notmatch "^ok" }
    if ($LASTEXITCODE -ne 0) { throw "a shader does not compile (scripts/wgsl-check.py)" }
}
$started = Get-Date
Push-Location (Join-Path $root "app")
try {
    pnpm tauri build --bundles nsis
    if ($LASTEXITCODE -ne 0) { throw "the build failed" }
} finally {
    Pop-Location
}
"test build done in {0:N1} min" -f ((Get-Date) - $started).TotalMinutes
