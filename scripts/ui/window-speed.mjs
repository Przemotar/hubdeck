// How fast the window follows the deck (owner, 2026-10-26: "apka jakoś wolno działa i wolno renderuje
// to co się dzieje na urządzeniu"; #9, sotomskir: "2-5s lag between changing the action and the
// image appearing in the app"): a test copy of its own (his profiles, virtual modules, the window
// off screen, never the focus) times a change of action as a user makes it, to the new picture on
// the key in the window, and counts the window's pictures of a moving deck.
//   node scripts/ui/window-speed.mjs [path to hubdeck.exe]   # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] || path.join(root, "target", "test-build", "release", "hubdeck.exe");
const shots = process.argv[3];
const dir = path.join(os.tmpdir(), "hubdeck-window-speed");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9491;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "nudges.json", "pet.json", "taste.json"]) {
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
const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));

let browser;
try {
  console.log(`exe: ${exe}`);
  await api(`profiles/${profile}/controls`, { control: "keys.6", binding: { action: "core.nextPage", params: {}, look: { label: "OLDKEY", bg: "#b91c1c" } } });
  await api(`profiles/${profile}/activate`, {});
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(3000);
  const key = win.locator("g.dk-ctl").filter({ hasNot: win.locator('circle[r="88"]') }).nth(6);
  const cap = async () => {
    const b = await key.locator(".dk-hov").first().boundingBox();
    return win.screenshot({ clip: { x: b.x + 8, y: b.y + 8, width: b.width - 16, height: b.height - 16 } });
  };
  const apart = async (a, b) =>
    win.evaluate(async ([a, b]) => {
      const read = async (b64) => {
        const img = new Image();
        img.src = "data:image/png;base64," + b64;
        await img.decode();
        const c = document.createElement("canvas");
        [c.width, c.height] = [img.width, img.height];
        const g = c.getContext("2d");
        g.drawImage(img, 0, 0);
        return g.getImageData(0, 0, c.width, c.height).data;
      };
      const [x, y] = [await read(a), await read(b)];
      let sum = 0;
      for (let i = 0; i < x.length; i += 4) sum += (Math.abs(x[i] - y[i]) + Math.abs(x[i + 1] - y[i + 1]) + Math.abs(x[i + 2] - y[i + 2])) / 3;
      return Math.round((sum / (x.length / 4)) * 10) / 10;
    }, [a.toString("base64"), b.toString("base64")]);
  // 1. a change of action as a user makes it: the click on the item to the new picture on the key
  for (const name of ["Clock", "Pomodoro", "Clock", "Pomodoro"]) {
    const kb = await key.locator(".dk-hov").first().boundingBox();
    await win.mouse.click(kb.x + kb.width / 2, kb.y + kb.height / 2);
    await sleep(800);
    const change = win.getByRole("button", { name: "Change", exact: true }).first();
    if (!(await change.isVisible().catch(() => false))) {
      await win.screenshot({ path: path.join(root, "logs", "window-speed-nochange.png") });
      console.log("no Change button (shot in logs/window-speed-nochange.png)");
      break;
    }
    await change.click();
    await sleep(800);
    const item = win.getByRole("button", { name, exact: true }).first();
    if ((await item.count()) !== 1) {
      console.log(`no "${name}" in the library`);
      await win.keyboard.press("Escape");
      await sleep(800);
      continue;
    }
    await item.scrollIntoViewIfNeeded();
    const before = await cap();
    const t0 = Date.now();
    await item.click();
    const clicked = Date.now() - t0;
    let ms = null;
    while (Date.now() - t0 < 10000) {
      if ((await apart(before, await cap())) > 8) {
        ms = Date.now() - t0;
        break;
      }
      await sleep(30);
    }
    console.log(`action -> ${name}: click took ${clicked} ms, the key's new picture in the window after ${ms ?? "> 10000"} ms`);
    await win.keyboard.press("Escape");
    await sleep(1500);
  }
  // 2. the window's picture of a moving deck: the living deck on, frames counted in the window
  const p = await api(`profiles/${profile}`);
  await win.evaluate((profile) => window.__TAURI_INTERNALS__.invoke("profile_save", { profile }), { ...p, effects: { layers: [{ kind: "snow" }, { kind: "fireflies" }] } });
  await sleep(4000);
  const rate = () => win.evaluate(() => window.__TAURI_INTERNALS__.invoke("deck_rate"));
  const changes = async (seconds) => {
    let last = await cap();
    let n = 0;
    const t0 = Date.now();
    while (Date.now() - t0 < seconds * 1000) {
      const now = await cap();
      if ((await apart(last, now)) > 0.3) n++;
      last = now;
    }
    return n / seconds;
  };
  console.log(`living deck on: the deck gets ${JSON.stringify(await rate())} (pictures a second, CPU)`);
  console.log(`living deck on: the window's key changed ${(await changes(10)).toFixed(1)} times a second (sampled as fast as screenshots go)`);
  await sleep(6000);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1500);
}
const logs = fs.readdirSync(dir, { recursive: true }).filter((f) => String(f).endsWith(".log"));
for (const f of logs) {
  const lines = fs.readFileSync(path.join(dir, f), "utf8").split("\n").filter((l) => /window: smoothness|deck: /.test(l));
  console.log(`${f}:\n${lines.slice(-8).join("\n")}`);
}
console.log(`screen clean: ${JSON.stringify(screenClean(dir))}`);
fs.rmSync(dir, { recursive: true, force: true });
