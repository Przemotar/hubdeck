// The screensaver's panel (owner, 2026-10-26, a voice note: "oddzielny panel do ustawienia wygaszacza
// dla danego profilu … z ustawieniem czasu"; his "1a": one shared saver and delay, a profile may
// have its own): as a user does it in the window, the Screensaver chip under the deck, a scene for
// all profiles, its preview on the deck (the keys' picture, the dial's clock, the log), the
// profile's own saver and its delay, back to the shared one, the dial and the lights, Settings >
// Screens pointing to the panel; then his Fun page's "Screensaver" key, which now goes through the
// scenes too. A test copy of its own with the owner's profiles (virtual modules, its window off
// screen, never the focus).
//   node scripts/ui/saver-panel.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-saver-panel");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "saver-panel");
const PORT = 9493;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "nudges.json", "pet.json"]) {
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
const logText = () => {
  const f = path.join(dir, "logs", "hubdeck.log");
  return fs.existsSync(f) ? fs.readFileSync(f, "utf8") : "";
};
const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
// the keys' picture the deck got, as a PNG to look at, and how bright it is (0..255)
const deckKeys = async (name) => {
  const f = await api("deck/keys");
  if (!f?.jpeg) return null;
  const jpeg = Buffer.from(f.jpeg, "base64");
  fs.writeFileSync(path.join(shots, `${name}.jpg`), jpeg);
  return jpeg;
};
let browser;
try {
  const p0 = await saved();
  console.log(`exe: ${exe}, profile ${p0.name} (${profile}); its own saver before: ${JSON.stringify(p0.saver ?? null)}`);
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
  await sleep(3000);
  const screens = () => win.evaluate(() => window.__TAURI_INTERNALS__.invoke("screens_get"));

  // --- the chip under the deck
  const chip = win.locator('button.dk-chip[data-guide="saver"]');
  check("a Screensaver chip under the deck, beside Living deck", (await chip.count()) === 1 && (await win.locator('button.dk-chip[data-guide="living"]').count()) === 1);
  const chipText = (await chip.innerText()).replace(/\s+/g, " ");
  check("the chip says it is off (his saver is off)", /Screensaver\s*Off/i.test(chipText), chipText);
  await chip.click();
  await sleep(900);
  const panel = win.locator("aside.living-panel");
  check("the chip opens the Screensaver panel in the inspector's place", (await panel.getByText("Screensaver", { exact: true }).count()) >= 1, await panel.locator(".panel-head").innerText().catch(() => ""));
  await panel.screenshot({ path: path.join(shots, "1-panel.png") });
  const forProfile = panel.getByRole("radiogroup", { name: "This profile" });
  check("Same as all profiles is marked", (await forProfile.getByRole("radio", { name: "Same as all profiles" }).getAttribute("aria-checked")) === "true");

  // --- a scene for all profiles
  await panel.getByRole("radio", { name: /Aurora/ }).click();
  await sleep(900);
  let s = await screens();
  check("Aurora is the shared screensaver", s.saver === "aurora", JSON.stringify({ saver: s.saver, after: s.saverAfterMinutes }));
  check("the profile keeps no saver of its own", !(await saved()).saver);
  check("Aurora's tile is marked", (await panel.getByRole("radio", { name: /Aurora/ }).getAttribute("aria-checked")) === "true");
  check("the chip names it with its delay", /Aurora.*5 minutes/.test((await chip.innerText()).replace(/\s+/g, " ")), (await chip.innerText()).replace(/\s+/g, " "));
  await panel.getByLabel("Starts after").selectOption("10");
  await sleep(700);
  check("Starts after 10 minutes, for all", (await screens()).saverAfterMinutes === 10);

  // --- the preview on the deck: the keys, the dial's clock and the lights
  const before = await deckKeys("2-before");
  await panel.getByRole("button", { name: "Preview on the deck" }).click();
  await sleep(4000);
  const during = await deckKeys("3-aurora-on-deck");
  check("the log says the screensaver is on with Aurora", /screensaver on \(aurora\)/.test(logText()));
  check("the deck's picture changed to the scene", !!during && !!before && !during.equals(before), `${before?.length} -> ${during?.length} bytes`);
  await sleep(1000);
  const later = await deckKeys("3b-aurora-a-second-later");
  check("the scene moves on the deck", !!later && !later.equals(during));
  await win.screenshot({ path: path.join(shots, "4-window-during-preview.png") });
  check("no error banner in the window", (await win.locator(".banner.is-error, .banner.error").count()) === 0);

  // --- its own for this profile
  await forProfile.getByRole("radio", { name: "Its own" }).click();
  await sleep(800);
  let p = await saved();
  check("Its own: the profile takes the shared one as its start", p.saver?.effect === "aurora" && p.saver?.afterMinutes === 10, JSON.stringify(p.saver));
  await panel.getByRole("radio", { name: /Big clock/ }).click();
  await sleep(800);
  await panel.getByLabel("Starts after").selectOption("2");
  await sleep(800);
  p = await saved();
  s = await screens();
  check("the profile's own: Big clock after 2 minutes; the shared stays Aurora after 10", p.saver?.effect === "clock" && p.saver?.afterMinutes === 2 && s.saver === "aurora" && s.saverAfterMinutes === 10, `${JSON.stringify(p.saver)} / ${s.saver} ${s.saverAfterMinutes}`);
  check("the chip shows the profile's own", /Big clock.*2 minutes/.test((await chip.innerText()).replace(/\s+/g, " ")), (await chip.innerText()).replace(/\s+/g, " "));
  await panel.getByRole("button", { name: "Preview on the deck" }).click();
  await sleep(3000);
  await deckKeys("5-clock-on-deck");
  check("the clock's preview takes the place of Aurora's", /screensaver (on|now) \(clock\)/.test(logText()));
  await panel.screenshot({ path: path.join(shots, "6-own-clock.png") });

  // --- the dial and the lights, for all profiles
  await panel.getByLabel("The dial meanwhile").selectOption("off");
  await panel.getByLabel("The lights meanwhile").selectOption("same");
  await sleep(800);
  s = await screens();
  check("the dial and the lights kept for all", s.saverDial === "off" && s.saverLights === "same", `${s.saverDial} ${s.saverLights}`);
  await panel.getByLabel("The dial meanwhile").selectOption("clock");
  await panel.getByLabel("The lights meanwhile").selectOption("scene");
  await sleep(800);

  // --- back to the shared one
  await forProfile.getByRole("radio", { name: "Same as all profiles" }).click();
  await sleep(800);
  check("Same as all profiles drops the profile's own", !(await saved()).saver);
  check("Aurora is marked again", (await panel.getByRole("radio", { name: /Aurora/ }).getAttribute("aria-checked")) === "true");

  // --- text that fits, in his window sizes, light and dark
  for (const theme of ["dark", "light"]) {
    await win.evaluate((t) => document.documentElement.setAttribute("data-theme", t), theme);
    for (const [w, h] of [[960, 640], [1280, 800], [1600, 1000]]) {
      await win.setViewportSize({ width: w, height: h });
      await sleep(500);
      // the top bar's profile tab is not this panel's (cut at 960 before it, a known item)
      const cut = (await clipped(win)).filter((x) => x.text !== p0.name).filter((x) => /Screensaver|profile|Aurora|Aquarium|Lava|Fireplace|Snowy|Summer|Neon|Haunted|Big clock|Code rain|Night sky|Waves|Starts|Preview|dial|lights|Screens turn/i.test(x.text));
      check(`nothing of the panel cut at ${w} wide, ${theme}`, cut.length === 0, cut.map((x) => x.text).join(" | "));
    }
    await win.setViewportSize({ width: 1280, height: 800 });
    await sleep(400);
    await panel.screenshot({ path: path.join(shots, `7-panel-${theme}.png`) });
  }
  await win.evaluate(() => document.documentElement.removeAttribute("data-theme"));

  // --- Settings > Screens points to the panel
  await panel.getByRole("button", { name: "Close" }).first().click().catch(() => {});
  await sleep(500);
  await win.getByRole("button", { name: "Settings" }).first().click();
  await sleep(900);
  await win.getByRole("button", { name: "Lights and screens" }).first().click();
  await sleep(700);
  const moved = win.getByText("The screensaver is set under the deck", { exact: false });
  check("Settings > Screens says where the screensaver went", (await moved.count()) === 1);
  if ((await moved.count()) === 1) {
    await moved.scrollIntoViewIfNeeded();
    await win.screenshot({ path: path.join(shots, "8-settings.png") });
    await win.getByRole("button", { name: "Open", exact: true }).first().click();
    await sleep(900);
    check("its Open button opens the Screensaver panel", (await win.locator('aside.living-panel select#saver-after').count()) === 1);
  }

  // --- his Fun page's "Screensaver" key: now through the scenes too
  await api("test/input", { kind: "keys", index: 0, gesture: "press" }); // a touch ends a preview still running
  await sleep(1200);
  await api("test/input", { kind: "keys", index: 14, gesture: "press" }); // to Fun
  await sleep(1200);
  const pressed = [];
  for (let i = 0; i < 4; i++) {
    await api("test/input", { kind: "keys", index: 11, gesture: "press" });
    await sleep(1500);
    pressed.push((logText().match(/screensaver (on|now) \(([a-z]+)\)/g) ?? []).pop());
    await api("test/input", { kind: "keys", index: 0, gesture: "press" }); // a touch ends it
    await sleep(1200);
  }
  check("his Fun page's Screensaver key shows one after another", new Set(pressed).size >= 3, pressed.join(", "));
  check("no page errors", errors === 0, String(errors));
  const warn = logText().split("\n").filter((l) => /ERROR|panicked|does not compile/.test(l));
  check("no errors in the log", warn.length === 0, warn.slice(-3).join(" | "));
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1500);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
fs.rmSync(dir, { recursive: true, force: true });
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}  (shots: ${shots})`);
process.exit(passed === results.length ? 0 : 1);
