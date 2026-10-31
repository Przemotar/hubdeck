// Looks by conditions and values in labels (owner's "2 a", 2026-10-25, step 4; as Bitfocus
// Companion's feedbacks): in the full editor, a value put in the label with a click, a rule "CPU
// over 0 %" turning the key red, a time rule adding a frame, a rule removed; what the deck shows
// is read back from the engine (the looks it draws), not from the saved profile. A test copy of its
// own with the owner's profiles (virtual modules, its window off screen, never the focus).
//   node scripts/ui/look-rules.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-look-rules");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "look-rules");
const PORT = 9486;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
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
const saved = async () => (await api(`profiles/${profile}`)).controls?.["keys.6"]?.look ?? {};

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  console.log(`exe: ${exe}, profile ${profile}`);
  await api(`profiles/${profile}/controls`, { control: "keys.6", binding: { action: "core.hotkey", params: { keys: "Ctrl+Alt+F8", mode: "normal" }, look: { label: "CPU", icon: "cpu", bg: "#222233" } } });
  await api(`profiles/${profile}/activate`, {});
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(2500);
  // what the deck shows for the key now (the engine's own looks)
  const shown = async () => (await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("live_looks"))).looks?.["keys.6"] ?? null;

  const key = win.locator('g.dk-ctl[data-kind="keys"][data-index="6"]').first();
  const box = await key.locator(".dk-hov").first().boundingBox();
  await win.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await sleep(800);
  await win.getByRole("button", { name: "Full editor" }).click();
  await sleep(1200);
  const studio = win.getByRole("region", { name: "Key editor" });

  // a value in the label with a click
  await studio.getByRole("group", { name: "Show a value" }).getByRole("button", { name: "{cpu}" }).click();
  await sleep(1200);
  check("the {cpu} chip puts the value's name in the label", (await saved()).label === "CPU {cpu}", (await saved()).label);
  await sleep(1500);
  let s = await shown();
  check("the deck shows the CPU load in its place", /^CPU (\d+%|–)$/.test(s?.label ?? ""), s?.label);

  // a rule: CPU over 0 % (always) turns the key red
  await studio.getByRole("tab", { name: "When", exact: true }).click();
  await sleep(400);
  await studio.getByRole("button", { name: "Add a rule" }).click();
  await sleep(400);
  await studio.getByRole("spinbutton", { name: "over" }).fill("0");
  await sleep(1500);
  const rules = (await saved()).rules ?? [];
  check("the rule is kept: CPU over 0, red", rules.length === 1 && rules[0].when.kind === "metric" && rules[0].when.above === 0 && rules[0].look.bg === "#c0392b", JSON.stringify(rules));
  await sleep(1200);
  s = await shown();
  check("while it holds the deck shows the key red", s?.bg === "#c0392b", s?.bg);
  await studio.screenshot({ path: path.join(shots, "1-rule.png") });

  // a second rule: all day (00:00 to 23:59) with a frame; the first rule's red stays
  await studio.getByRole("button", { name: "Add a rule" }).click();
  await sleep(400);
  const second = studio.getByRole("region", { name: "Rule 2" });
  await second.getByRole("combobox", { name: "When" }).selectOption({ label: "The time is between" });
  await sleep(300);
  await second.getByLabel("From").fill("00:00");
  await second.getByLabel("To").fill("23:59");
  // this rule only adds a frame: its background off, the frame on
  await second.getByLabel("Background", { exact: true }).uncheck();
  await second.getByLabel("Frame", { exact: true }).check();
  await sleep(1500);
  s = await shown();
  check("the time rule adds a frame over the red", s?.bg === "#c0392b" && /^#/.test(s?.ring ?? "") && s.ring.toLowerCase() === "#f5c542", JSON.stringify({ bg: s?.bg, ring: s?.ring }));
  // the first removed: the key is back to its own color, the frame stays
  await studio.getByRole("button", { name: "Remove rule 1" }).click();
  await sleep(1800);
  s = await shown();
  check("with the CPU rule gone the key is its own color again", s?.bg === "#222233" && !!s?.ring, JSON.stringify({ bg: s?.bg, ring: s?.ring }));
  await studio.screenshot({ path: path.join(shots, "2-time-rule.png") });

  for (const [w, h] of [[960, 640], [1280, 800]]) {
    await win.setViewportSize({ width: w, height: h });
    await sleep(500);
    const cut = (await clipped(win)).filter((x) => /When|Add a rule|The time is between|Then the key gets|Show a value|\{cpu\}|\{song\}|\{gpuTemp\}|No motion/.test(x.text));
    check(`nothing of the rules cut at ${w} wide`, cut.length === 0, cut.map((x) => x.text).join(" | "));
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
