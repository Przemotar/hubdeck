# Resize the off-screen test window (physical pixels) without moving or activating it.
param([int]$W, [int]$H, [string]$Dir = "$env:TEMP\hubdeck-automation")
Add-Type @'
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public class WR {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
  public static List<IntPtr> Find(uint pid, string title) {
    var r = new List<IntPtr>();
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); var s = new StringBuilder(256); GetWindowText(h, s, 256); if (p == pid && s.ToString() == title) r.Add(h); return true; }, IntPtr.Zero);
    return r;
  }
}
'@
$ids = Get-Content "$Dir\automation.pid"
$p = Get-Process -Id $ids -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -eq 'hubdeck' }
foreach ($hwnd in [WR]::Find([uint32]$p.Id, "HubDeck")) {
  # SWP_NOMOVE 0x2 | SWP_NOZORDER 0x4 | SWP_NOACTIVATE 0x10
  [void][WR]::SetWindowPos($hwnd, [IntPtr]::Zero, 0, 0, $W, $H, 0x16)
  "resized window $hwnd of $($p.Id) to $W x $H"
}
