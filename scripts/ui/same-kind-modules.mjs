// Two modules of the same kind on one base (owner, 2026-11-01: "dwa takie same moduły", and "co jak
// ktoś wepnie modul nr 1, a potem taki sam modul nr 2, a potem ten nr 1 odepnie?"). On a test copy
// with his profiles and two virtual knobs modules: the second one's controls are knobs.3 .. knobs.5
// in the API, the engine and the window; a colour set on knobs.4 lights only the second module; its
// turn reaches knobs.4. Then the copy starts again without the first knobs module: the second keeps
// knobs.3 .. knobs.5 (remembered in module-numbers.json) and its colour. Screenshots in
// logs/same-kind-modules/.
//   node scripts/ui/same-kind-modules.mjs [path to hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-same-kind");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "same-kind-modules");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "nudges.json"]) if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
// his case step by step: knobs 1 alone (VIRTUAL3: the serial is the place in this list), then knobs 2
// added in the rollers' slot (his layout has the faders' cell under the dial: a module there hides
// it), a lower port path than knobs 1's (so only the remembered numbers keep knobs 1 first), then
// knobs 1 taken off
const FIRST_ONLY = "base@6.1.4.4,keys@6.2.3,dial@6.1.3.4,knobs@6.1.4.3";
const BOTH = "base@6.1.4.4,keys@6.2.3,dial@6.1.3.4,knobs@6.1.4.3,knobs@6.1.3.3";
// knobs 1 taken off: a rollers module holds its place in the list, so knobs 2 is VIRTUAL4 again
const SECOND_ONLY = "base@6.1.4.4,keys@6.2.3,dial@6.1.3.4,rollers@6.3.3,knobs@6.1.3.3";
const GREEN = "#00ff66";

async function run(spec, step) {
  for (const f of ["api-http-port", "api-port"]) fs.rmSync(path.join(dir, f), { force: true });
  const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9491", HUBDECK_VIRTUAL_MODULES: spec }, stdio: "ignore" });
  for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
  await sleep(1500);
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const api = async (method, p, body) => {
    const r = await fetch(`http://127.0.0.1:${port}/api/${p}`, { method, headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined });
    return r.json();
  };
  let browser;
  try {
    await step(api, async () => {
      browser = await chromium.connectOverCDP("http://127.0.0.1:9491");
      let win;
      for (let i = 0; i < 40 && !win; i++) {
        win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
        if (!win) await sleep(500);
      }
      await win.setViewportSize({ width: 1606, height: 1000 });
      await sleep(2500);
      for (const name of [/^Not now$/, /^Got it$/, /^Skip/, /^Close$/]) {
        const b = win.getByRole("button", { name }).first();
        if (await b.isVisible().catch(() => false)) (await b.click(), await sleep(600));
      }
      return win;
    });
  } finally {
    await browser?.close().catch(() => {});
    app.kill();
    spawn("taskkill", ["/F", "/T", "/PID", String(app.pid)]);
    await sleep(2000);
  }
}

const knobLeds = async (api) => {
  const l = await api("GET", "leds");
  return Object.fromEntries(Object.entries(l.shown ?? {}).filter(([id]) => id.startsWith("knobs:")).map(([id, v]) => [id, (v ?? []).map((x) => x.color)]));
};

await run(FIRST_ONLY, async (api) => {
  const m = await api("GET", "modules");
  check("knobs 1 alone has knobs.0 .. knobs.2", JSON.stringify(m.numbering?.knobs) === "[0]", JSON.stringify(m.numbering));
});

await run(BOTH, async (api, window) => {
  const m = await api("GET", "modules");
  // the API lists modules by id: VIRTUAL3 (knobs 1) first
  check("knobs 2 added in a lower slot: knobs 1 keeps 0, knobs 2 gets 3", JSON.stringify(m.numbering?.knobs) === "[0,3]", JSON.stringify(m.numbering));
  const set = await api("POST", `profiles/${settings.active}/controls`, { control: "knobs.4", binding: { action: "core.deckBrightness", params: { step: 10 }, look: { label: "Second", led: GREEN } } });
  check("a binding on knobs.4 is accepted", set.ok === true, JSON.stringify(set));
  const bad = await api("POST", `profiles/${settings.active}/controls`, { control: "knobs.12", binding: { action: "core.deckBrightness", params: {}, look: {} } });
  check("knobs.12 (a fifth module) is refused", !!bad.error, bad.error);
  await sleep(1200);
  const l = await knobLeds(api);
  console.log("knob LEDs:", JSON.stringify(l));
  check("knobs.4's colour lights the second module's middle LED", l["knobs:VIRTUAL4"]?.[1] === GREEN, JSON.stringify(l["knobs:VIRTUAL4"]));
  check("the first module does not take it", !(l["knobs:VIRTUAL3"] ?? []).includes(GREEN), JSON.stringify(l["knobs:VIRTUAL3"]));
  const win = await window();
  const second = win.locator(`[data-kind="knobs"][data-module="knobs:VIRTUAL4"]`);
  const idx = await second.evaluateAll((els) => els.map((e) => e.getAttribute("data-index")).sort());
  check("the window draws the second module's knobs as 3, 4, 5", JSON.stringify(idx) === '["3","4","5"]', JSON.stringify(idx));
  const label = await win.locator(`[data-kind="knobs"][data-index="4"]`).getAttribute("aria-label");
  check("the window names knobs.4 by its binding", /Second|Knob 5/.test(label ?? ""), label);
  check("the dial is still drawn", (await win.locator(`[data-kind="dial"]`).count()) === 1);
  await win.screenshot({ path: path.join(shots, "1-two-knobs.png") });
  await win.locator(`[data-kind="knobs"][data-index="4"]`).click();
  await sleep(800);
  await win.screenshot({ path: path.join(shots, "2-knob-5-selected.png") });
  // its action runs (Deck brightness: a test copy changes only its own settings)
  const bright = () => JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8")).screenBrightness ?? 100;
  const before = bright();
  const turned = await api("POST", "test/input", { kind: "knobs", index: 4, gesture: "turn", value: -1 });
  await sleep(1200);
  check("a turn of the second module's middle knob runs knobs.4's action", turned.ok === true && bright() === before - 10, `${before} -> ${bright()} ${JSON.stringify(turned)}`);
  // and a turn of the first module's middle knob does not
  const b2 = bright();
  await api("POST", "test/input", { kind: "knobs", index: 1, gesture: "turn", value: -1 });
  await sleep(800);
  check("the first module's middle knob (knobs.1) does not run it", bright() === b2, `${b2} -> ${bright()}`);
  const log = fs.readdirSync(path.join(dir, "logs")).map((f) => fs.readFileSync(path.join(dir, "logs", f), "utf8")).join("\n");
  check("the log names the second module's numbers", /knobs:VIRTUAL4 -> 3/.test(log), (log.match(/more than one module of a kind: .*/) ?? [""])[0]);
});
const numbers = JSON.parse(fs.readFileSync(path.join(dir, "module-numbers.json"), "utf8"));
check("module-numbers.json remembers both", numbers["knobs:VIRTUAL3"] === 0 && numbers["knobs:VIRTUAL4"] === 1, JSON.stringify(numbers));

await run(SECOND_ONLY, async (api, window) => {
  const m = await api("GET", "modules");
  check("knobs 1 taken off: knobs 2 keeps 3 .. 5", JSON.stringify(m.numbering?.knobs) === "[3]", JSON.stringify(m.numbering));
  await sleep(1200);
  const l = await knobLeds(api);
  check("and keeps its colour on knobs.4", l["knobs:VIRTUAL4"]?.[1] === GREEN, JSON.stringify(l));
  const win = await window();
  const idx = await win.locator(`[data-kind="knobs"]`).evaluateAll((els) => els.map((e) => e.getAttribute("data-index")).sort());
  check("the window draws it as 3, 4, 5", JSON.stringify(idx) === '["3","4","5"]', JSON.stringify(idx));
  await win.screenshot({ path: path.join(shots, "3-second-only.png") });
});

check("nothing came up on the owner's screen", ...screenClean(dir));
fs.rmSync(dir, { recursive: true, force: true });
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
