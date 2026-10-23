// Do Stream Deck plugins learn their keys after HubDeck restarts? (#5, Zendeman, 2026-10-19:
// Discord keys did nothing after a restart until any key was reassigned; he has Auto-switch off.)
// A test copy of its own (no deck, never the focus) with a small plugin that writes down every
// event it gets: put its action on a key, restart HubDeck, and check the plugin heard willAppear
// for that key without anything else changing.
//   node scripts/ui/plugin-restart.mjs [path to hubdeck.exe]    # default: the test build
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-plugin-restart");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PLUGIN = "dev.hubdeck.test-appear";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// a copy of the real profiles and settings, Auto-switch off as in the report
fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

// the plugin: Stream Deck's register handshake, then every event into events.log
const pdir = path.join(dir, "plugins", `${PLUGIN}.sdPlugin`);
fs.mkdirSync(pdir, { recursive: true });
fs.writeFileSync(
  path.join(pdir, "manifest.json"),
  JSON.stringify({
    Name: "Test appear", Version: "1.0.0", Author: "HubDeck", Description: "Writes down what it hears.", Icon: "icon", CodePath: "plugin.js",
    Nodejs: { Version: "20" }, SDKVersion: 2, OS: [{ Platform: "windows", MinimumVersion: "10" }], Software: { MinimumVersion: "6.4" },
    Actions: [{ Name: "Ping", UUID: `${PLUGIN}.ping`, Icon: "icon", States: [{ Image: "icon" }], Controllers: ["Keypad"] }],
  }),
);
fs.writeFileSync(
  path.join(pdir, "plugin.js"),
  `const a = process.argv; const arg = (k) => a[a.indexOf(k) + 1];
const fs = require("node:fs"); const log = require("node:path").join(__dirname, "events.log");
const ws = new WebSocket("ws://127.0.0.1:" + arg("-port"));
ws.onopen = () => ws.send(JSON.stringify({ event: arg("-registerEvent"), uuid: arg("-pluginUUID") }));
ws.onmessage = (m) => { const e = JSON.parse(m.data); fs.appendFileSync(log, e.event + " " + (e.context ?? "") + "\\n"); };
`,
);
const events = () => (fs.existsSync(path.join(pdir, "events.log")) ? fs.readFileSync(path.join(pdir, "events.log"), "utf8") : "");

let app;
const start = async () => {
  for (const f of ["api-http-port", "api-token"]) fs.rmSync(path.join(dir, f), { force: true });
  app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9471" }, stdio: "ignore" });
  for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
  await sleep(500);
};
const stop = async () => {
  app.kill();
  await sleep(1500);
};
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
try {
  console.log(`exe: ${exe}`);
  await start();
  const profile = settings.active;
  // the first key of the start page
  await api(`profiles/${profile}/controls`, { control: "keys.0", binding: { action: `${PLUGIN}/${PLUGIN}.ping`, params: {}, look: {} } });
  await sleep(4000);
  check("put on a key, the plugin hears it", /willAppear keys\.0/.test(events()), events().trim().split("\n").join(" | "));

  await stop();
  fs.rmSync(path.join(pdir, "events.log"), { force: true });
  await start();
  await sleep(8000);
  check("after a restart, nothing else changed, the plugin hears its key", /willAppear keys\.0/.test(events()), events().trim().split("\n").join(" | ") || "nothing");
  // a press reaches it as Stream Deck's keyDown
  await api("test/input", { kind: "keys", index: 0, gesture: "press" }).catch(() => null);
  await sleep(1000);
  check("a press after the restart reaches it", /keyDown keys\.0/.test(events()), events().trim().split("\n").slice(-2).join(" | "));
} finally {
  if (app) await stop();
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
