// Moving thumbnails for the Deck look view (owner, 2026-10-28, "2 a": the scene alone, without his
// icons; the big preview shows his deck): a test copy (his profiles, the active one's keys emptied,
// virtual modules, the window off screen) puts each preset and each element on the deck and keeps
// 30 pictures, 15 a second, with a press of key 8 halfway (logs/look-loops/<id>/NNN.jpg);
// `scripts/ui/look-loops.py` packs them into app/public/looks/<id>.webp, as the deck shows them.
// An item with `icons` is drawn on neutral sample icons (the light, the presses and the visitors
// act on what a key shows: on empty keys they showed nothing); one with `long` (a ghost crossing
// every 24 s) keeps 26 s, of which the packer takes the liveliest 2 s.
//   ITEMS_JSON='[{"id":"x","layers":[...],"icons":true}]' node scripts/ui/look-loops.mjs [hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] || path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-look-loops");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9495;
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

const ITEMS = JSON.parse(process.env.ITEMS_JSON);
const out = path.join(root, "logs", "look-loops");
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
  const base = await api(`profiles/${profile}`);
  // the scene alone: no key of the shown page keeps an icon, a label or a picture
  const controls = Object.fromEntries(Object.entries(base.controls).filter(([id]) => !/^(keys\.|[^/]+\/keys\.)/.test(id)));
  const SAMPLE = ["play", "music", "star", "heart", "mic", "camera", "folder", "globe", "mail", "moon", "sun", "zap", "rocket", "gamepad-2", "headphones"];
  const sample = { ...controls };
  SAMPLE.forEach((icon, i) => (sample[`keys.${i}`] = { action: "core.media", params: {}, look: { icon, bg: "#1b1a20", fg: "#e6e4ee", iconColor: "#d9d6ea" } }));
  for (const it of ITEMS) {
    // a key style (film, glitch, neon, glass, pumpkin) is worn by every sample key, with its motion
    const keys = it.skin ? Object.fromEntries(Object.entries(sample).map(([id, b]) => [id, id.startsWith("keys.") ? { ...b, look: { ...b.look, skin: it.skin, motion: it.motion } } : b])) : it.icons ? sample : controls;
    await win.evaluate((profile) => window.__TAURI_INTERNALS__.invoke("profile_save", { profile }), { ...base, controls: keys, effects: it.layers.length ? { layers: it.layers, theme: it.theme } : undefined });
    await sleep(3000);
    const to = path.join(out, it.id);
    fs.rmSync(to, { recursive: true, force: true });
    fs.mkdirSync(to, { recursive: true });
    let last = "";
    const count = it.long ? 390 : 30;
    for (let n = 0; n < count; n++) {
      if (n === 15 || (it.long && n % 60 === 15)) await api("effects/press", { key: 7 });
      const f = await api("deck/keys");
      if (f?.jpeg && f.jpeg !== last) last = f.jpeg;
      if (last) fs.writeFileSync(path.join(to, `${String(n).padStart(3, "0")}.jpg`), Buffer.from(last, "base64"));
      await sleep(66);
    }
    console.log(`${it.id}: ${fs.readdirSync(to).length} pictures`);
  }
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1500);
}
const log = fs.readFileSync(path.join(dir, "logs", "hubdeck.log"), "utf8").split("\n").filter((l) => /does not compile|panicked|ERROR/.test(l));
console.log(log.slice(-8).join("\n"));
console.log(`screen clean: ${JSON.stringify(screenClean(dir))}`);
fs.rmSync(dir, { recursive: true, force: true });
