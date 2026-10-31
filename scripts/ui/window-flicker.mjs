// Do the window's keys flicker while the deck moves (owner, 2026-10-27: "migoczą przyciski na apce
// … spada czytelnosc")? A test copy with his profiles picks a theme by a click in the Living deck
// panel, as he would, then records the window's own frames (CDP screencast, every frame the page
// paints) for `seconds` and measures each key's brightness in each frame: a key that goes much
// darker for a frame or two and comes back is a flash (a picture swapped in before it was decoded).
// Its window off screen, never the focus.
//   node scripts/ui/window-flicker.mjs [theme name] [seconds] [path to hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const themeName = process.argv[2] || "Poland: golden autumn";
const seconds = Number(process.argv[3] || 12);
const exe = process.argv[4] || path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-window-flicker");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "window-flicker");
const PORT = 9495;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "nudges.json", "pet.json", "taste.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const profile = settings.active;

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
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
  // the theme by a click, as he picks it
  await win.getByRole("button", { name: /^Living deck/ }).first().click();
  await sleep(800);
  const side = win.locator("aside.living-panel").first();
  await side.getByRole("radiogroup", { name: "Theme", exact: true }).getByRole("radio", { name: themeName, exact: true }).click();
  await sleep(1500);
  const fx = (await api(`profiles/${profile}`)).effects;
  check(`${themeName} goes on by a click`, !!fx?.theme && (fx.layers?.length ?? 0) > 0, JSON.stringify(fx?.theme));
  const chip = await win.getByRole("button", { name: /^Living deck/ }).first().innerText();
  check("the Living deck chip names the theme", chip.includes(themeName), chip.replace(/\s+/g, " "));
  await sleep(4000);
  // the keys' boxes in the page, the mirror of the deck's picture under them
  const keys = await win.evaluate(() => {
    const g = document.querySelector('g.dk-ctl[data-kind="keys"]');
    const caps = [...document.querySelectorAll('g.dk-ctl[data-kind="keys"] [data-key], g.dk-ctl[data-kind="keys"] rect')];
    const boxes = caps.map((e) => e.getBoundingClientRect()).filter((r) => r.width > 30 && r.width < 200 && Math.abs(r.width - r.height) < 6);
    const seen = new Set();
    const out = [];
    for (const r of boxes) {
      const k = `${Math.round(r.x / 4)}:${Math.round(r.y / 4)}`;
      if (seen.has(k)) continue;
      seen.add(k);
      out.push({ x: r.x + r.width * 0.2, y: r.y + r.height * 0.2, w: r.width * 0.6, h: r.height * 0.6 });
    }
    return { all: g?.getBoundingClientRect().toJSON(), keys: out.slice(0, 15) };
  });
  check("15 keys found in the window", keys.keys.length === 15, `${keys.keys.length}`);
  // every painted frame of the page for `seconds`
  const cdp = await win.context().newCDPSession(win);
  const frames = [];
  cdp.on("Page.screencastFrame", async (f) => {
    frames.push({ t: f.metadata.timestamp, data: f.data });
    await cdp.send("Page.screencastFrameAck", { sessionId: f.sessionId }).catch(() => {});
  });
  await cdp.send("Page.startScreencast", { format: "png", everyNthFrame: 1 });
  await sleep(seconds * 1000);
  await cdp.send("Page.stopScreencast");
  await sleep(300);
  check("the page painted frames while the deck moved", frames.length > seconds * 5, `${frames.length} frames in ${seconds} s`);
  // each key's mean brightness in each frame, measured in the page itself
  const series = [];
  for (const f of frames) {
    series.push(
      await win.evaluate(
        async ([b64, keys]) => {
          const img = new Image();
          img.src = `data:image/png;base64,${b64}`;
          await img.decode();
          const c = document.createElement("canvas");
          c.width = img.naturalWidth;
          c.height = img.naturalHeight;
          const x = c.getContext("2d");
          x.drawImage(img, 0, 0);
          const sx = img.naturalWidth / window.innerWidth;
          // each key: its mean lightness (for flashes) and a 4x4 grid of cell means (for motion: a
          // scrolling grid or a slow paint keeps the mean, not the cells)
          return keys.map((k) => {
            const w = Math.max(4, Math.round(k.w * sx));
            const h = Math.max(4, Math.round(k.h * sx));
            const d = x.getImageData(Math.round(k.x * sx), Math.round(k.y * sx), w, h).data;
            const cells = new Array(16).fill(0);
            const counts = new Array(16).fill(0);
            let s = 0;
            for (let yy = 0; yy < h; yy++) {
              for (let xx = 0; xx < w; xx++) {
                const i = (yy * w + xx) * 4;
                const l = 0.2126 * d[i] + 0.7152 * d[i + 1] + 0.0722 * d[i + 2];
                s += l;
                const c = Math.min(3, Math.floor((yy / h) * 4)) * 4 + Math.min(3, Math.floor((xx / w) * 4));
                cells[c] += l;
                counts[c]++;
              }
            }
            return { mean: s / (w * h), cells: cells.map((v, i) => v / Math.max(1, counts[i])) };
          });
        },
        [f.data, keys.keys],
      ),
    );
  }
  // a flash: a key's brightness falls by more than half against both frames around it, or to near black
  let flashes = 0;
  const where = [];
  for (let i = 1; i < series.length - 1; i++) {
    for (let k = 0; k < series[i].length; k++) {
      const a = series[i - 1][k].mean, b = series[i][k].mean, c = series[i + 1][k].mean;
      const around = Math.min(a, c);
      if (around > 12 && b < around * 0.5) {
        flashes++;
        if (where.length < 8) where.push(`frame ${i} key ${k + 1}: ${a.toFixed(0)} -> ${b.toFixed(0)} -> ${c.toFixed(0)}`);
      }
    }
  }
  // against the first frame: a slow scene (the spectrum breathing with no music) changes less than
  // half a level from one frame to the next, but plenty over the run
  const changed = series.slice(1).filter((s) => s.some((v, k) => v.cells.some((c, j) => Math.abs(c - series[0][k].cells[j]) > 2.0))).length;
  check("the keys in the window change (the deck's motion reaches it)", changed > seconds * 3, `${changed} frames differ from the first of ${series.length}`);
  check("no key flashes dark for a frame", flashes === 0, flashes ? where.join("; ") : "0 flashes");
  const mean = series.length ? series.map((s) => s.reduce((x, y) => x + y.mean, 0) / s.length) : [];
  console.log(`keys' mean brightness: min ${Math.min(...mean).toFixed(1)}, max ${Math.max(...mean).toFixed(1)} of 255`);
  fs.writeFileSync(path.join(shots, "first.png"), Buffer.from(frames[0]?.data ?? "", "base64"));
  fs.writeFileSync(path.join(shots, "last.png"), Buffer.from(frames.at(-1)?.data ?? "", "base64"));
} catch (e) {
  check("the walk ran", false, String(e?.stack ?? e));
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1500);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
fs.rmSync(dir, { recursive: true, force: true });
console.log(`${results.filter(Boolean).length}/${results.length}  (shots: ${shots})`);
process.exit(results.every(Boolean) ? 0 : 1);
