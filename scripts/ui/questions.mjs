// Every question HubDeck asks before something that cannot be undone, as a user meets it (owner,
// 2026-10-23: "dodaj ten test siedmiu pytań do bramki"; the questions are HubDeck's own since his
// pick 1b that day). Each must come up in the window, name what it is about, and Cancel (or
// Escape) must change nothing: no delete, uninstall, restore or rollback reaches the engine.
// On 2026-10-18 a question stopped asking and a profile went without one; this catches that.
// The import question is the one whose Cancel ("Remove it") does remove: the copy just imported.
// In the browser demo (headless, never the focus), the window's own code over the demo's engine.
//   node scripts/ui/questions.mjs [shots dir]     # needs the Vite server (cd app && pnpm dev)
import { chromium } from "playwright-core";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const base = process.env.HUBDECK_URL || "http://localhost:1460/";
const shots = process.argv[2];
function findChromium() {
  const cache = path.join(os.homedir(), process.platform === "win32" ? "AppData/Local/ms-playwright" : process.platform === "darwin" ? "Library/Caches/ms-playwright" : ".cache/ms-playwright");
  for (const dir of fs.existsSync(cache) ? fs.readdirSync(cache).filter((d) => d.startsWith("chromium")).sort().reverse() : []) {
    const stack = [path.join(cache, dir)];
    while (stack.length) {
      const d = stack.pop();
      for (const e of fs.readdirSync(d, { withFileTypes: true })) {
        const p = path.join(d, e.name);
        if (e.isDirectory() && !p.includes(".app/Contents/Frameworks")) stack.push(p);
        else if (/^(chrome-headless-shell|chrome|Chromium|headless_shell)(\.exe)?$/.test(e.name)) return p;
      }
    }
  }
}
const CHANGES = ["profile_delete", "page_delete", "plugin_uninstall", "backup_restore", "update_rollback", "iconpack_remove"];
const results = [];
const check = (what, ok, note = "") => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${what}${note ? `: ${note}` : ""}`));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
if (shots) fs.mkdirSync(shots, { recursive: true });

const browser = await chromium.launch({ executablePath: findChromium() });
async function open(params = "") {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  // a step that cannot be reached fails in seconds, not in Playwright's 30
  page.setDefaultTimeout(5000);
  await page.goto(`${base}?demo${params}`);
  await page.locator(".dk-ctl").first().waitFor();
  await sleep(300);
  return page;
}
const settings = async (page, section) => {
  if (!(await page.locator(".settings-nav-item").count())) await page.getByRole("button", { name: /^Settings$/ }).first().click();
  await page.locator(".settings-nav-item", { hasText: section }).first().click();
  await sleep(400);
};
/** Ask by `trigger`, check the question names `expect`, answer with `answer` (Cancel or Escape). */
async function question(page, name, trigger, expect, { answer = "Cancel", changes = [] } = {}) {
  // from before the click: a change made without a question counts too
  const before = await page.evaluate(() => (window.__demoCalls ?? []).length);
  try {
    await trigger();
  } catch (e) {
    return check(`${name}: reached`, false, e.message.split("\n")[0]);
  }
  const q = page.getByRole("alertdialog");
  await q.waitFor({ timeout: 3000 }).catch(() => {});
  const said = ((await q.innerText().catch(() => "")) ?? "").replace(/\s+/g, " ").trim();
  check(`${name}: asks in the window`, said.length > 0 && expect.every((e) => said.includes(e)), said || "no question");
  if (shots && said) await q.screenshot({ path: path.join(shots, `question-${name.replace(/\W+/g, "-").toLowerCase()}.png`) });
  if (answer === "Escape") await page.keyboard.press("Escape");
  else await q.getByRole("button", { name: answer, exact: true }).click().catch(() => {});
  await sleep(600);
  const asked = await page.evaluate((n) => (window.__demoCalls ?? []).slice(n), before);
  const made = asked.filter((c) => CHANGES.includes(c));
  check(`${name}: ${answer} closes it${changes.length ? ` and does only ${changes.join(", ")}` : " and changes nothing"}`, (await q.count()) === 0 && JSON.stringify(made) === JSON.stringify(changes), made.join(", ") || "no change");
}

try {
  // 1. Delete page
  let page = await open();
  await page.getByRole("button", { name: "New page" }).click();
  await page.keyboard.type("Walk");
  await page.keyboard.press("Enter");
  await page.locator("button.tab", { hasText: /^Walk$/ }).first().click();
  await sleep(400);
  await question(page, "Delete page", () => page.getByRole("button", { name: "Delete page" }).click(), ['Delete the page "Walk"']);
  await question(page, "Delete page (Escape)", () => page.getByRole("button", { name: "Delete page" }).click(), ['Delete the page "Walk"'], { answer: "Escape" });
  check("Delete page: the page is still there", (await page.locator("button.tab", { hasText: /^Walk$/ }).count()) > 0);

  // 2. Delete profile, from its row in the profile list
  await question(
    page,
    "Delete profile",
    async () => {
      const row = page.locator("li.prow", { hasText: "Night focus" }).first();
      await row.hover();
      await row.getByRole("button", { name: "Delete", exact: true }).click();
    },
    ['Delete the profile "Night focus"'],
  );
  await page.close();

  // 3. Uninstall a plugin, naming the key that uses it
  page = await open();
  await settings(page, "Plugins");
  await question(page, "Uninstall", () => page.locator("li.card", { hasText: "Weather" }).getByRole("button", { name: "Uninstall" }).click(), ["Uninstall Weather?", "Everyday desk", "Key 5"]);

  // 3b. Uninstall from the catalog's card of an installed plugin (owner's pick 1a, 2026-10-23: he
  // looked for it there); the demo's Weather plugin put in the catalog
  const catalogPage = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  catalogPage.setDefaultTimeout(5000);
  await catalogPage.addInitScript(() => {
    window.__DEMO_CATALOG__ = [{ id: "com.example.weather", name: "Weather", author: "Example", description: "Weather on a key", repository: "", icon: "", aiAssisted: false, source: "elgato", popular: 3, stars: 0, updated: "2026-10-01", systems: ["macOS", "Windows"], category: "Weather", dial: false, tried: "Windows" }];
  });
  await catalogPage.goto(`${base}?demo`);
  await catalogPage.locator(".dk-ctl").first().waitFor();
  await catalogPage.getByRole("button", { name: "Plugins", exact: true }).first().click();
  await sleep(600);
  await question(catalogPage, "Uninstall (catalog card)", () => catalogPage.locator("li.plugin-card", { hasText: "Weather" }).getByRole("button", { name: "Uninstall" }).click(), ["Uninstall Weather?", "Everyday desk", "Key 5"]);
  if (shots) await catalogPage.locator("li.plugin-card", { hasText: "Weather" }).screenshot({ path: path.join(shots, "catalog-card-installed.png") });
  await catalogPage.close();

  // 4. Restore a backup
  await settings(page, "Backups");
  await question(page, "Restore backup", () => page.getByRole("button", { name: "Restore", exact: true }).first().click(), ["2026-10-11 09:12"]);

  // 5. Go back to the version before
  await settings(page, "Updates");
  await question(page, "Go back", () => page.getByRole("button", { name: /^Go back to/ }).click(), ["Go back to HubDeck 0.1.0"]);
  await page.close();

  // 6. Remove an icon pack, from a key's Change icon
  page = await open("&icons=pack:neon-shapes");
  await page.locator("g.dk-ctl").filter({ hasNot: page.locator('circle[r="88"]') }).nth(2).click({ force: true });
  await sleep(400);
  await page.getByRole("button", { name: "Change icon" }).first().click();
  await sleep(500);
  await question(page, "Remove icon pack", () => page.getByRole("button", { name: "Remove this pack" }).click(), ["Remove this icon pack?"]);
  await page.close();

  // 7. Import a profile that runs a script: Keep it or Remove it (Remove deletes the copy)
  page = await open("&runs=1");
  // Import sits in the profile's own settings: the active tab of the top bar opens them
  await page.locator("button.tab.is-active").first().click();
  await sleep(500);
  await question(page, "Import that runs things", () => page.getByRole("button", { name: /^Import/ }).first().click(), ["This profile runs things on your computer", "backup-photos.ps1"], { answer: "Remove it", changes: ["profile_delete"] });
  await page.close();
} finally {
  await browser.close();
}
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
