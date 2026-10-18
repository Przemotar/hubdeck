// Get started must not show the tour's big purple button for a moment when the tour was done
// (owner, 2026-10-18: "jak wchodzę w Getting started to dalej mi przez chwilę blinkuje ten duży
// fioletowy przycisk"), nor the AI assistant's loading line when the tab opens again. On a test
// copy of the owner's data (the tour done there): every frame after a click on the tab is recorded
// (requestAnimationFrame), plus screenshots of the first ones.
//   node scripts/ui/getstarted-check.mjs     # screenshots in logs/getstarted-*.png
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const dir = path.join(os.tmpdir(), "hubdeck-getstarted-check");
const PORT = 9478;
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
const tab = (i) => page.locator(".lib-tabs [role=tab]").nth(i);

// records each frame for ms: whether the big button and the "again" row are drawn
const record = (ms) =>
  page.evaluate(
    (ms) =>
      new Promise((done) => {
        const frames = [];
        const t0 = performance.now();
        const step = () => {
          frames.push({ at: Math.round(performance.now() - t0), big: !!document.querySelector(".as-show"), again: !!document.querySelector(".as-intro .as-row-lead"), looking: !!document.querySelector(".assistant")?.textContent?.includes("Looking for Claude Code") });
          if (performance.now() - t0 < ms) requestAnimationFrame(step);
          else done(frames);
        };
        requestAnimationFrame(step);
      }),
    ms,
  );
const open = async (name) => {
  const frames = record(1500);
  await tab(0).click();
  for (let i = 0; i < 3; i++) await page.screenshot({ path: path.join(root, "logs", `getstarted-${name}-${i}.png`) });
  const got = await frames;
  const big = got.filter((f) => f.big);
  check(`${name}: the big button never drawn`, big.length === 0, `${got.length} frames, big in ${big.length}${big.length ? ` from ${big[0].at} ms` : ""}`);
  check(`${name}: "Show me again" drawn at the end`, got.at(-1)?.again === true);
  return got;
};
try {
  await page.waitForSelector(".lib-tabs [role=tab]", { timeout: 20000 });
  await sleep(1500);
  check("the test copy's tour was done (as on the owner's PC)", (await invoke("tour_done")) === true);
  if ((await tab(0).getAttribute("aria-selected")) === "true") await tab(1).click();
  await sleep(500);
  await open("first");
  await tab(1).click();
  await sleep(500);
  const again = await open("again");
  // the AI assistant's card shows what it knew, not its loading line, when the tab opens again
  const looking = again.filter((f) => f.looking).length;
  check('again: no "Looking for Claude Code..." for a moment', looking === 0, `in ${looking} frames`);
  // the window opened anew (the window's own start, as after a restart of HubDeck)
  await page.reload();
  await page.waitForSelector(".lib-tabs [role=tab]", { timeout: 20000 });
  await sleep(300);
  if ((await tab(0).getAttribute("aria-selected")) === "true") await tab(1).click();
  await open("reloaded");
} catch (e) {
  await page.screenshot({ path: path.join(root, "logs", "getstarted-error.png") }).catch(() => {});
  console.log(`page: ${page.url()}`);
  throw e;
} finally {
  await browser.close().catch(() => {});
  ps("-Stop");
}
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\nFAILED: ${failed} of ${results.length}` : `\npassed ${results.length}/${results.length}`);
process.exit(failed ? 1 : 0);
