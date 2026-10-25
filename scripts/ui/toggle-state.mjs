// Does a "Multi action switch" show the right side after a restart? (issue #10, killa8boll,
// 2026-10-25: Windows kept his headphones after a reboot, the switch still showed speakers.)
// A test copy of its own (virtual modules, never the focus) with the owner's data and a profile
// of two switches:
//   keys.0  first press: the PC's default speakers now  -> must start on its second side (read)
//   keys.1  first press: an LED effect (no state to read) -> pressed once, then the copy restarts:
//           must still be on its second side (remembered, toggles.json)
// The key caps are read from the window (it mirrors the deck): red = first side next, green = second.
// No press changes his speakers (keys.0 is never pressed).
//   node scripts/ui/toggle-state.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-toggle-state");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9479;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const RED = "#d02020";
const GREEN = "#20b040";

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

let app;
let browser;
let win;
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
async function start() {
  fs.rmSync(path.join(dir, "api-http-port"), { force: true });
  app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
  for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
  await sleep(1500);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  win = undefined;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1450, height: 780 });
}
async function stop() {
  await browser?.close().catch(() => {});
  app.kill();
  spawn("taskkill", ["/F", "/T", "/PID", String(app.pid)]);
  await sleep(2500);
}
// the cap's mean color as the window mirrors it: "red", "green" or what it is
async function side(index, shot) {
  const key = win.locator(`g.dk-ctl[data-kind="keys"][data-index="${index}"]`).first();
  const box = await key.locator(".dk-hov").first().boundingBox();
  const png = await win.screenshot({ path: path.join(root, "logs", `toggle-state-${shot}.png`), clip: { x: box.x + 6, y: box.y + 6, width: box.width - 12, height: box.height - 12 } });
  const [r, g, b] = await win.evaluate(async (uri) => {
    const img = new Image();
    img.src = uri;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = img.width;
    c.height = img.height;
    const x = c.getContext("2d");
    x.drawImage(img, 0, 0);
    const d = x.getImageData(Math.floor(img.width * 0.3), Math.floor(img.height * 0.3), Math.ceil(img.width * 0.4), Math.ceil(img.height * 0.4)).data;
    const sum = [0, 0, 0];
    for (let i = 0; i < d.length; i += 4) for (let k = 0; k < 3; k++) sum[k] += d[i + k];
    return sum.map((s) => Math.round(s / (d.length / 4)));
  }, `data:image/png;base64,${png.toString("base64")}`);
  const seen = r > g + 40 ? "red" : g > r + 40 ? "green" : `rgb(${r},${g},${b})`;
  console.log(`      keys.${index} (${shot}): ${seen}`);
  return seen;
}

try {
  console.log(`exe: ${exe}`);
  await start();
  const invoke = (cmd, args) => win.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args]);
  const [outputs] = await invoke("audio_devices");
  const now = outputs.find((d) => d.default);
  check("the PC has default speakers to read", !!now, now?.name);
  const other = outputs.find((d) => !d.default);
  const look = (bg, label) => ({ label, bg, fg: "#ffffff" });
  const sw = (first, second) => ({ action: "core.toggle", params: { first, second }, look: look(RED, "first"), lookOn: look(GREEN, "second") });
  const effect = (e) => [{ action: "core.lighting", params: { target: "base", effect: e } }];
  const made = await api("profiles", {
    name: "Toggle test",
    controls: {
      "keys.0": sw([{ action: "core.audioDevice", params: { kind: "output", device: now.id } }], [{ action: "core.audioDevice", params: { kind: "output", device: other?.id ?? now.id } }]),
      "keys.1": sw(effect("breathing"), effect("")),
    },
  });
  const pid = made.profile?.id ?? made.id;
  check("the test profile is saved", !!pid, JSON.stringify(made).slice(0, 160));
  await api(`profiles/${pid}/activate`, {});
  await sleep(3500);
  check("keys.0 reads the system: its first device is the default, so the second side is next", (await side(0, "read")) === "green");
  check("keys.1 starts on its first side (nothing pressed, nothing to read)", (await side(1, "fresh")) === "red");
  await api("test/input", { kind: "keys", index: 1, gesture: "press", value: 1 });
  await sleep(1500);
  check("keys.1 after a press: second side", (await side(1, "pressed")) === "green");
  const saved = JSON.parse(fs.readFileSync(path.join(dir, "toggles.json"), "utf8"));
  check("toggles.json keeps it by profile and key", saved[`${pid}/keys.1`] === true, JSON.stringify(saved));
  await stop();

  await start();
  await api(`profiles/${pid}/activate`, {});
  await sleep(3500);
  check("after a restart keys.1 still shows its second side", (await side(1, "restarted")) === "green");
  check("after a restart keys.0 still reads the system", (await side(0, "restarted-read")) === "green");
  const text = await win.locator("body").innerText();
  check("no device error on the window", !/no (output|input) device named/.test(text));
  for (const l of ["dark", "light"]) {
    await win.evaluate((t) => document.documentElement.setAttribute("data-theme", t), l);
    await sleep(300);
    await win.screenshot({ path: path.join(root, "logs", `toggle-state-${l}.png`) });
  }
} finally {
  if (app) await stop();
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
