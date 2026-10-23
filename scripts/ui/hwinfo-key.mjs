// Does a HWiNFO plugin key show its reading, not the plugin's logo? (owner's deck, 2026-10-20: the
// key kept the HWiNFO logo with a sensor picked; a user with an AMD card saw the same.) Needs
// HWiNFO running with Shared Memory Support on, and com.moeilijk.hwinfo installed in HubDeck.
// A test copy of its own (virtual modules, never the focus) with the owner's data and plugins:
// a key with the plugin's reading action, then what the plugin put on it, sampled for 8 s.
//   node scripts/ui/hwinfo-key.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-hwinfo-key");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9477;
const ACTION = "com.moeilijk.hwinfo/com.moeilijk.hwinfo.reading";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "plugins"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const profile = settings.active;

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
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
let browser;
try {
  console.log(`exe: ${exe}`);
  // the owner's own HWiNFO key if he has one (its sensor picked), else total CPU usage of sensor 0
  let params = null;
  for (const f of fs.readdirSync(path.join(dir, "profiles"))) {
    const p = JSON.parse(fs.readFileSync(path.join(dir, "profiles", f), "utf8"));
    const b = Object.values(p.controls ?? {}).find((x) => x.action === ACTION && x.params?.isValid);
    if (b) params = b.params;
  }
  check("a HWiNFO key with a sensor picked is in his profiles", !!params, params ? `${params.readingLabel}` : "none");
  await api(`profiles/${profile}/controls`, { control: "keys.0", binding: { action: ACTION, params, look: {} } });
  await api(`profiles/${profile}/activate`, {});

  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  // the plugin's own still picture (its logo over a made-up graph) is what the key showed instead
  const pdir = path.join(dir, "plugins", "com.moeilijk.hwinfo.sdPlugin");
  const logos = new Set(["defaultImage.png", "defaultImage@2x.png"].filter((f) => fs.existsSync(path.join(pdir, f))).map((f) => fs.readFileSync(path.join(pdir, f)).toString("base64")));
  const images = new Set();
  let drawable = 0;
  // what HubDeck's drawing accepts (images.rs safe_data_uri): a base64 image with nothing else in it
  const safe = (s) => /^data:image\/(png|jpeg|gif|webp|svg\+xml);base64,[A-Za-z0-9+/=]+$/.test(s);
  for (let i = 0; i < 16; i++) {
    await sleep(500);
    const looks = await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("live_looks"));
    const img = looks?.looks?.["keys.0"]?.image;
    if (img) {
      images.add(crypto.createHash("sha1").update(img).digest("hex"));
      if (safe(img) && !logos.has(img.split(",")[1])) drawable++;
    }
  }
  check("the plugin's picture reaches the key", images.size > 0, `${images.size} different pictures in 8 s`);
  check("HubDeck draws the reading, not the plugin's logo", drawable > 0, `${drawable} of 16 samples are a drawable picture other than the logo`);
  check("and it keeps changing (the reading's graph)", images.size > 1);
  // the key's label is the plugin's title (owner's 1a, 2026-10-20): it draws "CPU" itself
  const picture = async (name) => {
    const looks = await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("live_looks"));
    const l = looks?.looks?.["keys.0"];
    if (l?.image) fs.writeFileSync(path.join(root, "logs", `hwinfo-key-${name}.png`), Buffer.from(l.image.split(",")[1], "base64"));
    return l;
  };
  const plain = await picture("no-label");
  check("no HubDeck label over the plugin's picture", !plain?.label, JSON.stringify(plain?.label ?? null));
  await api(`profiles/${profile}/controls`, { control: "keys.0", binding: { action: ACTION, params, look: { label: "CPU" } } });
  await sleep(3000);
  const titled = await picture("cpu");
  check("with the label CPU HubDeck still writes nothing over it", !titled?.label, JSON.stringify(titled?.label ?? null));
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(800);
  await win.screenshot({ path: path.join(root, "logs", "hwinfo-key.png") });
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
