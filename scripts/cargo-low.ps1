# Any cargo command at below-normal priority (owner, 2026-10-26: a release test compile slowed his
# PC down): powershell -File scripts/cargo-low.ps1 test --release -p hubdeck --lib --no-run
(Get-Process -Id $PID).PriorityClass = "Idle"
# at most half the cores (owner's "1 a", 2026-10-26)
$env:CARGO_BUILD_JOBS = "$([Math]::Max(2, [Math]::Floor([Environment]::ProcessorCount / 2)))"
& cargo @args
exit $LASTEXITCODE
