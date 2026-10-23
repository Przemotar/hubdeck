// A control's right-click menu in the window (owner's 1a, 2026-10-20; Stream Deck: "Right-click keys
// to copy, delete or move them"): Copy, Paste, Clear with their shortcuts, Open page on a page key,
// greyed out where they do nothing; Escape and a click outside close it. A test copy of its own
// (virtual modules, its window off screen, never the focus). Screenshots in logs/ctl-menu-*.png.
//   node scripts/ui/ctl-menu.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-ctl-menu");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9480;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
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
const controls = async () => (await api(`profiles/${profile}`)).controls ?? {};
const shownPage = async () => (await api("controls")).page ?? null;

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  console.log(`exe: ${exe}`);
  const pg = await api(`profiles/${profile}/pages`, { name: "Menu test", id: "menutest" });
  const set = {
    "keys.0": { action: "core.hotkey", params: { keys: "Ctrl+Alt+F5", mode: "normal" }, look: { label: "SRC" } },
    "keys.1": null,
    "keys.1:hold": null,
    "keys.1:double": null,
    "keys.2": { action: "core.page", params: { page: pg.id }, look: { label: "PAGE" } },
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
  // a key's own centre: its ring (a key showing the deck's picture has a box as wide as the panel)
  const centre = async (i) => {
    const b = await win.locator(`g.dk-ctl[data-kind="keys"][data-index="${i}"] .dk-hov`).first().boundingBox();
    return [b.x + b.width / 2, b.y + b.height / 2];
  };
  const menu = win.locator(".ctx-menu");
  const rightClick = async (i) => {
    const [x, y] = await centre(i);
    await win.mouse.click(x, y, { button: "right" });
    await sleep(300);
  };
  const item = (name) => menu.getByRole("menuitem", { name: new RegExp(name) });

  // a set key: Copy, Paste (nothing copied yet: greyed), Clear, with their shortcuts
  await rightClick(0);
  check("a right click opens the key's menu", await menu.isVisible(), (await menu.innerText().catch(() => "")).replace(/\s+/g, " "));
  check("titled with the key", /Key 1/i.test(await menu.innerText()));
  check("Copy and Clear are on, Paste greyed with nothing copied", (await item("Copy").isEnabled()) && (await item("Clear").isEnabled()) && !(await item("Paste").isEnabled()));
  check("the shortcuts are shown", /Ctrl\+C/.test(await menu.innerText()) && /Delete/.test(await menu.innerText()));
  check("the key is selected", await win.locator('g.dk-ctl[data-kind="keys"][data-index="0"]').evaluate((el) => el.classList.contains("is-selected")));
  for (const look of ["dark", "light"]) {
    await win.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
    await sleep(200);
    await win.screenshot({ path: path.join(root, "logs", `ctl-menu-1-set-${look}.png`) });
  }
  await win.evaluate(() => document.documentElement.removeAttribute("data-theme"));
  await item("Copy").click();
  await sleep(300);
  check("Copy closes the menu", !(await menu.isVisible()));

  // an empty key: Copy and Clear greyed, Paste on; Paste puts the copy there
  await rightClick(1);
  check("on an empty key Copy and Clear are greyed, Paste is on", !(await item("Copy").isEnabled()) && !(await item("Clear").isEnabled()) && (await item("Paste").isEnabled()));
  await item("Paste").click();
  await sleep(800);
  let c = await controls();
  check("Paste puts the copied key there", c["keys.1"]?.params?.keys === "Ctrl+Alt+F5", c["keys.1"]?.params?.keys);

  // Clear, and Ctrl+Z brings it back
  await rightClick(1);
  await item("Clear").click();
  await sleep(800);
  c = await controls();
  check("Clear empties it", !c["keys.1"]);
  await win.keyboard.press("Control+z");
  await sleep(800);
  check("Ctrl+Z brings it back", (await controls())["keys.1"]?.params?.keys === "Ctrl+Alt+F5");

  // a page key has Open page first
  await rightClick(2);
  check("a page key's menu has Open page", await item("Open page").isVisible());
  await item("Open page").click();
  await sleep(800);
  check("Open page opens its page", (await shownPage()) === pg.id, String(await shownPage()));
  await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("page_set", { page: null }));
  await sleep(600);

  // Escape and a click outside close it; nothing cut at 960 and 1280
  await rightClick(0);
  await win.keyboard.press("Escape");
  await sleep(200);
  check("Escape closes it", !(await menu.isVisible()));
  for (const [w, h] of [[960, 640], [1280, 800]]) {
    await win.setViewportSize({ width: w, height: h });
    await sleep(400);
    await rightClick(0);
    const cut = (await clipped(win)).filter((x) => /Copy|Paste|Clear|Open page|Ctrl/.test(x.text));
    check(`nothing in the menu cut at ${w} wide`, cut.length === 0, cut.map((x) => x.text).join(" | "));
    await win.mouse.click(5, h - 5);
    await sleep(200);
    check(`a click outside closes it at ${w}`, !(await menu.isVisible()));
  }
  check("no page errors", errors === 0, `${errors}`);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1000);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
