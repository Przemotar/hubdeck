// "Show" on the banner for a profile that made itself (owner, 2026-10-22: "jak klikam show to nic
// się nie dzieje"): the profile list was already open, so the click changed nothing. Now Show
// brings the new profile's row into view, lights it once and the banner goes. On the owner's own
// data (his Counter-Strike 2 profile made itself on 2026-10-21). A test copy of its own (virtual
// modules, never the focus). Screenshots in logs/auto-show-*.png.
//   node scripts/ui/auto-show.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-auto-show");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9482;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const shot = (name) => path.join(root, "logs", `auto-show-${name}.png`);

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
// once the owner has clicked Show or OK nothing is new: mark his Counter-Strike 2 profile new again
if (!settings.autoMade?.length) {
  const profiles = fs.readdirSync(path.join(dir, "profiles")).filter((f) => f.endsWith(".json")).map((f) => JSON.parse(fs.readFileSync(path.join(dir, "profiles", f), "utf8")));
  const game = profiles.find((p) => (p.apps ?? []).some((a) => String(a).toLowerCase() === "cs2.exe")) ?? profiles.find((p) => (p.apps ?? []).length);
  settings.autoMade = game ? [game.id] : [];
}
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const madeIds = settings.autoMade ?? [];

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  console.log(`exe: ${exe}`);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  await win.setViewportSize({ width: 1600, height: 900 });
  await sleep(2500);

  const s = await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("engine_state"));
  const made = madeIds.map((id) => s.profiles.find((p) => p.id === id)?.name).filter(Boolean);
  console.log(`made by itself, not yet seen: ${made.join(", ") || "none"}`);
  const banner = win.locator(".banner", { hasText: /ready-made profile/i });
  check("the banner is there", (await banner.count()) === 1 && made.length > 0, made.join(", "));

  // the profile list open, as in the owner's window, and scrolled away from the new row
  await win.getByRole("tab", { name: "Profiles" }).or(win.getByRole("button", { name: "Profiles" })).first().click();
  await sleep(600);
  const id = madeIds.find((x) => s.profiles.some((p) => p.id === x));
  const inView = () =>
    win.evaluate((id) => {
      const row = document.querySelector(`.prow[data-profile="${id}"]`);
      if (!row) return null;
      let box = row.parentElement;
      while (box && !(box.scrollHeight > box.clientHeight && /auto|scroll/.test(getComputedStyle(box).overflowY))) box = box.parentElement;
      const r = row.getBoundingClientRect();
      const v = box ? box.getBoundingClientRect() : { top: 0, bottom: innerHeight };
      return r.top >= v.top - 1 && r.bottom <= Math.min(v.bottom, innerHeight) + 1;
    }, id);
  await win.evaluate((id) => {
    const row = document.querySelector(`.prow[data-profile="${id}"]`);
    let box = row?.parentElement;
    while (box && !(box.scrollHeight > box.clientHeight && /auto|scroll/.test(getComputedStyle(box).overflowY))) box = box.parentElement;
    if (box) box.scrollTop = box.scrollHeight;
  }, id);
  await sleep(400);
  if (await inView()) {
    await win.evaluate((id) => {
      let box = document.querySelector(`.prow[data-profile="${id}"]`)?.parentElement;
      while (box && !(box.scrollHeight > box.clientHeight && /auto|scroll/.test(getComputedStyle(box).overflowY))) box = box.parentElement;
      if (box) box.scrollTop = 0;
    }, id);
    await sleep(400);
  }
  const before = await inView();
  console.log(`the new row in view before Show: ${before}`);
  await win.screenshot({ path: shot("1-before") });

  await banner.getByRole("button", { name: "Show" }).click();
  await sleep(500);
  await win.screenshot({ path: shot("2-lit") });
  const lit = await win.evaluate((id) => document.querySelector(`.prow[data-profile="${id}"]`)?.classList.contains("is-shown") ?? false, id);
  check("Show lights the new profile's row", lit);
  check("and brings it into view", (await inView()) === true);
  await sleep(1500);
  check("the banner goes after Show", (await banner.count()) === 0);
  const after = await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("engine_state"));
  check("the profile stays (Show changes nothing in it)", after.profiles.some((p) => p.id === id) && after.active === s.active);
  await win.screenshot({ path: shot("3-after") });
  check("no page errors", errors === 0, `${errors}`);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1000);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
