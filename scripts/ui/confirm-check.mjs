// Questions before something that cannot be undone wait for the answer (2026-10-18: in HubDeck's
// window window.confirm returned a promise, taken as yes, and the owner's "test" profile went
// without a question). In the browser demo, headless, never the focus: Delete page answered Cancel
// keeps the page, answered Delete deletes it. Since 2026-10-23 the question is HubDeck's own, in the
// window (owner's pick 1b), so the walk clicks its buttons as the user does.
//   node scripts/ui/confirm-check.mjs     # needs the Vite server (cd app && pnpm dev)
import { chromium } from "playwright-core";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const base = process.env.HUBDECK_URL || "http://localhost:1460/";
function findChromium() {
  const cache = path.join(os.homedir(), process.platform === "win32" ? "AppData/Local/ms-playwright" : process.platform === "darwin" ? "Library/Caches/ms-playwright" : ".cache/ms-playwright");
  for (const dir of fs.existsSync(cache) ? fs.readdirSync(cache).filter((d) => d.startsWith("chromium")).sort().reverse() : []) {
    const stack = [path.join(cache, dir)];
    while (stack.length) {
      const d = stack.pop();
      for (const e of fs.readdirSync(d, { withFileTypes: true })) {
        const p = path.join(d, e.name);
        if (e.isDirectory() && !p.includes(".app/Contents/Frameworks")) stack.push(p);
        else if (/^(chrome-headless-shell|chrome|Chromium|headless_shell)(\.exe)?$/.test(e.name)) return p;
      }
    }
  }
}
const results = [];
const check = (what, ok, note = "") => (results.push({ ok }), console.log(`${ok ? "ok  " : "FAIL"} ${what}${note ? ` (${note})` : ""}`));
const browser = await chromium.launch({ executablePath: findChromium() });
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
try {
  await page.goto(`${base}?demo`);
  await page.locator(".dk-ctl").first().waitFor();
  // a page of its own to delete: New page, a name, Enter
  const tabs = () => page.locator("button.tab", { hasText: /^Walk$/ }).count();
  await page.getByRole("button", { name: "New page" }).click();
  await page.keyboard.type("Walk");
  await page.keyboard.press("Enter");
  await page.locator("button.tab", { hasText: /^Walk$/ }).first().click();
  await page.waitForTimeout(500);
  const before = await tabs();
  // the question is HubDeck's own, in the window (owner's pick 1b, 2026-10-23)
  const question = page.getByRole("alertdialog");
  await page.getByRole("button", { name: "Delete page" }).click();
  await page.waitForTimeout(500);
  const asked = (await question.innerText().catch(() => "")).replace(/\s+/g, " ");
  check("Delete page asks first, in the window", asked.includes("Walk"), asked);
  await question.getByRole("button", { name: "Cancel" }).click().catch(() => {});
  await page.waitForTimeout(500);
  check("answered Cancel, the page stays", (await tabs()) === before && before > 0, `pages named Walk: ${before} -> ${await tabs()}`);
  await page.getByRole("button", { name: "Delete page" }).click();
  await page.waitForTimeout(500);
  await question.getByRole("button", { name: "Delete" }).click().catch(() => {});
  await page.waitForTimeout(800);
  check("answered OK, the page goes", (await tabs()) === before - 1);
} finally {
  await browser.close();
}
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\nFAILED: ${failed} of ${results.length}` : `\npassed ${results.length}/${results.length}`);
process.exit(failed ? 1 : 0);
