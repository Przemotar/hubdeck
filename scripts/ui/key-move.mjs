// Keys moved by dragging and page keys opened by a double click in the window (Discussions #3,
// 8bitcanvascode, 2026-10-19: "rearrange keys simply by dragging them", "double-clicking a page
// navigation key in the HubDeck editor could navigate to the corresponding page"). The mouse is
// the test copy's own (CDP input into its window, off screen): never the owner's cursor or focus.
// Then every page key in the owner's own profiles (the test copy has his data) is double-clicked.
// Screenshots in logs/key-move-*.png, in the dark and the light look.
//   node scripts/ui/key-move.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-key-move");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9475;
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
const controls = async (id = profile) => (await api(`profiles/${id}`)).controls ?? {};
const shownPage = async () => (await api("controls")).page ?? null;

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
  const hot = (keys, label) => ({ action: "core.hotkey", params: { keys, mode: "normal" }, look: { label } });
  const pg = await api(`profiles/${profile}/pages`, { name: "Moved here", id: "movetest" });
  const set = {
    "keys.0": hot("Ctrl+Alt+F1", "SRCA"),
    "keys.0:hold": hot("Ctrl+Alt+F3", "HOLDA"),
    "keys.1": null,
    "keys.1:hold": null,
    "keys.1:double": null,
    "keys.2": hot("Ctrl+Alt+F2", "TGTX"),
    "keys.3": { action: "core.page", params: { page: pg.id }, look: { label: "PAGEKEY" } },
    "keys.4": { action: "core.nextPage", params: {}, look: { label: "NEXTKEY" } },
    [`${pg.id}/keys.14`]: { action: "core.page", params: { page: "" }, look: { label: "BACKKEY" } },
  };
  const before = await controls();
  for (const [control, binding] of Object.entries(set)) {
    const r = await api(`profiles/${profile}/controls`, { control, binding });
    if (r.error) console.log(`${control}: ${r.error}`);
  }
  await api(`profiles/${profile}/activate`, {});
  check("the test keys are set", (await controls())["keys.3"]?.action === "core.page", `${Object.keys(before).length} controls before`);

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
  const keyAt = (i) => win.locator(`g.dk-ctl[data-kind="keys"][data-index="${i}"]`).first();
  const centre = (i) => ringCentre(keyAt(i));
  const dblclickKey = async (i) => win.mouse.dblclick(...(await centre(i)));
  const shot = async (name) => {
    for (const look of ["dark", "light"]) {
      await win.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
      await sleep(150);
      await win.screenshot({ path: path.join(root, "logs", `key-move-${name}-${look}.png`) });
    }
    await win.evaluate(() => document.documentElement.removeAttribute("data-theme"));
  };
  const drag = async (from, to, name) => {
    const [x0, y0] = await centre(from);
    const [x1, y1] = await centre(to);
    await win.mouse.move(x0, y0);
    await win.mouse.down();
    await win.mouse.move((x0 + x1) / 2, (y0 + y1) / 2, { steps: 5 });
    await win.mouse.move(x1, y1, { steps: 5 });
    if (name) await shot(name);
    const lifted = await keyAt(from).evaluate((el) => el.classList.contains("is-lifted"));
    const ringed = await keyAt(to).evaluate((el) => el.classList.contains("is-drop"));
    await win.mouse.up();
    await sleep(800);
    return { lifted, ringed };
  };
  const toast = async () => (await win.locator(".toast").first().textContent().catch(() => "")) ?? "";

  // 1. a key onto an empty key: it moves, press and hold together
  let r = await drag(0, 1, "1-dragging");
  check("while dragging the key fades and the key under the pointer is ringed", r.lifted && r.ringed, JSON.stringify(r));
  let c = await controls();
  check("dropped on an empty key it moves there", c["keys.1"]?.params?.keys === "Ctrl+Alt+F1" && !c["keys.0"], `keys.1 ${c["keys.1"]?.params?.keys}, keys.0 ${c["keys.0"]?.action ?? "empty"}`);
  check("its hold goes with it", c["keys.1:hold"]?.params?.keys === "Ctrl+Alt+F3" && !c["keys.0:hold"]);
  check("the window says Moved", /Moved/.test(await toast()), await toast());
  check("the moved key is selected", await keyAt(1).evaluate((el) => el.classList.contains("is-selected")));
  await shot("2-moved");

  // 2. onto a key that has something: they swap
  await drag(1, 2);
  c = await controls();
  check("dropped on a set key they swap", c["keys.2"]?.params?.keys === "Ctrl+Alt+F1" && c["keys.1"]?.params?.keys === "Ctrl+Alt+F2", `keys.2 ${c["keys.2"]?.params?.keys}, keys.1 ${c["keys.1"]?.params?.keys}`);
  check("the window says Swapped", /Swapped/.test(await toast()), await toast());

  // 3. Ctrl+Z undoes the swap in one step
  await win.keyboard.press("Control+z");
  await sleep(800);
  c = await controls();
  check("Ctrl+Z puts the swap back", c["keys.1"]?.params?.keys === "Ctrl+Alt+F1" && c["keys.2"]?.params?.keys === "Ctrl+Alt+F2");

  // 4. a click that barely moves only selects; a drag let go outside the keys does nothing
  const [x, y] = await centre(2);
  await win.mouse.move(x, y);
  await win.mouse.down();
  await win.mouse.move(x + 3, y + 2);
  await win.mouse.up();
  await sleep(500);
  c = await controls();
  check("a click with a 3 px wobble selects and moves nothing", (await keyAt(2).evaluate((el) => el.classList.contains("is-selected"))) && c["keys.2"]?.params?.keys === "Ctrl+Alt+F2");
  await win.mouse.move(x, y);
  await win.mouse.down();
  await win.mouse.move(x, 5, { steps: 8 });
  await win.mouse.up();
  await sleep(500);
  c = await controls();
  check("let go away from the keys: nothing moves", c["keys.2"]?.params?.keys === "Ctrl+Alt+F2" && c["keys.1"]?.params?.keys === "Ctrl+Alt+F1");
  check("and nothing stays faded", (await win.locator("g.dk-ctl.is-lifted, g.dk-ctl.is-drop").count()) === 0);

  // 5. double clicks on page keys
  await dblclickKey(3);
  await sleep(800);
  check("a double click on a page key opens its page", (await shownPage()) === pg.id, String(await shownPage()));
  await shot("3-page-opened");
  await dblclickKey(14);
  await sleep(800);
  check("a double click on the page's back key goes to the main page", (await shownPage()) === null, String(await shownPage()));
  await dblclickKey(4);
  await sleep(800);
  const firstPage = (await api(`profiles/${profile}`)).pages?.[0]?.id ?? null;
  check("a double click on Next page opens the next page", (await shownPage()) === firstPage, `${await shownPage()} (first page ${firstPage})`);
  await api(`profiles/${profile}/activate`, {});
  await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("page_set", { page: null }));
  await sleep(500);
  await dblclickKey(2);
  await sleep(800);
  check("a double click on another key does not change the page", (await shownPage()) === null, String(await shownPage()));

  // 6. every page key on the main page of the owner's own profiles
  const list = await api("profiles");
  let tried = 0;
  for (const p of list.profiles ?? list) {
    const full = await api(`profiles/${p.id}`);
    const keys = Object.entries(full.controls ?? {}).filter(([k, b]) => /^keys\.\d+$/.test(k) && b.action === "core.page");
    if (!keys.length || p.id === profile) continue;
    await api(`profiles/${p.id}/activate`, {});
    await sleep(1200);
    for (const [k, b] of keys) {
      await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("page_set", { page: null }));
      await sleep(400);
      await dblclickKey(Number(k.split(".")[1]));
      await sleep(700);
      const want = b.params?.page || null;
      const name = full.pages?.find((x) => x.id === want)?.name ?? "main";
      check(`${full.name}: ${b.look?.label || k} opens "${name}"`, (await shownPage()) === want, String(await shownPage()));
      tried++;
    }
  }
  console.log(`page keys of his own profiles tried: ${tried}`);
  // the longer hint under the deck, at the owner's window sizes
  for (const [w, h] of [[960, 640], [1280, 800]]) {
    await win.setViewportSize({ width: w, height: h });
    await sleep(400);
    const cut = (await clipped(win)).filter((c) => /Drag a key|double-click|Moved|Swapped/.test(c.text));
    check(`the hint is not cut at ${w} wide`, cut.length === 0, cut.map((c) => c.text).join(" | "));
  }
  check("no page errors", errors === 0, `${errors}`);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1000);
}
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
