// Does the test copy's screen watch catch a window that is no longer off screen? The walk moves the
// copy's own window from -20000 to -9000 (still off every monitor, past the watch's -10000 line,
// so nothing reaches the owner's screen) and expects the copy to log `test copy: on the screen:`,
// move it back, and `screenClean` to fail (owner's pick 1a, 2026-10-23).
//   node scripts/ui/screen-check.mjs [path to hubdeck.exe]    # default: the test build
import { execFileSync, spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] || path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-screen-check");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
// the copy's main window: its position, or moved to x, y (no activation, not shown if hidden)
const win32 = (pid, move) =>
  execFileSync("powershell", ["-NoProfile", "-Command", `
Add-Type @'
using System; using System.Runtime.InteropServices; using System.Text;
public static class W {
  public delegate bool P(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(P f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
  public struct R { public int L, T, Ri, B; }
}
'@
$out = @()
[W]::EnumWindows({ param($h, $l) $p = 0; [void][W]::GetWindowThreadProcessId($h, [ref]$p); if ($p -eq ${pid} -and [W]::IsWindowVisible($h) -and [W]::GetWindowTextLength($h) -gt 0) { $r = New-Object W+R; [void][W]::GetWindowRect($h, [ref]$r); ${move ? `[void][W]::SetWindowPos($h, [IntPtr]1, ${move[0]}, ${move[1]}, 0, 0, 0x0011);` : ""} $script:out += "$($r.L),$($r.T)" }; $true }, [IntPtr]::Zero) | Out-Null
$out -join ";"
`], { encoding: "utf8" }).trim();

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9478", HUBDECK_SCREEN_FROM: "-15000" }, stdio: "ignore" });
try {
  for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
  await sleep(4000);
  // windows with a title only: Tao's 18 px event window (never drawn) sits at 0,0 in every Tauri app
  const at = win32(app.pid);
  check("the copy's window starts off screen", at.length > 0 && at.split(";").every((p) => Number(p.split(",")[0]) <= -10000), at);
  check("nothing logged before", screenClean(dir)[0]);
  win32(app.pid, [-9000, -9000]);
  await sleep(2000);
  const back = win32(app.pid);
  check("the watch moved it back off screen", back.split(";").every((p) => Number(p.split(",")[0]) <= -12000), back);
  const [clean, said] = screenClean(dir);
  check("screenClean fails on it, naming the window", !clean && said.includes("HubDeck"), said);
} finally {
  app.kill();
  await sleep(1500);
}
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
