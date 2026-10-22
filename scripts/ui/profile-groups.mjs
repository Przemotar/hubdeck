// Where profiles are shown, variant A (owner's pick, 2026-10-22: "1 a" to the mockups at
// https://claude.ai/artifact/H6ftEWxm4L6VHhiuRsn1Zu): the home profile is the first tab of the
// top bar with a thin line after it, and the list on the left is grouped into Home, Your profiles
// and For apps and games, with Ready for your apps folded. On the owner's own profiles; a test
// copy of its own (virtual modules, never the focus). Screenshots in logs/profile-groups-*.png
// at his window sizes.
//   node scripts/ui/profile-groups.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-profile-groups");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9485;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const shot = (name) => path.join(root, "logs", `profile-groups-${name}.png`);

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
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
  console.log(`exe: ${exe}`);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  for (const [w, h] of [
    [1600, 900],
    [1280, 800],
  ]) {
    await win.setViewportSize({ width: w, height: h });
    await sleep(2000);
    const s = await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("engine_state"));
    const home = s.profiles.find((p) => p.id === s.fallback)?.name;
    await win.getByRole("tab", { name: "Profiles" }).or(win.getByRole("button", { name: "Profiles" })).first().click();
    await sleep(500);
    const tabs = await win.locator(".topbar .tabs-row > button.tab:visible").allInnerTexts();
    check(`${w}x${h}: the home profile is the first tab`, tabs[0]?.includes(home), `${tabs.join(" | ")}`);
    check(`${w}x${h}: a line after it`, (await win.locator(".topbar .tabs-row > button.tab.is-home:visible").count()) === 1 || tabs.length === 1);
    const heads = await win.locator(".lib-profiles .plist-group .plist-head").allInnerTexts();
    check(`${w}x${h}: the list is grouped`, /Home/i.test(heads[0] ?? "") && heads.some((x) => /Your profiles/i.test(x)) && heads.some((x) => /apps and games/i.test(x)), heads.map((x) => x.replace(/\s+/g, " ")).join(" | "));
    const firstRow = await win.locator(".lib-profiles .plist-group").first().locator(".prow b").first().innerText();
    check(`${w}x${h}: home under Home`, firstRow === home, firstRow);
    const appsRows = await win.locator(".lib-profiles .plist-group").filter({ hasText: /apps and games/i }).locator(".prow .mono").allInnerTexts();
    check(`${w}x${h}: every row under For apps and games names its app`, appsRows.length > 0 && appsRows.every((x) => x.trim().length > 0), `${appsRows.length} rows`);
    const ready = win.locator("details.ready-apps");
    if (await ready.count()) {
      check(`${w}x${h}: Ready for your apps is folded`, !(await ready.evaluate((d) => d.open)));
      await ready.locator("summary").click();
      await sleep(300);
      check(`${w}x${h}: and opens with a click`, await ready.evaluate((d) => d.open));
      await win.screenshot({ path: shot(`${w}-ready-open`) });
      await ready.locator("summary").click();
    }
    // text that does not fit: a tab or a group title cut off
    const cut = await win.evaluate(() =>
      [...document.querySelectorAll(".topbar .tabs-row > button.tab, .lib-profiles .plist-head span")]
        .filter((e) => e.offsetParent && e.scrollWidth > e.clientWidth + 1)
        .map((e) => e.textContent.trim()),
    );
    check(`${w}x${h}: nothing cut`, cut.length === 0, cut.join(", "));
    await win.screenshot({ path: shot(`${w}`) });
    // a click on the home tab still only switches when another profile shows
    const other = s.profiles.find((p) => p.id !== s.fallback && !p.draft && !p.ready);
    await win.evaluate((id) => window.__TAURI_INTERNALS__.invoke("profile_activate", { id }), other.id);
    await sleep(800);
    await win.locator(".topbar .tabs-row > button.tab.is-home").click();
    await sleep(800);
    const after = await win.evaluate(() => window.__TAURI_INTERNALS__.invoke("engine_state"));
    check(`${w}x${h}: the home tab switches to home`, after.active === s.fallback && after.fallback === s.fallback);
  }
  // light look too
  await win.evaluate(() => document.documentElement.setAttribute("data-theme", "light"));
  await sleep(500);
  await win.screenshot({ path: shot("light") });
  check("no page errors", errors === 0, `${errors}`);
} catch (e) {
  check("the walk ran to the end", false, String(e).split("\n")[0]);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1000);
}
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
