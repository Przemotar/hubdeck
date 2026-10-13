// Walk the window as a user would and check that each click shows what it should: a fresh install's
// tour from its desk check (Fix it goes and comes back, Yes, Skip), a key's editor, Settings is a page with Back, the left tabs switch,
// panels never stay over the editor. Runs on any system against the browser demo (`pnpm dev` in
// app/, the vite server on :1420), headless, no focus taken.
//
//   node scripts/ui/flows.mjs [out-dir]        # screenshots of every step land in out-dir
//   CHROMIUM=/path/to/chrome node scripts/ui/flows.mjs   # a browser of your own
//
// Exit code 1 when a step fails. (Owner, 2026-10-13: "testowałeś różne flow klikania?")
import { chromium } from 'playwright-core';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const out = process.argv[2] || path.join(os.tmpdir(), 'hubdeck-flows');
fs.mkdirSync(out, { recursive: true });
const base = process.env.HUBDECK_URL || 'http://localhost:1420/';

/** A browser Playwright downloaded before, when none is given. */
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
const results = [];
let n = 0;

async function step(page, name, check) {
  n += 1;
  let ok = false;
  let note = '';
  try {
    const r = await check();
    // true passes; false or a string (what went wrong) fails
    ok = r === true;
    if (typeof r === 'string') note = r;
  } catch (e) {
    note = String(e.message || e).split('\n')[0];
  }
  const errs = page.hubdeckErrors?.splice(0) ?? [];
  if (errs.length) {
    ok = false;
    note = `${note ? `${note}; ` : ''}script error: ${errs[0]}`;
  }
  const shot = path.join(out, `${String(n).padStart(2, '0')}-${name.replace(/[^a-z0-9]+/gi, '-').toLowerCase()}.png`);
  await page.screenshot({ path: shot }).catch(() => {});
  results.push({ step: n, name, ok, note });
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${n}. ${name}${note ? ` (${note})` : ''}`);
}

const visible = (page, sel) => page.locator(sel).first().isVisible();

for (const [label, viewport] of [['wide', { width: 1440, height: 900 }], ['half', { width: 960, height: 760 }]]) {
  const page = await browser.newPage({ viewport });
  page.setDefaultTimeout(5000);
  // a script error in the window is a failure of the step it happens in
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e.message || e).split('\n')[0]));
  page.hubdeckErrors = errors;
  // a fresh install: straight into the tour, at its desk check (owner's pick 1a, 2026-10-14)
  await page.goto(`${base}?demo=welcome`);
  await page.waitForSelector('.tour-page, .deck-area', { timeout: 15000 });
  await step(page, `${label}: fresh start shows the tour's desk check over the whole window`, async () => (await visible(page, '.tour-desk .dk-ctl')) && !(await visible(page, '.deck-area')));
  await step(page, `${label}: Fix it opens Edit layout and Done comes back to the desk check`, async () => {
    await page.locator('.tour-desk button', { hasText: /Fix it/ }).first().click();
    await page.waitForTimeout(300);
    if (!(await visible(page, '.deck-area'))) return 'the deck did not show for Edit layout';
    await page.locator('.chip.layout-chip').click();
    await page.waitForTimeout(300);
    return visible(page, '.tour-desk');
  });
  await step(page, `${label}: Yes goes to the tour's question, Exit leaves for the deck`, async () => {
    await page.locator('.tour-desk button', { hasText: /Yes, make my deck/ }).click();
    await page.waitForTimeout(300);
    if (!(await visible(page, '.tour-tiles'))) return 'no question';
    await page.locator('.tour-head button', { hasText: 'Exit' }).click();
    await page.waitForTimeout(400);
    return (await visible(page, '.dk-ctl')) && !(await visible(page, '.tour-page'));
  });
  await page.goto(`${base}?demo=welcome`);
  await page.waitForSelector('.tour-page, .deck-area', { timeout: 15000 });
  await step(page, `${label}: Skip leaves the first start for the deck`, async () => {
    await page.locator('.tour-head button', { hasText: 'Skip' }).click();
    await page.waitForTimeout(400);
    return (await visible(page, '.dk-ctl')) && !(await visible(page, '.tour-page'));
  });
  await step(page, `${label}: a key click opens its editor`, async () => {
    await page.locator('.dk-ctl').first().click();
    await page.waitForTimeout(300);
    return visible(page, '#action');
  });
  await step(page, `${label}: Settings is a page of its own`, async () => {
    await page.getByRole('button', { name: /^Settings$/ }).first().click();
    await page.waitForTimeout(300);
    return (await visible(page, '.settings-page')) && !(await visible(page, '.dk-ctl'));
  });
  await step(page, `${label}: Settings sections switch`, async () => {
    for (const s of ['Lights and screens', 'Connections', 'Plugins', 'Backups', 'Updates', 'Help and about', 'General']) {
      await page.locator('.settings-nav-item', { hasText: s }).click();
      await page.waitForTimeout(120);
      if (!(await page.locator('.settings-nav-item.is-active', { hasText: s }).isVisible())) return `section ${s} not active`;
    }
    return true;
  });
  await step(page, `${label}: Back to the deck`, async () => {
    await page.getByRole('button', { name: /Back to the deck/ }).click();
    await page.waitForTimeout(300);
    return (await visible(page, '.dk-ctl')) && !(await visible(page, '.settings-page'));
  });
  await step(page, `${label}: the left tabs switch (Profiles, Get started, Actions)`, async () => {
    for (const tab of ['Profiles', 'Get started', 'Actions']) {
      await page.locator('.lib-tabs [role="tab"]', { hasText: tab }).click();
      await page.waitForTimeout(200);
      const sel = await page.locator('.lib-tabs [role="tab"][aria-selected="true"]').innerText();
      if (!sel.includes(tab)) return `tab ${tab} not selected`;
    }
    return true;
  });
  await step(page, `${label}: another control's click keeps the editor`, async () => {
    const ctl = page.locator('.dk-ctl');
    await ctl.nth(Math.min(5, (await ctl.count()) - 1)).click();
    await page.waitForTimeout(250);
    return visible(page, '#action');
  });
  await step(page, `${label}: Settings while a key is picked, then back to the same editor`, async () => {
    await page.getByRole('button', { name: /^Settings$/ }).first().click();
    await page.waitForTimeout(200);
    await page.getByRole('button', { name: /Back to the deck/ }).click();
    await page.waitForTimeout(250);
    return visible(page, '#action');
  });
  await step(page, `${label}: Build my deck step by step opens its page, a card can be tried, Back returns`, async () => {
    await page.locator('.lib-tabs [role="tab"]', { hasText: 'Get started' }).click();
    await page.waitForTimeout(200);
    // the builder now sits in the Assistant tab's Details (the tour is the way in)
    await page.locator('.as-details > summary').click();
    await page.waitForTimeout(150);
    await page.getByRole('button', { name: /Build my deck step by step/ }).click();
    await page.waitForTimeout(500);
    if (!(await visible(page, '.builder-page'))) return 'no builder page';
    if ((await page.locator('.builder-card').count()) < 2) return 'no option cards';
    await page.locator('.builder-card button', { hasText: 'Pick' }).first().click();
    await page.waitForTimeout(300);
    if (!(await page.locator('.builder-step.is-current', { hasText: 'Knob 1' }).isVisible())) return 'Pick did not move on to the knobs';
    await page.getByRole('button', { name: /Back to the deck/ }).click();
    await page.waitForTimeout(300);
    await page.locator('.lib-tabs [role="tab"]', { hasText: 'Actions' }).click();
    return (await visible(page, '.deck-area')) && !(await visible(page, '.builder-page'));
  });
  await step(page, `${label}: the tour: one question, the parts with Keep, the finale, the end, Exit back to the deck`, async () => {
    await page.locator('.lib-tabs [role="tab"]', { hasText: 'Get started' }).click();
    await page.waitForTimeout(200);
    await page.getByRole('button', { name: /Show me on my deck/ }).click();
    await page.waitForTimeout(400);
    if (!(await visible(page, '.tour-tiles'))) return 'no question';
    await page.getByRole('button', { name: 'Make my deck' }).click();
    await page.waitForTimeout(500);
    if ((await page.locator('.tour-part').count()) < 4) return 'no parts';
    if (!(await visible(page, '.tour-deck .dk-ctl'))) return 'no deck';
    await page.locator('.tour-part input[type=checkbox]').nth(1).click();
    await page.waitForTimeout(200);
    for (let i = 0; i < 8 && (await page.getByRole('button', { name: 'Next', exact: true }).isVisible()); i++) {
      await page.getByRole('button', { name: 'Next', exact: true }).click();
      await page.waitForTimeout(200);
    }
    if (!(await visible(page, '.tour-super'))) return 'no finale';
    await page.getByRole('button', { name: 'See my deck' }).click();
    await page.waitForTimeout(300);
    if (!(await visible(page, '.tour-end'))) return 'no end page';
    await page.getByRole('button', { name: /^Exit/ }).click();
    await page.waitForTimeout(300);
    await page.locator('.lib-tabs [role="tab"]', { hasText: 'Actions' }).click();
    return (await visible(page, '.deck-area')) && !(await visible(page, '.tour-page'));
  });
  // the tour to its end page, from the Get started tab
  const tourToEnd = async () => {
    await page.locator('.lib-tabs [role="tab"]', { hasText: 'Get started' }).click();
    await page.waitForTimeout(200);
    await page.getByRole('button', { name: /Show me on my deck/ }).click();
    await page.waitForTimeout(400);
    await page.getByRole('button', { name: 'Make my deck' }).click();
    await page.waitForTimeout(500);
    for (let i = 0; i < 8 && (await page.getByRole('button', { name: 'Next', exact: true }).isVisible()); i++) {
      await page.getByRole('button', { name: 'Next', exact: true }).click();
      await page.waitForTimeout(150);
    }
    await page.getByRole('button', { name: 'See my deck' }).click();
    await page.waitForTimeout(300);
  };
  await step(page, `${label}: Use as my home deck shows where things are once, in 5 steps with a place lit each`, async () => {
    await page.evaluate(() => localStorage.removeItem('hubdeck.guideSeen'));
    await tourToEnd();
    await page.getByRole('button', { name: /Use as my home deck/ }).click();
    await page.waitForTimeout(500);
    for (let i = 1; i <= 5; i++) {
      if (!(await page.locator('.guide-step', { hasText: `${i} of 5` }).isVisible())) return `no step ${i}`;
      if (!(await visible(page, '.guide-spot'))) return `step ${i} lights nothing`;
      await page.locator('.guide-card button.primary').click();
      await page.waitForTimeout(150);
    }
    if (await visible(page, '.guide')) return 'Done did not close it';
    await tourToEnd();
    await page.getByRole('button', { name: /Use as my home deck/ }).click();
    await page.waitForTimeout(400);
    return !(await visible(page, '.guide')) && (await visible(page, '.deck-area'));
  });
  await step(page, `${label}: the assistant card sends with Enter, a short consent asks once, then never again`, async () => {
    await page.evaluate(() => localStorage.removeItem('hubdeck.assistantConsent'));
    await page.locator('.lib-tabs [role="tab"]', { hasText: 'Get started' }).click();
    await page.waitForTimeout(400);
    if ((await page.locator('.as-examples .chip').count()) !== 3) return 'no three examples';
    await page.locator('#as-know').fill('Use my two free knobs');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(500);
    if (!(await page.locator('.as-consent .as-msg', { hasText: 'Use my two free knobs' }).isVisible())) return 'no consent with the message';
    if (await page.locator('.as-starter').count()) return 'Start with buttons are back';
    if (!(await page.locator('.as-consent input[type=checkbox]').isChecked())) return "Don't ask again is not checked";
    await page.keyboard.press('Enter');
    await page.waitForTimeout(700);
    if (!(await page.locator('.as-body > .as-msg.is-user', { hasText: 'Use my two free knobs' }).isVisible()) || (await visible(page, '.as-consent'))) return 'Enter did not send';
    await page.locator('.as-head .icon-btn').click();
    await page.waitForTimeout(300);
    await page.locator('.as-examples .chip').first().click();
    await page.waitForTimeout(700);
    const again = (await visible(page, '.as-body > .as-msg.is-user')) && !(await visible(page, '.as-consent'));
    await page.locator('.as-head .icon-btn').click();
    await page.locator('.lib-tabs [role="tab"]', { hasText: 'Actions' }).click();
    return again || 'asked again';
  });
  await step(page, `${label}: Ready for your apps lists its profiles, off, and a switch turns one on`, async () => {
    await page.locator('.lib-tabs [role="tab"]', { hasText: 'Profiles' }).click();
    await page.waitForTimeout(300);
    if (!(await page.locator('.ready-apps .plist-head', { hasText: /Ready for your apps \(3\)/ }).isVisible())) return 'no group';
    const sw = page.locator('.ready-apps input[type=checkbox]').first();
    if (await sw.isChecked()) return 'on by default';
    await sw.click();
    await page.waitForTimeout(300);
    const on = await sw.isChecked();
    await sw.click();
    await page.locator('.lib-tabs [role="tab"]', { hasText: 'Actions' }).click();
    return on || 'the switch did not turn on';
  });
  await step(page, `${label}: Settings toggles closed from the top bar too`, async () => {
    await page.getByRole('button', { name: /^Settings$/ }).first().click();
    await page.waitForTimeout(200);
    await page.getByRole('button', { name: /^Settings$/ }).first().click();
    await page.waitForTimeout(250);
    return (await visible(page, '.dk-ctl')) && !(await visible(page, '.settings-page'));
  });
  await page.close();
}

await browser.close();
const failed = results.filter((r) => !r.ok);
console.log(`\n${results.length - failed.length}/${results.length} steps passed; screenshots in ${out}`);
process.exit(failed.length ? 1 : 0);
