// A draft on the deck for a try when HubDeck restarts is offered back (owner, 2026-10-22: the
// assistant's changes were on his deck as a try, an install restarted HubDeck and "wróciło mi do
// poprzedniej wersji"). A test copy tries a draft, is stopped and started again: the window shows
// the banner, "Put it back on the deck" shows the draft again, "Not now" drops the banner, "Use
// this" keeps the draft as a profile. Screenshot in logs/try-restart.png.
//   node scripts/ui/try-restart.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-try-restart");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9488;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let app, browser, win;
async function launch() {
  fs.rmSync(path.join(dir, "api-http-port"), { force: true });
  app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
  for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
  await sleep(1500);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  win = undefined;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(2500);
}
async function quit() {
  await browser?.close().catch(() => {});
  app?.kill();
  await sleep(1500);
}
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
const state = () => win.evaluate(() => window.__TAURI_INTERNALS__.invoke("engine_state"));

try {
  console.log(`exe: ${exe}`);
  await launch();
  const made = await api("profiles", { name: "Walk draft", draft: true, controls: { "keys.0": { action: "core.launch", params: { target: "notepad.exe" }, look: { label: "Notes" } } } });
  const id = made.profile?.id;
  await api(`profiles/${id}/try`, {});
  await sleep(800);
  let s = await state();
  check("the draft is on the deck for a try", s.trying === id);
  await quit();

  await launch();
  s = await state();
  const banner = win.locator(".banner", { hasText: /restarted while the draft/ });
  check("after a restart the deck shows the user's own profile", !s.trying && s.active !== id);
  check("and the window offers the draft back", (await banner.count()) === 1 && s.interruptedTry === id);
  await win.screenshot({ path: path.join(root, "logs", "try-restart.png") });
  await banner.getByRole("button", { name: /Put it back/ }).click();
  await sleep(1000);
  s = await state();
  check("Put it back shows the draft again", s.trying === id && (await banner.count()) === 0);
  await quit();

  await launch();
  await win.locator(".banner", { hasText: /restarted while the draft/ }).getByRole("button", { name: /Not now/ }).click();
  await sleep(800);
  s = await state();
  check("Not now drops the banner and keeps the draft", (await win.locator(".banner", { hasText: /restarted while the draft/ }).count()) === 0 && !s.interruptedTry && s.profiles.some((p) => p.id === id && p.draft));
  await api(`profiles/${id}/try`, {});
  await quit();

  await launch();
  await win.locator(".banner", { hasText: /restarted while the draft/ }).getByRole("button", { name: /Use this/ }).click();
  await sleep(1200);
  s = await state();
  check("Use this keeps it as a profile and shows it", s.profiles.some((p) => p.id === id && !p.draft) && s.active === id && !s.interruptedTry);
} catch (e) {
  check("the walk ran to the end", false, String(e).split("\n")[0]);
} finally {
  await quit();
}
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
