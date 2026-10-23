// An update as a user gets it (owner, 2026-10-18: "Sprawdź"): the published older release is
// installed; the installed program is started as a test copy (scripts/automation.ps1's mode: its
// window off-screen, never the focus, its own data, virtual modules; a tray app's hidden window
// has no page to drive until it is opened), and the same commands the Install button of
// Settings > Updates runs (`update_check`, `update_install`) download, verify and install the
// newest release over it. Then the real app starts as installed, out of this session's process
// tree (a session that ends takes its children: docs/KNOWLEDGE.md), and must be the newest with
// every module of the deck back. The click on the button itself is not part of this.
//   node scripts/ui/update-check.mjs <older setup.exe> <newest version, e.g. 0.1.4>
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const [setup, want] = process.argv.slice(2);
if (!setup || !want) throw new Error("usage: update-check.mjs <older setup.exe> <newest version>");
const PORT = 9461;
const data = path.join(process.env.APPDATA, "dev.hubdeck.app");
const exe = path.join(process.env.LOCALAPPDATA, "HubDeck", "hubdeck.exe");
const realLog = path.join(process.env.LOCALAPPDATA, "dev.hubdeck.app", "logs", "hubdeck.log");
const copy = path.join(process.env.TEMP, "hubdeck-update-check");
const copyLog = path.join(copy, "logs", "hubdeck.log");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const ps = (cmd) => execFileSync("powershell", ["-NoProfile", "-Command", cmd], { encoding: "utf8" }).trim();
const running = () => ps(`@(Get-Process -Name hubdeck -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${exe}' }).Count`) !== "0";
const read = (file) => (fs.existsSync(file) ? fs.readFileSync(file, "utf8") : "");
const starting = (version) => new RegExp(`HubDeck ${version.replaceAll(".", "\\.")} starting`);
const results = [];
const check = (what, ok, note = "") => (results.push({ ok }), console.log(`${ok ? "ok  " : "FAIL"} ${what}${note ? ` (${note})` : ""}`));
// the real app is asked to quit through its API: it hands the deck back
const quit = async () => {
  if (!running()) return;
  const token = fs.readFileSync(path.join(data, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(data, "api-http-port"), "utf8").trim();
  await fetch(`http://127.0.0.1:${port}/api/quit`, { method: "POST", headers: { Authorization: `Bearer ${token}` } }).catch(() => {});
  for (let i = 0; i < 30 && running(); i++) await sleep(500);
  if (running()) throw new Error("HubDeck did not quit within 15 s");
};

// 1. the older release, installed
await quit();
execFileSync(setup, ["/S"]);
await sleep(3000);
await quit();

// 2. the installed program as a test copy
fs.rmSync(copy, { recursive: true, force: true });
fs.mkdirSync(path.join(copy, "logs"), { recursive: true });
fs.copyFileSync(path.join(data, "settings.json"), path.join(copy, "settings.json"));
ps(`$env:HUBDECK_AUTOMATION='${copy}'; $env:HUBDECK_CDP_PORT='${PORT}'; Start-Process '${exe}' -WindowStyle Hidden`);
let old = "";
for (let i = 0; i < 60 && !old; i++) {
  await sleep(1000);
  old = read(copyLog).match(/HubDeck (\d+\.\d+\.\d+) starting/)?.[1] ?? "";
}
check("the older release starts", !!old && old !== want, `HubDeck ${old}`);
let browser;
for (let i = 0; i < 40 && !browser; i++) browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`).catch(() => sleep(500).then(() => null));
let page;
for (let i = 0; i < 40 && !page; i++) {
  page = browser?.contexts()[0]?.pages().find((p) => /tauri.localhost/.test(p.url()));
  if (!page) await sleep(500);
}
if (!page) throw new Error("the test copy's window has no page to drive");
const invoke = (cmd, args) => page.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args ?? {}]);

// 3. Check, then Install
const found = await invoke("update_check").catch((e) => ({ error: String(e) }));
check("Check finds the newest release", found?.version === want, JSON.stringify(found).slice(0, 160));
const t1 = Date.now();
invoke("update_install").catch(() => {});
const installed = () => ps(`(Get-Item '${exe}').VersionInfo.ProductVersion`);
let done = false;
for (let i = 0; i < 240 && !done; i++) {
  await sleep(1000);
  done = starting(want).test(read(copyLog)) || installed().startsWith(want);
}
await browser?.close().catch(() => {});
check("Install downloads, verifies and installs it", done, `${Math.round((Date.now() - t1) / 1000)} s, installed ${installed()}`);

// 4. the test copy goes; the real app starts as installed and takes the deck
await sleep(3000);
ps(`Get-Process -Name hubdeck -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${exe}' } | Stop-Process -Force`);
await sleep(2000);
const from = read(realLog).length;
ps(`Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine = '"${exe}" --hidden' } | Out-Null`);
await sleep(12000);
const real = read(realLog).slice(from);
check("the installed app is now the newest", starting(want).test(real), real.match(/HubDeck [\d.]+ starting/)?.[0] ?? "no start line");
const modules = [...new Set([...real.matchAll(/connected (keys|dial|knobs|rollers|base|faders) /g)].map((m) => m[1]))];
check("every module of the deck back", ["keys", "dial", "base"].every((m) => modules.includes(m)), modules.join(", "));
const errors = real.split("\n").filter((l) => l.includes("[ERROR]"));
check("no errors in its log", errors.length === 0, errors[0]?.slice(0, 160) ?? "");
check("nothing came up on the owner's screen", ...screenClean(copy));
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\nFAILED: ${failed} of ${results.length}` : `\npassed ${results.length}/${results.length}`);
process.exit(failed ? 1 : 0);
