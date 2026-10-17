// The installed HubDeck's own updater installs a signed release: "Go back to the version before"
// run by the installed app (its window reached through WebView2's DevTools port), then the log must
// show that version starting with every module back. Only while the owner is away: the window opens.
// Put the newer build back afterwards (scripts/win-install.ps1).
//   node scripts/ui/rollback-check.mjs [version, default: the one before]
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const PORT = 9555;
const data = path.join(process.env.APPDATA, "dev.hubdeck.app");
const log = path.join(process.env.LOCALAPPDATA, "dev.hubdeck.app", "logs", "hubdeck.log");
const exe = path.join(process.env.LOCALAPPDATA, "HubDeck", "hubdeck.exe");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const api = (route) => {
  const token = fs.readFileSync(path.join(data, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(data, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: "POST", headers: { Authorization: `Bearer ${token}` } }).catch(() => {});
};
const running = () => execFileSync("powershell", ["-NoProfile", "-Command", `(Get-Process hubdeck -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${exe}' }).Count`], { encoding: "utf8" }).trim() !== "0";

await api("quit");
for (let i = 0; i < 30 && running(); i++) await sleep(500);
const before = fs.readFileSync(log, "utf8").length;
// a start with the window (no --hidden) and DevTools on its WebView2
execFileSync("powershell", ["-NoProfile", "-Command", `$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=${PORT}'; Start-Process '${exe}'`]);
let browser;
for (let i = 0; i < 40 && !browser; i++) browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`).catch(() => sleep(500).then(() => null));
let page;
for (let i = 0; i < 40 && !page; i++) {
  page = browser.contexts()[0]?.pages().find((p) => /tauri.localhost/.test(p.url()));
  if (!page) await sleep(500);
}
const invoke = (cmd, args) => page.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args ?? {}]);
const previous = process.argv[2] ?? (await invoke("update_previous"))?.version;
console.log("going back to", previous);
const t0 = Date.now();
invoke("update_rollback", { version: previous }).catch((e) => console.log("rollback:", String(e)));
let started = "";
for (let i = 0; i < 180 && !started; i++) {
  await sleep(1000);
  const tail = fs.readFileSync(log, "utf8").slice(before);
  started = tail.match(new RegExp(`HubDeck ${previous.replace(/\./g, "\\.")} starting`))?.[0] ?? "";
}
await browser.close().catch(() => {});
await sleep(8000);
const tail = fs.readFileSync(log, "utf8").slice(before);
const modules = [...tail.matchAll(/connected (\w+) /g)].map((m) => m[1]);
console.log(started ? `ok   ${started} after ${Math.round((Date.now() - t0) / 1000)} s` : "FAIL the old version did not start within 3 min");
console.log(`     modules after it: ${[...new Set(modules)].join(", ")}`);
console.log(tail.split("\n").filter((l) => /going back|ERROR|update/i.test(l)).slice(-6).join("\n"));
process.exit(started ? 0 : 1);
