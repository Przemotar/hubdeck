// A key's picture in the window follows a change of its action (issue #9, sotomskir, 2026-10-23,
// macOS: "The action image appeared on the hardware, but in the app I still see the old action
// image on the button", until another profile and back). The window shows each key's cut of the
// picture the hardware got, through an SVG <use> of one <image>; WebKit does not repaint a <use>
// when that image's address changes (scratch test in Playwright's WebKit, 2026-10-23).
// A test copy of its own (the owner's profiles, virtual modules, no deck, never the focus): select
// a key, pick another action in the library as a user does, and compare the key's cap in the
// window before and after, and with what the deck got.
//   node scripts/ui/key-image.mjs [path to hubdeck.exe] [shots dir]   # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] || path.join(root, "target", "test-build", "release", "hubdeck.exe");
const shots = process.argv[3];
const dir = path.join(os.tmpdir(), "hubdeck-key-image");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9477;
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
  // the key starts as a text key the window draws from the deck's picture
  await api(`profiles/${profile}/controls`, { control: "keys.6", binding: { action: "core.nextPage", params: {}, look: { label: "OLDKEY", bg: "#b91c1c" } } });
  await api(`profiles/${profile}/activate`, {});
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(2500);
  const key = win.locator("g.dk-ctl").filter({ hasNot: win.locator('circle[r="88"]') }).nth(6);
  const cap = async () => {
    const b = await key.locator(".dk-hov").first().boundingBox();
    return win.screenshot({ clip: { x: b.x + 8, y: b.y + 8, width: b.width - 16, height: b.height - 16 } });
  };
  // how far two pictures of the cap are apart: the mean difference of their pixels (0..255)
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
  const mirrored = () => key.locator("use").count();
  const before = await cap();
  await sleep(1000);
  const still = await apart(before, await cap());
  check("the key's cap stands still before (so a change is the new action)", still < 2, `${still} apart, ${await mirrored()} <use>`);
  if (shots) fs.mkdirSync(shots, { recursive: true }), fs.writeFileSync(path.join(shots, "key-image-before.png"), before);

  // as the user: click the key, Change, pick another action in "What should it do?"
  const b = await key.locator(".dk-hov").first().boundingBox();
  await win.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
  await sleep(800);
  await win.getByRole("button", { name: "Change", exact: true }).first().click();
  await sleep(800);
  const item = win.getByRole("button", { name: "Clock", exact: true }).first();
  const found = (await item.count()) === 1;
  check("What should it do? offers Clock", found);
  if (found) {
    await item.scrollIntoViewIfNeeded();
    await item.click();
    await sleep(2500);
    const saved = (await api(`profiles/${profile}`)).controls?.["keys.6"];
    check("the key's action changed", saved?.action === "core.clock", saved?.action);
    const after = await cap();
    const moved = await apart(before, after);
    check("the window's key shows the new action", moved > 8, `${moved} apart from before`);
    if (shots) fs.writeFileSync(path.join(shots, "key-image-after.png"), after);
  }
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1500);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
