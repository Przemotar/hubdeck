// The README's Deck look screenshot (docs/images/hubdeck-living-deck.png): a test copy with an empty
// data folder (nothing of the owner's: the profile a first start makes), virtual modules, its window
// off screen, never the focus. Deck look opens with a preset tried on the deck; 1440x900 at the
// screen's scale (1.25 on his PC: 1800x1125). Needs the GPU: the preview is the engine's picture.
//   node scripts/ui/readme-deck-look.mjs [path to hubdeck.exe] [preset name]   # default: the test build, Hollow Hill
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const preset = process.argv[3] ?? "Hollow Hill";
const dir = path.join(os.tmpdir(), "hubdeck-readme-look");
const out = path.join(root, "docs", "images", "hubdeck-living-deck.png");
const PORT = 9487;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify({ autoSwitch: false }));

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
let browser;
let ok = false;
try {
  for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
  await sleep(1500);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  const cdp = await win.context().newCDPSession(win);
  await cdp.send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 900, deviceScaleFactor: 1.5, mobile: false });
  await sleep(2500);
  // a first start may greet with a tour or What's new: closed as a user would
  for (const name of [/^Not now$/, /^Got it$/, /^Skip/, /^Close$/]) {
    const b = win.getByRole("button", { name }).first();
    if (await b.isVisible().catch(() => false)) (await b.click(), await sleep(600));
  }
  await win.getByRole("button", { name: /^Living deck/ }).first().click();
  await sleep(1500);
  await win.locator(".dl-tile", { hasText: preset }).first().click();
  // the preview's picture comes from the engine: a few seconds for the scene to settle
  await sleep(5000);
  await win.mouse.move(5, 5);
  await win.screenshot({ path: out });
  const cut = (await clipped(win)).filter((x) => x.text);
  console.log(`${out}: ${preset} tried; cut text: ${cut.length ? cut.map((x) => x.text).join(" | ") : "none"}; page errors ${errors}`);
  ok = errors === 0;
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
