// What's new (owner's "2 a", 2026-10-30): on a test copy with his profiles (the window off screen) it
// does not come up by itself for a version it has no notes for, Help and feedback opens the newest,
// Try shows the preset on the deck, a row opens its place, Escape closes. Screenshots in logs/whats-new/.
//   node scripts/ui/whats-new.mjs [path to hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-whats-new");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "whats-new");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9492" }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
const api = (route) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { headers: { Authorization: `Bearer ${token}` } }).then((r) => r.json());
};
let browser;
try {
  browser = await chromium.connectOverCDP("http://127.0.0.1:9492");
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  await win.setViewportSize({ width: 1606, height: 1000 });
  await sleep(3000);
  check("it does not come up by itself for a version without notes", (await win.locator(".whatsnew").count()) === 0);
  await win.locator(".help-menu > button").click();
  await sleep(500);
  await win.getByRole("menuitem", { name: /What's new/ }).click();
  await sleep(1500);
  const dlg = win.locator(".whatsnew");
  check("Help and feedback opens it", await dlg.isVisible());
  await dlg.screenshot({ path: path.join(shots, "1-dialog.png") });
  const cut = (await clipped(win)).filter((x) => x.text);
  check("nothing cut in it", cut.length === 0, cut.map((x) => x.text).join(" | "));
  const before = ((await api("deck/keys"))?.jpeg ?? "").slice(-400);
  await dlg.getByRole("button", { name: /Try it on my deck/ }).click();
  await sleep(2000);
  check("Try shows the preset on the deck", ((await api("deck/keys"))?.jpeg ?? "").slice(-400) !== before);
  await dlg.locator(".wn-item.is-link").first().click();
  await sleep(1200);
  check("a row opens its place (the Screensaver panel)", (await win.locator(".whatsnew").count()) === 0 && (await win.locator("aside.inspector.living-panel").innerText()).includes("Screensaver"));
  await win.locator(".help-menu > button").click();
  await sleep(400);
  await win.getByRole("menuitem", { name: /What's new/ }).click();
  await sleep(800);
  await win.keyboard.press("Escape");
  await sleep(500);
  check("Escape closes it", (await win.locator(".whatsnew").count()) === 0);
  await win.setViewportSize({ width: 960, height: 700 });
  await win.locator(".help-menu > button").click();
  await sleep(400);
  await win.getByRole("menuitem", { name: /What's new/ }).click();
  await sleep(800);
  await win.screenshot({ path: path.join(shots, "2-small-window.png") });
  const box = await win.locator(".whatsnew").boundingBox();
  check("it fits a small window", box && box.y >= 0 && box.y + box.height <= 700, JSON.stringify(box));
  check("no page errors", errors === 0, `${errors}`);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  spawn("taskkill", ["/F", "/T", "/PID", String(app.pid)]);
  await sleep(1500);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
