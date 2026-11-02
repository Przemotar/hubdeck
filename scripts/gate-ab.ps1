# The soak's CPU against the last release on the same PC, in turns (owner's strategy, 2026-11-01:
# "the gate compares with the last release"; ROADMAP > Now N2). One soak per version decides nothing:
# on 2026-11-01 the same 0.2.0 build gave 14.3, 17.5 and 22.8 % in three soaks while the PC was busy.
# This installs the two release installers in turns (A B A B ...), soaks each with the gate's profile,
# and prints both means; the candidate is installed again at the end. The user's data is copied first
# and stays in the scratch folder named below until it is deleted by hand.
#
#   powershell -ExecutionPolicy Bypass -File scripts/gate-ab.ps1 -Baseline 0.2.0 -Candidate 0.2.1 [-Pairs 2] [-Minutes 4]
#
# Both installers must be in target/release/bundle/nsis (HubDeck_<version>_x64-setup.exe); an older one
# can be downloaded from its GitHub release into that folder. The verdict: the candidate passes when its
# mean is at most 2 points above the baseline's, or under the gate's 15 % limit.
param(
    [Parameter(Mandatory = $true)][string]$Baseline,
    [Parameter(Mandatory = $true)][string]$Candidate,
    [int]$Pairs = 2,
    [int]$Minutes = 4
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$nsis = Join-Path $root "target\release\bundle\nsis"
foreach ($v in @($Baseline, $Candidate)) {
    if (-not (Test-Path (Join-Path $nsis "HubDeck_${v}_x64-setup.exe"))) { throw "no installer for $v in $nsis" }
}
$data = Join-Path $env:APPDATA "dev.hubdeck.app"
$backup = Join-Path $env:TEMP ("hubdeck-ab-backup-" + (Get-Date -Format "yyyyMMdd-HHmmss"))
Copy-Item -Recurse $data $backup
"his data copied to $backup"
$cpu = @{ $Baseline = @(); $Candidate = @() }
$order = @()
for ($i = 0; $i -lt $Pairs; $i++) { $order += $Baseline; $order += $Candidate }
foreach ($v in $order) {
    # win-install.ps1 installs the newest installer: make this one the newest
    (Get-Item (Join-Path $nsis "HubDeck_${v}_x64-setup.exe")).LastWriteTime = Get-Date
    powershell -ExecutionPolicy Bypass -File (Join-Path $root "scripts\win-install.ps1") | Select-Object -Last 1
    Start-Sleep -Seconds 20
    Push-Location $root
    $out = node scripts/release-gate.mjs --minutes $Minutes --skip audit,sizes,readme,calls,restarts,window,questions,catalog 2>&1 | Out-String
    Pop-Location
    if ($out -match "soak CPU: average ([0-9.]+) %") {
        $cpu[$v] += [double]$Matches[1]
        "{0}: {1} %" -f $v, $Matches[1]
    } else {
        "{0}: no CPU line in the gate's output" -f $v
    }
}
# the candidate stays installed
(Get-Item (Join-Path $nsis "HubDeck_${Candidate}_x64-setup.exe")).LastWriteTime = Get-Date
if ($order[-1] -ne $Candidate) { powershell -ExecutionPolicy Bypass -File (Join-Path $root "scripts\win-install.ps1") | Select-Object -Last 1 }
$a = ($cpu[$Baseline] | Measure-Object -Average).Average
$b = ($cpu[$Candidate] | Measure-Object -Average).Average
"baseline {0}: {1:N1} % ({2})" -f $Baseline, $a, ($cpu[$Baseline] -join ", ")
"candidate {0}: {1:N1} % ({2})" -f $Candidate, $b, ($cpu[$Candidate] -join ", ")
if ($b -le $a + 2 -or $b -lt 15) { "PASS  soak CPU against $Baseline" } else { "FAIL  soak CPU: $Candidate {0:N1} % against {1:N1} % (more than 2 points)" -f $b, $a; exit 1 }
