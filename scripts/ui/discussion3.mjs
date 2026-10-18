// The three asks of Discussions #3 (2026-10-18) on a test copy of its own (virtual modules, never
// the focus): a font on the deck picked from the computer's list, Japanese on a key; a language
// file in languages/ picked in Settings > General, and Make a language file (Mochi's own pictures
// were tried and left out: owner, 2026-10-18). Screenshots in logs/discussion3-*.png, in the dark and the light look, and no
// cut text at 960 and 1280 wide.
//   node scripts/ui/discussion3.mjs
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const dir = path.join(os.tmpdir(), "hubdeck-discussion3");
const PORT = 9473;
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
const check = (what, ok, note = "") => (results.push({ what, ok, note }), console.log(`${ok ? "ok  " : "FAIL"} ${what}${note ? ` (${note})` : ""}`));
const cuts = [];
let errors = 0;

ps("-Stop");
ps("-Fresh");
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
let page;
for (let i = 0; i < 40 && !page; i++) {
  page = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
  if (!page) await sleep(500);
}
page.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
const invoke = (cmd, args) => page.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args ?? {}]);
const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
const api = (route, body) => fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
const shot = async (name) => {
  for (const width of [960, 1280]) {
    await page.setViewportSize({ width, height: width === 960 ? 640 : 800 });
    await sleep(200);
    const profiles = await page.evaluate(() => [...document.querySelectorAll(".topbar button, header button")].map((b) => b.innerText.trim()));
    for (const c of await clipped(page)) if (!profiles.includes(c.text)) cuts.push(`${name} at ${width}: ${c.text}`);
  }
  for (const look of ["dark", "light"]) {
    await page.evaluate((l) => document.documentElement.setAttribute("data-theme", l), look);
    await sleep(200);
    await page.screenshot({ path: path.join(root, "logs", `discussion3-${name}-${look}.png`) });
  }
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "dark"));
};
const settings = async (section) => {
  await page.getByRole("button", { name: /^(Settings|設定)$/ }).first().click();
  await page.locator(".settings-nav-item", { hasText: section }).click();
  await sleep(500);
};
const back = () => page.locator(".settings-head button").first().click();

try {
  await page.setViewportSize({ width: 1280, height: 800 });
  const state = await invoke("engine_state");
  const profile = state.profiles.find((p) => p.id === state.active) ?? state.profiles[0];

  // 1. a font on the deck, Japanese on a key
  await settings("Text on the deck");
  const select = page.locator("#deck-font");
  await select.scrollIntoViewIfNeeded();
  const fonts = await select.locator("option").allTextContents();
  check("the font list is the computer's fonts", fonts.length > 10, `${fonts.length} options, e.g. ${fonts.slice(1, 4).join(", ")}`);
  const pick = ["Yu Gothic", "Meiryo", "Segoe UI", "Arial"].find((f) => fonts.includes(f)) ?? fonts[1];
  await select.selectOption(pick);
  await sleep(600);
  check("the picked font is kept", (await invoke("screens_get")).deckFont === pick, pick);
  await shot("1-font-setting");
  await back();
  await api(`profiles/${profile.id}/controls`, { control: "keys.0", binding: { action: "core.nextPage", params: {}, look: { label: "ミュート", icon: "mic-off" } } });
  await api(`profiles/${profile.id}/controls`, { control: "keys.1", binding: { action: "core.nextPage", params: {}, look: { label: "配信開始", icon: "radio" } } });
  await sleep(1500);
  await shot("2-japanese-keys");

  // 2. a language file
  fs.mkdirSync(path.join(dir, "languages"), { recursive: true });
  const eo = { name: "Esperanto (test)", strings: { "topbar.settings": "設定", "topbar.plugins": "プラグイン", "topbar.language": "言語", "settings.back": "デッキに戻る", "topbar.modules": { other: "{count} 個のモジュール" } } };
  fs.writeFileSync(path.join(dir, "languages", "eo.json"), JSON.stringify(eo));
  await settings("General");
  const language = page.locator("#language");
  const langs = await language.locator("option").allTextContents();
  check("the language list has the file", langs.includes("Esperanto (test)"), langs.join(", "));
  await language.selectOption("eo");
  await sleep(500);
  const top = await page.locator(".topbar").innerText();
  check("the window speaks the file's words", top.includes("設定") && top.includes("プラグイン") && top.includes("個のモジュール"), top.replace(/\s+/g, " ").slice(0, 160));
  check("what the file lacks stays English", (await page.locator(".settings-nav").innerText()).includes("Lights and screens"));
  await shot("3-language");
  await page.getByRole("button", { name: "Make a language file" }).click();
  await sleep(1500);
  const template = path.join(dir, "languages", "template.json");
  const t = fs.existsSync(template) ? JSON.parse(fs.readFileSync(template, "utf8")) : {};
  check("Make a language file writes every English text", Object.keys(t.strings ?? {}).length > 1000 && Object.keys(t.actions ?? {}).length > 50 && t.strings["topbar.settings"] === "Settings", `${Object.keys(t.strings ?? {}).length} texts, ${Object.keys(t.actions ?? {}).length} actions, ${Object.keys(t.presets ?? {}).length} presets`);
  // Japanese that comes with HubDeck (owner, 2026-10-18)
  check("Japanese is on the list", (await language.locator("option").allTextContents()).filter((x) => x === "日本語").length === 1);
  await language.selectOption({ label: "日本語" });
  await page.waitForFunction(() => document.querySelector(".settings-nav")?.textContent?.includes("ライトと画面"), null, { timeout: 8000 }).catch(() => {});
  const jaTop = await page.locator(".topbar").innerText();
  const jaNav = await page.locator(".settings-nav").innerText();
  check("in Japanese the window is Japanese", jaTop.includes("設定") && jaTop.includes("プラグイン") && jaNav.includes("ライトと画面") && jaNav.includes("デッキの文字"), jaNav.replace(/\s+/g, " ").slice(0, 120));
  await shot("3b-japanese");
  await language.selectOption("en");
  await sleep(300);
  await back();

  check("no page errors", errors === 0, String(errors));
  check("no cut or overflowing text (960 and 1280 wide)", cuts.length === 0, cuts.slice(0, 5).join("; "));
} finally {
  await browser.close().catch(() => {});
  ps("-Stop");
}
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\nFAILED: ${failed} of ${results.length}` : `\npassed ${results.length}/${results.length}`);
process.exit(failed ? 1 : 0);
