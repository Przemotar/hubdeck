// The README's moving picture of the Plugins tab (owner, 2026-10-18: "Jak chcesz zrobić gif do
// readme to śmiało"): the browser demo with the real catalog, a search typed, "Tried in HubDeck"
// turned on. Frames from headless Chromium, the GIF from ffmpeg (two passes, one palette).
//   node scripts/ui/readme-gif.mjs     # needs the Vite server and logs/plugins-catalog.json
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const base = process.env.HUBDECK_URL || "http://localhost:1460/";
const frames = fs.mkdtempSync(path.join(os.tmpdir(), "hubdeck-gif-"));
const out = path.join(root, "docs", "images", "hubdeck-plugins.gif");
function findChromium() {
  const cache = path.join(os.homedir(), process.platform === "win32" ? "AppData/Local/ms-playwright" : process.platform === "darwin" ? "Library/Caches/ms-playwright" : ".cache/ms-playwright");
  for (const dir of fs.existsSync(cache) ? fs.readdirSync(cache).filter((d) => d.startsWith("chromium")).sort().reverse() : []) {
    const stack = [path.join(cache, dir)];
    while (stack.length) {
      const d = stack.pop();
      for (const e of fs.readdirSync(d, { withFileTypes: true })) {
        const p = path.join(d, e.name);
        if (e.isDirectory() && !p.includes(".app/Contents/Frameworks")) stack.push(p);
        else if (/^(chrome-headless-shell|chrome|Chromium|headless_shell)(\.exe)?$/.test(e.name)) return p;
      }
    }
  }
}
const shared = JSON.parse(fs.readFileSync(path.join(root, "app/src-tauri/src/engine/sdcatalog.json"), "utf8"));
const plugins = JSON.parse(fs.readFileSync(path.join(root, "logs/plugins-catalog.json"), "utf8"))
  .plugins.filter((p) => !shared.wontRun?.[p.id])
  .map((p) => ({ author: "", description: "", icon: "", aiAssisted: false, elgato: null, popular: null, stars: null, updated: null, category: null, dial: false, ...p, systems: p.systems ?? [], tried: shared.tried[p.id] ?? null }));
const browser = await chromium.launch({ executablePath: findChromium() });
const page = await browser.newPage({ viewport: { width: 1280, height: 760 }, colorScheme: "dark" });
await page.addInitScript((list) => (window.__DEMO_CATALOG__ = list), plugins);
await page.goto(`${base}?demo`);
await page.locator(".dk-ctl").first().waitFor();
await page.getByRole("button", { name: "Plugins" }).first().click();
await page.waitForTimeout(3000);
let n = 0;
const grab = async (count = 1) => {
  for (let i = 0; i < count; i++) await page.screenshot({ path: path.join(frames, `f${String(n++).padStart(4, "0")}.png`) });
};
await grab(10);
const search = page.getByPlaceholder("Search plugins");
for (const ch of "spotify") {
  await search.type(ch);
  await page.waitForTimeout(60);
  await grab(2);
}
await page.waitForTimeout(500);
await grab(12);
await search.fill("");
await page.waitForTimeout(500);
await grab(4);
await page.getByText("Tried in HubDeck", { exact: true }).first().click();
await page.waitForTimeout(700);
await grab(14);
await browser.close();
const pattern = path.join(frames, "f%04d.png");
const palette = path.join(frames, "palette.png");
execFileSync("ffmpeg", ["-y", "-v", "error", "-framerate", "6", "-i", pattern, "-vf", "scale=960:-1:flags=lanczos,palettegen=max_colors=128", palette]);
execFileSync("ffmpeg", ["-y", "-v", "error", "-framerate", "6", "-i", pattern, "-i", palette, "-lavfi", "scale=960:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=4", "-loop", "0", out]);
fs.rmSync(frames, { recursive: true, force: true });
console.log(`${out}: ${n} frames, ${(fs.statSync(out).size / 1e6).toFixed(1)} MB`);
