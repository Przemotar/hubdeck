// Modules hanging over the base's edge (owner, 2026-10-19: "mozna wpiac panele w dowolny styk";
// #4: keys in 1-1-1 were drawn on top of the faders). A test copy (virtual modules, no deck, never
// the focus) with Zendeman's deck from the #4 diagnostics, then the owner's slot 4-4 for the keys:
// the deck view and Edit layout in dark and light, at two window sizes; nothing may overlap or be
// cut off by the drawing's edge.
//   node scripts/ui/overhang-shots.mjs      # screenshots in scripts/ui/overhang/
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const dir = path.join(os.tmpdir(), "hubdeck-overhang");
const out = path.join(root, "scripts", "ui", "overhang");
const PORT = 9473;
fs.mkdirSync(out, { recursive: true });
const DECKS = {
  // #4, Zendeman: port paths from his diagnostics
  zendeman: "base@6.1.4.4,keys@6.1.1.1,faders@6.2.3,knobs@6.1.3.4,dial@6.1.4.1",
  // the owner's deck with the keys in 4-4, as he tried at 12:21 on 2026-10-19
  owner44: "base@6.1.4.4,keys@6.4.4,dial@6.1.3.4,knobs@6.1.4.3,rollers@6.3.3",
};
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};

for (const [deck, modules] of Object.entries(DECKS)) {
  ps("-Stop");
  ps("-Fresh", "-Modules", modules);
  // the user's own arrangement would hide what the table gives
  fs.writeFileSync(path.join(dir, "placements.json"), JSON.stringify({ version: 1, placements: {} }));
  ps("-Modules", modules);
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
  for (const theme of ["dark", "light"]) {
    await page.evaluate((t) => localStorage.setItem("hubdeck.theme", t), theme);
    for (const [w, h] of [
      [1440, 900],
      [1100, 760],
    ]) {
      await page.setViewportSize({ width: w, height: h });
      await page.reload();
      await page.locator(".dk-slot").first().waitFor({ timeout: 30000 });
      await sleep(1500);
      for (const arranging of [false, true]) {
        if (arranging) await page.getByRole("button", { name: /Edit layout|Układ/ }).click();
        await sleep(600);
        const name = `${deck}-${theme}-${w}${arranging ? "-layout" : ""}`;
        await page.locator(".dk-stage").screenshot({ path: path.join(out, `${name}.png`) });
        // every module inside the drawing, none on top of another
        const boxes = await page.locator(".dk-front .dk-slot").evaluateAll((els) => els.map((e) => e.getBoundingClientRect().toJSON()));
        const stage = await page.locator(".dk-stage").evaluate((e) => e.getBoundingClientRect().toJSON());
        const inside = boxes.every((b) => b.left >= stage.left - 1 && b.right <= stage.right + 1 && b.top >= stage.top - 1 && b.bottom <= stage.bottom + 1);
        const overlap = boxes.some((a, i) => boxes.some((b, j) => j > i && a.left < b.right - 2 && b.left < a.right - 2 && a.top < b.bottom - 2 && b.top < a.bottom - 2));
        check(`${name}: ${boxes.length} modules inside the drawing, none overlapping`, inside && !overlap && boxes.length >= 4, `${boxes.length} modules${inside ? "" : ", one cut off"}${overlap ? ", two overlap" : ""}`);
        if (arranging) await page.getByRole("button", { name: /Done|Gotowe/ }).click().catch(() => {});
      }
    }
  }
  await page.evaluate(() => localStorage.setItem("hubdeck.theme", "dark"));
  await browser.close().catch(() => {});
}
ps("-Stop");
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}; screenshots in ${out}`);
process.exit(passed === results.length ? 0 : 1);
