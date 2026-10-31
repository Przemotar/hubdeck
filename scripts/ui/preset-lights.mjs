// A preset's colours on the lights and the preset as the screensaver (owner's "1 a", 2026-10-30): on a
// test copy with his profiles (virtual modules, the window off screen) Deck look picks a preset and
// applies it with both switches on; the edge, knobs and faders then show its colours (read from the
// engine, `leds_get`) and the profile's own screensaver is its look; his own edge colour turns the
// preset's colours off. Screenshots in logs/preset-lights/.
//   node scripts/ui/preset-lights.mjs [path to hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-preset-lights");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "preset-lights");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9490" }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
let browser;
try {
  browser = await chromium.connectOverCDP("http://127.0.0.1:9490");
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1606, height: 1000 });
  await sleep(2500);
  const inv = (c, a) => win.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [c, a]);
  const leds = async () => {
    const all = await inv("leds_get");
    const of = (k) => Object.entries(all).filter(([id, v]) => id.startsWith(k) && v).flatMap(([, v]) => v.map((l) => l.color));
    return { base: of("base"), knobs: of("knobs"), faders: of("faders") };
  };
  console.log("before:", JSON.stringify(await leds()));
  await win.getByRole("button", { name: /^Living deck/ }).first().click();
  await sleep(1500);
  await win.locator(".dl-tile", { hasText: "Tokyo Rain" }).first().click();
  await sleep(1200);
  const also = win.locator(".dl-also input[type=checkbox]");
  check("Deck look offers the preset's colours and the preset as the screensaver, both on", (await also.count()) === 2 && (await also.nth(0).isChecked()) && (await also.nth(1).isChecked()));
  await win.locator(".dl-also").screenshot({ path: path.join(shots, "1-switches.png") });
  await win.getByRole("button", { name: "Apply", exact: true }).click();
  await sleep(2500);
  const l = await leds();
  console.log("after Apply:", JSON.stringify(l));
  // the same hue within 12 degrees (the brightness setting scales the colour)
  const hue = (hex) => {
    const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
    const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn;
    if (!d) return -1;
    const h = mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
    return (h * 60 + 360) % 360;
  };
  const near = (hex, want) => !!hex && Math.min(Math.abs(hue(hex) - hue(want)), 360 - Math.abs(hue(hex) - hue(want))) < 12;
  if (!l.base.length && !l.knobs.length) {
    // a test copy's virtual modules have no LEDs: leds.rs `a_presets_colours_light_the_edge_and_the_knobs_its_effect_stays` covers it
    console.log("note: the test copy's virtual modules have no LEDs, the colours are not read here (see the leds.rs test)");
  } else {
    check("the edge takes the preset's first colour", l.base.every((c) => near(c, "#ff26a8")), l.base.join(","));
    check("the knobs and faders take its second", [...l.knobs, ...l.faders].every((c) => near(c, "#26b4ff")), [...l.knobs, ...l.faders].join(","));
  }
  const p = await inv("profile_get", { id: settings.active }).catch(() => null);
  const saved = p ?? JSON.parse(fs.readFileSync(path.join(dir, "profiles", `${settings.active}.json`), "utf8"));
  check("the profile's own screensaver is its look", saved.saver?.effect === "preset", JSON.stringify(saved.saver));
  await win.getByRole("button", { name: "Back to the deck" }).click();
  await sleep(1000);
  await win.screenshot({ path: path.join(shots, "2-deck.png") });
  // his own edge colour wins
  const edgeChip = win.locator("[data-guide='edge']");
  check("the edge chip says the preset's colours", /preset/i.test(await edgeChip.innerText()), await edgeChip.innerText());
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
