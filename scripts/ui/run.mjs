// Usage: node run.mjs <step.mjs> [shot.png]
// step.mjs exports default async (page) => any; result is printed as JSON.
import { chromium } from 'playwright-core';
import { pathToFileURL } from 'node:url';

const [, , stepPath, shot] = process.argv;
const browser = await chromium.connectOverCDP('http://127.0.0.1:' + (process.env.PW_PORT || 9444));
const page = browser.contexts()[0].pages().find(p => /tauri.localhost|localhost:1420/.test(p.url()));
page.setDefaultTimeout(+process.env.PW_TIMEOUT || 6000);
try {
  const step = (await import(pathToFileURL(stepPath).href + '?t=' + Date.now())).default;
  const out = await step(page);
  if (out !== undefined) console.log(JSON.stringify(out, null, 1));
  if (shot) await page.screenshot({ path: shot });
} finally {
  // Leave the app's own page alone: only drop the CDP connection.
  await browser.close().catch(() => {});
}
