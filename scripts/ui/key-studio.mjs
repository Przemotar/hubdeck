// The full key editor (owner's "2 a", 2026-10-25: mockup A): as a user does it in the window, the
// Look panel's "Full editor" opens it on the whole window; an icon from its library, the icon and
// the label dragged on the big key and nudged with the arrows, the key's own font, a moving look
// playing in the preview, another key ticked to change with it, Escape and Done back to the deck;
// the profile keeps each change. A test copy of its own with the owner's profiles (virtual
// modules, its window off screen, never the focus).
//   node scripts/ui/key-studio.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-key-studio");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "key-studio");
const PORT = 9484;
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

  // the Look panel's "Full editor" opens the editor on the whole window
  await win.getByRole("button", { name: "Full editor" }).click();
  await sleep(1200);
  const studio = win.getByRole("region", { name: "Key editor" });
  check("Full editor opens the key editor on the window", await studio.isVisible());
  await win.screenshot({ path: path.join(shots, "1-open.png") });

  // an icon from the library on the left
  await studio.getByRole("tab", { name: "Emoji" }).click();
  await sleep(400);
  await studio.getByRole("button", { name: "fire hot" }).click();
  await sleep(900);
  check("an emoji from the library becomes the icon", (await look()).icon === "emoji:🔥", (await look()).icon);

  // drag the icon 8 key pixels right on the big key, then nudge it up with the arrow
  const big = studio.getByRole("img", { name: "Preview of the key" });
  const b = await big.boundingBox();
  const zoom = b.width / 72;
  await win.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await win.mouse.down();
  await win.mouse.move(b.x + b.width / 2 + 8 * zoom, b.y + b.height / 2, { steps: 6 });
  await win.mouse.up();
  await big.press("ArrowUp");
  await sleep(900);
  let l = await look();
  check("the dragged icon moved 8 right and the arrow 1 up", l.iconDx === 8 && l.iconDy === -1, JSON.stringify({ iconDx: l.iconDx, iconDy: l.iconDy }));
  // the label: Drag > Label, then down 3
  await studio.getByRole("radiogroup", { name: "Drag" }).getByRole("radio", { name: "Label" }).click();
  for (let i = 0; i < 3; i++) await big.press("ArrowDown");
  await sleep(900);
  check("the label moved 3 down", (await look()).labelDy === 3, String((await look()).labelDy));
  await studio.getByRole("button", { name: "Back to its place" }).click();
  await sleep(900);
  l = await look();
  check("Back to its place takes the moves off", !l.iconDx && !l.iconDy && !l.labelDy, JSON.stringify(l));
  // undo and redo, with the keys (owner: "brakuje mi opcji cofania i powtarzania akcji i skrótów")
  await big.focus();
  await win.keyboard.press("Control+z");
  await sleep(900);
  check("Ctrl+Z undoes Back to its place (the label is 3 down again)", (await look()).labelDy === 3, JSON.stringify(await look()));
  await win.keyboard.press("Control+y");
  await sleep(900);
  check("Ctrl+Y redoes it", !(await look()).labelDy, JSON.stringify(await look()));
  await studio.getByRole("button", { name: "Undo", exact: true }).click();
  await sleep(900);
  check("the Undo button undoes too", (await look()).labelDy === 3);
  await studio.getByRole("button", { name: "Redo", exact: true }).click();
  await sleep(900);
  // the checkered stage hugs the key
  const stage = await win.evaluate(() => {
    const c = document.querySelector(".studio-canvas")?.getBoundingClientRect();
    const k = document.querySelector(".studio-key")?.getBoundingClientRect();
    return c && k ? { canvas: Math.round(c.width), key: Math.round(k.width) } : null;
  });
  check("the checkered stage is only around the key", !!stage && stage.canvas <= stage.key + 40, JSON.stringify(stage));

  // the key's own font, from the list of this computer's fonts
  await studio.getByRole("tab", { name: "Label", exact: true }).click();
  await sleep(600);
  const fontSelect = studio.locator("label.field", { hasText: "Font" }).locator("select");
  const fonts = await fontSelect.locator("option").allTextContents();
  const font = fonts.find((f) => /Segoe UI|Arial|Georgia/.test(f)) ?? fonts[2];
  await fontSelect.selectOption({ label: font });
  await sleep(900);
  check("the key gets its own font from the list", (await look()).font === font, `${(await look()).font} (${fonts.length - 1} fonts listed)`);
  check("the big key draws it", (await big.innerHTML()).includes(`font-family="${font}"`));

  // a moving look plays in the big preview
  await studio.getByRole("tab", { name: "Extras", exact: true }).click();
  await sleep(400);
  await studio.getByRole("radiogroup", { name: "Motion" }).getByRole("radio", { name: "Pulse" }).click();
  await sleep(1200);
  const f1 = await big.innerHTML();
  await sleep(700);
  const f2 = await big.innerHTML();
  check("the pulse plays in the preview (its frames change)", f1 !== f2);

  // key 8 ticked: a template goes to both, key 8 keeps its label and emoji
  await studio.getByRole("button", { name: "8. Oceń" }).click();
  await sleep(300);
  await studio.getByRole("tab", { name: "Templates", exact: true }).click();
  await sleep(500);
  await studio.getByRole("option", { name: "Forest" }).click();
  await sleep(1200);
  const other = await lookOf("keys.7");
  check("the ticked key 8 takes the template and keeps its label and emoji", other.bg === "#0f766e" && other.label === "Oceń" && other.icon === "emoji:🔥", JSON.stringify(other));
  await win.screenshot({ path: path.join(shots, "2-edited.png") });

  // the profile's style: new keys start in this look
  await studio.getByLabel(/New keys in .* start in this look/).check();
  await sleep(900);
  const style = (await api(`profiles/${profile}`)).keyStyle;
  check("the profile keeps the style (the look's colors, not its label or icon)", style?.bg === "#0f766e" && !style.label && !style.icon, JSON.stringify(style));
  // the page as a Stream Deck icon pack and the key as a PNG: the save dialog is the system's (a
  // test copy skips it), so the commands the buttons call are called as the window does
  const packPath = path.join(dir, "page.streamDeckIconPack");
  const pngPath = path.join(dir, "key.png");
  const count = await win.evaluate(([p, f]) => window.__TAURI_INTERNALS__.invoke("keys_export_pack", { profile: p, page: null, path: f }), [profile, packPath]);
  check("the page's keys export as an icon pack", count > 0 && fs.statSync(packPath).size > 1000, `${count} keys, ${fs.statSync(packPath).size} B`);
  await win.evaluate(([l, f]) => window.__TAURI_INTERNALS__.invoke("key_png_save", { look: l, path: f }), [await look(), pngPath]);
  const head = fs.readFileSync(pngPath).subarray(0, 24);
  check("the key saves as a 288 px PNG", head.readUInt32BE(16) === 288 && head.readUInt32BE(20) === 288, `${head.readUInt32BE(16)} x ${head.readUInt32BE(20)}`);

  // the owner's window sizes, light and dark: nothing cut, nothing outside the window
  for (const [w, h] of [[960, 640], [1280, 800], [1600, 1000]]) {
    await win.setViewportSize({ width: w, height: h });
    await sleep(500);
    for (const name of ["Templates", "Background", "Icon", "Label", "Extras"]) {
      await studio.getByRole("tab", { name, exact: true }).click();
      await sleep(250);
      const cut = (await clipped(win)).filter((x) => /Key editor|Back|Done|Drag|Icon|Label|Back to its place|Font|Templates|Background|Extras|Motion|Pulse|keys ticked|key ticked|Save PNG|Export page|New keys in|Undo|Redo/.test(x.text));
      check(`nothing of ${name} cut at ${w} wide`, cut.length === 0, cut.map((x) => x.text).join(" | "));
    }
    const out = await win.evaluate(() => [...document.querySelectorAll(".studio *")].filter((e) => e.getBoundingClientRect().right > window.innerWidth + 1).length);
    check(`nothing past the window's edge at ${w} wide`, out === 0, String(out));
    // the icon library uses its column: tall, and no sideways scroll (it had the popover's 248 px)
    const lib = await win.evaluate(() => {
      const g = document.querySelector(".studio-library .icon-grid");
      const pane = document.querySelector(".studio-library");
      return g && pane ? { h: g.clientHeight, pane: pane.clientHeight, sideways: g.scrollWidth > g.clientWidth + 1 } : null;
    });
    check(`the icon library fills its column at ${w} wide, no sideways scroll`, !!lib && lib.h > lib.pane * 0.6 && !lib.sideways, JSON.stringify(lib));
    await win.screenshot({ path: path.join(shots, `3-${w}.png`) });
  }
  for (const t of ["light", "dark"]) {
    await win.evaluate((x) => document.documentElement.setAttribute("data-theme", x), t);
    await sleep(300);
    await win.screenshot({ path: path.join(shots, `4-${t}.png`) });
  }

  // Escape (not in a field) closes it; the deck view is back with the key selected
  await big.focus();
  await win.keyboard.press("Escape");
  await sleep(800);
  check("Escape closes the editor and the deck is back", !(await studio.isVisible()) && (await key.isVisible()));
  await win.getByRole("button", { name: "Full editor" }).click();
  await sleep(800);
  await studio.getByRole("button", { name: "Done", exact: true }).click();
  await sleep(800);
  check("Done closes it too", !(await studio.isVisible()));
  // a key given an action now starts in the profile's style (an empty key, then Clock)
  await api(`profiles/${profile}/controls`, { control: "keys.12", binding: null });
  await sleep(900);
  const empty = win.locator('g.dk-ctl[data-kind="keys"][data-index="12"]').first();
  const eb = await empty.locator(".dk-hov").first().boundingBox();
  await win.mouse.click(eb.x + eb.width / 2, eb.y + eb.height / 2);
  await sleep(900);
  const clock = win.getByRole("button", { name: "Clock", exact: true }).first();
  if ((await clock.count()) === 1) {
    await clock.scrollIntoViewIfNeeded();
    await clock.click();
    await sleep(1200);
  }
  const fresh = await lookOf("keys.12");
  check("a new key starts in the profile's style", fresh.bg === "#0f766e" && fresh.style === "gradient", JSON.stringify(fresh));
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
