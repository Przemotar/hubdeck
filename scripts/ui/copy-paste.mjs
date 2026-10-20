// Copy and paste of keys in the window, the way a fast user does it (Discussions #3, 8bitcanvascode,
// 2026-10-19: "Occasionally, a previously selected key gets copied instead of the intended one, or
// the copy operation doesn't work as expected"): click, Ctrl+C, click, Ctrl+V, click, Ctrl+V with no
// pause between them, then Delete on two keys and Ctrl+Z. A test copy of its own (virtual modules,
// no deck, its window off screen, never the focus).
//   node scripts/ui/copy-paste.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-copy-paste");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9474;
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
const controls = async () => (await api(`profiles/${profile}`)).controls ?? {};

// a key's own centre: its ring, not its box (the box of a key showing the deck's picture spans
// the whole panel, so a click at the box's centre lands on key 8)
const ringCentre = async (loc) => {
  const b = await loc.locator(".dk-hov").first().boundingBox();
  return [b.x + b.width / 2, b.y + b.height / 2];
};
const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  console.log(`exe: ${exe}`);
  // two keys to copy from, four to paste on, each with a label the window shows
  const hot = (keys, label) => ({ action: "core.hotkey", params: { keys, mode: "normal" }, look: { label } });
  const page = (label) => ({ action: "core.nextPage", params: {}, look: { label } });
  const set = { "keys.0": hot("Ctrl+Alt+F1", "SRCA"), "keys.1": hot("Ctrl+Alt+F2", "SRCB"), "keys.2": page("TGTW"), "keys.3": page("TGTX"), "keys.4": page("TGTY"), "keys.5": page("TGTZ") };
  for (const [control, binding] of Object.entries(set)) await api(`profiles/${profile}/controls`, { control, binding });
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
  const keyLoc = (label) => win.locator(`g.dk-ctl[aria-label$=" · ${label}"]`).first();
  const key = (label) => ({ click: async () => win.mouse.click(...(await ringCentre(keyLoc(label)))) });
  check("the window shows the six keys", (await win.locator('g.dk-ctl[aria-label*=" · SRC"], g.dk-ctl[aria-label*=" · TGT"]').count()) === 6);

  // copy A, paste on W, X and Y with no pause between the steps
  await key("SRCA").click();
  await win.keyboard.press("Control+c");
  for (const t of ["TGTW", "TGTX", "TGTY"]) {
    await key(t).click();
    await win.keyboard.press("Control+v");
  }
  const t0 = Date.now();
  let c = await controls();
  while (Date.now() - t0 < 3000 && !["keys.2", "keys.3", "keys.4"].every((k) => c[k]?.params?.keys === "Ctrl+Alt+F1")) c = await controls();
  const took = Date.now() - t0;
  for (const k of ["keys.2", "keys.3", "keys.4"]) check(`fast paste kept on ${k}`, c[k]?.params?.keys === "Ctrl+Alt+F1", `${c[k]?.action} ${c[k]?.params?.keys ?? ""}`);
  check("the three pastes landed within 1 s", took < 1000, `${took} ms`);
  check("the source key is untouched", c["keys.0"]?.params?.keys === "Ctrl+Alt+F1");

  // a click on B and Ctrl+C at once copies B, not A selected before
  await key("SRCB").click();
  await win.keyboard.press("Control+c");
  await key("TGTZ").click();
  await win.keyboard.press("Control+v");
  await sleep(800);
  c = await controls();
  check("Ctrl+C right after a click copies the clicked key", c["keys.5"]?.params?.keys === "Ctrl+Alt+F2", `${c["keys.5"]?.params?.keys}`);

  // Delete on two keys, fast, then Ctrl+Z brings the last one back
  const labelOf = async (k) => (await controls())[k]?.look?.label;
  await key(await labelOf("keys.0") ?? "SRCA").click();
  await win.keyboard.press("Delete");
  await sleep(300); // a person's pace between two keys
  await key("SRCB").click();
  await win.keyboard.press("Delete");
  await sleep(800);
  c = await controls();
  check("two fast deletes both cleared", !c["keys.0"] && !c["keys.1"], `keys.0 ${c["keys.0"]?.action ?? "empty"}, keys.1 ${c["keys.1"]?.action ?? "empty"}`);
  await win.keyboard.press("Control+z");
  await sleep(800);
  c = await controls();
  check("Ctrl+Z brings the last deleted key back", c["keys.1"]?.params?.keys === "Ctrl+Alt+F2" && !c["keys.0"], `keys.1 ${c["keys.1"]?.params?.keys ?? "empty"}`);
  check("no page errors", errors === 0, `${errors}`);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1000);
}
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
