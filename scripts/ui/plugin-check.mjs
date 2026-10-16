// Do Stream Deck plugins from the catalog run in HubDeck? (owner, 2026-10-16: "A te stream deck
// plugins to dobrze działają? Bo nawet tego dobrze nie sprawdzałem".) On a test copy of its own
// (no deck, never the focus): installs each plugin from the catalog, waits for it to start and
// connect, puts its first action on a key and checks the key got a picture or a title from it.
//   node scripts/ui/plugin-check.mjs [catalog id ...]     # default: five popular ones
// Prints a table; the deck part (presses on the real deck) is for the owner afterwards.
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const dir = path.join(os.tmpdir(), "hubdeck-plugin-check");
const PORT = 9470;
const ids = process.argv.slice(2).length
  ? process.argv.slice(2)
  : ["com.ntanis.essentials-for-spotify", "me.amankhanna.oadiscord", "me.amankhanna.oasystem", "com.jk.weather", "com.victormarin.volume-controller"];
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

ps("-Stop");
ps("-Fresh");
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
let page;
for (let i = 0; i < 40 && !page; i++) {
  page = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1420/.test(p.url()));
  if (!page) await sleep(500);
}
const invoke = (cmd, args) => page.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args ?? {}]);
const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
const api = (route, body) => fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());

const rows = [];
try {
  const state = await invoke("engine_state");
  const profile = state.active;
  for (const [n, id] of ids.entries()) {
    const row = { id, installed: "", started: "", connected: false, actions: 0, drew: false, cpu: "", memory: "", note: "" };
    try {
      const pluginId = await invoke("sd_catalog_install", { id });
      row.installed = pluginId;
      // it starts and registers in a few seconds
      let info;
      for (let i = 0; i < 30; i++) {
        await sleep(1000);
        info = (await invoke("plugins_list")).find((p) => p.id === pluginId);
        if (info?.connected) break;
      }
      row.started = info?.status ?? "not listed";
      row.connected = !!info?.connected;
      row.actions = info?.actions ?? 0;
      const action = (await invoke("actions_list")).actions.find((a) => a.id.startsWith(`${pluginId}/`));
      if (action) {
        const control = `keys.${n % 15}`;
        await api(`profiles/${profile}/controls`, { control, binding: { action: action.id, params: {}, look: {} } });
        await sleep(8000);
        const looks = (await invoke("live_looks"))?.looks ?? {};
        const look = looks[control];
        row.drew = !!(look && (look.image || look.title || look.label));
        row.note = action.name;
      } else row.note = "no action in the catalog";
      info = (await invoke("plugins_list")).find((p) => p.id === pluginId);
      row.cpu = info?.cpu != null ? `${info.cpu.toFixed(1)} %` : "";
      row.memory = info?.memoryMb != null ? `${info.memoryMb} MB` : "";
    } catch (e) {
      row.note = String(e).slice(0, 120);
    }
    rows.push(row);
    console.log(JSON.stringify(row));
  }
} finally {
  await browser.close().catch(() => {});
  ps("-Stop");
}
console.log("\nplugin | started | connected | drew a key | CPU | memory | first action / note");
for (const r of rows) console.log([r.id, r.started, r.connected ? "yes" : "no", r.drew ? "yes" : "no", r.cpu, r.memory, r.note].join(" | "));
