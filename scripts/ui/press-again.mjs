// A second press on a scene that shows one press plays at once (owner, 2026-10-30: "przycisk nacisnąć i
// sekundę później nacisnąć i powinien być ten sam efekt powtórzony"): on a test copy, Tokyo Rain tried on
// the deck, key 0 pressed, a second later key 14; the picture over key 14 must change within 0.5 s.
//   node scripts/ui/press-again.mjs [path to hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-press-again");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9495" }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
let ok = false;
let browser;
try {
  browser = await chromium.connectOverCDP("http://127.0.0.1:9495");
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await sleep(2000);
  for (const kind of ["tokyorain", "hollowhill", "lanternhollow"]) {
    await win.evaluate((k) => window.__TAURI_INTERNALS__.invoke("effects_try", { effects: { layers: [{ kind: k }] }, bare: true }), kind);
    await sleep(2500);
    await api("effects/press", { key: 0 });
    await sleep(1000);
    // the pictures as pixels (JPEG bytes shift with any change), the bottom right key's window: how much
    // it changes in 0.5 s without a press, then with key 14 pressed
    // Lantern Hollow's press is a gust over its six candles, wherever they stand: the whole picture there
    const whole = kind === "lanternhollow";
    const keyArea = async () => {
      const j = (await api("deck/keys")).jpeg;
      return win.evaluate(async ([b64, whole]) => {
        const img = new Image();
        img.src = "data:image/jpeg;base64," + b64;
        await img.decode();
        const c = document.createElement("canvas");
        c.width = img.width; c.height = img.height;
        const g = c.getContext("2d");
        g.drawImage(img, 0, 0);
        const d = whole ? g.getImageData(0, 0, img.width, img.height).data : g.getImageData(Math.round(img.width * 0.8), Math.round(img.height * 0.66), Math.round(img.width * 0.2), Math.round(img.height * 0.34)).data;
        let s = 0;
        for (let i = 0; i < d.length; i += 4) s += d[i] + d[i + 1] + d[i + 2];
        return s / (d.length / 4) / 3;
      }, [j, whole]);
    };
    // Hollow Hill's and Lantern Hollow's press is a ring that runs out from the key (240 px a second):
    // looked at 0.2 s after it
    const q0 = await keyArea();
    await sleep(200);
    const q1 = await keyArea();
    await api("effects/press", { key: 14 });
    await sleep(whole ? 150 : 200);
    const q2 = await keyArea();
    const calm = Math.abs(q1 - q0), pressed = Math.abs(q2 - q1);
    const pass = pressed > Math.max(3, calm * 3);
    console.log(`${pass ? "PASS" : "FAIL"}  ${kind}: the second press a second later shows on key 14 (brightness change ${pressed.toFixed(1)} vs ${calm.toFixed(1)} without a press)`);
    ok = pass && (ok || kind === "tokyorain");
  }
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  spawn("taskkill", ["/F", "/T", "/PID", String(app.pid)]);
  await sleep(1500);
}
console.log(screenClean(dir));
