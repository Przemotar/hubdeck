// The window while the deck shows its screensaver (owner's pick 1a, 2026-10-18: "jak na wygaszaczu
// klikam na przycisk to nie znika wygaszacz"). On a test copy of its own (virtual modules, never the
// focus): a Screensaver key starts one, the window must say so with End and keep showing the keys,
// a click on a key in the window must end it, and End must end the next one.
//   node scripts/ui/saver-check.mjs           # screenshots in logs/saver-check-*.png
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const dir = path.join(os.tmpdir(), "hubdeck-saver-check");
const PORT = 9477;
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
const check = (what, ok, note = "") => (results.push({ ok }), console.log(`${ok ? "ok  " : "FAIL"} ${what}${note ? ` (${note})` : ""}`));
ps("-Stop");
ps("-Fresh");
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
let page;
for (let i = 0; i < 40 && !page; i++) {
  page = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
  if (!page) await sleep(500);
}
await page.setViewportSize({ width: 1280, height: 800 });
const invoke = (cmd, a) => page.evaluate(([c, x]) => window.__TAURI_INTERNALS__.invoke(c, x), [cmd, a ?? {}]);
const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
const api = (route, body) => fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
const chip = page.locator(".stage-bar .chip", { hasText: "Screensaver on the deck" });
const start = async () => {
  await api("test/input", { kind: "keys", index: 0, gesture: "press" });
  await page.waitForFunction(() => document.querySelector(".stage-bar")?.textContent?.includes("Screensaver on the deck"), null, { timeout: 6000 }).catch(() => {});
};
try {
  const state = await invoke("engine_state");
  const profile = state.profiles.find((p) => p.id === state.active) ?? state.profiles[0];
  await api(`profiles/${profile.id}/controls`, { control: "keys.0", binding: { action: "core.saver", params: { effect: "stars" }, look: { label: "Saver" } } });
  await api(`profiles/${profile.id}/controls`, { control: "keys.4", binding: { action: "core.nextPage", params: {}, look: { label: "Next", icon: "arrow-down" } } });
  await sleep(800);
  await start();
  check("the window says the deck shows its screensaver", (await chip.count()) === 1);
  const keys = await page.evaluate(() => [...document.querySelectorAll("g.dk-ctl")].filter((g) => (g.getAttribute("aria-label") ?? "").startsWith("Key")).map((g) => g.innerHTML.includes("ksaver")).filter(Boolean).length);
  check("the keys in the window show what they do, not the screensaver", keys === 0, `${keys} keys drew the screensaver`);
  await page.screenshot({ path: path.join(root, "logs", "saver-check-1-on.png") });
  await page.locator('g.dk-ctl[aria-label^="Key 5"]').first().dispatchEvent("click");
  await page.waitForFunction(() => !document.querySelector(".stage-bar")?.textContent?.includes("Screensaver on the deck"), null, { timeout: 6000 }).catch(() => {});
  check("a click on a key in the window ends it", (await chip.count()) === 0 && (await invoke("saver_shown")) === null);
  await page.screenshot({ path: path.join(root, "logs", "saver-check-2-clicked.png") });
  await start();
  await chip.getByRole("button", { name: "End" }).click();
  await page.waitForFunction(() => !document.querySelector(".stage-bar")?.textContent?.includes("Screensaver on the deck"), null, { timeout: 6000 }).catch(() => {});
  check("End ends it", (await chip.count()) === 0 && (await invoke("saver_shown")) === null);
} finally {
  await browser.close().catch(() => {});
  ps("-Stop");
}
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\nFAILED: ${failed} of ${results.length}` : `\npassed ${results.length}/${results.length}`);
process.exit(failed ? 1 : 0);
