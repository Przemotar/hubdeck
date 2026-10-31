// The README's and the site's screenshots, from the browser demo (sample deck and profiles, nothing
// of the owner's): the main window with a key selected, the tour, Settings. 1440x900 at 1.5x, the
// size the README had (2160x1350). Needs the Vite server (cd app && pnpm dev).
//   node scripts/ui/readme-shots.mjs [out-dir]     # default docs/images
import { chromium } from 'playwright-core';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')), '..', '..');
const out = process.argv[2] || path.join(root, 'docs', 'images');
const base = process.env.HUBDECK_URL || 'http://localhost:1460/';

function findChromium() {
  if (process.env.CHROMIUM) return process.env.CHROMIUM;
  const cache = process.platform === 'win32' ? path.join(os.homedir(), 'AppData', 'Local', 'ms-playwright') : process.platform === 'darwin' ? path.join(os.homedir(), 'Library', 'Caches', 'ms-playwright') : path.join(os.homedir(), '.cache', 'ms-playwright');
  if (!fs.existsSync(cache)) return undefined;
  for (const dir of fs.readdirSync(cache).filter((d) => d.startsWith('chromium')).sort().reverse()) {
    const stack = [path.join(cache, dir)];
    while (stack.length) {
      const d = stack.pop();
      for (const e of fs.readdirSync(d, { withFileTypes: true })) {
        const p = path.join(d, e.name);
        if (e.isDirectory() && !p.includes('.app/Contents/Frameworks')) stack.push(p);
        else if (/^(chrome-headless-shell|chrome|Chromium|headless_shell)(\.exe)?$/.test(e.name)) return p;
      }
    }
  }
  return undefined;
}

const browser = await chromium.launch({ executablePath: findChromium() });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1.5, colorScheme: 'dark' });
page.setDefaultTimeout(15000);
const fresh = async () => {
  await page.goto(`${base}?demo&tests=off`);
  await page.locator('.dk-ctl').first().waitFor();
  await page.waitForTimeout(1200);
};
const shot = (name) => page.screenshot({ path: path.join(out, `${name}.png`) });

// the main window: a key selected, "What should it do?" with its ideas
await fresh();
await page.locator('.dk-ctl[aria-label^="Key 4"]').first().click({ force: true });
await page.waitForTimeout(800);
await shot('hubdeck-main');

// the tour, from Get started
await fresh();
await page.locator('.lib-tabs [role="tab"]', { hasText: 'Get started' }).click();
await page.waitForTimeout(500);
await shot('hubdeck-start');
// the tour itself: Show me on my deck, then its first question answered
await page.getByText('Show me on my deck').click();
await page.waitForTimeout(1000);
await shot('hubdeck-tour-question');
await page.getByRole('button', { name: 'Make my deck' }).click();
await page.waitForTimeout(2500);
await shot('hubdeck-tour');

// Settings
await fresh();
await page.getByRole('button', { name: 'Settings' }).click();
await page.waitForTimeout(500);
// the plugin catalog says more than General
await page.getByRole('button', { name: 'Plugins' }).first().click();
await page.waitForTimeout(1500);
await shot('hubdeck-settings');

// the Plugins tab with the real catalog (logs/plugins-catalog.json, made by
// scripts/plugins/catalog.mjs), tried and left-out plugins as the app has them now; the fields the
// app fills in when the file leaves them out (sdcatalog.rs Entry)
const catalogFile = path.join(root, 'logs', 'plugins-catalog.json');
if (fs.existsSync(catalogFile)) {
  const shared = JSON.parse(fs.readFileSync(path.join(root, 'app', 'src-tauri', 'src', 'engine', 'sdcatalog.json'), 'utf8'));
  const plugins = JSON.parse(fs.readFileSync(catalogFile, 'utf8')).plugins.filter((p) => !shared.wontRun?.[p.id]).map((p) => ({ author: '', description: '', icon: '', aiAssisted: false, elgato: null, popular: null, stars: null, updated: null, category: null, dial: false, ...p, systems: p.systems ?? [], tried: shared.tried[p.id] ?? null }));
  await page.addInitScript((list) => (window.__DEMO_CATALOG__ = list), plugins);
  await fresh();
  await page.getByRole('button', { name: 'Plugins' }).first().click();
  await page.waitForTimeout(3000);
  await shot('hubdeck-plugins');
} else console.log('no logs/plugins-catalog.json: the Plugins tab is not shot');

await browser.close();
console.log(`screenshots in ${out}`);
