// A ready-made profile is tried on the deck before it is kept (owner's 1a, 2026-10-20: picking
// Photoshop only to look at it added it and made it home). New profile > Ready-made > Photo &
// design > Photoshop: the deck shows it, the trying bar has Add and Back; Back leaves nothing,
// Add keeps it, another profile picked during the try throws it away; home stays until Add.
// A test copy of its own (virtual modules, never the focus). Screenshots in logs/preset-try-*.png.
//   node scripts/ui/preset-try.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-preset-try");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9481;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(2000);
  const state = () => win.evaluate(() => window.__TAURI_INTERNALS__.invoke("engine_state"));
  const name = (s, id) => s.profiles.find((p) => p.id === id)?.name ?? null;
  const photoshops = (s) => s.profiles.filter((p) => p.name === "Photoshop").length;
  const bar = win.locator(".chip.is-trying");
  const pick = async () => {
    await win.getByRole("button", { name: /New profile/ }).first().click();
    await win.getByText("Ready-made").first().click();
    await win.getByRole("button", { name: /Photo & design/ }).first().click();
    await win.getByRole("button", { name: /^Photoshop$/ }).first().click();
    await sleep(1500);
  };

  const s0 = await state();
  const home = s0.home;
  const before = photoshops(s0);
  console.log(`home ${name(s0, home)}, Photoshop profiles before: ${before}`);

  // 1. picked: on the deck for a try, home unchanged, Add and Back on the bar
  await pick();
  let s = await state();
  check("Photoshop is on the deck for a try", name(s, s.trying) === "Photoshop", String(name(s, s.trying)));
  check("home stays", s.home === home, String(name(s, s.home)));
  check("it is a draft, not one of the profiles yet", s.profiles.some((p) => p.id === s.trying && p.draft));
  check("the bar says it and offers Add and Back", (await bar.isVisible()) && /Photoshop/.test(await bar.innerText()) && (await bar.getByRole("button", { name: "Add" }).isVisible()) && (await bar.getByRole("button", { name: "Back" }).isVisible()), (await bar.innerText().catch(() => "")).replace(/\s+/g, " "));
  for (const look of ["dark", "light"]) {
    await win.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
    await sleep(200);
    await win.screenshot({ path: path.join(root, "logs", `preset-try-${look}.png`) });
  }
  await win.evaluate(() => document.documentElement.removeAttribute("data-theme"));

  // 2. Back: nothing left
  await bar.getByRole("button", { name: "Back" }).click();
  await sleep(1500);
  s = await state();
  check("Back ends the try", !s.trying);
  check("and leaves no Photoshop profile", photoshops(s) === before, `${photoshops(s)} (before ${before})`);
  check("home is still the same", s.home === home);

  // 3. another profile picked during the try: thrown away too
  await pick();
  const other = s.profiles.find((p) => !p.draft && p.id !== home);
  await win.evaluate((id) => window.__TAURI_INTERNALS__.invoke("profile_activate", { id }), other.id);
  await sleep(1500);
  s = await state();
  check("picking another profile ends the try and throws it away", !s.trying && photoshops(s) === before, `${photoshops(s)} Photoshop`);
  await win.evaluate((id) => window.__TAURI_INTERNALS__.invoke("profile_activate", { id }), home);
  await sleep(800);

  // 4. Add: kept, on the deck, one of the profiles
  await pick();
  await bar.getByRole("button", { name: "Add" }).click();
  await sleep(2000);
  s = await state();
  const kept = s.profiles.find((p) => p.name === "Photoshop" && !p.draft && s.profiles.filter((q) => q.name === "Photoshop").length === before + 1);
  check("Add keeps it as a profile", photoshops(s) === before + 1 && !!kept, `${photoshops(s)} Photoshop`);
  check("and it is on the deck", name(s, s.active) === "Photoshop" && !s.trying, String(name(s, s.active)));
  check("no page errors", errors === 0, `${errors}`);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1000);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
