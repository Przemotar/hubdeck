// Persona checks of the deck assistant (design reviews, 2026-10-12): each persona gets its own
// test copy with its own apps, time in front, games and modules, says its first message, and the
// run measures what the user would feel and what the drafts are worth.
//
//   node personas.mjs [id ...]      # all personas in ../personas/personas.json, or the named ones
//
// Every persona is one real turn of Claude Code (the owner's plan): about a minute each.
// Writes ../personas/report-<date>.json and prints a table.
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "..", "..");
const personas = JSON.parse(fs.readFileSync(path.join(root, "scripts", "personas", "personas.json"), "utf8"));
const wanted = process.argv.slice(2);
const PORT = 9445;
const dir = path.join(process.env.TEMP, "hubdeck-persona");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");

const ps = (...args) => execFileSync("powershell", ["-NoProfile", "-File", path.join(root, "scripts", "automation.ps1"), ...args], { encoding: "utf8" });

/** The persona's data folder: no profiles of the owner, its own apps, games and time in front. */
function prepare(p) {
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(path.join(dir, "profiles"), { recursive: true });
  const settings = JSON.parse(fs.readFileSync(path.join(real, "settings.json"), "utf8"));
  Object.assign(settings, { drafts: [], pinned: false });
  delete settings.home;
  fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
  const apps = {};
  for (const [exe, hours, days] of p.used) apps[exe] = { seconds: Math.round(hours * 3600), days, lastDay: "2026-10-12" };
  fs.writeFileSync(path.join(dir, "usage.json"), JSON.stringify({ since: "2026-09-28 10:00", apps }, null, 2));
  const installed = {
    apps: p.apps.map(([name, exe]) => ({ name, target: exe, exe, source: "startMenu" })),
    games: p.games.map((name, i) => ({ name, target: `steam://rungameid/${1000 + i}`, exe: "", source: "steam" })),
    version: 2,
  };
  fs.writeFileSync(path.join(dir, "installed.json"), JSON.stringify(installed, null, 2));
  // the apps it was seen in front with (a real user's HubDeck knows their programs' paths)
  const seen = p.used.map(([exe]) => ({ exe, path: `C:/Persona/${exe}`, title: exe.replace(/\.exe$/i, ""), lastSeen: 1791800000 }));
  fs.writeFileSync(path.join(dir, "apps.json"), JSON.stringify(seen, null, 2));
}

async function run(p) {
  prepare(p);
  ps("-Dir", dir, "-CdpPort", String(PORT), "-Modules", p.modules);
  let browser, page;
  try {
    // the window's page comes a few seconds after the start
    for (let i = 0; i < 60 && !page; i++) {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`).catch(() => undefined);
      page = browser?.contexts()[0]?.pages().find((x) => /tauri.localhost|localhost:1420/.test(x.url()));
      if (!page) {
        await browser?.close().catch(() => {});
        await new Promise((r) => setTimeout(r, 1000));
      }
    }
    if (!page) throw new Error("the test copy's window did not come up");
    page.setDefaultTimeout(30000);
    await page.locator(".lib-tabs .tab").nth(2).waitFor();
    if (p.message.startsWith("Zaskocz")) {
      await page.evaluate(() => localStorage.setItem("hubdeck.language", "pl"));
      await page.reload();
    }
    await page.locator(".lib-tabs .tab").nth(2).click();
    // the talk starts from "I know what I want" (the big button is the tour, which needs no AI)
    await page.fill("#as-know", p.message);
    await page.locator(".as-intro .row .tool").click();
    await page.locator(".as-consent").waitFor();
    await page.fill("#as-first", p.message);
    const t0 = Date.now();
    const at = {};
    const mark = (k) => (at[k] ??= Math.round((Date.now() - t0) / 100) / 10);
    await page.locator(".as-consent .tool.primary").first().click();
    let quick;
    for (;;) {
      const s = await page.evaluate(() => ({
        sketches: document.querySelectorAll(".as-msg.is-cards .as-sketch").length,
        text: [...document.querySelectorAll(".as-msg.is-assistant:not(.is-cards) p")].map((x) => x.textContent).join(" ").length,
        busy: !!document.querySelector(".as-work.is-live"),
        err: [...document.querySelectorAll(".as-msg.is-error")].map((e) => e.textContent).join(" | "),
      }));
      if (quick === undefined && Date.now() - t0 > 1500) quick = s.sketches;
      if (s.sketches > 0) mark("firstCard");
      if (s.text > 0) mark("firstText");
      if (quick !== undefined && s.sketches > quick) mark("firstModelIdea");
      if (s.err) return { id: p.id, at, error: s.err };
      if (!s.busy && Date.now() - t0 > 3000) break;
      if (Date.now() - t0 > 300000) {
        at.timeout = true;
        break;
      }
      await page.waitForTimeout(250);
    }
    mark("done");
    // the drafts and what is in them
    const attached = p.modules.split(",").map((m) => m.split("@")[0]);
    // the persona's apps are made up: their paths do not exist here, so they do not count
    const fake = [...p.apps.map(([, exe]) => exe), ...p.used.map(([exe]) => `C:/Persona/${exe}`)].map((x) => x.toLowerCase());
    const drafts = await page.evaluate(async ([attached, fake]) => {
      const inv = window.__TAURI_INTERNALS__.invoke;
      const st = await inv("engine_state");
      const out = [];
      for (const d of st.profiles.filter((x) => x.draft)) {
        const prof = await inv("profile_get", { id: d.id }).catch(() => null);
        const issues = await inv("profile_check", { id: d.id }).catch(() => []);
        const controls = prof ? Object.entries(prof.controls) : [];
        const kind = (id) => id.split("/").pop().split(/[.:]/)[0];
        out.push({
          name: d.name,
          controls: controls.length,
          keys: controls.filter(([id]) => kind(id) === "keys").length,
          notAttached: controls.filter(([id]) => !attached.includes(kind(id))).length,
          unchecked: issues.filter((i) => i.message.startsWith("Shortcut not checked")).map((i) => i.message.split(": ")[1]),
          longLabels: controls.filter(([id, b]) => kind(id) === "keys" && (b.look?.label ?? "").split("\n").some((l) => l.length > 10)).length,
          offHouse: controls.filter(([id, b]) => kind(id) === "keys" && b.look?.bg && b.look.bg.toLowerCase() !== "#1b1a20").length,
          // what the user sees and presses (session 6: text-only keys, keys that did not open,
          // a card promising covers the keys did not have)
          bare: controls.filter(([id, b]) => kind(id) === "keys" && !id.includes(":") && b.action && !b.look?.icon && !b.look?.image).length,
          bareKeys: controls.filter(([id, b]) => kind(id) === "keys" && !id.includes(":") && b.action && !b.look?.icon && !b.look?.image).map(([id, b]) => `${id} ${b.action} ${b.look?.label ?? ""}`),
          wontOpen: issues.filter((i) => i.message.startsWith("Not found on this PC")).map((i) => i.message.split(": ")[1]).filter((t) => !fake.includes(t.toLowerCase())),
          about: [...document.querySelectorAll(".as-sketch")].find((c) => c.querySelector(".as-draft-name")?.textContent === d.name)?.querySelector(".as-sketch-about")?.textContent ?? "",
          promiseBroken: (() => {
            const card = [...document.querySelectorAll(".as-sketch")].find((c) => c.querySelector(".as-draft-name")?.textContent === d.name);
            const about = card?.querySelector(".as-sketch-about")?.textContent ?? "";
            // a picture on a key, one across the panel, the album cover of Now playing, or a game key
            // (made-up persona games have no cover on disk; real ones get theirs)
            const pictures = controls.some(([, b]) => b.look?.image || b.action === "core.nowPlaying" || b.action === "core.game") || !!prof?.panel;
            return /cover|okładk|picture|obraz|animat|gif/i.test(about) && !pictures ? 1 : 0;
          })(),
        });
      }
      return out;
    }, [attached, fake]);
    const text = await page.locator(".as-msg.is-assistant:not(.is-cards)").allTextContents();
    await page.screenshot({ path: path.join(root, "scripts", "personas", `shot-${p.id}.png`) });
    return { id: p.id, at, drafts, words: text.join(" ").split(/\s+/).filter(Boolean).length, text: text.map((x) => x.slice(0, 400)) };
  } catch (e) {
    await page?.screenshot({ path: path.join(root, "scripts", "personas", `fail-${p.id}.png`) }).catch(() => {});
    throw e;
  } finally {
    await browser?.close().catch(() => {});
    ps("-Dir", dir, "-Stop");
  }
}

const results = [];
for (const p of personas.filter((x) => !wanted.length || wanted.includes(x.id))) {
  process.stderr.write(`${p.id}...\n`);
  results.push(await run(p).catch((e) => ({ id: p.id, error: String(e) })));
}
const date = new Date().toISOString().slice(0, 16).replace(/[:T]/g, "-");
fs.writeFileSync(path.join(root, "scripts", "personas", `report-${date}.json`), JSON.stringify(results, null, 2));
for (const r of results) {
  const d = r.drafts ?? [];
  const sum = (k) => d.reduce((a, x) => a + (Array.isArray(x[k]) ? x[k].length : x[k]), 0);
  console.log(
    [r.id.padEnd(13), `card ${r.at?.firstCard ?? "-"}s`, `model ${r.at?.firstModelIdea ?? "-"}s`, `done ${r.at?.done ?? "-"}s`, `drafts ${d.length}`, `controls ${sum("controls")}`, `unchecked ${sum("unchecked")}`, `not attached ${sum("notAttached")}`, `long ${sum("longLabels")}`, `off-house ${sum("offHouse")}`, `bare ${sum("bare")}`, `won't open ${sum("wontOpen")}`, `broken promise ${sum("promiseBroken")}`, `words ${r.words ?? "-"}`, r.error ?? ""].join(" | "),
  );
}
// what must be zero: a persona with any of these failed
const failed = results.filter((r) => r.error || (r.drafts ?? []).some((d) => d.unchecked.length || d.notAttached || d.wontOpen.length || d.promiseBroken || d.bare));
if (failed.length) {
  console.log(`FAILED: ${failed.map((r) => r.id).join(", ")} (unchecked shortcuts, unattached modules, keys that would not open, keys without an icon or promised pictures missing)`);
  process.exitCode = 1;
}
