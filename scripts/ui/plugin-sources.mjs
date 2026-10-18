// Stream Deck plugins from the places people have them (0.1.3; owner's 1a, 2026-10-17: "bezproblemowe
// zainstalowanie pluginow z elgato"). On a test copy of its own (virtual modules, never the focus):
//   1. started with a plugin file as a double click starts it: the window asks, Install installs it;
//   2. a file encrypted for Elgato's app says so and offers no Install;
//   3. Settings > Plugins lists the Stream Deck app's readable plugins (when it is installed), the
//      encrypted ones as one line (owner's 5a, 2026-10-18), and "Use in HubDeck" copies one;
//   4. the Plugins tab (owner's pick A, 2026-10-18): Elgato Marketplace, OpenAction and GitHub with
//      counts, nothing only Elgato's app installs, the filters and the sort do what they say, a
//      GitHub plugin says "Not tried yet", one only on GitHub installs.
// Screenshots in logs/plugin-sources-*.png, in the dark and the light look.
//   node scripts/ui/plugin-sources.mjs [catalog.json]    # the catalog file (scripts/plugins/catalog.mjs);
//                                                         # without it, the published one

import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const dir = path.join(os.tmpdir(), "hubdeck-plugin-sources");
const files = path.join(os.tmpdir(), "hubdeck-plugin-sources-files");
const PORT = 9472;
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const cuts = [];
const shot = async (page, name) => {
  // the owner's smallest window too: nothing cut there either
  for (const width of [960, 1280]) {
    await page.setViewportSize({ width, height: width === 960 ? 640 : 800 });
    await sleep(150);
    // profile tabs in the top bar end in "…" at 960 on purpose (not this change)
    const profiles = await page.evaluate(() => [...document.querySelectorAll(".topbar button, header button")].map((b) => b.innerText.trim()));
    for (const c of await clipped(page)) if (!profiles.includes(c.text)) cuts.push(`${name} at ${width}: ${c.text}`);
  }
  for (const look of ["dark", "light"]) {
    await page.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
    await sleep(150);
    await page.screenshot({ path: path.join(root, "logs", `plugin-sources-${name}-${look}.png`) });
  }
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "dark"));
};

// a real plugin file from its author's GitHub, and one encrypted for Elgato's app
fs.mkdirSync(files, { recursive: true });
const good = path.join(files, "com.fredemmott.audiooutputswitch.streamDeckPlugin");
if (!fs.existsSync(good)) {
  const r = await fetch("https://github.com/fredemmott/StreamDeck-AudioSwitcher/releases/download/v3.2.1/com.fredemmott.audiooutputswitch.streamDeckPlugin");
  fs.writeFileSync(good, Buffer.from(await r.arrayBuffer()));
}
const locked = path.join(files, "com.example.locked.streamDeckPlugin");
execFileSync("powershell", [
  "-NoProfile",
  "-Command",
  `$d = Join-Path $env:TEMP 'hubdeck-locked'; Remove-Item -Recurse -Force $d -ErrorAction SilentlyContinue; $p = New-Item -ItemType Directory -Force (Join-Path $d 'com.example.locked.sdPlugin'); [IO.File]::WriteAllBytes((Join-Path $p 'manifest.json'), [byte[]](69,76,71,65,84,79,1,0,9,9)); Remove-Item -Force '${locked}' -ErrorAction SilentlyContinue; Compress-Archive -Path (Join-Path $d '*') -DestinationPath '${locked}.zip'; Move-Item -Force '${locked}.zip' '${locked}'`,
]);

// a folder like the Stream Deck app's: an encrypted plugin (a copy of one it installed here, when
// there is one), a readable one, and its tutorial
const sdDir = path.join(files, "StreamDeckPlugins");
fs.rmSync(sdDir, { recursive: true, force: true });
fs.mkdirSync(sdDir, { recursive: true });
const elgatoDir = path.join(process.env.APPDATA ?? "", "Elgato", "StreamDeck", "Plugins");
const enc = fs.existsSync(elgatoDir) && fs.readdirSync(elgatoDir).find((d) => d.endsWith(".sdPlugin") && fs.readFileSync(path.join(elgatoDir, d, "manifest.json")).subarray(0, 6).toString() === "ELGATO");
if (enc) fs.cpSync(path.join(elgatoDir, enc), path.join(sdDir, enc), { recursive: true });
else fs.cpSync(path.join(os.tmpdir(), "hubdeck-locked", "com.example.locked.sdPlugin"), path.join(sdDir, "com.example.locked.sdPlugin"), { recursive: true });
const mute = path.join(files, "com.fredemmott.micmutetoggle.streamDeckPlugin");
if (!fs.existsSync(mute)) {
  const r = await fetch("https://github.com/fredemmott/StreamDeck-AudioMute/releases/download/v3.3.1/com.fredemmott.micmutetoggle.streamDeckPlugin");
  fs.writeFileSync(mute, Buffer.from(await r.arrayBuffer()));
}
execFileSync("powershell", ["-NoProfile", "-Command", `Copy-Item '${mute}' '${mute}.zip'; Expand-Archive -Force '${mute}.zip' '${sdDir}'; Remove-Item '${mute}.zip'`]);
fs.mkdirSync(path.join(sdDir, "com.elgato.tutorial.sdPlugin"));
fs.writeFileSync(path.join(sdDir, "com.elgato.tutorial.sdPlugin", "manifest.json"), JSON.stringify({ Name: "Tutorial", CodePath: "app.html", Actions: [{ UUID: "com.elgato.tutorial.a" }] }));
process.env.HUBDECK_STREAM_DECK_PLUGINS = sdDir;
if (process.argv[2]) process.env.HUBDECK_PLUGIN_CATALOG = path.resolve(process.argv[2]);

const results = [];
const check = (what, ok, note = "") => (results.push({ what, ok, note }), console.log(`${ok ? "ok  " : "FAIL"} ${what}${note ? ` (${note})` : ""}`));
let errors = 0;

async function open(file) {
  ps("-Stop");
  ps("-Fresh", "-Open", file);
  const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let page;
  for (let i = 0; i < 40 && !page; i++) {
    page = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!page) await sleep(500);
  }
  page.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  await page.setViewportSize({ width: 1280, height: 800 });
  const invoke = (cmd, args) => page.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args ?? {}]);
  return { browser, page, invoke };
}

try {
  // 1. a double click on a plugin file
  {
    const { browser, page, invoke } = await open(good);
    const sheet = page.locator(".plugin-file");
    await sheet.waitFor({ timeout: 15000 }).catch(() => {});
    await sheet.getByText("Fred Emmott").first().waitFor({ timeout: 8000 }).catch(() => {});
    const text = (await sheet.textContent().catch(() => "")) ?? "";
    check("the window asks before installing the opened file", text.includes("Install this Stream Deck plugin?") && text.includes("Fred Emmott") && text.includes("com.fredemmott.audiooutputswitch.streamDeckPlugin"), text.slice(0, 200));
    await shot(page, "1-asked");
    await sheet.getByRole("button", { name: "Install" }).click();
    await page.getByText(/Installed .*What should it do/).first().waitFor({ timeout: 20000 }).catch(() => {});
    const plugins = await invoke("plugins_list");
    check("Install installs it", plugins.some((p) => p.id === "com.fredemmott.audiooutputswitch"), plugins.map((p) => p.id).join(", "));
    await shot(page, "2-installed");
    await sheet.locator("button.tool", { hasText: "Close" }).click();
    check("Close closes the question", (await sheet.count()) === 0);

    // 3. the Stream Deck app's plugins: a folder like its own (a copy of an encrypted plugin it
    // installed on this PC, a readable one, and its tutorial)
    const sd = await invoke("sd_stream_deck");
    console.log("     Stream Deck app's plugins:", sd.map((p) => `${p.id}${p.problem ? ` [${p.problem}]` : ""}`).join("; ") || "none");
    await page.getByRole("button", { name: /^Settings$/ }).first().click();
    await page.locator(".settings-nav-item", { hasText: "Plugins" }).click();
    await sleep(800);
    if (sd.length) {
      const section = page.locator("section.sec", { hasText: "Plugins from the Stream Deck app" });
      await section.waitFor({ timeout: 5000 }).catch(() => {});
      check("Settings > Plugins lists the Stream Deck app's plugins", (await section.count()) === 1);
      const enc = sd.filter((p) => p.problem?.includes("encrypted"));
      const sectionText = (await section.textContent()) ?? "";
      if (enc.length) check("encrypted ones are one line, not cards", enc.every((p) => !sectionText.includes(p.id) && !sectionText.includes(p.name)) && /more from the Stream Deck app: Elgato encrypted/.test(sectionText), sectionText.slice(-160));
      await section.scrollIntoViewIfNeeded().catch(() => {});
      await shot(page, "3-stream-deck-app");
      const usable = sd.find((p) => !p.problem);
      check("the Stream Deck app's own parts are not listed", !sd.some((p) => p.id === "com.elgato.tutorial"));
      if (usable) {
        await section.locator("li", { hasText: usable.name }).getByRole("button", { name: "Use in HubDeck" }).click();
        await sleep(3000);
        check(`"Use in HubDeck" copies ${usable.id}`, (await invoke("plugins_list")).some((p) => p.id === usable.id));
      }
    } else {
      check("no Stream Deck app: no section", (await page.getByText("Plugins from the Stream Deck app").count()) === 0);
    }

    // 4. the Plugins tab
    await page.getByRole("button", { name: "Browse plugins" }).click();
    const search = page.getByRole("searchbox", { name: "Search plugins" });
    await search.waitFor({ timeout: 30000 });
    await page.locator(".plugin-card").first().waitFor({ timeout: 60000 }).catch(() => {});
    const catalog = await invoke("sd_catalog");
    const by = (src) => catalog.filter((e) => e.source === src).length;
    check("the tab has Elgato Marketplace, OpenAction and GitHub", by("elgato") >= 30 && by("openaction") >= 100 && by("github") >= 50, `elgato ${by("elgato")}, openaction ${by("openaction")}, github ${by("github")}`);
    check("nothing only Elgato's app installs", !catalog.some((e) => e.id.startsWith("elgato:") || (e.name === "Volume Controller" && e.author === "Elgato")));
    const filters = page.locator(".plugins-filters");
    const ghCount = Number(await filters.locator("label", { hasText: "GitHub" }).locator(".n").textContent());
    check("the source counts are the catalog's", ghCount > 0 && ghCount <= by("github"), `GitHub ${ghCount}`);
    const firstCard = (await page.locator(".plugin-card strong").first().textContent()) ?? "";
    check("most popular first: Elgato's best ranked", catalog.filter((e) => e.popular).sort((a, b) => a.popular - b.popular)[0]?.name === firstCard, firstCard);
    await shot(page, "4-catalog");
    const cards = () => page.locator(".plugin-card").count();
    await filters.getByLabel("Tried in HubDeck").check();
    await sleep(300);
    const triedTags = await page.locator(".plugin-card").evaluateAll((els) => els.every((el) => el.textContent.includes("Tried in HubDeck")));
    check("Tried in HubDeck shows only tried ones", (await cards()) > 5 && triedTags, String(await cards()));
    await filters.getByLabel("Tried in HubDeck").uncheck();
    await filters.getByLabel("Uses the dial and knobs").check();
    await sleep(300);
    const dialTags = await page.locator(".plugin-card").evaluateAll((els) => els.every((el) => [...el.querySelectorAll(".pill")].some((p) => p.textContent === "Dial")));
    check("Uses the dial shows only dial plugins", (await cards()) > 5 && dialTags, String(await cards()));
    await filters.getByLabel("Uses the dial and knobs").uncheck();
    for (const s of ["Elgato Marketplace", "OpenAction"]) await filters.getByLabel(s).uncheck();
    await page.getByLabel("Sort").selectOption("updated");
    await sleep(300);
    const gh = await page.locator(".plugin-card").evaluateAll((els) => els.slice(0, 12).map((el) => el.textContent));
    check("GitHub only: every card from GitHub, not tried yet", gh.length > 0 && gh.every((x) => x.includes("GitHub") && (x.includes("Not tried yet") || x.includes("Tried in HubDeck"))), gh[0]?.slice(0, 80));
    await shot(page, "5-github");
    for (const s of ["Elgato Marketplace", "OpenAction"]) await filters.getByLabel(s).check();
    await page.getByLabel("Sort").selectOption("popular");
    const toSearch = () => page.evaluate(() => document.querySelector('input[type="search"]')?.scrollIntoView({ block: "start" }));
    await search.fill("RAM");
    await sleep(300);
    await toSearch();
    const ram = page.locator(".plugin-card", { hasText: "Michail" }).first();
    await ram.getByRole("button", { name: "Install" }).click();
    await page.waitForFunction(() => !document.body.innerText.includes("Installing…"), null, { timeout: 60000 }).catch(() => {});
    await sleep(1500);
    const err = (await ram.locator(".catalog-err").textContent().catch(() => "")) ?? "";
    check("a plugin only on GitHub installs from the catalog", (await invoke("plugins_list")).some((p) => p.id === "eu.dirondin.streamdeck.ram"), err);
    check("an installed plugin says so on its card", (await ram.locator(".pill.is-ok", { hasText: "Installed" }).count()) === 1);
    await shot(page, "6-github-only");
    await browser.close().catch(() => {});
  }
  // 2. a file encrypted for Elgato's app
  {
    const { browser, page } = await open(locked);
    const sheet = page.locator(".plugin-file");
    await sheet.waitFor({ timeout: 15000 }).catch(() => {});
    await page.getByText("HubDeck cannot run it").first().waitFor({ timeout: 8000 }).catch(() => {});
    const text = (await sheet.textContent().catch(() => "")) ?? "";
    check("an encrypted file says so and offers no Install", text.includes("encrypted for Elgato's Stream Deck app") && (await sheet.getByRole("button", { name: "Install" }).count()) === 0, text.slice(0, 160));
    await shot(page, "7-encrypted");
    await browser.close().catch(() => {});
  }
  check("no page errors", errors === 0, String(errors));
  check("no cut or overflowing text (960 and 1280 wide)", cuts.length === 0, cuts.slice(0, 5).join("; "));
} finally {
  ps("-Stop");
}
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\nFAILED: ${failed} of ${results.length}` : `\npassed ${results.length}/${results.length}`);
process.exit(failed ? 1 : 0);
