// A second keys module and a second dial with controls of their own (owner, 2026-11-02: "dodać te
// obsługę wielu modułów tego samego typu dla każdego z modułów a po prostu screensaver i preset
// będą jedne"). On a test copy with his profiles and virtual modules: the second keys module is
// keys.15 .. keys.29 and the second dial dial.1 in the API, the window and the engine; a press of the
// second module's third key runs keys.17's action, a turn of the second dial runs dial.1's; then the
// copy starts without the first keys module and the second keeps keys.15 .. keys.29 (remembered in
// module-numbers.json). Only actions set by this script are used (his own keys would run for real).
// Screenshots in logs/two-keys/.
//   node scripts/ui/two-keys.mjs [path to hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-two-keys");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "two-keys");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 500 });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "nudges.json"]) if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
// the serial is the place in this list: keys 1 = VIRTUAL1, keys 2 = VIRTUAL3, dial 1 = VIRTUAL2, dial 2 = VIRTUAL4
const BOTH = "base@6.1.4.4,keys@6.2.3,dial@6.1.3.4,keys@6.3.3,dial@6.1.1.3";
// keys 1 taken off: a rollers module holds its place in the list, so keys 2 is VIRTUAL3 again
const SECOND_ONLY = "base@6.1.4.4,rollers@6.2.3,dial@6.1.3.4,keys@6.3.3,dial@6.1.1.3";

async function run(spec, step) {
  for (const f of ["api-http-port", "api-port"]) fs.rmSync(path.join(dir, f), { force: true });
  const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9494", HUBDECK_VIRTUAL_MODULES: spec }, stdio: "ignore" });
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
      browser = await chromium.connectOverCDP("http://127.0.0.1:9494");
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
    await sleep(2500);
  }
}

const bright = () => JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8")).screenBrightness ?? 100;

await run(BOTH, async (api, window) => {
  const m = await api("GET", "modules");
  check("two keys modules: keys.0 and keys.15", JSON.stringify([...(m.numbering?.keys ?? [])].sort((a, b) => a - b)) === "[0,15]", JSON.stringify(m.numbering));
  check("two dials: dial and dial.1", JSON.stringify([...(m.numbering?.dial ?? [])].sort((a, b) => a - b)) === "[0,1]", JSON.stringify(m.numbering));
  const keysSecond = Object.entries(JSON.parse(fs.readFileSync(path.join(dir, "module-numbers.json"), "utf8"))).find(([id, g]) => id.startsWith("keys:") && g === 1)?.[0];
  const dialSecond = Object.entries(JSON.parse(fs.readFileSync(path.join(dir, "module-numbers.json"), "utf8"))).find(([id, g]) => id.startsWith("dial:") && g === 1)?.[0];
  check("module-numbers.json names the second keys module and dial", !!keysSecond && !!dialSecond, `${keysSecond} ${dialSecond}`);
  const set = await api("POST", `profiles/${settings.active}/controls`, { control: "keys.17", binding: { action: "core.deckBrightness", params: { press: "darker", step: 10 }, look: { label: "K18", bg: "#1d6b3a" } } });
  check("a binding on keys.17 is accepted", set.ok === true, JSON.stringify(set));
  const setDial = await api("POST", `profiles/${settings.active}/controls`, { control: "dial.1", binding: { action: "core.deckBrightness", params: { step: 10 }, look: { label: "Dial 2" } } });
  check("a binding on dial.1 is accepted", setDial.ok === true, JSON.stringify(setDial));
  for (const [control, why] of [["keys.60", "a fifth keys module"], ["dial.0", "the first dial is dial"], ["dial.4", "a fifth dial"]]) {
    const bad = await api("POST", `profiles/${settings.active}/controls`, { control, binding: { action: "core.deckBrightness", params: {}, look: {} } });
    check(`${control} (${why}) is refused`, !!bad.error, bad.error);
  }
  await sleep(800);
  const win = await window();
  const keyIdx = await win.locator(`[data-kind="keys"][data-module="${keysSecond}"]`).evaluateAll((els) => els.map((e) => Number(e.getAttribute("data-index"))).sort((a, b) => a - b));
  check("the window draws the second keys module as Key 16-30", keyIdx.length === 15 && keyIdx[0] === 15 && keyIdx[14] === 29, JSON.stringify(keyIdx));
  const label = await win.locator(`[data-kind="keys"][data-index="17"]`).getAttribute("aria-label");
  check("the window names keys.17 by its binding", /K18/.test(label ?? ""), label);
  const dialIdx = await win.locator(`[data-kind="dial"][data-module="${dialSecond}"]`).getAttribute("data-index");
  check("the second dial is index 1 in the window", dialIdx === "1", dialIdx);
  const dialLabel = await win.locator(`[data-kind="dial"][data-module="${dialSecond}"]`).getAttribute("aria-label");
  check("the window names the second dial Dial 2", /Dial 2/.test(dialLabel ?? ""), dialLabel);
  await win.screenshot({ path: path.join(shots, "1-two-keys-two-dials.png") });
  await win.locator(`[data-kind="keys"][data-index="17"]`).click();
  await sleep(800);
  await win.screenshot({ path: path.join(shots, "2-key-18-selected.png") });
  const inspector = await win.locator(".inspector").innerText().catch(() => "");
  check("the inspector says Key 18", /Key 18/.test(inspector), inspector.split("\n").slice(0, 3).join(" / "));

  const before = bright();
  const pressed = await api("POST", "test/input", { kind: "keys", index: 17, gesture: "press" });
  await sleep(1200);
  check("a press of the second module's third key runs keys.17's action", pressed.ok === true && bright() === before - 10, `${before} -> ${bright()} ${JSON.stringify(pressed)}`);
  const b2 = bright();
  const turned = await api("POST", "test/input", { kind: "dial", index: 1, gesture: "turn", value: -1 });
  await sleep(1200);
  check("a turn of the second dial runs dial.1's action", turned.ok === true && bright() === b2 - 10, `${b2} -> ${bright()} ${JSON.stringify(turned)}`);
  const log = fs.readdirSync(path.join(dir, "logs")).map((f) => fs.readFileSync(path.join(dir, "logs", f), "utf8")).join("\n");
  const line = (log.match(/more than one module of a kind: .*/g) ?? []).pop() ?? "";
  check("the log names the second modules' numbers", line.includes(`${keysSecond} -> 15`) && line.includes(`${dialSecond} -> 1`), line);
  check("no panic or error in the log", !/panicked| ERROR /.test(log), (log.match(/.*(panicked| ERROR ).*/) ?? [""])[0]);
});

await run(SECOND_ONLY, async (api, window) => {
  const m = await api("GET", "modules");
  check("keys 1 taken off: keys 2 keeps keys.15 .. keys.29", JSON.stringify(m.numbering?.keys) === "[15]", JSON.stringify(m.numbering));
  const win = await window();
  const idx = await win.locator(`[data-kind="keys"]`).evaluateAll((els) => els.map((e) => Number(e.getAttribute("data-index"))).sort((a, b) => a - b));
  check("the window draws it as Key 16-30", idx[0] === 15 && idx.length === 15, JSON.stringify(idx));
  const label = await win.locator(`[data-kind="keys"][data-index="17"]`).getAttribute("aria-label");
  check("and its keys.17 keeps its binding", /K18/.test(label ?? ""), label);
  await win.screenshot({ path: path.join(shots, "3-second-keys-only.png") });
});

check("nothing came up on the owner's screen", ...screenClean(dir));
fs.rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 500 });
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
