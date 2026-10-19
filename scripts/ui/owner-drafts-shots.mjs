// Each draft owner-drafts.mjs left in its test copy, tried on the deck drawing (the local API's
// "try"), one screenshot of the deck each; the copy is started again without a fresh copy of data.
//   node scripts/ui/owner-drafts-shots.mjs     # scripts/ui/owner/try-<n>.png
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const PORT = 9475;
const dir = path.join(process.env.TEMP, "hubdeck-owner-drafts");
const out = path.join(root, "scripts", "ui", "owner");
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const report = JSON.parse(fs.readFileSync(path.join(out, "report.json"), "utf8"));

ps();
let browser, page;
for (let i = 0; i < 60 && !page; i++) {
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`).catch(() => undefined);
  page = browser?.contexts()[0]?.pages().find((x) => /tauri.localhost|localhost:1460/.test(x.url()));
  if (!page) {
    await browser?.close().catch(() => {});
    await sleep(1000);
  }
}
if (!page) throw new Error("the test copy's window did not come up");
await page.setViewportSize({ width: 1440, height: 900 });
await page.locator(".dk-slot").first().waitFor({ timeout: 30000 });
try {
  for (const [i, d] of report.drafts.entries()) {
    await page.evaluate((id) => window.__TAURI_INTERNALS__.invoke("profile_try", { id }).catch(() => window.__TAURI_INTERNALS__.invoke("try_profile", { id })), d.id).catch(() => {});
    await sleep(2500);
    await page.locator(".dk-stage").screenshot({ path: path.join(out, `try-${i + 1}.png`) });
    console.log(`try-${i + 1}.png ${d.name}`);
  }
} finally {
  await browser.close().catch(() => {});
  ps("-Stop");
}
