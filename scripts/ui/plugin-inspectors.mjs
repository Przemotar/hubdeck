// The settings pages of Stream Deck plugins (their property inspectors) in HubDeck's inspector, as
// a user sees them after putting an action on a key (owner, 2026-10-18: Toggle Audio Device's page
// ran off the panel and left empty space under it; "Powinieneś być w stanie z kodu przypisać
// element pluginu do przycisku i sprawdzić jak wygląda"). On a test copy of its own (no deck, never
// the focus): installs each plugin from the catalog, puts each of its actions with a settings page
// on a key, opens it in the window and measures the page: anything past its right edge, empty space
// under it, a cut-off box. A screenshot of each in logs/plugin-inspectors/.
//   node scripts/ui/plugin-inspectors.mjs [catalog.json] [plugin id ...]
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const dir = path.join(os.tmpdir(), "hubdeck-plugin-inspectors");
const out = path.join(root, "logs", "plugin-inspectors");
const PORT = 9476;
const args = process.argv.slice(2);
if (args[0]?.endsWith(".json")) process.env.HUBDECK_PLUGIN_CATALOG = path.resolve(args.shift());
const ids = args.length
  ? args
  : ["com.fredemmott.audiooutputswitch", "com.fredemmott.micmutetoggle", "cz.danol.discordmixer", "com.geekyeggo.sounddeck", "de.perdoctus.streamdeck.homeassistant", "com.moeilijk.hwinfo", "com.chrisregado.googlemeet", "com.jk.weather", "me.amankhanna.oasystem", "com.ntanis.essentials-for-spotify", "com.elgato.philips-hue", "com.github.mjbnz.sd-api-request"];
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });

ps("-Stop");
ps("-Fresh");
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
let page;
for (let i = 0; i < 40 && !page; i++) {
  page = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
  if (!page) await sleep(500);
}
await page.setViewportSize({ width: 1280, height: 800 });
const invoke = (cmd, a) => page.evaluate(([c, x]) => window.__TAURI_INTERNALS__.invoke(c, x), [cmd, a ?? {}]);
const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
const api = (route, body) => fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
const rows = [];
try {
  const state = await invoke("engine_state");
  const profile = state.profiles.find((p) => p.id === state.active) ?? state.profiles[0];
  for (const id of ids) {
    console.log(`installing ${id}`);
    const installed = await Promise.race([invoke("sd_catalog_install", { id }), sleep(90000).then(() => { throw new Error("install took over 90 s"); })]).catch((e) => `ERR ${e}`);
    if (String(installed).startsWith("ERR")) {
      rows.push({ plugin: id, action: "", problem: String(installed).slice(0, 100) });
      continue;
    }
    await sleep(3000);
    const actions = (await invoke("actions_list")).actions.filter((a) => a.id.startsWith(`${installed}/`)).slice(0, 3);
    for (const a of actions) {
      await api(`profiles/${profile.id}/controls`, { control: "keys.14", binding: { action: a.id, params: {}, look: {} } });
      await sleep(600);
      // select key 15 in the window, as a click does
      await page.locator('g.dk-ctl[aria-label^="Key 15"]').first().dispatchEvent("click", {}, { timeout: 5000 }).catch((e) => console.log("click:", e.message.split("\n")[0]));
      const frameEl = page.locator(".sd-inspector iframe");
      const has = await frameEl.waitFor({ timeout: 6000 }).then(() => true, () => false);
      if (!has) {
        rows.push({ plugin: installed, action: a.name, problem: "no settings page" });
        continue;
      }
      await sleep(2500);
      const frame = page.frames().find((f) => f.url().includes("hubdeck=1") && f.url().includes(encodeURIComponent(a.id.split("/").pop())) ) ?? page.frames().find((f) => f.url().includes("hubdeck=1"));
      const box = await frameEl.boundingBox();
      const m = frame
        ? await frame.evaluate(() => {
            const w = window.innerWidth;
            // rects are the page's own layout; a zoomed page is drawn that much smaller, and what a
            // closed <details> holds is not drawn at all
            const z = parseFloat(document.body.style.zoom || "1");
            const past = [...document.querySelectorAll("body *")]
              .filter((el) => {
                const r = el.getBoundingClientRect();
                const s = getComputedStyle(el);
                const shown = !el.checkVisibility || el.checkVisibility({ contentVisibilityAuto: true, visibilityProperty: true });
                return shown && r.width > 0 && r.height > 0 && s.visibility !== "hidden" && r.right * z > w + 1;
              })
              .map((el) => `${el.tagName.toLowerCase()}${el.className ? "." + String(el.className).split(" ")[0] : ""}`);
            const bottom = Math.max(0, ...[...document.body.children].map((el) => el.getBoundingClientRect().bottom));
            return { w, h: window.innerHeight, bottom: Math.round(bottom), past: [...new Set(past)].slice(0, 4), font: getComputedStyle(document.body).fontFamily.split(",")[0], zoom: document.body.style.zoom || "1" };
          })
        : null;
      const problems = [];
      if (!m) problems.push("page not reachable");
      else {
        if (m.past.length) problems.push(`past the edge: ${m.past.join(", ")}`);
        if (m.h - m.bottom > 60) problems.push(`empty space ${m.h - m.bottom}px`);
        if (m.bottom > m.h + 4) problems.push(`cut off ${m.bottom - m.h}px`);
      }
      const file = `${installed}-${a.name}`.replace(/[^a-z0-9.-]+/gi, "_") + ".png";
      if (box) await page.screenshot({ path: path.join(out, file), clip: { x: box.x - 20, y: Math.max(0, box.y - 60), width: Math.min(box.width + 40, 1280 - box.x + 20), height: Math.min(box.height + 80, 800) } }).catch(() => {});
      rows.push({ plugin: installed, action: a.name, size: m ? `${m.w}x${m.h} content to ${m.bottom}` : "", font: m?.font ?? "", zoom: m?.zoom ?? "", problem: problems.join("; ") || "ok" });
      console.log(`${problems.length ? "FAIL" : "ok  "} ${installed} / ${a.name}: ${problems.join("; ") || "fits"}`);
    }
  }
} finally {
  await browser.close().catch(() => {});
  ps("-Stop");
}
console.log("\nplugin | action | page | font | zoom | result");
for (const r of rows) console.log([r.plugin, r.action, r.size ?? "", r.font ?? "", r.zoom ?? "", r.problem].join(" | "));
// actions without a settings page have nothing to fit
const pages = rows.filter((r) => r.problem !== "no settings page");
const bad = pages.filter((r) => r.problem !== "ok").length;
console.log(`\n${pages.length - bad}/${pages.length} settings pages fit (${rows.length - pages.length} actions have none)`);
process.exit(bad ? 1 : 0);
