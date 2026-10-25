// Does an "Audio device" key that keeps the device's system id work? (owner's deck, 2026-10-23:
// Key 15 of his home profile said "no output device named {0.0.0.00000000}.{9c79...}" after his
// Audio Switcher key became a built-in one.) A test copy of its own (virtual modules, never the
// focus) with the owner's data: the profile check, the Device field, a picture of the key, and a
// press only when it would not change his default speakers.
//   node scripts/ui/audio-device-key.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-audio-device-key");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9478;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
// every key of his that keeps a device id (a main-page key, so the deck shows it)
const keys = [];
for (const f of fs.readdirSync(path.join(dir, "profiles"))) {
  const p = JSON.parse(fs.readFileSync(path.join(dir, "profiles", f), "utf8"));
  for (const [control, b] of Object.entries(p.controls ?? {})) {
    if (b.action === "core.audioDevice" && /^\{/.test(b.params?.device ?? "") && /^keys\.\d+$/.test(control)) keys.push({ profile: p.id, name: p.name, control, binding: b });
  }
}
check("a key with a device id is in his profiles", keys.length > 0, keys.map((k) => `${k.name} ${k.control}`).join(", "));
if (!keys.length) process.exit(1);

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
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
  await win.setViewportSize({ width: 1450, height: 780 });
  const invoke = (cmd, args) => win.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args]);
  const [outputs, inputs] = await invoke("audio_devices");
  for (const k of keys) {
    const id = k.binding.params.device;
    const index = Number(k.control.split(".")[1]);
    const device = [...outputs, ...inputs].find((d) => d.id.toLowerCase() === id.toLowerCase());
    check(`${k.control}: the device is on this PC`, !!device, device ? `${device.name}, default: ${device.default}` : id);
    if (!device) continue;
    await api(`profiles/${k.profile}/activate`, {});
    await sleep(2500);
    const issues = await invoke("profile_check", { id: k.profile });
    const mine = issues.filter((i) => i.control === k.control);
    check(`${k.control}: the profile check finds nothing wrong`, mine.length === 0, JSON.stringify(mine));
    // the window: the key's Device field names the device
    const key = win.locator(`g.dk-ctl[data-kind="keys"][data-index="${index}"]`).first();
    const b = await key.locator(".dk-hov").first().boundingBox();
    await win.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
    await sleep(1200);
    // the key as the window draws it (lit = its "on" look) is looked at, not read: live_looks holds plugin looks only
    await key.screenshot({ path: path.join(root, "logs", "audio-device-key-key.png") });
    console.log(`look: logs/audio-device-key-key.png should show ${JSON.stringify(device.default ? (k.binding.lookOn?.label ?? k.binding.look?.label) : k.binding.look?.label)}`);
    const field = win.locator("select.input").filter({ has: win.locator("optgroup") }).first();
    const shown = await field.evaluate((s) => ({ text: s.options[s.selectedIndex]?.text, value: s.value, count: s.options.length }));
    check(`${k.control}: the Device field shows the device's name`, shown.text === device.name, JSON.stringify(shown));
    const listed = await field.evaluate((s, name) => [...s.options].filter((o) => o.text === name).length, device.name);
    check(`${k.control}: the device is listed once`, listed === 1, `${listed}`);
    // a press sets the device; only when it is the default already, so his speakers stay
    if (device.default) {
      await api("test/input", { kind: "keys", index, gesture: "press", value: 1 });
      await sleep(1800);
      const banner = await win.locator("body").innerText();
      check(`${k.control}: a press shows no device error`, !/no (output|input) device named/.test(banner));
    } else {
      console.log(`nie sprawdzone: a press of ${k.control} would make ${device.name} his default speakers`);
    }
    for (const look of ["dark", "light"]) {
      await win.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
      await sleep(300);
      await win.screenshot({ path: path.join(root, "logs", `audio-device-key-${look}.png`) });
    }
    const text = await win.locator("body").innerText();
    check(`${k.control}: no device error on the window`, !/no (output|input) device named/.test(text));
  }
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
