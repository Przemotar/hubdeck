// The window's X hides the window and keeps what was open in it; its web view is freed only after
// it stayed hidden (owner's pick 1a, 2026-10-22: "jak zamknąłem okno hubdecka to mi je zamknęło
// zamiast zminimalizować"). The test copy closes the window the way the system does (the test
// API sends CloseRequested), with the wait cut to 8 s. Reopening from the tray icon is the owner's
// step: a click on the tray would take his focus.
//   node scripts/ui/window-close.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-window-close");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9487;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT), HUBDECK_KEEP_HIDDEN_SECS: "8" }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
const log = () => (fs.existsSync(path.join(dir, "logs", "hubdeck.log")) ? fs.readFileSync(path.join(dir, "logs", "hubdeck.log"), "utf8") : "");

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  console.log(`exe: ${exe}`);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(2000);
  // something open in the window: Settings
  await win.getByRole("button", { name: /^Settings$/ }).first().click();
  await sleep(800);
  const marker = await win.evaluate(() => (window.__closeWalk = Date.now()));
  check("Settings open before the X", (await win.locator(".settings-page").count()) > 0);

  await api("test/close", {});
  await sleep(1500);
  const alive = await win.evaluate(() => window.__closeWalk).catch(() => null);
  check("the X keeps the page (same page, nothing reloaded)", alive === marker);
  check("and what was open stays open", (await win.locator(".settings-page").count().catch(() => 0)) > 0);
  check("the log says hidden", /window: hidden by its X/.test(log()));
  check("the deck keeps working", (await api("profiles")).profiles?.length > 0);

  await sleep(15000);
  check("after the wait the web view is freed", /window: freed after 8 s hidden/.test(log()));
  const gone = await win.evaluate(() => 1).then(() => false, () => true);
  check("and its page is gone", gone);
} catch (e) {
  check("the walk ran to the end", false, String(e).split("\n")[0]);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1000);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
