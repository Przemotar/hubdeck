// Does Mochi keep its name and body when a key takes it off the dial and puts it back? (Discussions
// #3, 8bitcanvascode, 2026-10-18: "the pet's name and custom body image seem to reset".) A test copy
// of its own (virtual modules, no deck, never the focus), driven through the local API: a key that
// puts Mochi on the dial, the dial's Mochi given a name and a body, two presses of the key.
//   node scripts/ui/mochi-toggle.mjs [path to hubdeck.exe]    # default: the test build
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-mochi-toggle");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "pet.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const profile = settings.active;

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9472" }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
const dial = async () => (await api(`profiles/${profile}`)).controls?.dial;
const press = async () => {
  await api("test/input", { kind: "keys", index: 14, gesture: "press" });
  await sleep(1500);
};

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
try {
  console.log(`exe: ${exe}`);
  // the last key brings Mochi to the dial, as the Fun page's Mochi key does
  await api(`profiles/${profile}/controls`, { control: "keys.14", binding: { action: "core.pet", params: { press: "dial" }, look: {} } });
  await sleep(500);
  if ((await dial())?.action === "core.pet") await press();
  await press();
  let d = await dial();
  check("the key puts Mochi on the dial", d?.action === "core.pet", d?.action);
  // the user names it and gives it a body on the dial
  await api(`profiles/${profile}/controls`, { control: "dial", binding: { ...d, params: { ...d.params, name: "Tama", body: "cat.png", bodyFit: { size: 80, x: 4, y: -6 } } } });
  await sleep(500);
  await press();
  d = await dial();
  check("a second press takes it off", d?.action !== "core.pet", d?.action ?? "nothing");
  await press();
  d = await dial();
  const p = d?.params ?? {};
  check("back on the dial with its name", p.name === "Tama", JSON.stringify(p.name));
  check("and its body and fit", p.body === "cat.png" && p.bodyFit?.size === 80 && p.bodyFit?.y === -6, JSON.stringify({ body: p.body, bodyFit: p.bodyFit }));
  check("brought by the same key", p.from === "keys.14", p.from);
} finally {
  app.kill();
  await sleep(1000);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
