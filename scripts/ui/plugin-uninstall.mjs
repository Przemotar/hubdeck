// Uninstalling a plugin from Settings > Plugins, as a user does (issue #8, sotomskir, 2026-10-23:
// "I installed some plugin that I don't need anymore, and I can't find a way to uninstall it").
// A test copy of its own (the owner's profiles, virtual modules, no deck, never the focus) with a
// small plugin on a key: Uninstall answered Cancel keeps it, answered Uninstall stops it, takes it
// off the list and deletes its folder. The question is HubDeck's own, in the window (owner's pick
// 1b, 2026-10-23), and names the keys that use the plugin; the walk clicks its buttons.
//   node scripts/ui/plugin-uninstall.mjs [path to hubdeck.exe] [shots dir]   # default: the test build
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] || path.join(root, "target", "test-build", "release", "hubdeck.exe");
const shots = process.argv[3];
const dir = path.join(os.tmpdir(), "hubdeck-plugin-uninstall");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PLUGIN = "dev.hubdeck.test-uninstall";
const PORT = 9476;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json", "plugins"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

// a plugin that answers Stream Deck's handshake and writes a line every second while it runs
const pdir = path.join(dir, "plugins", `${PLUGIN}.sdPlugin`);
fs.mkdirSync(pdir, { recursive: true });
fs.writeFileSync(
  path.join(pdir, "manifest.json"),
  JSON.stringify({
    Name: "Test uninstall", Version: "1.0.0", Author: "HubDeck", Description: "A plugin to uninstall.", Icon: "icon", CodePath: "plugin.js",
    Nodejs: { Version: "20" }, SDKVersion: 2, OS: [{ Platform: "windows", MinimumVersion: "10" }, { Platform: "mac", MinimumVersion: "12" }], Software: { MinimumVersion: "6.4" },
    Actions: [{ Name: "Ping", UUID: `${PLUGIN}.ping`, Icon: "icon", States: [{ Image: "icon" }], Controllers: ["Keypad"] }],
  }),
);
const beat = path.join(dir, "beat.log");
fs.writeFileSync(
  path.join(pdir, "plugin.js"),
  `const a = process.argv; const arg = (k) => a[a.indexOf(k) + 1];
const fs = require("node:fs");
const ws = new WebSocket("ws://127.0.0.1:" + arg("-port"));
ws.onopen = () => ws.send(JSON.stringify({ event: arg("-registerEvent"), uuid: arg("-pluginUUID") }));
setInterval(() => fs.appendFileSync(${JSON.stringify(beat)}, Date.now() + "\\n"), 1000);
`,
);
const beats = () => (fs.existsSync(beat) ? fs.readFileSync(beat, "utf8").trim().split("\n").length : 0);

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
const results = [];
const check = (name, ok, detail) => (results.push(ok), console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`));
let browser;
try {
  console.log(`exe: ${exe}`);
  await api(`profiles/${settings.active}/controls`, { control: "keys.0", binding: { action: `${PLUGIN}/${PLUGIN}.ping`, params: {}, look: {} } });
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(1000);
  const running = beats();
  await sleep(2500);
  check("the plugin runs before", beats() > running, `${beats()} beats`);

  await win.getByRole("button", { name: /^Settings$/ }).first().click();
  await win.locator(".settings-nav-item", { hasText: "Plugins" }).click();
  await sleep(800);
  const card = () => win.locator("li.card", { hasText: "Test uninstall" });
  check("Settings > Plugins lists it", (await card().count()) === 1);
  const button = card().getByRole("button", { name: "Uninstall" });
  check("its card has Uninstall", (await button.count()) === 1);
  if (shots) {
    fs.mkdirSync(shots, { recursive: true });
    await card().screenshot({ path: path.join(shots, "plugin-uninstall-card.png") });
  }
  const question = win.getByRole("alertdialog");
  const asked = async () => ((await question.innerText().catch(() => "")) ?? "").replace(/\s+/g, " ");
  if ((await button.count()) === 1) {
    await button.click();
    await sleep(1000);
    const said = await asked();
    check("it asks first in the window, naming the plugin and its key", said.includes("Uninstall Test uninstall?") && /Key 1(?!\d)/.test(said), said);
    if (shots) await question.screenshot({ path: path.join(shots, "plugin-uninstall-question.png") });
    await win.keyboard.press("Escape");
    await sleep(500);
    check("Escape closes the question, the settings stay open", (await question.count()) === 0 && (await card().count()) === 1);
    await button.click();
    await sleep(800);
    await question.getByRole("button", { name: "Cancel" }).click();
    await sleep(800);
    check("answered Cancel, it stays", (await card().count()) === 1 && fs.existsSync(pdir));

    await button.click();
    await sleep(800);
    await question.getByRole("button", { name: "Uninstall" }).click();
    await sleep(3000);
    check("answered Yes, it leaves the list", (await card().count()) === 0);
    check("its folder is deleted", !fs.existsSync(pdir));
    const after = beats();
    await sleep(5000);
    check("its program has ended (and is not started again)", beats() === after, `${beats() - after} beats in 5 s`);
    const message = await win.locator("section.sec p.help").allTextContents();
    check("no error under the list", !message.some((m) => /could not|error|failed/i.test(m)), message.filter((m) => /could|error|fail/i.test(m)).join(" | "));
    // the key that used it says what is missing
    const controls = (await api(`profiles/${settings.active}`)).controls ?? {};
    check("the key keeps its binding (it can come back with the plugin)", controls["keys.0"]?.action === `${PLUGIN}/${PLUGIN}.ping`);
    const log = fs.readFileSync(path.join(dir, "logs", fs.readdirSync(path.join(dir, "logs")).find((f) => f.endsWith(".log"))), "utf8");
    check("the log says it", log.includes(`plugin ${PLUGIN} uninstalled by the user`));
    if (shots) await win.locator("section.sec", { hasText: "Installed" }).last().screenshot({ path: path.join(shots, "plugin-uninstall-after.png") });

    // one of the owner's own plugins, where it stands in his profiles: Audio Switcher on a key of
    // "Test: real apps"
    const owner = "com.fredemmott.audiooutputswitch";
    const theirs = Object.values(await Promise.all(fs.readdirSync(path.join(dir, "profiles")).filter((f) => f.endsWith(".json")).map(async (f) => JSON.parse(fs.readFileSync(path.join(dir, "profiles", f), "utf8"))))).filter((p) => Object.values(p.controls ?? {}).some((b) => b.action?.startsWith(`${owner}/`)));
    console.log(`     his profiles with ${owner}: ${theirs.map((p) => p.name).join(", ") || "none"}`);
    const ownCard = win.locator("li.card", { hasText: owner });
    if (theirs.length && (await ownCard.count()) === 1) {
      await ownCard.getByRole("button", { name: "Uninstall" }).click();
      await sleep(1000);
      const said = await asked();
      check("his question names his key", said.includes("Test: real apps") && said.includes("Key 15"), said);
      if (shots) await question.screenshot({ path: path.join(shots, "plugin-uninstall-his-question.png") });
      await question.getByRole("button", { name: "Uninstall" }).click();
      await sleep(3000);
      check("his Audio Switcher uninstalls", (await ownCard.count()) === 0 && !fs.existsSync(path.join(dir, "plugins", `${owner}.sdPlugin`)));
      for (const p of theirs) {
        const control = Object.entries(p.controls).find(([, b]) => b.action?.startsWith(`${owner}/`))[0];
        const issues = await win.evaluate((id) => window.__TAURI_INTERNALS__.invoke("profile_check", { id }), p.id);
        const said = issues.filter((i) => i.control.split(":")[0] === control).map((i) => i.message);
        check(`"${p.name}" ${control} says its plugin is missing`, said.length > 0, said.join(" | "));
        // on the deck view, the key carries the warning badge
        await win.getByRole("button", { name: /^Settings$/ }).first().click().catch(() => {});
        await api(`profiles/${p.id}/activate`, {});
        await sleep(1500);
        const badges = await win.locator("g.dk-ctl .dk-check").count();
        check(`"${p.name}" on the deck view shows a warning badge`, badges > 0, `${badges} badge(s)`);
        if (shots) await win.screenshot({ path: path.join(shots, "plugin-uninstall-his-profile.png") });
      }
    } else check("his Audio Switcher and a profile using it are in the test copy", false);
  }
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  await sleep(1500);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
