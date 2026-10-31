// Where the window's time goes while the deck moves (owner, 2026-10-26: "wolno renderuje to co się
// dzieje na urządzeniu"): a test copy (his profiles, the living deck on, virtual modules, the window
// off screen, never the focus) measures the page's frames as it is, without the keys' blur
// filters, and with the deck's drawing hidden.
//   node scripts/ui/window-diag.mjs [path to hubdeck.exe]   # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] || path.join(root, "target", "test-build", "release", "hubdeck.exe");
const shots = process.argv[3];
const dir = path.join(os.tmpdir(), "hubdeck-window-diag");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9492;
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

// the living deck on the active profile, as the owner's deck runs it
{
  const file = path.join(dir, "profiles", `${profile}.json`);
  const p = JSON.parse(fs.readFileSync(file, "utf8"));
  p.effects = { layers: [{ kind: "snow" }, { kind: "fireflies" }] };
  fs.writeFileSync(file, JSON.stringify(p, null, 2));
}
const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
let browser;
try {
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(5000);
  // frames of the page (requestAnimationFrame) and deck pictures that came, for `seconds`
  const measure = (label, seconds) =>
    win.evaluate(
      ([label, seconds]) =>
        new Promise((done) => {
          let pictures = 0;
          const was = window.__mirrorFrame;
          window.__mirrorFrame = () => (pictures++, was?.());
          const gaps = [];
          let last = performance.now();
          const t0 = last;
          const tick = (now) => {
            gaps.push(now - last);
            last = now;
            if (now - t0 < seconds * 1000) requestAnimationFrame(tick);
            else {
              window.__mirrorFrame = was;
              const over = gaps.filter((g) => g > 50).length;
              const long = gaps.filter((g) => g > 100).length;
              const sorted = [...gaps].sort((a, b) => a - b);
              done(`${label}: pictures ${(pictures / seconds).toFixed(1)}/s, frames ${(gaps.length / seconds).toFixed(0)}/s, over 50 ms ${over}, over 100 ms ${long}, p95 ${sorted[Math.floor(sorted.length * 0.95)].toFixed(0)} ms, slowest ${sorted[sorted.length - 1].toFixed(0)} ms`);
            }
          };
          requestAnimationFrame(tick);
        }),
      [label, seconds],
    );
  console.log(await measure("A as it is", 10));
  // B: the keys' soft shadows (an SVG blur filter on each of the 15 caps) taken off
  await win.evaluate(() => document.querySelectorAll("[filter]").forEach((e) => (e.dataset.f = e.getAttribute("filter"), e.removeAttribute("filter"))));
  console.log(await measure("B no blur filters", 10));
  await win.evaluate(() => document.querySelectorAll("[data-f]").forEach((e) => e.setAttribute("filter", e.dataset.f)));
  // C: the deck's drawing hidden: what is left is the JS (base64, React) and not the painting
  const deck = await win.evaluate(() => {
    const svg = [...document.querySelectorAll("svg")].sort((a, b) => b.getBoundingClientRect().width * b.getBoundingClientRect().height - a.getBoundingClientRect().width * a.getBoundingClientRect().height)[0];
    svg.dataset.hid = "1";
    svg.style.visibility = "hidden";
    return `${Math.round(svg.getBoundingClientRect().width)}x${Math.round(svg.getBoundingClientRect().height)}, ${svg.querySelectorAll("*").length} elements`;
  });
  console.log(`(the deck's svg: ${deck})`);
  console.log(await measure("C deck hidden", 10));
  await win.evaluate(() => document.querySelectorAll("[data-hid]").forEach((e) => (e.style.visibility = "")));
  console.log(await measure("D as it is again", 10));
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1500);
}
console.log(`screen clean: ${JSON.stringify(screenClean(dir))}`);
fs.rmSync(dir, { recursive: true, force: true });
