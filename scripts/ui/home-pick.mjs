// Picking a profile only switches to it; home is set with its own button (owner, 2026-10-22: "nie
// podoba mi się to, że zaznaczenie ustawia dany panel jako home a nie po prostu na niego
// przełącza", and his "1 a": an app's profile gives back to the profile picked last). On the
// owner's own profiles: a click in the list and in the top bar, his Counter-Strike 2 profile
// taking over and giving back, "Set as home" and back, Auto-switch off, and a restart that starts
// on home. A test copy of its own (virtual modules, never the focus; the app in front is faked
// through the test API). Screenshots in logs/home-pick-*.png.
//   node scripts/ui/home-pick.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-home-pick");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9483;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const shot = (name) => path.join(root, "logs", `home-pick-${name}.png`);

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = true;
settings.pinned = false;
settings.autoMade = [];
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};

let app, browser, win, errors = 0;
async function launch() {
  fs.rmSync(path.join(dir, "api-http-port"), { force: true });
  app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
  for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
  await sleep(1500);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  win = undefined;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  await win.setViewportSize({ width: 1600, height: 900 });
  await sleep(2500);
}
async function quit() {
  await browser?.close().catch(() => {});
  app?.kill();
  await sleep(1500);
}
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
const state = () => win.evaluate(() => window.__TAURI_INTERNALS__.invoke("engine_state"));
const front = async (exe) => (await api("test/foreground", { exe }), await sleep(1200));
const row = (name) => win.locator(".lib-profiles .prow", { has: win.locator("b", { hasText: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`) }) }).first();
const homeTag = async (name) => (await row(name).locator(".tag", { hasText: /^home$/ }).count()) === 1;

try {
  console.log(`exe: ${exe}`);
  await launch();
  let s = await state();
  const nameOf = (id) => s.profiles.find((p) => p.id === id)?.name;
  const home = s.fallback;
  const cs2 = s.profiles.find((p) => p.apps.some((a) => a.toLowerCase() === "cs2.exe"));
  const other = s.profiles.find((p) => p.id !== home && !p.draft && !p.ready && p.apps.length === 0 && p.name === "Song sorter") ?? s.profiles.find((p) => p.id !== home && !p.draft && !p.ready && p.apps.length === 0);
  console.log(`home: ${nameOf(home)}, picked in the walk: ${other?.name}, game: ${cs2?.name}`);
  check("starts on home", s.active === home, nameOf(s.active));
  await win.getByRole("tab", { name: "Profiles" }).or(win.getByRole("button", { name: "Profiles" })).first().click();
  await sleep(500);

  // 1. a click in the list only switches
  await row(other.name).locator(".prow-main").click();
  await sleep(1200);
  s = await state();
  check("a click in the list switches to it", s.active === other.id, nameOf(s.active));
  check("and home stays", s.fallback === home && (await homeTag(nameOf(home))) && !(await homeTag(other.name)), nameOf(s.fallback));
  await win.screenshot({ path: shot("1-picked") });

  // 2. the game takes over and gives back to the profile picked last (owner's 1a)
  if (cs2) {
    await front("cs2.exe");
    s = await state();
    check("Counter-Strike 2 in front: its profile", s.active === cs2.id, nameOf(s.active));
    await front("notepad.exe");
    s = await state();
    check("the game leaves the front: back to the profile picked last", s.active === other.id, nameOf(s.active));
  } else check("the owner's Counter-Strike 2 profile is in the copy", false);

  // 3. a click on a tab in the top bar only switches too
  const tab = win.locator(".topbar button.tab:visible").filter({ hasText: nameOf(home) }).first();
  check("the home profile has a tab in the top bar", (await tab.count()) === 1);
  {
    await tab.click();
    await sleep(1000);
    s = await state();
    check("a tab in the top bar switches", s.active === home, nameOf(s.active));
  }
  await row(other.name).locator(".prow-main").click();
  await sleep(1000);

  // 4. Set as home, from the row's tools
  await row(other.name).hover();
  const setHome = row(other.name).getByRole("button", { name: /Set as home/ });
  check("the row has Set as home", (await setHome.count()) === 1);
  await setHome.click();
  await sleep(1000);
  s = await state();
  check("Set as home makes it home", s.fallback === other.id && (await homeTag(other.name)) && !(await homeTag(nameOf(home))), nameOf(s.fallback));
  check("and the home row has no Set as home", (await row(other.name).getByRole("button", { name: /Set as home/ }).count()) === 0);
  await win.screenshot({ path: shot("2-set-home") });
  await row(nameOf(home)).hover();
  await row(nameOf(home)).getByRole("button", { name: /Set as home/ }).click();
  await sleep(1000);
  s = await state();
  check("and back", s.fallback === home, nameOf(s.fallback));

  // 5. Auto-switch off keeps home
  await win.locator(".auto-switch input").click();
  await sleep(800);
  s = await state();
  check("Auto-switch off keeps home", s.pinned && s.fallback === home, `${nameOf(s.fallback)}, showing ${nameOf(s.active)}`);
  await win.locator(".auto-switch input").click();
  await sleep(800);

  // 6. a restart starts on home, not on the profile picked last
  s = await state();
  check("before the restart the deck shows the profile picked last", s.active === other.id, nameOf(s.active));
  await quit();
  await launch();
  s = await state();
  check("after a restart it starts on home", s.active === home && s.fallback === home, nameOf(s.active));
  await win.screenshot({ path: shot("3-restarted") });
  check("no page errors", errors === 0, `${errors}`);
} catch (e) {
  check("the walk ran to the end", false, String(e).split("\n")[0]);
  await win?.screenshot({ path: shot("failed") }).catch(() => {});
} finally {
  await quit();
}
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
