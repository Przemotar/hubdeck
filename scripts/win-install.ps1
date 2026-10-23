# Install the newest built HubDeck on this PC without taking the owner's focus (the Windows side of
# scripts/mac-install.sh). Build first: scripts/test-build.ps1 (a test build, faster), or
# cd app; pnpm tauri build --bundles nsis (a release build)
#
#   powershell -ExecutionPolicy Bypass -File scripts/win-install.ps1
#   ... -File scripts/win-install.ps1 -Force   # install even while the owner uses the assistant
#   ... -File scripts/win-install.ps1 -Check   # only say whether an install now would interrupt him
#
# It does not install while the owner is in the middle of something the restart would take away:
# an assistant turn running or finished in the last 15 min, or a version on the deck for a try
# (owner's pick 2a, 2026-10-23; at 18:59 on 2026-10-22 an install restarted HubDeck 20 s after his
# assistant turn and took its changes off his deck).
#
# The running copy is asked to quit through the local API (it hands the deck back; killing it can
# leave the keys module mid-picture, docs/KNOWLEDGE.md), the installer runs silently (/S), and the
# app starts in the tray (--hidden).
param([switch]$Force, [switch]$Check)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
# the newest of a release build and a test build (scripts/test-build.ps1)
$setup = Get-ChildItem "$root\target\release\bundle\nsis\*-setup.exe", "$root\target\test-build\release\bundle\nsis\*-setup.exe" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setup) { Write-Error "no installer in target\release or target\test-build: build first" }
$data = "$env:APPDATA\dev.hubdeck.app"
$exe = "$env:LOCALAPPDATA\HubDeck\hubdeck.exe"

# the app and its guard (hubdeck --guard, which leaves when the app quits) are the same program
$running = Get-Process -Name hubdeck -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }
if ($running -and -not $Force) {
    $busy = @()
    # a turn running now: Claude Code started by HubDeck
    $ids = @($running | ForEach-Object { $_.Id })
    if (Get-CimInstance Win32_Process -Filter "Name = 'claude.exe'" | Where-Object { $ids -contains $_.ParentProcessId }) { $busy += "an assistant turn is running" }
    # a turn finished in the last 15 min
    $log = "$env:LOCALAPPDATA\dev.hubdeck.app\logs\hubdeck.log"
    $since = (Get-Date).AddMinutes(-15)
    $last = Get-Content $log -Tail 20000 -ErrorAction SilentlyContinue | Where-Object { $_ -match '^\[(\d{4}-\d\d-\d\d)\]\[(\d\d:\d\d:\d\d)\].*assistant: turn finished' } | Select-Object -Last 1
    if ($last -match '^\[(\d{4}-\d\d-\d\d)\]\[(\d\d:\d\d:\d\d)\]') {
        $at = [datetime]::ParseExact("$($Matches[1]) $($Matches[2])", "yyyy-MM-dd HH:mm:ss", $null)
        if ($at -gt $since) { $busy += "an assistant turn finished at $($Matches[2])" }
    }
    # a version on the deck for a try
    try {
        $token = (Get-Content "$data\api-token" -Raw).Trim()
        $port = (Get-Content "$data\api-http-port" -Raw).Trim()
        $state = Invoke-RestMethod -Uri "http://127.0.0.1:$port/api/state" -Headers @{ Authorization = "Bearer $token" } -TimeoutSec 5
        if ($state.trying) { $busy += "a version is on the deck for a try" }
    } catch { Write-Output "reading the state through the API failed: $_" }
    if ($busy) {
        Write-Output ("not installed: the owner is using HubDeck ({0}); wait, or run with -Force when he says so" -f ($busy -join "; "))
        exit 2
    }
}
if ($Check) { Write-Output "free: nothing of the owner's would be interrupted"; exit 0 }
if ($running) {
    try {
        $token = (Get-Content "$data\api-token" -Raw).Trim()
        $port = (Get-Content "$data\api-http-port" -Raw).Trim()
        Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:$port/api/quit" -Headers @{ Authorization = "Bearer $token" } -TimeoutSec 5 | Out-Null
    } catch { Write-Output "quit through the API failed: $_" }
    for ($i = 0; $i -lt 30 -and (Get-Process -Id $running.Id -ErrorAction SilentlyContinue); $i++) { Start-Sleep -Milliseconds 500 }
    if (Get-Process -Id $running.Id -ErrorAction SilentlyContinue) { Write-Error "HubDeck did not quit within 15 s" }
}

Write-Output ("installing {0} ({1:N1} MB, built {2})" -f $setup.Name, ($setup.Length / 1MB), $setup.LastWriteTime.ToString("HH:mm"))
$p = Start-Process -FilePath $setup.FullName -ArgumentList "/S" -PassThru -Wait
if ($p.ExitCode -ne 0) { Write-Error "the installer ended with $($p.ExitCode)" }
# a silent NSIS install may start the app itself; start it in the tray only if it did not
Start-Sleep -Seconds 2
if (-not (Get-Process -Name hubdeck -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe })) {
    # started through WMI, not as a child of this shell: a Claude Code session that ends takes its
    # child processes with it, and the app and its guard died with the session at 14:02 on
    # 2026-10-16 without handing the deck back (docs/KNOWLEDGE.md)
    $r = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine = "`"$exe`" --hidden" }
    if ($r.ReturnValue -ne 0) { Write-Error "starting HubDeck failed: $($r.ReturnValue)" }
}
Write-Output ("installed and running: {0}" -f (Get-Item $exe).LastWriteTime.ToString("yyyy-MM-dd HH:mm"))
