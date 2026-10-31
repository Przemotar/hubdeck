// The living deck on the deck (docs/LIVING-DECK.md; owner, 2026-10-26: "chce tez miec builda gdzie
// przetestuje to co zbudowales w nocy"): as a user does it in the window, a key style (skin) from the
// editor's first tab with the motion it was made with, the warning on a flickering motion, a template
// taking the style off; then every preset of the Deck look view tried on the deck (its picture moves
// and keeps its content, drawn by the graphics card), Discard, and every GPU layer compiled. The
// view's own steps (Apply, the Keys section, his own looks, the turned preview) are deck-look.mjs's.
// A test copy of its own with the owner's profiles (virtual modules, its window off screen, never
// the focus).
//   node scripts/ui/living-deck.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-living-deck");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "living-deck");
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
const saved = () => api(`profiles/${profile}`);
const look = async () => (await saved()).controls?.["keys.6"]?.look ?? {};

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  const p0 = await saved();
  console.log(`exe: ${exe}, profile ${p0.name} (${profile}); effects before: ${JSON.stringify(p0.effects ?? null)}`);
  await api(`profiles/${profile}/controls`, { control: "keys.6", binding: { action: "core.hotkey", params: { keys: "Ctrl+Alt+F8", mode: "normal" }, look: { label: "Lubię to", icon: "heart", bg: "#3060c0" } } });
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

  // --- a key style, from the key's Look panel > Edit look > Templates
  const key = win.locator('g.dk-ctl[data-kind="keys"][data-index="6"]').first();
  const box = await key.locator(".dk-hov").first().boundingBox();
  await win.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await sleep(800);
  const panel = win.locator("section.look").first();
  const tab = async (name) => (await win.getByRole("tab", { name, exact: true }).click(), sleep(350));
  await win.getByText("Edit look", { exact: false }).first().click();
  await sleep(1500);
  const styles = win.getByRole("listbox", { name: "Styles" }).getByRole("option");
  check("five styles, each drawn with the key's own label", (await styles.count()) === 5 && /Lubię to/.test(await styles.first().innerHTML()), `${await styles.count()}`);
  const drawn = await styles.evaluateAll((els) => els.map((e) => e.innerHTML.length));
  check("every style's tile has its own picture", new Set(drawn).size === 5, drawn.join(", "));
  await panel.screenshot({ path: path.join(shots, "1-styles.png") });
  await win.getByRole("option", { name: "Film" }).click();
  await sleep(900);
  let l = await look();
  check("Film: the key wears the film style with the projector's motion; label, icon and color stay", l.skin === "film" && l.motion === "film" && l.label === "Lubię to" && l.icon === "heart" && l.bg === "#3060c0", JSON.stringify(l));
  check("the Film tile is marked", (await win.getByRole("option", { name: "Film" }).getAttribute("aria-selected")) === "true");
  await win.getByRole("option", { name: "Neon tube" }).click();
  await sleep(900);
  l = await look();
  check("Neon tube in place of film, with its flicker", l.skin === "neon" && l.motion === "flicker", JSON.stringify(l));
  // the deck's own picture moves (the window's mirror of the key, half a second apart)
  const keyBox = { x: box.x, y: box.y, width: box.width, height: box.height };
  await sleep(1500);
  const frames = [];
  for (let i = 0; i < 8; i++) (frames.push(await win.screenshot({ clip: keyBox })), await sleep(500));
  check("the neon key moves on the deck", frames.some((f) => !f.equals(frames[0])));
  await tab("Extras");
  const motion = win.getByRole("radiogroup", { name: "Motion" });
  check("Extras offers the neon's own motions, Flicker on", (await motion.getByRole("radio", { name: "Flicker" }).getAttribute("aria-checked")) === "true" && (await motion.getByRole("radio", { name: "Sparks" }).count()) === 1);
  check("a flickering motion carries the warning", await win.locator(".help.warn").first().isVisible());
  await panel.screenshot({ path: path.join(shots, "2-extras-warning.png") });
  await motion.getByRole("radio", { name: "Sparks" }).click();
  await sleep(700);
  check("Sparks is kept and the warning goes", (await look()).motion === "sparks" && (await win.locator(".help.warn").count()) === 0);
  await tab("Templates");
  await win.getByRole("option", { name: "Neon tube" }).click();
  await sleep(800);
  l = await look();
  check("a second click takes the style off; a motion picked by hand stays", !l.skin && l.motion === "sparks", JSON.stringify(l));
  await win.getByRole("option", { name: "Glass" }).click();
  await sleep(700);
  await win.getByRole("option", { name: "Forest" }).click();
  await sleep(800);
  l = await look();
  check("a template takes the style off", !l.skin && l.style === "gradient", JSON.stringify(l));
  await win.getByRole("option", { name: "Pumpkin" }).click();
  await sleep(800);
  check("Pumpkin goes on", (await look()).skin === "pumpkin");
  await panel.screenshot({ path: path.join(shots, "3-pumpkin.png") });
  for (const [w, h] of [[960, 640], [1280, 800], [1600, 1000]]) {
    await win.setViewportSize({ width: w, height: h });
    await sleep(500);
    const cut = (await clipped(win)).filter((x) => /Styles|Film|Glitch|Neon tube|Glass|Pumpkin|A style draws|Templates/.test(x.text));
    check(`nothing of the styles cut at ${w} wide`, cut.length === 0, cut.map((x) => x.text).join(" | "));
  }
  await win.setViewportSize({ width: 1280, height: 800 });
  await win.keyboard.press("Escape");
  await sleep(500);

  // --- every preset of the Deck look view on the deck (the side panel these steps walked gave way to
  // the view in session 36; the view's own steps are in deck-look.mjs): each tried in turn moves the
  // deck's picture and keeps its content, and Discard leaves the profile as it was
  // what the deck is shown (the engine's last key picture): its mean brightness, and the picture
  const deckPicture = async (name) => {
    const shown = await api("deck/keys").catch(() => null);
    if (!shown?.jpeg) return -1;
    fs.writeFileSync(path.join(shots, `${name}.jpg`), Buffer.from(shown.jpeg, "base64"));
    return win.evaluate(async (b64) => {
      const img = new Image();
      img.src = `data:image/jpeg;base64,${b64}`;
      await img.decode();
      const c = document.createElement("canvas");
      [c.width, c.height] = [img.width, img.height];
      const g = c.getContext("2d");
      g.drawImage(img, 0, 0);
      const d = g.getImageData(0, 0, c.width, c.height).data;
      let sum = 0;
      for (let i = 0; i < d.length; i += 4) sum += 0.299 * d[i] + 0.587 * d[i + 1] + 0.114 * d[i + 2];
      return sum / (d.length / 4);
    }, shown.jpeg);
  };
  await win.getByRole("button", { name: /^Living deck/ }).first().click();
  await sleep(1200);
  check("the Living deck chip opens the Deck look view", await win.locator(".studio.dl").isVisible());
  const deckBox = await win.locator(".dl-preview").first().boundingBox();
  const deckBefore = await deckPicture("deck-before");
  check("the engine gives the deck's key picture (GET /api/deck/keys)", deckBefore > 0, deckBefore.toFixed(1));
  // the built-in presets; a test build's trial slots ("(próba)") are deck-look.mjs's
  const titles = (await win.locator(".dl-tile:not(.dl-none)").evaluateAll((els) => els.map((e) => e.getAttribute("title")))).filter((x) => x && !/próba|trial/i.test(x));
  check("the view lists the presets", titles.length >= 10, titles.join(", "));
  const keysJpeg = async () => (await api("deck/keys"))?.jpeg ?? "";
  for (const [i, name] of titles.entries()) {
    await win.locator(`.dl-tile[title="${name}"]`).first().click();
    await sleep(2500);
    const a = await keysJpeg();
    await sleep(700);
    const b = await keysJpeg();
    check(`${name}: the deck's picture moves`, !!a && !!b && a !== b);
    const lit = await deckPicture(`preset-${i + 1}`);
    // dark by design (the scenes keep to about 40 %), but never a deck gone black
    check(`${name}: the deck keeps its content (not darker than a quarter of before)`, lit > deckBefore * 0.25, `${deckBefore.toFixed(1)} -> ${lit.toFixed(1)}`);
    await win.screenshot({ path: path.join(shots, `preset-${i + 1}.png`), clip: deckBox ?? undefined });
  }
  await win.getByRole("button", { name: "Discard changes", exact: true }).click();
  await sleep(1200);
  check("Discard leaves the profile's effects as they were", JSON.stringify((await saved()).effects ?? null) === JSON.stringify(p0.effects ?? null));
  // every GPU layer compiled (2026-10-26: the eyes' shader never compiled and the deck only lacked them)
  const logText = fs.existsSync(path.join(dir, "logs")) ? fs.readdirSync(path.join(dir, "logs")).map((f) => fs.readFileSync(path.join(dir, "logs", f), "utf8")).join(" ") : "";
  const broken = [...new Set([...logText.matchAll(/GPU layer (\w+) does not compile/g)].map((m) => m[1]))];
  check("every GPU layer compiles (the test copy's log)", broken.length === 0, broken.join(", "));
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
