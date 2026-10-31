// The deck as the window draws it with a living-deck preset on, for the site's link preview
// (site/images/og.png, `scripts/og.py`): a test copy with an empty data folder (the profile a first
// start makes, nothing of the owner's), virtual modules, its window off screen, never the focus. Each
// preset is applied in Deck look as a user would; the deck in the main window is shot at 2x after the
// scene settles, several times, so the liveliest picture can be picked (logs/og-deck/<preset>-N.png).
//   node scripts/ui/og-deck.mjs [hubdeck.exe] ["Preset", ...]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] && process.argv[2].endsWith(".exe") ? process.argv[2] : path.join(root, "target", "test-build", "release", "hubdeck.exe");
const presets = process.argv.slice(process.argv[2]?.endsWith(".exe") ? 3 : 2);
const PRESETS = presets.length ? presets : ["Hollow Hill", "Fuji and sakura", "Synthwave", "Tokyo Rain"];
const dir = path.join(os.tmpdir(), "hubdeck-og-deck");
const out = path.join(root, "logs", "og-deck");
const PORT = 9489;
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
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  const cdp = await win.context().newCDPSession(win);
  await cdp.send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 900, deviceScaleFactor: 2, mobile: false });
  await sleep(2500);
  for (const name of [/^Not now$/, /^Got it$/, /^Skip/, /^Close$/]) {
    const b = win.getByRole("button", { name }).first();
    if (await b.isVisible().catch(() => false)) (await b.click(), await sleep(600));
  }
  for (const preset of PRESETS) {
    await win.getByRole("button", { name: /^Living deck/ }).first().click();
    await sleep(1500);
    await win.locator(".dl-tile", { hasText: preset }).first().click();
    await sleep(1200);
    await win.getByRole("button", { name: "Apply", exact: true }).click();
    await sleep(800);
    await win.getByRole("button", { name: /Back to the deck/ }).first().click();
    await sleep(3000);
    await win.mouse.move(5, 5);
    const slug = preset.toLowerCase().replace(/[^a-z]+/g, "-");
    for (let n = 0; n < 6; n++) {
      await win.locator(".dk-stage").first().screenshot({ path: path.join(out, `${slug}-${n}.png`) });
      await sleep(700);
    }
    console.log(`${preset}: 6 pictures`);
  }
} catch (e) {
  ok = false;
  console.log(e.message);
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
