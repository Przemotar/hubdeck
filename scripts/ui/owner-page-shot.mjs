// One of the owner's own profiles on a test copy of his data, a page of it on the deck drawing:
// the profile is shown (try), a press of the key that opens the page goes to the virtual keys, and
// the deck is screenshotted in dark and light.
//   node scripts/ui/owner-page-shot.mjs <profile id> <key index that opens the page> [name] [page tab name]
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const [profile, key, name = "page"] = process.argv.slice(2);
const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const PORT = 9476;
const dir = path.join(process.env.TEMP, "hubdeck-owner-page");
const out = path.join(root, "scripts", "ui", "owner");
fs.mkdirSync(out, { recursive: true });
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

ps("-Stop");
ps("-Fresh");
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
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: "POST", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: JSON.stringify(body ?? {}) }).then((r) => r.json());
};
try {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.locator(".dk-slot").first().waitFor({ timeout: 30000 });
  await api(`profiles/${profile}/activate`);
  await sleep(1500);
  await page.locator(".dk-stage").screenshot({ path: path.join(out, `${name}-main.png`) });
  // the page as the window shows it: its tab above the deck (a press on the virtual deck changes
  // the deck's page, not the window's)
  const pageName = process.argv[5] ?? "";
  if (pageName) await page.locator(".page-bar .seg .tab", { hasText: pageName }).first().click();
  else await api("test/input", { kind: "keys", index: Number(key), gesture: "press" });
  await sleep(2000);
  for (const theme of ["dark", "light"]) {
    await page.evaluate((t) => localStorage.setItem("hubdeck.theme", t), theme);
    await page.evaluate(() => window.dispatchEvent(new StorageEvent("storage", { key: "hubdeck.theme" })));
    await sleep(800);
    await page.locator(".dk-stage").screenshot({ path: path.join(out, `${name}-${theme}.png`) });
  }
  const cut = await page.evaluate(() => [...document.querySelectorAll(".dk-front text")].filter((t) => t.getComputedTextLength && t.getComputedTextLength() > 96).map((t) => t.textContent));
  console.log(`screenshots ${name}-main/dark/light.png; labels wider than a key: ${JSON.stringify(cut)}`);
} finally {
  await page.evaluate(() => localStorage.setItem("hubdeck.theme", "dark")).catch(() => {});
  await browser.close().catch(() => {});
  ps("-Stop");
}
