// Frames of themes from the app itself, to look at before the owner sees them: a test copy (his
// profiles, virtual modules, the window off screen) puts each theme on the active profile, keeps
// two frames at rest and three after a press of key 8 (logs/theme-shots/), and the deck's rate.
//   THEMES_JSON='[{"id":"x","layers":[...]}]' node scripts/ui/theme-shots.mjs [hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] || path.join(root, "target", "test-build", "release", "hubdeck.exe");
const shots = process.argv[3];
const dir = path.join(os.tmpdir(), "hubdeck-theme-shots");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9494;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "nudges.json", "pet.json", "taste.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const profile = settings.active;


const THEMES = JSON.parse(process.env.THEMES_JSON);
const out = path.join(root, "logs", "theme-shots");
fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });
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
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(3000);
  const shot = async (name) => {
    const f = await api("deck/keys");
    if (f?.jpeg) fs.writeFileSync(path.join(out, `${name}.jpg`), Buffer.from(f.jpeg, "base64"));
  };
  for (const th of THEMES) {
    const p = await api(`profiles/${profile}`);
    await win.evaluate((profile) => window.__TAURI_INTERNALS__.invoke("profile_save", { profile }), { ...p, effects: { layers: th.layers, theme: th.id } });
    await sleep(7000);
    await shot(`${th.id}-a`);
    await sleep(2500);
    await shot(`${th.id}-b`);
    await api("effects/press", { key: 7 });
    for (const ms of [250, 700, 1500]) {
      await sleep(ms === 250 ? 250 : ms === 700 ? 450 : 800);
      await shot(`${th.id}-press-${ms}`);
    }
    const rate = await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("deck_rate"));
    console.log(`${th.id}: ${JSON.stringify(rate)}`);
  }
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1500);
}
const log = fs.readFileSync(path.join(dir, "logs", "hubdeck.log"), "utf8").split("\n").filter((l) => /does not compile|panicked|ERROR|WARN|deck: keys|living deck/.test(l));
console.log(log.slice(-12).join("\n"));
console.log(`screen clean: ${JSON.stringify(screenClean(dir))}`);
fs.rmSync(dir, { recursive: true, force: true });
