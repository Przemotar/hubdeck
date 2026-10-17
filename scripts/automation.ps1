# Starts a test copy of HubDeck for checks driven from a script (Playwright over DevTools),
# without touching the running app, its data or the deck, and without taking the focus:
# its own data folder (a copy of the real data), virtual modules, the window off-screen.
# See app/src-tauri/src/automation.rs and docs/KNOWLEDGE.md > "Testing the window yourself".
#
#   scripts/automation.ps1            # start (copies the real data the first time)
#   scripts/automation.ps1 -Fresh     # copy the real data again first
#   scripts/automation.ps1 -Stop      # stop the test copy (Vite stays: other copies use it)
#   scripts/automation.ps1 -StopVite  # stop the shared Vite server too
#   scripts/automation.ps1 -Open <f>  # start it as a double click on a plugin file would
param(
    [string]$Dir = "$env:TEMP\hubdeck-automation",
    [int]$CdpPort = 9444,
    [string]$Modules = "",
    [string]$Open = "",
    [switch]$Fresh,
    [switch]$Stop,
    [switch]$StopVite
)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $root "target\debug\hubdeck.exe"
$pidFile = Join-Path $Dir "automation.pid"
# one Vite server for all copies; only -StopVite ends it
$vitePidFile = Join-Path $env:TEMP "hubdeck-automation-vite.pid"

function Stop-Copy {
    if (Test-Path $pidFile) {
        foreach ($id in (Get-Content $pidFile)) {
            try { Stop-Process -Id ([int]$id) -Force -ErrorAction Stop } catch {}
        }
        Remove-Item $pidFile -Force
    }
}

if ($StopVite -and (Test-Path $vitePidFile)) {
    & taskkill /PID (Get-Content $vitePidFile) /T /F | Out-Null
    Remove-Item $vitePidFile -Force
}
if ($Stop -or $StopVite) { Stop-Copy; "stopped"; exit 0 }
Stop-Copy

$real = Join-Path $env:APPDATA "dev.hubdeck.app"
# the logs stay (an old Vite server may still write there)
if ($Fresh -and (Test-Path $Dir)) { Get-ChildItem $Dir -Exclude logs | Remove-Item -Recurse -Force }
if (-not (Test-Path (Join-Path $Dir "settings.json"))) {
    New-Item -ItemType Directory -Force $Dir | Out-Null
    foreach ($item in "profiles", "images", "settings.json", "placements.json", "apps.json", "installed.json", "games.json", "usage.json") {
        $src = Join-Path $real $item
        if (Test-Path $src) { Copy-Item -Recurse -Force $src $Dir }
    }
}
New-Item -ItemType Directory -Force (Join-Path $Dir "logs") | Out-Null

if (-not (Test-Path $exe)) { throw "build it first: cargo build -p hubdeck" }

$pids = @()
# a debug build loads the window from Vite (devUrl, port 1420)
$vite = $null
try { $vite = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 http://localhost:1420/ } catch {}
if (-not $vite) {
    $p = Start-Process -PassThru -WindowStyle Hidden -WorkingDirectory (Join-Path $root "app") cmd.exe -ArgumentList "/c", "pnpm dev > `"$env:TEMP\hubdeck-automation-vite.log`" 2>&1"
    $p.Id | Set-Content $vitePidFile
    for ($i = 0; $i -lt 30 -and -not $vite; $i++) {
        Start-Sleep -Milliseconds 500
        try { $vite = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 http://localhost:1420/ } catch {}
    }
}

$env:HUBDECK_AUTOMATION = $Dir
$env:HUBDECK_CDP_PORT = "$CdpPort"
if ($Modules) { $env:HUBDECK_VIRTUAL_MODULES = $Modules }
$app = if ($Open) { Start-Process -PassThru -WindowStyle Hidden $exe -ArgumentList "`"$Open`"" } else { Start-Process -PassThru -WindowStyle Hidden $exe }
$pids += $app.Id
$pids | Set-Content $pidFile

for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep -Milliseconds 500
    try {
        Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 "http://127.0.0.1:$CdpPort/json/version" | Out-Null
        "test copy running: data $Dir, DevTools 127.0.0.1:$CdpPort, API port in $Dir\api-http-port"
        exit 0
    } catch {}
}
throw "the test copy did not open DevTools on $CdpPort (log: $Dir\logs)"
