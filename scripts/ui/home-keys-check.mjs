// The owner's home profile after keys were set for him (2026-10-23: Music Toolkit "love" and tag 1,
// a key to "Song sorter"): a test copy of its own with his data shows the keys, and a press of the
// profile key really switches. Pictures: logs/home-keys-*.png.
//   node scripts/ui/home-keys-check.mjs <profile id> <profile key control> <target profile id>
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const [home, control, target] = process.argv.slice(2);
const exe = path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-home-keys");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9479;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "plugins"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
let browser;
try {
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  await win.setViewportSize({ width: 1450, height: 780 });
  const invoke = (cmd, args) => win.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args]);
  await api(`profiles/${home}/activate`, {});
  await sleep(3000);
  const grid = win.locator(`g.dk-ctl[data-kind="keys"]`).first().locator("xpath=ancestor::*[name()='svg'][1]");
  for (const look of ["dark", "light"]) {
    await win.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
    await sleep(1500);
    await win.screenshot({ path: path.join(root, "logs", `home-keys-${look}.png`) });
  }
  // one key at a larger size, to read its label
  const index = Number(control.split(".")[1]);
  const key = win.locator(`g.dk-ctl[data-kind="keys"][data-index="${index}"]`).first();
  await key.screenshot({ path: path.join(root, "logs", "home-keys-profile-key.png"), scale: "device" });
  const text = await win.locator("body").innerText();
  check("no error banner on the window", !/no output device|not found|cannot/i.test(text.split("\n").slice(0, 6).join(" ")), text.split("\n").slice(0, 3).join(" | "));
  await api("test/input", { kind: "keys", index, gesture: "press", value: 1 });
  await sleep(2000);
  const state = await invoke("engine_state");
  check(`a press of ${control} switches to the profile`, state.active === target, `active: ${state.active}`);
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
