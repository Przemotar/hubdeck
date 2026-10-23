// The setup line under the HWiNFO plugins in the Plugins tab (owner, 2026-10-20: "Dodaj tę
// linijkę"): shown for both, nothing cut, in the dark and the light look at 960 and 1280 wide.
// A test copy of its own (virtual modules, never the focus). Screenshots in logs/plugin-setup-*.png.
//   node scripts/ui/plugin-setup-line.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-plugin-setup");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9478;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const tag = process.argv[3] ?? "after";

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
let browser;
try {
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(1500);
  await win.getByRole("button", { name: "Plugins" }).first().click();
  await win.getByRole("searchbox", { name: "Search plugins" }).fill("HWiNFO");
  await sleep(1200);
  const lines = await win.locator(".plugin-card-setup").allTextContents();
  console.log(`setup lines: ${lines.length}`);
  if (tag === "after") check("both HWiNFO plugins show their setup line", lines.length === 2, lines.map((l) => l.slice(0, 50)).join(" | "));
  for (const [w, h] of [[960, 640], [1280, 800]]) {
    await win.setViewportSize({ width: w, height: h });
    await sleep(400);
    const cut = (await clipped(win)).filter((c) => /HWiNFO|sensors|Shared Memory|Gadget/.test(c.text));
    check(`nothing about HWiNFO cut at ${w} wide`, cut.length === 0, cut.map((c) => c.text.slice(0, 60)).join(" | "));
    for (const look of ["dark", "light"]) {
      await win.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
      await sleep(200);
      await win.locator(".plugin-card").first().scrollIntoViewIfNeeded();
      await win.screenshot({ path: path.join(root, "logs", `plugin-setup-${tag}-${w}-${look}.png`) });
    }
  }
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1000);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
