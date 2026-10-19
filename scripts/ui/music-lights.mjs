// Settings > Lights with the "to the music" style (owner, 2026-10-19: presets and a range for the
// lights to the music): a test copy of the owner's data with that style, the section screenshotted
// in dark and light at two window sizes, nothing cut; a preset picked moves both sliders.
//   node scripts/ui/music-lights.mjs     # scripts/ui/music/
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { clipped } from "./clipped.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const PORT = 9477;
const dir = path.join(process.env.TEMP, "hubdeck-music-lights");
const out = path.join(root, "scripts", "ui", "music");
fs.mkdirSync(out, { recursive: true });
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
const check = (what, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${what}${detail ? `: ${detail}` : ""}`);
};

ps("-Stop");
ps("-Fresh");
ps("-Stop");
const sp = path.join(dir, "settings.json");
const settings = JSON.parse(fs.readFileSync(sp, "utf8"));
settings.lights = { ...(settings.lights ?? {}), style: "music", musicDim: 60, musicFrom: 45 };
settings.language = "pl";
fs.writeFileSync(sp, JSON.stringify(settings, null, 2));
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
try {
  for (const theme of ["dark", "light"]) {
    await page.evaluate((t) => localStorage.setItem("hubdeck.theme", t), theme);
    for (const [w, h] of [
      [1440, 900],
      [1100, 760],
    ]) {
      await page.setViewportSize({ width: w, height: h });
      await page.reload();
      await page.getByRole("button", { name: /^(Settings|Ustawienia)$/ }).first().click();
      await page.locator(".settings-nav-item", { hasText: /Lights and screens|Światła i ekrany/ }).click();
      const tuning = page.locator(".music-tuning");
      await tuning.waitFor({ timeout: 15000 });
      await tuning.scrollIntoViewIfNeeded();
      await sleep(300);
      const name = `${theme}-${w}`;
      await page.locator(".music-tuning").locator("xpath=ancestor::section[1]").screenshot({ path: path.join(out, `${name}.png`) });
      const cut = await clipped(page);
      const preset = await page.locator("#lights-music-preset").inputValue();
      check(`${name}: the section shows the recommended preset, nothing cut`, preset === "beat" && cut.length === 0, `${preset}${cut.length ? ` cut: ${JSON.stringify(cut)}` : ""}`);
    }
  }
  await page.locator("#lights-music-preset").selectOption("beats");
  await sleep(600);
  const [dim, from] = [await page.locator("#lights-music-dim").inputValue(), await page.locator("#lights-music-from").inputValue()];
  check("a preset sets both sliders", dim === "100" && from === "100", `${dim} / ${from}`);
} finally {
  await page.evaluate(() => localStorage.setItem("hubdeck.theme", "dark")).catch(() => {});
  await browser.close().catch(() => {});
  ps("-Stop");
}
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}; screenshots in ${out}`);
process.exit(passed === results.length ? 0 : 1);
