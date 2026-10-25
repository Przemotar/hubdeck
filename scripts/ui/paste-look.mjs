// "Paste look" and the startup switch (issue #10, killa8boll, 2026-10-25; owner's picks 1a):
// a control's colors, light and style pasted onto another without its action, from the right-click
// menu and with Ctrl+Shift+V; and Settings > General > Startup, the same switch as the tray's. A
// test copy of its own (virtual modules, its window off screen, never the focus) with the owner's
// data; the test copy never writes the system's startup list (checked against the registry).
//   node scripts/ui/paste-look.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { execSync, spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-paste-look");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9481;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const runEntry = () => {
  try {
    return execSync('reg query "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" /v HubDeck', { encoding: "utf8" }).trim().split("\n").pop().trim();
  } catch {
    return "(none)";
  }
};

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const profile = settings.active;
const entryBefore = runEntry();

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
const controls = async () => (await api(`profiles/${profile}`)).controls ?? {};

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  console.log(`exe: ${exe}`);
  const src = { action: "core.hotkey", params: { keys: "Ctrl+Alt+F5", mode: "normal" }, look: { label: "SRC", icon: "star", bg: "#123456", fg: "#ffeeaa", iconColor: "#ff8800", style: "glass" } };
  const set = {
    "keys.0": src,
    "keys.1": { action: "core.hotkey", params: { keys: "Ctrl+Alt+F6", mode: "normal" }, look: { label: "DST", icon: "music", bg: "#000000" } },
    "keys.3": null,
    "keys.3:hold": null,
    "keys.3:double": null,
    "keys.4": { action: "core.hotkey", params: { keys: "Ctrl+Alt+F7", mode: "normal" }, look: { label: "KEYB" } },
    "knobs.0": { action: "core.volume", params: { target: "system" }, look: { label: "Vol", led: "#00ff88", ledEffect: "breathing" } },
    "knobs.1": { action: "core.volume", params: { target: "mic" }, look: { label: "Mic", led: "#ff0000" } },
  };
  for (const [control, binding] of Object.entries(set)) await api(`profiles/${profile}/controls`, { control, binding });
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
  const centre = async (kind, i) => {
    const b = await win.locator(`g.dk-ctl[data-kind="${kind}"][data-index="${i}"] .dk-hov`).first().boundingBox();
    return [b.x + b.width / 2, b.y + b.height / 2];
  };
  const menu = win.locator(".ctx-menu");
  const rightClick = async (kind, i) => {
    const [x, y] = await centre(kind, i);
    await win.mouse.click(x, y, { button: "right" });
    await sleep(300);
  };
  const item = (name) => menu.getByRole("menuitem", { name: new RegExp(name) });

  // Copy key 1 from its menu, then Paste look on key 2: its action and label stay
  await rightClick("keys", 0);
  check("Paste look is greyed with nothing copied", !(await item("Paste look").isEnabled()));
  await item("Copy").click();
  await sleep(300);
  await rightClick("keys", 1);
  check("the menu shows Paste look with its shortcut", (await item("Paste look").isEnabled()) && /Ctrl\+Shift\+V/.test(await menu.innerText()), (await menu.innerText()).replace(/\s+/g, " "));
  for (const look of ["dark", "light"]) {
    await win.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
    await sleep(200);
    await win.screenshot({ path: path.join(root, "logs", `paste-look-menu-${look}.png`) });
  }
  await win.evaluate(() => document.documentElement.removeAttribute("data-theme"));
  const cut = (await clipped(win)).filter((x) => /Paste look|Ctrl\+Shift/.test(x.text));
  check("nothing in the menu cut", cut.length === 0, cut.map((x) => x.text).join(" | "));
  await item("Paste look").click();
  await sleep(900);
  let c = await controls();
  const k1 = c["keys.1"];
  check("its action stays", k1?.params?.keys === "Ctrl+Alt+F6", k1?.params?.keys);
  check("its label and icon stay", k1?.look?.label === "DST" && k1?.look?.icon === "music", JSON.stringify(k1?.look));
  check("the colors and style come over", k1?.look?.bg === "#123456" && k1?.look?.fg === "#ffeeaa" && k1?.look?.iconColor === "#ff8800" && k1?.look?.style === "glass", JSON.stringify(k1?.look));
  check("the window says what was pasted", /Look pasted/.test(await win.locator("body").innerText()));
  await win.locator('g.dk-ctl[data-kind="keys"][data-index="1"]').first().screenshot({ path: path.join(root, "logs", "paste-look-key.png") });

  // an empty key: Paste look is greyed (nothing whose look could change)
  await rightClick("keys", 3);
  check("on an empty key Paste look is greyed", !(await item("Paste look").isEnabled()));
  await win.keyboard.press("Escape");
  await sleep(200);

  // the shortcut: select key 5, Ctrl+Shift+V
  const [x5, y5] = await centre("keys", 4);
  await win.mouse.click(x5, y5);
  await sleep(400);
  await win.keyboard.press("Control+Shift+v");
  await sleep(900);
  c = await controls();
  check("Ctrl+Shift+V pastes the look on the selected key", c["keys.4"]?.look?.bg === "#123456" && c["keys.4"]?.params?.keys === "Ctrl+Alt+F7", JSON.stringify(c["keys.4"]?.look));
  await win.keyboard.press("Control+z");
  await sleep(900);
  check("Ctrl+Z takes it back", !(await controls())["keys.4"]?.look?.bg);

  // a knob's light onto another knob
  await rightClick("knobs", 0);
  await item("Copy").click();
  await sleep(300);
  await rightClick("knobs", 1);
  await item("Paste look").click();
  await sleep(900);
  c = await controls();
  check("a knob's LED color and effect go to another knob, its action stays", c["knobs.1"]?.look?.led === "#00ff88" && c["knobs.1"]?.look?.ledEffect === "breathing" && c["knobs.1"]?.params?.target === "mic", JSON.stringify(c["knobs.1"]));

  // Settings > General > Startup
  await win.getByRole("button", { name: /^Settings$/ }).first().click();
  await sleep(800);
  await win.locator(".settings-nav-item", { hasText: "General" }).click();
  await sleep(600);
  const box = win.getByRole("checkbox", { name: "Start with Windows" });
  check("Settings > General has Startup with Start with Windows", (await box.count()) === 1 && /startup/i.test(await win.locator(".settings-content").innerText()));
  const was = await box.isChecked();
  await box.click();
  await sleep(500);
  check("the switch turns", (await box.isChecked()) !== was);
  for (const look of ["dark", "light"]) {
    await win.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
    await sleep(200);
    await win.locator(".settings-content").screenshot({ path: path.join(root, "logs", `paste-look-startup-${look}.png`) });
  }
  const cutS = (await clipped(win)).filter((x) => /Startup|Start with Windows|tray/.test(x.text));
  check("nothing in Startup cut", cutS.length === 0, cutS.map((x) => x.text).join(" | "));
  await box.click();
  await sleep(500);
  check("the test copy left the system's startup entry alone", runEntry() === entryBefore, runEntry());
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
