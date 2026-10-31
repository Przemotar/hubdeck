// The key's look editor (owner's "1 a" twice, 2026-10-25; mockup variant A with the templates of
// C first): as a user does it in the window, a template, the background kind, angle and finish, a
// text effect, a badge and a frame, saving and deleting an own template, and the look applied to
// another key; the profile keeps each, the key keeps its own label and icon, and the preview (the
// deck's own drawing) shows them. A test copy of its own with the owner's profiles (virtual
// modules, its window off screen, never the focus).
//   node scripts/ui/key-designer.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-key-designer");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "key-designer");
const PORT = 9483;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
delete settings.lookTemplates;
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
const lookOf = async (id) => (await api(`profiles/${profile}`)).controls?.[id]?.look ?? {};
const look = () => lookOf("keys.6");

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  console.log(`exe: ${exe}, profile ${profile}`);
  await api(`profiles/${profile}/controls`, { control: "keys.6", binding: { action: "core.hotkey", params: { keys: "Ctrl+Alt+F8", mode: "normal" }, look: { label: "Lubię to", icon: "heart" } } });
  await api(`profiles/${profile}/controls`, { control: "keys.7", binding: { action: "core.hotkey", params: { keys: "Ctrl+Alt+F9", mode: "normal" }, look: { label: "Oceń", icon: "emoji:🔥" } } });
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
  const panel = win.locator("section.look").first();
  const shot = (name) => panel.screenshot({ path: path.join(shots, `${name}.png`) });
  const tab = async (name) => (await win.getByRole("tab", { name, exact: true }).click(), sleep(350));
  const preview = () => win.locator(".look .preview").first().innerHTML();

  // the editor opens on its templates, drawn with the key's own heart and label
  await win.getByText("Edit look", { exact: false }).first().click();
  await sleep(1200);
  const tiles = win.getByRole("listbox", { name: "Templates" }).getByRole("option");
  check("six templates, drawn with the key's own label", (await tiles.count()) === 6 && /Lubię to/.test(await tiles.first().innerHTML()), `${await tiles.count()}`);
  await shot("1-templates");
  await win.getByRole("option", { name: "Neon" }).click();
  await sleep(900);
  let l = await look();
  check("Neon: a gradient from violet to pink with glass; label and icon stay", l.style === "gradient" && l.bg === "#6a3fd8" && l.bg2 === "#e0457b" && l.gloss === "glass" && l.label === "Lubię to" && l.icon === "heart", JSON.stringify(l));
  check("the Neon tile is marked as the key's look", (await win.getByRole("option", { name: "Neon" }).getAttribute("aria-selected")) === "true");
  check("the preview draws the gradient", /linearGradient/.test(await preview()));

  // Background: the direction, then a matte finish, then a flat color keeps the finish
  await tab("Background");
  await win.locator('input[type="range"]').first().fill("90");
  await sleep(700);
  check("the direction slider sets 90 degrees", (await look()).angle === 90, String((await look()).angle));
  await win.getByRole("radiogroup", { name: "Finish" }).getByRole("radio", { name: "Matte" }).click();
  await sleep(700);
  check("Matte sets the matte finish", (await look()).gloss === "matte");
  await shot("2-background");
  await win.getByRole("radiogroup", { name: "Background" }).getByRole("radio", { name: "Color" }).click();
  await sleep(700);
  l = await look();
  check("Color: a flat key that keeps the matte finish", l.style === undefined && l.gloss === "matte", JSON.stringify(l));
  check("the preview draws the matte finish", /kmatte/.test(await preview()));

  // Label: a shadow
  await tab("Label");
  await win.getByRole("radiogroup", { name: "Text effect" }).getByRole("radio", { name: "Shadow" }).click();
  await sleep(700);
  check("Shadow sets the text effect", (await look()).labelEffect === "shadow");
  await shot("3-label");

  // Extras: a badge and a frame
  await tab("Extras");
  await win.getByPlaceholder("3, !, NEW").fill("3");
  await sleep(900);
  await win.getByLabel("Frame around the key").check();
  await sleep(900);
  l = await look();
  check("the badge and the frame are kept", l.badge === "3" && l.ring === "#f5c542", JSON.stringify(l));
  const p = await preview();
  check("the preview draws the badge and the frame", />3</.test(p) && /#f5c542/i.test(p));
  await win.getByRole("radiogroup", { name: "Pattern" }).getByRole("radio", { name: "Stripes" }).click();
  await sleep(700);
  await win.getByRole("slider").last().fill("40");
  await sleep(800);
  l = await look();
  check("Stripes at 40 % are kept", l.pattern === "stripes" && l.patternOpacity === 40, JSON.stringify(l));
  check("the preview draws the stripes", /stroke-opacity="0.4"/.test(await preview()));
  await win.getByRole("radiogroup", { name: "Motion" }).getByRole("radio", { name: "Move pattern" }).click();
  await sleep(700);
  check("Move pattern is kept", (await look()).motion === "slide");
  // the deck's own picture moves: two shots of the key in the window's mirror half a second apart
  const keyBox = { x: box.x, y: box.y, width: box.width, height: box.height };
  const a1 = await win.screenshot({ clip: keyBox });
  await sleep(600);
  const a2 = await win.screenshot({ clip: keyBox });
  check("the key moves on the deck (the window's mirror of it changes)", !a1.equals(a2));
  await shot("4-extras");

  // an own template: saved, shown, put on, deleted
  await tab("Templates");
  await win.getByRole("button", { name: "Save as my template" }).click();
  await win.getByPlaceholder("Template name").fill("Mój styl");
  await win.getByRole("button", { name: "Save", exact: true }).click();
  await sleep(1000);
  const saved = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8")).lookTemplates ?? [];
  check("the own template is saved without the key's label, icon or badge text", saved.length === 1 && saved[0].name === "Mój styl" && saved[0].look.ring === "#f5c542" && !saved[0].look.label && !saved[0].look.badge, JSON.stringify(saved));
  check("it shows as a seventh template", (await tiles.count()) === 7);
  await shot("5-own-template");
  await win.getByRole("option", { name: "Forest" }).click();
  await sleep(800);
  await win.getByRole("option", { name: "Mój styl" }).click();
  await sleep(800);
  l = await look();
  check("the own template puts its look back (frame, matte) and keeps the badge text", l.ring === "#f5c542" && l.gloss === "matte" && l.badge === "3" && l.label === "Lubię to", JSON.stringify(l));

  // the look on another key of the page
  await win.getByRole("button", { name: "Apply this look to other keys" }).click();
  await sleep(400);
  await win.getByLabel("8. Oceń").check();
  await shot("6-apply");
  // what the user sees: the whole window, the list scrolled into view
  await win.getByRole("button", { name: /Apply to selected/ }).scrollIntoViewIfNeeded();
  await win.screenshot({ path: path.join(shots, "6-apply-window.png") });
  await win.locator(".tpl-grid").scrollIntoViewIfNeeded();
  await win.screenshot({ path: path.join(shots, "6-apply-window-top.png") });
  // the list of keys open: names whole, and the panel not widened (a two-column list cut the
  // Night template and the Label tab, session 30)
  const cutOpen = (await clipped(win)).filter((x) => /\d+\. |Night|Label|Extras|Apply to selected/.test(x.text));
  check("with the key list open nothing of the editor is cut", cutOpen.length === 0, cutOpen.map((x) => x.text).join(" | "));
  // and nothing in it wider than the panel (the clip check reads text only; a wide child pushed the
  // templates past the panel's edge)
  const wide = await win.evaluate(() => {
    // the panel's scrolling parent: what the user sees of it
    let el = document.querySelector("section.look");
    if (!el) return ["no section.look"];
    while (el.parentElement && !/(auto|scroll|hidden)/.test(getComputedStyle(el.parentElement).overflowX + getComputedStyle(el.parentElement).overflowY)) el = el.parentElement;
    const view = el.parentElement ?? el;
    const r = view.getBoundingClientRect();
    const box = { right: Math.min(r.right, r.left + view.clientWidth) };
    return [document.querySelector("section.look"), ...document.querySelectorAll("section.look *")]
      .filter((e) => e.getBoundingClientRect().right > box.right + 1 && e.getBoundingClientRect().width > 0)
      .map((e) => `${e.tagName.toLowerCase()}.${[...e.classList].join(".")} right ${Math.round(e.getBoundingClientRect().right - box.right)}px w ${Math.round(e.getBoundingClientRect().width)}`)
      .slice(0, 8);
  });
  check("with the key list open nothing sticks out of the panel", wide.length === 0, wide.join(" | "));
  await win.getByRole("button", { name: /Apply to selected \(1\)/ }).click();
  await sleep(1000);
  const other = await lookOf("keys.7");
  check("key 8 takes the look and keeps its label, emoji and has no badge", other.ring === "#f5c542" && other.gloss === "matte" && other.label === "Oceń" && other.icon === "emoji:🔥" && !other.badge, JSON.stringify(other));
  await sleep(1500);
  await win.locator(".dk-keys, g.dk-ctl[data-kind=\"keys\"]").first().scrollIntoViewIfNeeded().catch(() => {});
  await win.screenshot({ path: path.join(shots, "6-applied-deck.png"), clip: { x: box.x - 160, y: box.y - 110, width: 340, height: 200 } });
  check("the window says it is done", await win.getByText("Look applied to 1 key", { exact: true }).isVisible());

  // delete the own template
  await win.getByRole("button", { name: "Delete template Mój styl" }).click();
  await sleep(800);
  check("the own template is deleted", (JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8")).lookTemplates ?? []).length === 0 && (await tiles.count()) === 6);

  // light and dark, the owner's window sizes, nothing of the editor cut
  for (const t of ["dark", "light"]) {
    await win.evaluate((x) => document.documentElement.setAttribute("data-theme", x), t);
    await sleep(300);
    await shot(`7-${t}`);
  }
  for (const [w, h] of [[960, 640], [1280, 800], [1600, 1000]]) {
    await win.setViewportSize({ width: w, height: h });
    await sleep(400);
    for (const name of ["Templates", "Background", "Icon", "Label", "Extras"]) {
      await tab(name);
      const cut = (await clipped(win)).filter((x) => /Templates|Background|Icon|Label|Extras|Color|Gradient|Outline|Finish|None|Glass|Matte|Direction|Shadow|Text effect|Badge|Frame|Save as|Apply|Neon|Forest|Ember|Night|Pattern|Stripes|Dots|Grid|Noise|Strength|Motion|Pulse|Shine|Move pattern/.test(x.text));
      check(`nothing of ${name} cut at ${w} wide`, cut.length === 0, cut.map((x) => x.text).join(" | "));
    }
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
