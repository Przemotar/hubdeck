// The look editor's new fields (issue #10, killa8boll; owner's 1a, 2026-10-25): a key's label
// placed at the top and sized, its icon made large, and an emoji from Change icon > Emoji, as a
// user does it in the window; the profile keeps them and the deck draws them (the window mirrors
// the deck). A test copy of its own (virtual modules, its window off screen, never the focus).
//   node scripts/ui/look-editor.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-look-editor");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9482;
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
const look = async () => (await api(`profiles/${profile}`)).controls?.["keys.6"]?.look ?? {};

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  console.log(`exe: ${exe}`);
  await api(`profiles/${profile}/controls`, { control: "keys.6", binding: { action: "core.hotkey", params: { keys: "Ctrl+Alt+F8", mode: "normal" }, look: { label: "Lubię to", icon: "heart" } } });
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
  const key = win.locator('g.dk-ctl[data-kind="keys"][data-index="6"]').first();
  const box = await key.locator(".dk-hov").first().boundingBox();
  await win.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await sleep(800);

  // Change icon > Emoji > the fire
  await win.getByRole("button", { name: "Change icon" }).first().click();
  await sleep(400);
  await win.getByRole("tab", { name: "Emoji" }).click();
  await sleep(300);
  const grid = win.locator(".icon-popover");
  await grid.screenshot({ path: path.join(root, "logs", "look-editor-emoji.png") });
  await win.getByRole("button", { name: "fire hot" }).click();
  await sleep(900);
  check("an emoji from the list becomes the key's icon", (await look()).icon === "emoji:🔥", (await look()).icon);

  // More looks: label at the top, size 16, a large icon
  await win.locator("details.more-looks > summary").first().click();
  await sleep(300);
  await win.getByRole("radiogroup", { name: "Label position" }).getByRole("radio", { name: "Top" }).click();
  await sleep(500);
  await win.locator("label.field", { hasText: "Label size" }).locator("select").selectOption("16");
  await sleep(500);
  await win.getByRole("radiogroup", { name: "Icon size" }).getByRole("radio", { name: "Large" }).click();
  await sleep(900);
  const l = await look();
  check("the profile keeps the label at the top, size 16, a large icon", l.labelPos === "top" && l.labelSize === 16 && l.iconSize === "large", JSON.stringify(l));
  const svg = await win.locator(".look .preview").first().innerHTML();
  check("the preview draws the emoji", /Segoe UI Emoji/.test(svg));
  await win.waitForTimeout(800);
  await win.screenshot({ path: path.join(root, "logs", "look-editor-key.png"), clip: { x: box.x, y: box.y, width: box.width, height: box.height } });
  for (const t of ["dark", "light"]) {
    await win.evaluate((x) => document.documentElement.setAttribute("data-theme", x), t);
    await sleep(250);
    await win.screenshot({ path: path.join(root, "logs", `look-editor-${t}.png`) });
  }
  for (const [w, h] of [[960, 640], [1280, 800]]) {
    await win.setViewportSize({ width: w, height: h });
    await sleep(400);
    const cut = (await clipped(win)).filter((x) => /Label|Icon size|Top|Middle|Bottom|Small|Large|Auto|More looks/.test(x.text));
    check(`nothing of the new fields cut at ${w} wide`, cut.length === 0, cut.map((x) => x.text).join(" | "));
  }
  // Auto takes them back off
  await win.getByRole("radiogroup", { name: "Label position" }).getByRole("radio", { name: "Auto" }).click();
  await sleep(700);
  check("Auto takes the label position off", (await look()).labelPos === undefined);
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
