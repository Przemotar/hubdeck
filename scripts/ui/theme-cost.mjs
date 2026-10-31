// What each theme costs (the performance audit before 0.2.0; owner, 2026-10-27: "dokładniejszy
// audyt wydajności i optymalizacji zarówno samego decku jak i apki"): a test copy with his profiles
// (virtual modules, its window off screen, never the focus) puts each theme from `engine.ts` on the
// active profile, lets it settle, then reads the frame cost debugger (`frame_cost`: each layer's
// ms, the keys, the JPEG) and HubDeck's CPU (`deck_rate`) once a second for `seconds`, and keeps a
// frame at rest and one after a press. USB is not in it (virtual modules): the gate measures that.
//   node scripts/ui/theme-cost.mjs [seconds] [hubdeck.exe]     # writes logs/theme-cost/cost.json
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const seconds = Number(process.argv[2] || 10);
const exe = process.argv[3] || path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-theme-cost");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const out = path.join(root, "logs", "theme-cost");
const PORT = 9496;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "nudges.json", "pet.json", "taste.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const profile = settings.active;

// the themes as the window offers them: read from engine.ts (ids and layers)
const src = fs.readFileSync(path.join(root, "app", "src", "engine.ts"), "utf8");
const block = src.slice(src.indexOf("export const THEMES"), src.indexOf("/** The kinds named by"));
const themes = [...block.matchAll(/id: "([^"]+)",\s*layers: (\[[\s\S]*?\]),\n  \}/g)].map((m) => ({
  id: m[1],
  // the layers' literal is JS: quote its keys to read it as JSON
  layers: JSON.parse(m[2].replace(/(\w+):/g, '"$1":').replace(/,(\s*[\]}])/g, "$1")),
}));
// the upright copies the window adds (`engine.ts` UPRIGHT_THEMES): the scenes swapped for `<kind>-up`,
// the layer whose falling the scene draws itself (OWN_FALL) left out
const list = (name) => JSON.parse(src.match(new RegExp("const " + name + "[^=]*= (\\[[^\\]]*\\])"))[1]);
const uprightKinds = list("UPRIGHT");
const ownFall = JSON.parse(src.match(/const OWN_FALL[^=]*= (\{[^}]*\})/)[1]);
for (const id of list("UPRIGHT_THEMES")) {
  const at = themes.findIndex((t) => t.id === id);
  if (at < 0) continue;
  const layers = themes[at].layers.filter((l) => l.kind !== ownFall[id]).map((l) => (uprightKinds.includes(l.kind) ? { ...l, kind: `${l.kind}-up` } : l));
  themes.splice(at + 1, 0, { id: `${id}-up`, layers });
}
// THEME_IDS=a,b: only those (in that order)
const only = process.env.THEME_IDS?.split(",").filter(Boolean);
if (only) themes.splice(0, themes.length, ...only.map((id) => themes.find((t) => t.id === id)).filter(Boolean));
console.log(`${themes.length} themes: ${themes.map((t) => t.id).join(", ")}`);

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
const results = [];
let browser;
try {
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1280, height: 800 });
  // the window's start (pages, icons, the first pictures) settles before anything is measured: the
  // first theme measured right after it read 50-90 % CPU (session 35)
  await sleep(15000);
  const invoke = (cmd, args) => win.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args]);
  const shot = async (name) => {
    const f = await api("deck/keys");
    if (f?.jpeg) fs.writeFileSync(path.join(out, `${name}.jpg`), Buffer.from(f.jpeg, "base64"));
  };
  for (const th of themes) {
    const p = await api(`profiles/${profile}`);
    await invoke("profile_save", { profile: { ...p, effects: { layers: th.layers, theme: th.id } } });
    await sleep(5000);
    await invoke("frame_cost");
    await invoke("deck_rate");
    const samples = [];
    const cpus = [];
    for (let s = 0; s < seconds; s++) {
      await sleep(1000);
      const c = await invoke("frame_cost");
      const r = await invoke("deck_rate");
      if (c.frames > 0) samples.push(c);
      if (r.cpu != null) cpus.push(r.cpu);
    }
    await shot(`${th.id}-rest`);
    await api("effects/press", { key: 7 });
    await sleep(350);
    await shot(`${th.id}-press`);
    await sleep(900);
    await shot(`${th.id}-press2`);
    const avg = (f) => (samples.length ? samples.reduce((a, s) => a + f(s), 0) / samples.length : 0);
    const kinds = [...new Set(samples.flatMap((s) => s.layers.map((l) => l.kind)))];
    const layers = kinds.map((k) => ({ kind: k, ms: avg((s) => s.layers.find((l) => l.kind === k)?.ms ?? 0) }));
    const frames = samples.reduce((a, s) => a + s.frames, 0) / Math.max(1, samples.length);
    const row = { id: th.id, framesPerSec: frames, layers, layersMs: avg((s) => s.layersMs), keysMs: avg((s) => s.keysMs), encodeMs: avg((s) => s.encodeMs), cpu: cpus.length ? cpus.reduce((a, b) => a + b, 0) / cpus.length : null };
    results.push(row);
    console.log(`${th.id.padEnd(22)} ${frames.toFixed(1).padStart(5)}/s  layers ${row.layersMs.toFixed(2)} ms (${layers.map((l) => `${l.kind} ${l.ms.toFixed(2)}`).join(", ")})  keys ${row.keysMs.toFixed(2)}  jpeg ${row.encodeMs.toFixed(2)}  CPU ${row.cpu?.toFixed(1) ?? "?"} %`);
  }
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1500);
}
fs.writeFileSync(path.join(out, "cost.json"), JSON.stringify(results, null, 2));
const log = fs.readFileSync(path.join(dir, "logs", "hubdeck.log"), "utf8").split("\n").filter((l) => /does not compile|panicked|ERROR \[hubdeck_lib::engine::canvas/.test(l));
if (log.length) console.log(log.slice(-8).join("\n"));
console.log(`screen clean: ${JSON.stringify(screenClean(dir))}`);
fs.rmSync(dir, { recursive: true, force: true });
