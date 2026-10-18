// From the Plugins tab to a plugin's action on a key, as a new user goes (owner, 2026-10-18: "Jak
// wchodzę w plugins to jak mam ich użyć? ... czy to jest intuicyjne i wygodne"). On a test copy of
// its own (virtual modules, never the focus): install a plugin in the tab, then find its actions
// and put one on a key; every step gets a screenshot (logs/plugin-use-*.png) and a note of what the
// window says next.
//   node scripts/ui/plugin-use.mjs [catalog.json]
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const dir = path.join(os.tmpdir(), "hubdeck-plugin-use");
const PORT = 9474;
if (process.argv[2]) process.env.HUBDECK_PLUGIN_CATALOG = path.resolve(process.argv[2]);
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
const check = (what, ok, note = "") => (results.push({ what, ok, note }), console.log(`${ok ? "ok  " : "FAIL"} ${what}${note ? ` (${note})` : ""}`));
let n = 0;

ps("-Stop");
ps("-Fresh");
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
let page;
for (let i = 0; i < 40 && !page; i++) {
  page = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
  if (!page) await sleep(500);
}
const invoke = (cmd, args) => page.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args ?? {}]);
const shot = async (name) => page.screenshot({ path: path.join(root, "logs", `plugin-use-${++n}-${name}.png`) });
try {
  await page.setViewportSize({ width: 1280, height: 800 });
  // a plugin installed here before the walk would hide the path: none of the one we use
  const id = "com.fredemmott.audiooutputswitch";
  await page.getByRole("button", { name: "Plugins" }).first().click();
  await page.locator(".plugin-card").first().waitFor({ timeout: 60000 });
  await page.getByRole("searchbox", { name: "Search plugins" }).fill("Audio Switcher");
  await sleep(400);
  const card = page.locator(".plugin-card", { hasText: "Audio Switcher" }).first();
  await shot("tab");
  await card.getByRole("button", { name: "Install" }).click();
  await page.waitForFunction(() => !document.body.innerText.includes("Installing…"), null, { timeout: 90000 }).catch(() => {});
  await sleep(1500);
  const after = ((await card.innerText()) ?? "").replace(/\s+/g, " ");
  check("installed from the tab", (await invoke("plugins_list")).some((p) => p.id === id), after.slice(0, 160));
  console.log("     the card after Install says:", after);
  await shot("installed");
  // "Use it": the deck, with the library open at the plugin's actions (owner's 1a, 2026-10-18)
  await card.getByRole("button", { name: "Use it" }).click();
  await sleep(1000);
  check("Use it closes the tab", (await page.locator(".plugins-tab").count()) === 0);
  const group = page.locator(".chooser-group.is-focus");
  const groupText = ((await group.innerText().catch(() => "")) ?? "").replace(/\s+/g, " ");
  check("the library opens at its actions, marked New, with how to use them", /Audio Switcher/.test(groupText) && /New/.test(groupText) && /Drag an action onto a key/.test(groupText), groupText.slice(0, 200));
  const firstGroup = ((await page.locator(".chooser-group .nm").first().innerText()) ?? "").trim();
  check("a new plugin comes first in the library", firstGroup === "Audio Switcher", firstGroup);
  await shot("use-it");
  // drag its first action onto a key, as the hint says
  const item = group.locator(".chooser-item").first();
  const actionName = (await item.innerText()).trim();
  const key = page.locator("g.dk-ctl").filter({ hasNot: page.locator('circle[r="88"]') }).nth(5);
  // a real mouse drag, as the owner does it (2026-10-18: synthetic drag events passed while the
  // window's system drop handler stopped every real drag on Windows)
  const from = await item.boundingBox();
  const to = await key.boundingBox();
  console.log("     drag from", JSON.stringify(from), "to", JSON.stringify(to));
  if (from && to) {
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(from.x + from.width / 2 - 40, from.y + from.height / 2, { steps: 5 });
    await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 20 });
    await page.mouse.up();
  }
  await sleep(1500);
  const inspector = ((await page.locator("aside.inspector").first().innerText().catch(() => "")) ?? "").replace(/\s+/g, " ");
  check("its action lands on the key and its settings show", !inspector.includes("ALL ACTIONS") && inspector.includes(actionName), `${actionName}: ${inspector.slice(0, 160)}`);
  await shot("on-a-key");
  // a plugin file dropped on the window (the window's own drop since 2026-10-18): the question
  // with its name comes up
  const file = path.join(os.tmpdir(), "hubdeck-plugin-sources-files", "com.fredemmott.micmutetoggle.streamDeckPlugin");
  if (fs.existsSync(file)) {
    const b64 = fs.readFileSync(file).toString("base64");
    await page.evaluate((data) => {
      const bytes = Uint8Array.from(atob(data), (c) => c.charCodeAt(0));
      const dt = new DataTransfer();
      dt.items.add(new File([bytes], "com.fredemmott.micmutetoggle.streamDeckPlugin"));
      for (const type of ["dragenter", "dragover", "drop"]) window.dispatchEvent(new DragEvent(type, { dataTransfer: dt, bubbles: true, cancelable: true }));
    }, b64);
    const sheet = page.locator(".plugin-file");
    await sheet.getByText("Fred Emmott").first().waitFor({ timeout: 15000 }).catch(() => {});
    const text = ((await sheet.textContent().catch(() => "")) ?? "").replace(/\s+/g, " ");
    check("a plugin file dropped on the window asks before installing", text.includes("Install this Stream Deck plugin?") && text.includes("Audio Mute"), text.slice(0, 160));
    await shot("dropped-file");
  } else console.log("     no plugin file for the drop (run plugin-sources.mjs once)");
} finally {
  await browser.close().catch(() => {});
  ps("-Stop");
}
console.log(`\n${results.filter((r) => r.ok).length}/${results.length} checks`);
