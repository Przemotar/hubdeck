# Footprint soak test (pain point 16): how much CPU and memory HubDeck uses over a long run.
#
#   powershell -ExecutionPolicy Bypass -File scripts/soak.ps1 [-Minutes 30] [-Every 10] [-Out soak.csv]
#
# Run it against the release build (installed or target/release), deck attached, the window closed
# (tray only), the profile you want to measure active. It samples the hubdeck process every
# -Every seconds, writes a CSV and prints the average and peak CPU (% of one core) and the memory
# at the start, at the end and its growth. Targets before a release (docs/KNOWLEDGE.md > Numbers):
# idle under 1 % of a core and under 60 MB, no steady memory growth.
param(
    [int]$Minutes = 30,
    [int]$Every = 10,
    [string]$Out = "soak.csv",
    [string]$Name = "hubdeck"
)

$p = Get-Process -Name $Name -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $p) { Write-Error "$Name is not running"; exit 1 }
Write-Output ("measuring {0} (pid {1}, {2}) for {3} min, a sample every {4} s" -f $Name, $p.Id, $p.Path, $Minutes, $Every)

$rows = @()
$lastCpu = $p.TotalProcessorTime.TotalSeconds
$lastAt = Get-Date
$end = $lastAt.AddMinutes($Minutes)
while ((Get-Date) -lt $end) {
    Start-Sleep -Seconds $Every
    $p.Refresh()
    if ($p.HasExited) { Write-Error "$Name exited during the test"; break }
    $now = Get-Date
    $cpu = $p.TotalProcessorTime.TotalSeconds
    $pct = 100 * ($cpu - $lastCpu) / ($now - $lastAt).TotalSeconds
    $rows += [pscustomobject]@{
        time       = $now.ToString("HH:mm:ss")
        cpu_pct    = [math]::Round($pct, 2)
        working_mb = [math]::Round($p.WorkingSet64 / 1MB, 1)
        private_mb = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
        handles    = $p.HandleCount
        threads    = $p.Threads.Count
    }
    $lastCpu = $cpu
    $lastAt = $now
}

if ($rows.Count -eq 0) { exit 1 }
$rows | Export-Csv -NoTypeInformation -Path $Out
$cpuStats = $rows | Measure-Object cpu_pct -Average -Maximum
$first = $rows[0]
$last = $rows[-1]
Write-Output ("samples {0}; CPU average {1:N2} %, peak {2:N2} % of one core" -f $rows.Count, $cpuStats.Average, $cpuStats.Maximum)
Write-Output ("memory (private) {0} MB -> {1} MB ({2:+0.0;-0.0} MB); working set {3} MB -> {4} MB" -f $first.private_mb, $last.private_mb, ($last.private_mb - $first.private_mb), $first.working_mb, $last.working_mb)
Write-Output ("handles {0} -> {1}; threads {2} -> {3}; CSV: {4}" -f $first.handles, $last.handles, $first.threads, $last.threads, (Resolve-Path $Out))
