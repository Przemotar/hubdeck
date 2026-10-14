// Screenshots of the tour's "Your dial, many faces" step, its gestures ticked off one by one (the
// deck stood in for by the demo's `demoEmit`). Headless, no focus taken; `pnpm dev` in app/.
//   node scripts/ui/tour-dial.mjs [out-dir]
import { chromium } from 'playwright-core';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const out = process.argv[2] || path.join(os.tmpdir(), 'hubdeck-tour-dial');
fs.mkdirSync(out, { recursive: true });
const base = process.env.HUBDECK_URL || 'http://localhost:1420/';
const cache = path.join(os.homedir(), process.platform === 'darwin' ? 'Library/Caches/ms-playwright' : process.platform === 'win32' ? 'AppData/Local/ms-playwright' : '.cache/ms-playwright');
function find(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory() && !p.includes('.app/Contents/Frameworks')) {
      const hit = find(p);
      if (hit) return hit;
    } else if (/^(chrome-headless-shell|chrome|Chromium|headless_shell)(\.exe)?$/.test(e.name)) return p;
  }
  return undefined;
}
const exe = process.env.CHROMIUM || (fs.existsSync(cache) ? fs.readdirSync(cache).filter((d) => d.startsWith('chromium')).sort().reverse().map((d) => find(path.join(cache, d))).find(Boolean) : undefined);
const browser = await chromium.launch({ executablePath: exe });
for (const [label, viewport, scheme] of [['half-dark', { width: 960, height: 760 }, 'dark'], ['wide-light', { width: 1440, height: 900 }, 'light']]) {
  const page = await browser.newPage({ viewport, colorScheme: scheme });
  await page.goto(`${base}?demo`);
  await page.waitForSelector('.deck-area');
  await page.locator('.lib-tabs [role="tab"]', { hasText: 'Get started' }).click();
  await page.getByRole('button', { name: /Show me on my deck/ }).click();
  await page.getByRole('button', { name: 'Make my deck' }).click();
  await page.waitForTimeout(500);
  for (let i = 0; i < 12 && !(await page.locator('.tour-faces').isVisible()); i++) {
    await page.getByRole('button', { name: 'Next', exact: true }).click();
    await page.waitForTimeout(200);
  }
  // the second look (the clock), so the gestures are its own
  await page.locator('.tour-face button').first().click();
  await page.waitForTimeout(300);
  const shot = async (name) => {
    const el = page.locator('.tour-faces');
    await el.scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(out, `${label}-${name}.png`) });
  };
  await shot('0-none');
  for (const g of ['turn', 'press']) await page.evaluate((g) => window.demoEmit('tour://gesture', `dial:${g}`), g), await page.waitForTimeout(250);
  await page.waitForTimeout(300);
  // the bubble is up for a moment: catch it
  await page.screenshot({ path: path.join(out, `${label}-1-two.png`) });
  for (const g of ['double', 'hold']) await page.evaluate((g) => window.demoEmit('tour://gesture', `dial:${g}`), g);
  await page.waitForTimeout(500);
  await shot('2-all');
  await page.close();
}
await browser.close();
console.log(`screenshots in ${out}`);
