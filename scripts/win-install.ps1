# Install the newest built HubDeck on this PC without taking the owner's focus (the Windows side of
# scripts/mac-install.sh). Build first: scripts/test-build.ps1 (a test build, faster), or
# cd app; pnpm tauri build --bundles nsis (a release build)
#
#   powershell -ExecutionPolicy Bypass -File scripts/win-install.ps1
#
# The running copy is asked to quit through the local API (it hands the deck back; killing it can
# leave the keys module mid-picture, docs/KNOWLEDGE.md), the installer runs silently (/S), and the
# app starts in the tray (--hidden).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
# the newest of a release build and a test build (scripts/test-build.ps1)
$setup = Get-ChildItem "$root\target\release\bundle\nsis\*-setup.exe", "$root\target\test-build\release\bundle\nsis\*-setup.exe" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setup) { Write-Error "no installer in target\release or target\test-build: build first" }
$data = "$env:APPDATA\dev.hubdeck.app"
$exe = "$env:LOCALAPPDATA\HubDeck\hubdeck.exe"

# the app and its guard (hubdeck --guard, which leaves when the app quits) are the same program
$running = Get-Process -Name hubdeck -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }
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
