// The Deck look view (session 36; owner, 2026-10-28: "edycja może powinna być w dedykowanym oknie";
// mockups v1-v5 at https://claude.ai/artifact/YUqpk7baMNg5sePA8e7w8e): as a user does it in the
// window, on a test copy with the owner's profiles (virtual modules, its window off screen, never
// the focus): the Living deck chip opens the view; a preset tried shows on the deck while the
// profile stays as it was; no Elements tab, the Keys section's style and light (one at a time, kept
// across presets, the light's settings under it); the icons hidden on the deck; Apply
// saves, Discard and Back with changes put it back; a look saved as his own comes back after a
// restart of the view. Screenshots of each step in logs/deck-look/, cut text checked.
//   node scripts/ui/deck-look.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-deck-look");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "deck-look");
const PORT = 9486;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
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
// the deck's picture: how far two pictures are apart (mean of a coarse grid of bytes)
const picture = async () => (await api("deck/keys"))?.jpeg ?? "";
const apart = (a, b) => {
  if (!a || !b) return 0;
  const x = Buffer.from(a, "base64"), y = Buffer.from(b, "base64");
  let d = 0, n = 0;
  for (let i = 600; i < Math.min(x.length, y.length); i += 97) (d += Math.abs(x[i] - y[i])), n++;
  return n ? d / n : 0;
};

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  const before = await saved();
  console.log(`exe: ${exe}, profile ${before.name}; effects before: ${JSON.stringify(before.effects?.theme ?? null)}`);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  await win.setViewportSize({ width: 1606, height: 1000 });
  await sleep(2500);
  const shot = async (name) => win.screenshot({ path: path.join(shots, `${name}.png`) });
  const cut = async (step) => {
    const c = (await clipped(win)).filter((x) => x.text);
    check(`nothing cut: ${step}`, c.length === 0, c.map((x) => x.text).join(" | "));
  };

  // 1. the chip under the deck opens the view
  await win.getByRole("button", { name: /^Living deck/ }).first().click();
  await sleep(1200);
  const view = win.locator(".studio.dl");
  check("the Living deck chip opens the Deck look view", await view.isVisible());
  check("the view shows the deck's 15 key windows live", (await win.locator(".dl-hit").count()) === 15);
  await shot("1-open");
  await cut("open");

  // 1b. a key whose server does not answer (his trash key: Music Toolkit off) leaves the deck moving
  // (owner, 2026-10-28: "jak naciskam przycisk kosza … zawiesza mi na chwilę deck")
  const t0 = Date.now();
  await api("test/input", { kind: "keys", index: 7, gesture: "press" });
  const seen = new Set();
  for (let n = 0; n < 12; n++) (seen.add((await picture()).slice(-400)), await sleep(150));
  check("a key calling a server that is off does not stop the deck", seen.size >= 6, `${seen.size} different pictures in 1.8 s, the press answered in ${Date.now() - t0 - 1800} ms`);

  // 1b. the preview stands as the keys module does (owner, 2026-10-30: "jak się weźmie w pion decka to nie
  // przekręca się podgląd"): his placements have the keys turned a quarter
  const placements = JSON.parse(fs.readFileSync(path.join(dir, "placements.json"), "utf8")).placements ?? {};
  const keysTurn = Object.entries(placements).find(([k]) => k.startsWith("keys@"))?.[1]?.rotation ?? 0;
  const tall = (await win.locator(".dl-panel.is-upright").count()) === 1;
  check("the preview is turned with the keys module", tall === (keysTurn % 180 === 90), `keys turned ${keysTurn}, preview ${tall ? "upright" : "lying"}`);
  await shot("1b-preview-turn");

  // 2. a preset tried: the deck changes, the profile does not
  const pic0 = await picture();
  // a preset the profile does not wear already (his profile wore Lantern Hollow on 2026-10-30)
  const PICK = (before.effects?.theme ?? "") === "halloween-hollow" ? "Synthwave" : "Lantern Hollow";
  await win.locator(".dl-tile", { hasText: PICK }).first().click();
  await sleep(2500);
  const pic1 = await picture();
  check("a preset clicked shows on the deck", apart(pic0, pic1) > 2, `apart ${apart(pic0, pic1).toFixed(1)}`);
  check("the profile stays as it was until Apply", JSON.stringify((await saved()).effects ?? null) === JSON.stringify(before.effects ?? null));
  check("Apply is offered once something changed", await win.getByRole("button", { name: "Apply", exact: true }).isEnabled());
  await shot("2-preset");

  // 2b. the A/B branches as test presets in a test build (owner, 2026-10-29: "wyniki wrzucić do apki")
  // any trial slot (Tokyo Rain was one until it was built in, 2026-10-30)
  const trial = win.locator(`.dl-tile[title$="(próba)"]`);
  if (await trial.count()) {
    const before2 = await picture();
    await trial.first().click();
    await sleep(3000);
    check("a test preset (A/B branch) shows on the deck", apart(before2, await picture()) > 2, "");
    await shot("2b-trial");
    await win.locator(".dl-tile", { hasText: PICK }).first().click();
    await sleep(1500);
  } else console.log("note: no trial slot in this build (no x-*.wgsl in the shader folder), the trial step skipped");

  // 3. no Elements tab outside the dev app (owner, 2026-10-29: "same presety (plus keystyle i ewentualnie
  // light on the icons …)", "1 a"): the Keys section under the presets has Style and Light on the icons
  check("no Elements tab: presets and keys only", (await win.getByRole("tab", { name: /^Elements/ }).count()) === 0);
  const keys = win.locator(".dl-keys");
  check("the Keys section shows Style and Light on the icons", (await keys.getByRole("radiogroup").count()) === 2);
  const style = keys.getByRole("radiogroup", { name: "Style" });
  const lights = keys.getByRole("radiogroup", { name: "Light on the icons" });
  await style.locator(".dl-tile", { hasText: /^Film/ }).click();
  await sleep(1500);
  const filmKeys = Object.entries((await saved()).controls).filter(([id, c]) => /^keys\.\d+$/.test(id) && c.look?.skin === "film").length;
  check("a style clicked dresses the page's keys at once", filmKeys > 0, `${filmKeys} keys in film`);
  const pic2 = await picture();
  await lights.locator(".dl-tile", { hasText: /^Holo foil/ }).click();
  await sleep(2000);
  check("a light clicked is picked", (await lights.locator(".dl-tile", { hasText: /^Holo foil/ }).getAttribute("class")).includes("is-on"));
  check("the light shows on the deck", apart(pic2, await picture()) > 1, "");
  check("the preset stays picked with a light", (await win.locator(".dl-tile.is-sel", { hasText: PICK }).count()) === 1);
  await lights.locator(".dl-tile", { hasText: /^Lamp/ }).click();
  await sleep(600);
  check("the picked light's settings show under it", await keys.locator(".dl-insp").isVisible());
  check("one light at a time", (await lights.locator(".dl-tile.is-on").count()) === 1);
  await shot("3-keys");
  await cut("keys");

  // 4. the light stays when the preset changes (a preset without its own light)
  await win.locator(".dl-tile", { hasText: "code rain" }).first().click();
  await sleep(1200);
  check("the light stays across presets", (await lights.locator(".dl-tile.is-on", { hasText: /^Lamp/ }).count()) === 1);
  await shot("4-other-preset");

  // 5. the icons hidden on the deck while the view is open
  const withIcons = await picture();
  await win.getByRole("button", { name: "Icons shown" }).click();
  await sleep(1500);
  const bare = await picture();
  check("hiding the icons changes the deck", apart(withIcons, bare) > 2, `apart ${apart(withIcons, bare).toFixed(1)}`);
  await win.getByRole("button", { name: "Icons hidden" }).click();
  await sleep(800);
  await shot("5-icons");

  // 6. saved as his own, then Apply
  await win.getByRole("button", { name: "Save as mine" }).click();
  await sleep(400);
  await win.locator(".dl-pop input[type=text]").fill("Walk look");
  await win.getByRole("button", { name: "Save as new" }).click();
  await sleep(800);
  check("his own look is kept in looks.json", fs.existsSync(path.join(dir, "looks.json")) && fs.readFileSync(path.join(dir, "looks.json"), "utf8").includes("Walk look"));
  await win.getByRole("button", { name: "Apply", exact: true }).click();
  await sleep(1000);
  const applied = await saved();
  check("Apply saves the look in the profile", applied.effects?.theme === "matrix-rain" && applied.effects?.layers?.some((l) => l.kind === "light"), JSON.stringify(applied.effects?.layers?.map((l) => l.kind)));

  // 7. Back with changes asks; Discard puts it back
  await win.locator(".dl-tile", { hasText: "Lantern Hollow" }).first().click();
  await sleep(600);
  await win.getByRole("button", { name: "Back to the deck" }).click();
  await sleep(400);
  check("Back with changes asks first", await win.locator(".dl-leave").isVisible());
  await shot("6-leave");
  await win.locator(".dl-leave").getByRole("button", { name: "Discard changes" }).click();
  await sleep(1200);
  check("Discard leaves the profile as applied", JSON.stringify((await saved()).effects) === JSON.stringify(applied.effects));
  check("the view closes back to the deck", !(await view.isVisible()));

  // 8. the view again: his own look is there
  await win.getByRole("button", { name: /^Living deck/ }).first().click();
  await sleep(1200);
  check("his own look is offered under Mine", (await win.locator(".dl-tile", { hasText: "Walk look" }).count()) === 1);
  await shot("7-mine");
  for (const w of [1280, 960]) {
    await win.setViewportSize({ width: w, height: 800 });
    await sleep(500);
    await shot(`8-${w}`);
    await cut(`${w} wide`);
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
