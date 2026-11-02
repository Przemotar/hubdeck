// Settings > Diagnostics (owner, 2026-11-02: "panel do debugowania, w którym można zaznaczyć wybrany
// moduł i zdebugować czy działa"). On a test copy with his profiles and virtual modules of every kind
// (the window off screen): every module is listed, the lights test runs its steps and asks, a mark and
// a note give the hardware verdict, controls used on the virtual modules light up, the report is
// copied; dark and light, two window sizes, nothing cut. Virtual modules have no device, so the reads
// say "not connected" here (the reads themselves are checked on hardware). Screenshots in
// logs/diagnostics/.
//   node scripts/ui/diagnostics.mjs [path to hubdeck.exe]
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-diagnostics");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "diagnostics");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 500 });
fs.mkdirSync(dir, { recursive: true });
fs.rmSync(shots, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "nudges.json"]) if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
const MODULES = "base@6.1.4.4,keys@6.2.3,dial@6.1.3.4,knobs@6.1.4.3,faders@6.1.1.3,rollers@6.3.3";
const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9493", HUBDECK_VIRTUAL_MODULES: MODULES }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
const api = (method, route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method, headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
let browser;
try {
  browser = await chromium.connectOverCDP("http://127.0.0.1:9493");
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  await win.setViewportSize({ width: 1606, height: 1000 });
  await sleep(3000);
  for (const name of [/^Not now$/, /^Got it$/, /^Skip/, /^Close$/]) {
    const b = win.getByRole("button", { name }).first();
    if (await b.isVisible().catch(() => false)) (await b.click(), await sleep(600));
  }
  await win.getByRole("button", { name: /^(Settings|Ustawienia)$/ }).first().click();
  await win.locator(".settings-nav-item", { hasText: /^(Diagnostics|Diagnostyka)$/ }).click();
  await win.locator(".diag").waitFor({ timeout: 10000 });
  await sleep(800);
  const listed = await win.locator(".diag-mod").count();
  check("every attached module is listed", listed === 6, `${listed}`);
  await win.locator(".settings-content").screenshot({ path: path.join(shots, "1-open.png") });

  // faders: the lights test, its question, a mark and a note
  await win.locator(".diag-mod", { hasText: /fader/i }).click();
  await sleep(500);
  await win.getByRole("button", { name: /^(Run test|Uruchom test)$/ }).click();
  await sleep(2200);
  check("the test shows its step", await win.locator(".diag-now").isVisible());
  await win.locator(".diag-check").screenshot({ path: path.join(shots, "2-testing.png") });
  await win.locator(".diag-ask").waitFor({ timeout: 25000 });
  check("after the steps it asks what was seen", true);
  await win.locator(".diag-marks .chip").first().click();
  await win.locator(".diag-ask input").fill("light 1 stayed red");
  await win.locator(".diag-check").screenshot({ path: path.join(shots, "3-ask.png") });
  await win.getByRole("button", { name: /^(No, something was off|Nie, coś było nie tak)$/ }).click();
  await sleep(300);
  check("a marked light gives the hardware verdict", await win.locator(".diag-verdict.is-bad").isVisible());

  // controls on the virtual faders light up
  await api("POST", "test/input", { kind: "faders", index: 0, gesture: "slide", value: 80 });
  await api("POST", "test/input", { kind: "faders", index: 2, gesture: "slide", value: 30 });
  await sleep(600);
  const seen = await win.locator(".diag-control.is-seen").count();
  check("faders moved light up", seen === 2, `${seen}`);
  await win.evaluate(() => {
    const write = navigator.clipboard.writeText.bind(navigator.clipboard);
    navigator.clipboard.writeText = (text) => ((window.__copied = text), write(text).catch(() => {}));
  });
  await win.getByRole("button", { name: /^(Copy report|Kopiuj raport)$/ }).click();
  await sleep(400);
  check("the report is copied", await win.getByRole("button", { name: /^(Copied|Skopiowane)$/ }).isVisible());
  // the webview's clipboard read waits for a permission nobody gives off screen: the copy is read
  // through the same text the button wrote, kept by a wrapper put in before the click
  const report = await win.evaluate(() => window.__copied ?? "(nothing copied)");
  check("the report has the test, the input and no serial", /did not follow: 1 \(light 1 stayed red\)/.test(report) && /moved 1 3/.test(report) && !/VIRTUAL/.test(report), report.split("\n").slice(0, 4).join(" / "));
  await win.locator(".diag-check").screenshot({ path: path.join(shots, "4-faders-done.png") });

  // keys and dial
  await win.locator(".diag-mod", { hasText: /15-key|15 klawiszy/i }).click();
  await sleep(400);
  await api("POST", "test/input", { kind: "keys", index: 4, gesture: "press" });
  await sleep(500);
  check("a pressed key lights up", (await win.locator(".diag-control.is-seen").count()) === 1);
  await win.locator(".diag-check").screenshot({ path: path.join(shots, "5-keys.png") });
  await win.locator(".diag-mod", { hasText: /dial/i }).click();
  await sleep(400);
  await api("POST", "test/input", { kind: "dial", index: 0, gesture: "turn", value: -1 });
  await api("POST", "test/input", { kind: "dial", index: 0, gesture: "press" });
  await sleep(500);
  check("the dial's left turn and press light up, the right turn not", (await win.locator(".diag-check .pill.is-ok").count()) === 2);

  // looks and sizes
  await win.locator(".diag-mod", { hasText: /fader/i }).click();
  await sleep(400);
  for (const [w, h] of [
    [1606, 1000],
    [1100, 760],
  ]) {
    await win.setViewportSize({ width: w, height: h });
    for (const look of ["dark", "light"]) {
      await win.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
      await sleep(400);
      await win.screenshot({ path: path.join(shots, `6-${w}-${look}.png`) });
      const cut = (await clipped(win)).filter((x) => x.text);
      check(`nothing cut at ${w}x${h} ${look}`, cut.length === 0, cut.map((x) => x.text).join(" | "));
    }
  }
  await win.evaluate(() => document.documentElement.removeAttribute("data-theme"));
  check("no error in the page", errors === 0, `${errors}`);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  spawn("taskkill", ["/F", "/T", "/PID", String(app.pid)]);
  await sleep(2500);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const log = fs
  .readdirSync(path.join(dir, "logs"))
  .filter((f) => f.endsWith(".log"))
  .flatMap((f) => fs.readFileSync(path.join(dir, "logs", f), "utf8").split("\n"))
  .filter((l) => / ERROR /.test(l));
check("no ERROR in the copy's log", log.length === 0, log.slice(0, 3).join(" | "));
console.log(`${results.filter(Boolean).length}/${results.length}`);
process.exit(results.every(Boolean) ? 0 : 1);
