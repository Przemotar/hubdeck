// The Music chip under the deck (owner's "3 a", 2026-10-30) and the first switch to a big scene after
// start (his "5", the programs compiled ahead): on a test copy with his profiles (virtual modules, the
// window off screen). Screenshots in logs/music-chip/.
//   node scripts/ui/music-chip.mjs [path to hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-music-chip");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "music-chip");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9491" }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
const api = (route) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { headers: { Authorization: `Bearer ${token}` } }).then((r) => r.json());
};
let browser;
try {
  browser = await chromium.connectOverCDP("http://127.0.0.1:9491");
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  await win.setViewportSize({ width: 1606, height: 1000 });
  await sleep(2500);
  const inv = (c, a) => win.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [c, a]);
  const chip = win.locator("[data-guide='music']");
  check("the Music chip stands under the deck", await chip.isVisible(), await chip.innerText().catch(() => ""));
  const before = await inv("lights_get");
  await chip.click();
  await sleep(1000);
  const panel = win.locator("aside.inspector.living-panel");
  check("it opens the Lights to the music panel", (await panel.innerText()).includes("Lights to the music"));
  const on = panel.locator("input[type=checkbox]").first();
  const wasOn = await on.isChecked();
  await on.click();
  await sleep(800);
  const after = await inv("lights_get");
  check("the switch turns the lights' music on or off", (after.style === "music") === !wasOn, `${before.style} -> ${after.style}`);
  await panel.screenshot({ path: path.join(shots, "1-panel.png") });
  await win.locator("[data-guide='music']").screenshot({ path: path.join(shots, "2-chip.png") });
  await on.click();
  await sleep(800);
  const back = await inv("lights_get");
  check("switched off again it goes back to the style it had", back.style === before.style, `${after.style} -> ${back.style}`);
  await win.locator("aside.inspector.living-panel button[aria-label]").first().click().catch(() => {});
  await sleep(800);
  await win.screenshot({ path: path.join(shots, "3-deck.png") });

  // the first switch to Lantern Hollow, 25 s after start (the warm-up runs at 10 s)
  await sleep(20000);
  await win.getByRole("button", { name: /^Living deck/ }).first().click();
  await sleep(1500);
  const t0 = Date.now();
  await win.locator(".dl-tile", { hasText: "Lantern Hollow" }).first().click();
  const seen = [];
  let last = "";
  while (Date.now() - t0 < 2500) {
    const j = ((await api("deck/keys"))?.jpeg ?? "").slice(-300);
    if (j !== last) (seen.push(Date.now() - t0), (last = j));
    await sleep(60);
  }
  const gaps = seen.slice(1).map((t, i) => t - seen[i]);
  check("the first switch to Lantern Hollow does not hold the deck", Math.max(0, ...gaps) < 400, `pictures at ${seen.join(", ")} ms; longest gap ${Math.max(0, ...gaps)} ms`);
  check("no page errors", errors === 0, `${errors}`);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  spawn("taskkill", ["/F", "/T", "/PID", String(app.pid)]);
  await sleep(1500);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
