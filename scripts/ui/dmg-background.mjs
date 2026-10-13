// Renders the macOS .dmg window background (app/src-tauri/dmg/background.html) to background.png:
// 1440x1220 pixels marked 144 dpi, so Finder shows it 720x610 and sharp on Retina screens.
// Headless, no window, no focus taken. Needs app/node_modules (the fonts) and a Playwright Chromium.
//
//   node scripts/ui/dmg-background.mjs
import { chromium } from 'playwright-core';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const dir = path.join(root, 'app', 'src-tauri', 'dmg');
const out = path.join(dir, 'background.png');

function findChromium() {
  if (process.env.CHROMIUM) return process.env.CHROMIUM;
  const cache = path.join(os.homedir(), 'Library', 'Caches', 'ms-playwright');
  for (const d of fs.existsSync(cache) ? fs.readdirSync(cache).filter((x) => /^chromium-\d+$/.test(x)).sort().reverse() : []) {
    const mac = fs.readdirSync(path.join(cache, d)).find((x) => x.startsWith('chrome-mac'));
    if (!mac) continue;
    const app = fs.readdirSync(path.join(cache, d, mac)).find((x) => x.endsWith('.app'));
    if (app) {
      const bin = path.join(cache, d, mac, app, 'Contents', 'MacOS');
      return path.join(bin, fs.readdirSync(bin)[0]);
    }
  }
  return undefined;
}

const browser = await chromium.launch({ executablePath: findChromium(), headless: true });
const page = await browser.newPage({ viewport: { width: 720, height: 610 }, deviceScaleFactor: 2 });
await page.goto(pathToFileURL(path.join(dir, 'background.html')).href);
await page.evaluate(() => document.fonts.ready);
await page.screenshot({ path: out });
await browser.close();
execFileSync('sips', ['-s', 'dpiWidth', '144', '-s', 'dpiHeight', '144', out], { stdio: 'ignore' });
console.log(`wrote ${out}`);
