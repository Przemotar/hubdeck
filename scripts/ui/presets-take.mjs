// The pictures of the presets film (README > The living deck: "30 s of the presets one after another,
// with a press in each"): a test copy with an empty data folder (the profile a first start makes,
// nothing of the owner's), virtual modules, its window off screen, never the focus. Deck look tries
// each preset on the deck as a user would; the deck's own picture (`/api/deck/keys`) is kept about 30
// times a second with its time, and key 8 is pressed 1 s in. `scripts/film/presets-film.py` makes the film.
//   node scripts/ui/presets-take.mjs [hubdeck.exe] [seconds per preset, default 2.5]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const seconds = Number(process.argv[3] ?? 2.5);
const PRESETS = ["Hollow Hill", "Lantern Hollow", "Golden autumn", "Mushroom hunt", "Fuji and sakura", "Koi pond", "Tokyo Rain", "Code rain", "Synthwave", "Cyber rig", "Nebula", "Spectrum"];
const dir = path.join(os.tmpdir(), "hubdeck-presets-film");
const out = path.join(root, "logs", "presets-film");
const PORT = 9488;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify({ autoSwitch: false }));
fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
let browser;
let ok = true;
try {
  for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
  await sleep(1500);
  const api = (route, body) => {
    const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
    const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
    return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
  };
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1440, height: 900 });
  await sleep(2500);
  // a first start may greet with a tour or What's new: closed as a user would
  for (const name of [/^Not now$/, /^Got it$/, /^Skip/, /^Close$/]) {
    const b = win.getByRole("button", { name }).first();
    if (await b.isVisible().catch(() => false)) (await b.click(), await sleep(600));
  }
  await win.getByRole("button", { name: /^Living deck/ }).first().click();
  await sleep(1500);
  for (const [i, name] of PRESETS.entries()) {
    await win.locator(".dl-tile", { hasText: name }).first().click();
    // the first pictures of a scene settle (a program compiled, its state filled)
    await sleep(1800);
    const to = path.join(out, String(i).padStart(2, "0"));
    fs.mkdirSync(to);
    const t0 = Date.now();
    let last = "";
    let pressed = false;
    const times = [];
    while (Date.now() - t0 < seconds * 1000) {
      if (!pressed && Date.now() - t0 >= 1000) (pressed = true, await api("effects/press", { key: 7 }));
      const f = await api("deck/keys");
      if (f?.jpeg && f.jpeg !== last) {
        last = f.jpeg;
        const n = times.length;
        times.push((Date.now() - t0) / 1000);
        fs.writeFileSync(path.join(to, `${String(n).padStart(3, "0")}.jpg`), Buffer.from(last, "base64"));
      }
      await sleep(12);
    }
    fs.writeFileSync(path.join(to, "times.json"), JSON.stringify({ name, times }));
    const rate = times.length / seconds;
    console.log(`${name}: ${times.length} pictures, ${rate.toFixed(1)} a second`);
    if (rate < 15) ok = false;
  }
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1500);
  const [clean, seen] = screenClean(dir);
  console.log(`${clean ? "PASS" : "FAIL"}  nothing came up on the owner's screen: ${seen}`);
  ok &&= clean;
  fs.rmSync(dir, { recursive: true, force: true });
}
process.exit(ok ? 0 : 1);
