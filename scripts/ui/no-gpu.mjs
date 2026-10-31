// A PC without a graphics card HubDeck can use (before 0.2.0: "Layers without a graphics card say so
// in the panel; today they vanish silently"): a test copy with his profiles started as such a PC
// (`HUBDECK_NO_GPU`), the Living deck panel opened as a user does, its line saying the GPU effects
// are left out, their GPU marks struck through, the CPU effects still marked plainly. Its window off
// screen, never the focus.
//   node scripts/ui/no-gpu.mjs [hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] || path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-no-gpu");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "no-gpu");
const PORT = 9497;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT), HUBDECK_NO_GPU: "1" }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
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
  await win.getByRole("button", { name: /^Living deck/ }).first().click();
  await sleep(1500);
  const side = win.locator("aside.living-panel").first();
  const line = side.getByText(/no graphics card HubDeck can use/);
  check("the panel says the GPU effects are left out", await line.isVisible().catch(() => false));
  const off = await side.locator(".fx-gpu.is-off").count();
  const marks = await side.locator(".fx-gpu").count();
  check("every GPU mark is struck through", off > 0 && off === marks, `${off} of ${marks}`);
  const snow = side.getByRole("switch", { name: "Snow", exact: true });
  check("a CPU effect (Snow) carries no GPU mark", (await snow.locator(".fx-gpu").count()) === 0);
  await side.screenshot({ path: path.join(shots, "panel.png") });
  const cut = (await clipped(win)).filter((x) => /graphics card/.test(x.text));
  check("the line is not cut", cut.length === 0, JSON.stringify(cut));
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
