# Experiment: restart the 15-key module's USB device node (needs administrator).
# Tries `pnputil /restart-device` first, then disable + enable, printing what happened.
param([string]$ProductId = '01FE')

$dev = Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like "USB\VID_2516&PID_$ProductId*" } | Select-Object -First 1
if (-not $dev) { "module VID_2516 PID_$ProductId not found"; return }
"device: $($dev.InstanceId)"

"--- pnputil /restart-device"
pnputil /restart-device "$($dev.InstanceId)"
Start-Sleep -Seconds 3
"status after restart: $((Get-PnpDevice -InstanceId $dev.InstanceId).Status)"
"Now ask Claude to check the module. If still wedged, press Enter here to try disable + enable."
Read-Host | Out-Null

"--- pnputil /disable-device + /enable-device"
pnputil /disable-device "$($dev.InstanceId)"
Start-Sleep -Seconds 2
pnputil /enable-device "$($dev.InstanceId)"
Start-Sleep -Seconds 3
"status after disable/enable: $((Get-PnpDevice -InstanceId $dev.InstanceId).Status)"
"Done. Ask Claude to check the module again, then close this window."
