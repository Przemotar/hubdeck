// A Stream Deck action with three states (public repo's PR #11 from sotomskir, 2026-10-26: Google
// Meet's 0 disconnected, 1 muted, 2 unmuted; unmuted was drawn as muted). On a test copy with his
// profiles (virtual modules, the window off screen) a made-up plugin, harmless, sets state 2 when its
// key appears and moves on 0 -> 1 -> 2 at each press; the key in the window shows each state's title.
// The owner has no three-state plugin, and his two-state ones switch real audio devices.
//   node scripts/ui/plugin-states.mjs [path to hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-plugin-states");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "plugin-states");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "nudges.json"]) if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

// the plugin: three states with titles, and a program that sets them
const UUID = "dev.hubdeck.test.states";
const plugin = path.join(dir, "plugins", `${UUID}.sdPlugin`);
fs.mkdirSync(plugin, { recursive: true });
fs.writeFileSync(
  path.join(plugin, "manifest.json"),
  JSON.stringify({
    Name: "States test",
    UUID,
    Version: "1.0.0",
    SDKVersion: 2,
    Author: "HubDeck test",
    Description: "Three states, as Google Meet's mic",
    Icon: "icon",
    CodePath: "plugin.js",
    Nodejs: { Version: "20" },
    OS: [{ Platform: "windows", MinimumVersion: "10" }, { Platform: "mac", MinimumVersion: "10.15" }],
    Software: { MinimumVersion: "6.0" },
    Actions: [{ UUID: `${UUID}.mic`, Name: "Mic", Icon: "icon", States: [{ Title: "Disconnected" }, { Title: "Muted" }, { Title: "Live" }] }],
  }),
);
fs.writeFileSync(
  path.join(plugin, "plugin.js"),
  `const a = process.argv, get = (k) => a[a.indexOf(k) + 1];
const ws = new WebSocket("ws://127.0.0.1:" + get("-port"));
let state = 2;
ws.onopen = () => ws.send(JSON.stringify({ event: get("-registerEvent"), uuid: get("-pluginUUID") }));
ws.onmessage = (m) => {
  const e = JSON.parse(m.data);
  if (e.event === "willAppear") ws.send(JSON.stringify({ event: "setState", context: e.context, payload: { state } }));
  if (e.event === "keyUp") { state = (state + 1) % 3; ws.send(JSON.stringify({ event: "setState", context: e.context, payload: { state } })); }
};
`,
);
// on the first key of his home profile's main page
const profileFile = path.join(dir, "profiles", `${settings.active}.json`);
const profile = JSON.parse(fs.readFileSync(profileFile, "utf8"));
profile.controls["keys.0"] = { action: `${UUID}/${UUID}.mic`, params: {}, look: {} };
fs.writeFileSync(profileFile, JSON.stringify(profile, null, 2));

const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9492" }, stdio: "ignore" });
let browser;
try {
  for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
  await sleep(1500);
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const api = (method, p, body) => fetch(`http://127.0.0.1:${port}/api/${p}`, { method, headers: { Authorization: `Bearer ${token}` }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
  browser = await chromium.connectOverCDP("http://127.0.0.1:9492");
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
  const key = win.locator(`[data-kind="keys"][data-index="0"]`);
  const text = async () => (await key.textContent())?.replace(/\s+/g, " ").trim() ?? "";
  // the plugin starts with Node and sets its state when its key appears
  let t = "";
  for (let i = 0; i < 30 && !/Live/.test(t); i++) (await sleep(500), (t = await text()));
  check("state 2 (Live) is drawn as Live, not as Muted", /Live/.test(t) && !/Muted/.test(t), t);
  await win.screenshot({ path: path.join(shots, "1-live.png") });
  for (const [want, n] of [["Disconnected", 2], ["Muted", 3], ["Live", 4]]) {
    await api("POST", "test/input", { kind: "keys", index: 0, gesture: "press" });
    let now = "";
    for (let i = 0; i < 10 && !now.includes(want); i++) (await sleep(300), (now = await text()));
    check(`a press moves it on to ${want}`, now.includes(want), now);
    await win.screenshot({ path: path.join(shots, `${n}-${want.toLowerCase()}.png`) });
  }
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  spawn("taskkill", ["/F", "/T", "/PID", String(app.pid)]);
  await sleep(2000);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
fs.rmSync(dir, { recursive: true, force: true });
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
