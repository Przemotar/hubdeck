// Opening the Screensaver panel from its chip under the deck must not make the window jump (owner,
// 2026-10-30: "jak kikma przycisk screensavera … mi miga na chwile i cos dziwnego (jaky budyz deck)").
// On a test copy with his profiles (its window off screen, never the focus): every animation frame
// for 1.5 s after the click, where the deck stands and whether the panel is there; a frame with no
// panel on the right, or the deck moving, is the flash. Screenshots in logs/saver-open/.
//   node scripts/ui/saver-open.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-saver-open");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "saver-open");
const PORT = 9487;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "screens.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  await win.setViewportSize({ width: 1606, height: 1000 });
  await sleep(3000);
  await win.screenshot({ path: path.join(shots, "0-before.png") });

  for (const round of [1, 2]) {
    // the recorder runs in the page, so no frame is missed between two screenshots
    await win.evaluate(() => {
      const deck = () => document.querySelector("[data-guide='saver']")?.closest("section");
      const box = (el) => (el ? el.getBoundingClientRect() : null);
      window.__frames = [];
      const t0 = performance.now();
      const step = () => {
        const s = box(deck());
        const panel = document.querySelector("aside.inspector");
        window.__frames.push({
          t: Math.round(performance.now() - t0),
          deckW: s ? Math.round(s.width) : -1,
          panel: panel ? panel.className.includes("living-panel") ? "saver" : "other" : "none",
        });
        if (performance.now() - t0 < 1500) requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    });
    await win.locator("[data-guide='saver']").click();
    await sleep(80);
    await win.screenshot({ path: path.join(shots, `${round}-80ms.png`) });
    await sleep(1600);
    await win.screenshot({ path: path.join(shots, `${round}-open.png`) });
    const frames = await win.evaluate(() => window.__frames);
    const widths = [...new Set(frames.map((f) => f.deckW))];
    const panels = frames.map((f) => f.panel).filter((p, i, a) => i === 0 || p !== a[i - 1]);
    console.log(`round ${round}: ${frames.length} frames; panel: ${panels.join(" -> ")}; deck widths: ${widths.join(", ")}`);
    check(`round ${round}: the right panel never leaves while the Screensaver panel opens`, !panels.includes("none"), panels.join(" -> "));
    check(`round ${round}: the deck does not move`, widths.length === 1, widths.join(", "));
    check(`round ${round}: the Screensaver panel is open`, panels.at(-1) === "saver");
    // close it for the next round
    await win.locator("aside.inspector.living-panel button[aria-label]").first().click().catch(() => {});
    await sleep(800);
  }
  // a preset as the screensaver (owner, 2026-10-30: "presety dorzucamy jako screensavery tez"): its tile
  // previews it, the deck moves, a press only wakes it
  const shownNow = () => win.evaluate(() => window.__TAURI_INTERNALS__.invoke("saver_shown"));
  const api = (route, body) => {
    const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
    const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
    return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
  };
  await win.locator("[data-guide='saver']").click();
  await sleep(1000);
  const presetTiles = win.locator("aside.inspector.living-panel .saver-tiles").nth(1).locator(".dl-tile");
  check("the Screensaver panel lists the presets and the profile's own look", (await presetTiles.count()) >= 10, `${await presetTiles.count()} tiles`);
  await presetTiles.filter({ hasText: "Synthwave" }).first().click();
  await sleep(2000);
  const sh = await shownNow();
  check("a preset tile previews it as the screensaver", sh?.effect === "preset:neon-synthwave", JSON.stringify(sh));
  const seen = new Set();
  for (let n = 0; n < 6; n++) (seen.add(((await api("deck/keys"))?.jpeg ?? "").slice(-300)), await sleep(200));
  check("the preset moves on the deck as a screensaver", seen.size >= 4, `${seen.size} different pictures in 1.2 s`);
  await win.locator("aside.inspector.living-panel").screenshot({ path: path.join(shots, "2-presets.png") });
  await api("test/input", { kind: "keys", index: 7, gesture: "press" });
  await sleep(800);
  check("a press only wakes the deck", !(await shownNow()));
  await win.locator("aside.inspector.living-panel button[aria-label]").first().click().catch(() => {});
  await sleep(800);

  // a screensaver previewed from its tile, then Deck look: the preset tried shows, not the screensaver
  // (owner, 2026-10-30: "jak wchodze w living deck to wtedy dalej widze screensaver")
  await win.locator("[data-guide='saver']").click();
  await sleep(1000);
  await win.locator("aside.inspector.living-panel .dl-tile").first().click();
  await sleep(1500);
  check("a screensaver tile previews it on the deck", !!(await shownNow()));
  await win.getByRole("button", { name: /^Living deck/ }).first().click();
  await sleep(1500);
  await win.locator(".dl-tile", { hasText: "Synthwave" }).first().click();
  await sleep(1500);
  check("Deck look ends the screensaver: the preset tried is on the deck", !(await shownNow()), JSON.stringify(await shownNow()));
  await win.screenshot({ path: path.join(shots, "3-deck-look-after-saver.png") });
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
