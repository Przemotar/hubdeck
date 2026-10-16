// The task test (owner, 2026-10-13: "czy sama apka nie jest przekombinowana?"): eight things a
// newcomer does first, each done the shortest way the window offers, counting what it costs: clicks
// (and picks in a list), fields typed, and screens passed through (a page, a panel or a picker that
// replaces what was there). Runs headless against the browser demo (the vite server on :1420, i.e.
// the dev app or `pnpm dev` in app/), so it takes no focus. Run it before and after a
// simplification: the numbers should go down, never up.
//
//   node scripts/ui/tasks.mjs [out-dir]     # a screenshot of each task's end lands in out-dir
//
// Exit code 1 when a task cannot be done at all.
import { chromium } from 'playwright-core';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const out = process.argv[2] || path.join(os.tmpdir(), 'hubdeck-tasks');
fs.mkdirSync(out, { recursive: true });
const base = process.env.HUBDECK_URL || 'http://localhost:1420/';

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

/** Where the user is: the page or panel that holds the window now (a change = one more screen). */
const where = (page) =>
  page.evaluate(() => {
    const shown = (sel) => [...document.querySelectorAll(sel)].some((el) => el.getBoundingClientRect().width > 0);
    const parts = [];
    for (const [name, sel] of [
      ['settings', '.settings-page'],
      ['tour', '.tour-page'],
      ['builder', '.builder-page'],
      ['setup', '.setup-page'],
      ['dialog', '[role="dialog"], .modal, .picker-pop, .icon-picker'],
      ['profile panel', '.profile-panel, .side-panel'],
      ['new profile menu', '.new-menu, .menu-pop'],
    ])
      if (shown(sel)) parts.push(name);
    return parts.join('+') || 'deck';
  });

/** One task: its steps through `t`, which counts and checks as it goes. */
function tracker(page) {
  const t = { clicks: 0, typed: 0, screens: [], async look() {
    const w = await where(page);
    if (t.screens.at(-1) !== w) t.screens.push(w);
  } };
  t.click = async (locator) => {
    await locator.first().click();
    t.clicks += 1;
    await page.waitForTimeout(250);
    await t.look();
  };
  t.pick = async (select, option) => {
    await page.locator(select).first().selectOption(option);
    t.clicks += 1;
    await page.waitForTimeout(250);
    await t.look();
  };
  t.type = async (locator, text, enter = false) => {
    await locator.first().fill(text);
    if (enter) await locator.first().press('Enter');
    t.typed += 1;
    await page.waitForTimeout(250);
    await t.look();
  };
  return t;
}

// actions are chosen in the right panel only (owner's pick 1a, 2026-10-16): "What should it
// do?" is open on an empty control, behind Change on a set one; the first picks for the kind of
// control are one click, anything else is typed into its search
const libItem = async (page, t, name) => {
  if (!(await page.locator('.inspector .chooser').isVisible())) await t.click(page.locator('.inspector .action-card button', { hasText: 'Change' }));
  const tile = page.locator('.chooser-tile', { has: page.locator('.nm', { hasText: new RegExp(`^${name}$`) }) });
  if (await tile.count()) return tile;
  await t.type(page.locator('#action-search'), name);
  return page.locator('.chooser-item', { has: page.locator('.nm', { hasText: new RegExp(`^${name}$`) }) });
};
const key = (page, i) => page.locator('.dk-ctl').nth(i);
const KNOB = 16; // the first knob in the demo's drawing (after the 15 keys and the rollers)

const TASKS = [
  {
    name: 'A key opens an app (Spotify)',
    run: async (page, t) => {
      await t.click(key(page, 2));
      await t.click(await libItem(page, t, 'Open'));
      // an empty app field opens its list by itself
      if (!(await page.getByPlaceholder('Search apps').isVisible())) await t.click(page.getByRole('button', { name: 'Pick an app' }));
      await t.click(page.locator('.inspector button', { hasText: /^Spotify$/ }));
      return /spotify/i.test(await page.locator('#param-target').inputValue());
    },
  },
  {
    name: 'A key presses a shortcut (Copy)',
    run: async (page, t) => {
      await t.click(key(page, 3));
      await t.click(await libItem(page, t, 'Hotkey'));
      await t.pick('select:has(option:text-is("Common shortcut..."))', { index: 1 });
      return /C$/.test(await page.locator('input[placeholder^="e.g. Ctrl"]').inputValue());
    },
  },
  {
    name: "A knob sets one app's volume (Spotify)",
    run: async (page, t) => {
      await t.click(key(page, KNOB));
      await t.click(await libItem(page, t, 'Volume'));
      await t.pick('#param-target', { label: 'One app' });
      // the app is picked from a list (open now, used recently, installed), not typed
      if (!(await page.getByPlaceholder('Search apps').isVisible())) await t.click(page.locator('#param-app'));
      await t.click(page.locator('.menu-row', { hasText: /^Spotify/ }).first());
      return /Spotify/.test(await page.locator('#param-app').innerText());
    },
  },
  {
    name: 'A key mutes the microphone',
    run: async (page, t) => {
      await t.click(key(page, 4));
      await t.click(await libItem(page, t, 'Microphone mute'));
      return (await page.locator('#action-name').getAttribute('data-action')) === 'core.mic';
    },
  },
  {
    name: "A key's color and icon",
    run: async (page, t) => {
      await t.click(key(page, 5));
      await t.click(page.locator('.inspector .look button[aria-label^="#"], .inspector .look button[title^="#"]').nth(1));
      await t.click(page.getByRole('button', { name: 'Change icon' }));
      await t.click(page.locator('.icon-popover .icon-cell').nth(5));
      return !(await page.locator('.icon-popover').isVisible());
    },
  },
  {
    name: 'A deck for an app that comes by itself (Discord)',
    run: async (page, t) => {
      const before = await page.locator('.topbar .profile-chip, header button').count();
      await t.click(page.getByRole('button', { name: 'New profile' }));
      await t.click(page.getByRole('button', { name: /For an app or game/ }));
      await t.click(page.getByRole('button', { name: /^Discord/ }).last());
      return (await page.locator('header button').count()) >= before;
    },
  },
  {
    name: 'The edge light in one color',
    run: async (page, t) => {
      // the Edge chip opens this profile's lights; every profile's are one click on (owner's pick 3a, 2026-10-16)
      await t.click(page.locator('button.dk-chip', { hasText: 'Edge' }));
      await t.click(page.getByRole('button', { name: 'Lights for every profile' }));
      await t.pick(`select[aria-label="Light style"], .settings-content select`, 'color');
      const ok = await page.locator('.settings-content input[type=color]').first().isVisible();
      await page.getByRole('button', { name: /Back to the deck/ }).click();
      return ok;
    },
  },
  {
    name: 'A second page of keys and a key that opens it',
    run: async (page, t) => {
      await t.click(page.getByRole('button', { name: 'New page' }));
      await t.type(page.locator('.tab-input'), 'Games', true);
      await t.click(page.locator('.page-bar .tab', { hasText: 'Main' }));
      await t.click(key(page, 6));
      await t.click(await libItem(page, t, 'Go to page'));
      const select = page.locator('#param-page');
      if (await select.count()) await t.pick('#param-page', { label: 'Games' });
      return true;
    },
  },
];

const browser = await chromium.launch({ executablePath: findChromium() });
const rows = [];
for (const [i, task] of TASKS.entries()) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  page.setDefaultTimeout(4000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e.message || e).split('\n')[0]));
  await page.goto(`${base}?demo`);
  await page.waitForSelector('.dk-ctl', { timeout: 15000 });
  const t = tracker(page);
  await t.look();
  let ok = false;
  let note = '';
  try {
    ok = (await task.run(page, t)) === true;
  } catch (e) {
    note = String(e.message || e).split('\n')[0];
  }
  if (errors.length) (ok = false), (note = `${note ? `${note}; ` : ''}script error: ${errors[0]}`);
  await page.screenshot({ path: path.join(out, `${String(i + 1).padStart(2, '0')}.png`) }).catch(() => {});
  rows.push({ task: task.name, ok, clicks: t.clicks, typed: t.typed, screens: t.screens.length, path: t.screens.join(' > '), note });
  await page.close();
}
await browser.close();

for (const r of rows) console.log(`${r.ok ? 'ok  ' : 'FAIL'} ${r.task}: ${r.clicks} clicks, ${r.typed} typed, ${r.screens} screens (${r.path})${r.note ? ` (${r.note})` : ''}`);
const total = rows.reduce((n, r) => n + r.clicks + r.typed, 0);
console.log(`\n${rows.filter((r) => r.ok).length}/${rows.length} tasks done; ${total} actions in all; screenshots in ${out}`);
process.exit(rows.every((r) => r.ok) ? 0 : 1);
