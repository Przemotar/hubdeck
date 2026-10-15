#!/usr/bin/env node
// The release gate (owner's pick 1a, 2026-10-14: "ale może nie aż 30 min"): no public release
// unless this passes on Windows and on macOS. It checks, against the INSTALLED app with the deck
// attached:
//   1. audits: `cargo audit` (needs `cargo install cargo-audit`) and `pnpm audit --prod` (high+)
//   2. sizes: the newest installer in target/release/bundle and the installed app
//   3. soak: an animated profile shown for --minutes (default 5); memory and CPU every --every
//      seconds; fails on memory that keeps growing, a high peak, or a busy CPU
//   4. restarts: quit through the local API and start again in the tray, --restarts times; every
//      module that was attached must come back
//
//   node scripts/release-gate.mjs [--profile <name or id>, default: a draft with every GIF and Mochi] [--minutes 5] [--every 5] [--restarts 3]
//                                 [--skip audit,sizes,soak,restarts]
//
// The soak only shows the profile (a try, like the assistant's Try): the user's home and active
// profile come back at the end. Nothing takes the focus: the app is started with --hidden.
import { execFileSync, spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..");
const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const minutes = Number(opt("minutes", "5"));
const every = Number(opt("every", "5"));
const restarts = Number(opt("restarts", "3"));
const skip = new Set(opt("skip", "").split(",").filter(Boolean));
const win = process.platform === "win32";
const mac = process.platform === "darwin";

// limits (measured on the owner's machines, 2026-10-14: 92 MB flat, 2-10 % with Mochi on the dial)
const LIMITS = {
  growthMbPerMin: 3, // steady growth after the first minute
  peakMb: 400,
  cpuAvgPct: 15, // % of one core, with the animated profile on screen
  installerMb: 60,
  installedMb: 200,
};

const dataDir = win
  ? path.join(process.env.APPDATA, "dev.hubdeck.app")
  : path.join(os.homedir(), "Library", "Application Support", "dev.hubdeck.app");
const appPath = win ? path.join(process.env.LOCALAPPDATA, "HubDeck", "hubdeck.exe") : "/Applications/HubDeck.app";

const results = [];
const report = (step, ok, text) => {
  results.push({ step, ok, text });
  console.log(`${ok ? "PASS" : "FAIL"}  ${step}: ${text}`);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// --- the local API -------------------------------------------------------------------------
function api() {
  const token = fs.readFileSync(path.join(dataDir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dataDir, "api-http-port"), "utf8").trim();
  return async (method, route, body) => {
    const res = await fetch(`http://127.0.0.1:${port}/api/${route}`, {
      method,
      headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
      body: body ? JSON.stringify(body) : undefined,
      signal: AbortSignal.timeout(5000),
    });
    if (!res.ok) throw new Error(`${method} /api/${route}: ${res.status} ${await res.text()}`);
    return res.json();
  };
}

async function attached() {
  try {
    return (await api()("GET", "modules")).attached ?? [];
  } catch {
    return null;
  }
}

// --- the process ---------------------------------------------------------------------------
// the app, not its guard (`hubdeck --guard ...`, app/src-tauri/src/guard.rs), which runs the same program
function pid() {
  try {
    if (win) {
      const out = execFileSync("powershell", ["-NoProfile", "-Command", `(Get-CimInstance Win32_Process -Filter "Name='hubdeck.exe'" | Where-Object { $_.ExecutablePath -eq '${appPath}' -and $_.CommandLine -notmatch '--guard' } | Select-Object -First 1).ProcessId`], { encoding: "utf8" }).trim();
      return out ? Number(out) : null;
    }
    const lines = execFileSync("ps", ["-axo", "pid=,args="], { encoding: "utf8" }).split("\n");
    const app = lines.find((l) => l.includes(`${appPath}/Contents/MacOS/hubdeck`) && !l.includes("--guard"));
    return app ? Number(app.trim().split(/\s+/)[0]) : null;
  } catch {
    return null;
  }
}

/** Memory (MB: private bytes on Windows, resident on macOS) and CPU seconds used so far. */
function sample(p) {
  if (win) {
    const out = execFileSync("powershell", ["-NoProfile", "-Command", `$p = Get-Process -Id ${p}; "$($p.PrivateMemorySize64) $($p.TotalProcessorTime.TotalSeconds)"`], { encoding: "utf8" }).trim();
    const [bytes, cpu] = out.split(/\s+/).map((v) => Number(v.replace(",", ".")));
    return { mb: bytes / 1048576, cpu };
  }
  const [rss, time] = execFileSync("ps", ["-o", "rss=,time=", "-p", String(p)], { encoding: "utf8" }).trim().split(/\s+/);
  // time is [[dd-]hh:]mm:ss.cc
  const parts = time.split(/[-:]/).map(Number);
  const cpu = parts.reverse().reduce((sum, v, i) => sum + v * [1, 60, 3600, 86400][i], 0);
  return { mb: Number(rss) / 1024, cpu };
}

function start() {
  if (win) spawn(appPath, ["--hidden"], { detached: true, stdio: "ignore" }).unref();
  else execFileSync("open", ["-g", appPath, "--args", "--hidden"]);
}

// --- 1. audits -----------------------------------------------------------------------------
function audits() {
  cargoAudit();
  pnpmAudit();
}

function cargoAudit() {
  try {
    execFileSync("cargo", ["audit", "--version"], { stdio: "ignore" });
  } catch {
    report("cargo audit", false, "not installed: cargo install cargo-audit --locked");
    return;
  }
  try {
    const out = execFileSync("cargo", ["audit", "--json"], { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
    const j = JSON.parse(out);
    report("cargo audit", true, `${j.vulnerabilities?.count ?? 0} vulnerabilities, ${Object.values(j.warnings ?? {}).flat().length} warnings`);
  } catch (e) {
    let text = "failed to run";
    try {
      const j = JSON.parse(e.stdout);
      // glib only comes through gtk for Linux (docs/KNOWLEDGE.md): not shipped, so not counted
      const found = (j.vulnerabilities?.list ?? []).filter((v) => v.package.name !== "glib");
      if (found.length === 0) {
        report("cargo audit", true, `only Linux-only glib (${j.vulnerabilities.count}), not shipped`);
        return;
      }
      text = found.map((v) => `${v.package.name} ${v.package.version}: ${v.advisory.id} ${v.advisory.title}`).join("; ");
    } catch {}
    report("cargo audit", false, text);
  }
}

function pnpmAudit() {
  try {
    const audit = ["audit", "--prod", "--audit-level", "high"];
    // pnpm is a .cmd on Windows: through cmd, not a shell string
    execFileSync(win ? "cmd" : "pnpm", win ? ["/d", "/c", "pnpm", ...audit] : audit, { cwd: path.join(root, "app"), encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
    report("pnpm audit", true, "no high or critical advisories in production packages");
  } catch (e) {
    report("pnpm audit", false, (e.stdout || e.message).split("\n").filter(Boolean).slice(-3).join(" | "));
  }
}

// --- 2. sizes ------------------------------------------------------------------------------
function dirSize(p) {
  const st = fs.statSync(p);
  if (!st.isDirectory()) return st.size;
  return fs.readdirSync(p).reduce((sum, f) => sum + dirSize(path.join(p, f)), 0);
}

function sizes() {
  const bundle = path.join(root, "target", "release", "bundle", win ? "nsis" : "dmg");
  const files = fs.existsSync(bundle) ? fs.readdirSync(bundle).filter((f) => f.endsWith(win ? ".exe" : ".dmg")).map((f) => path.join(bundle, f)) : [];
  const newest = files.sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs)[0];
  if (newest) {
    const mb = fs.statSync(newest).size / 1048576;
    report("installer size", mb <= LIMITS.installerMb, `${path.basename(newest)} ${mb.toFixed(1)} MB (limit ${LIMITS.installerMb})`);
  } else report("installer size", false, `no installer in ${bundle}`);
  const installed = win ? path.dirname(appPath) : appPath;
  if (fs.existsSync(installed)) {
    const mb = dirSize(installed) / 1048576;
    report("installed size", mb <= LIMITS.installedMb, `${installed} ${mb.toFixed(1)} MB (limit ${LIMITS.installedMb})`);
  } else report("installed size", false, `${installed} is not there`);
}

// --- 3. soak -------------------------------------------------------------------------------
async function soak() {
  const p = pid();
  if (!p) return report("soak", false, `the installed app is not running (${appPath})`);
  const call = api();
  const want = opt("profile", null);
  let tried = null;
  let draft = null;
  const { profiles } = await call("GET", "profiles");
  if (want) {
    const found = profiles.find((x) => x.id === want || x.name === want);
    if (!found) return report("soak", false, `no profile "${want}"`);
    await call("POST", `profiles/${found.id}/try`);
    tried = found.name;
  } else {
    // the busiest deck HubDeck draws: an animation on every key (the GIFs already in the user's
    // pictures, from any profile) and Mochi on the dial, as a draft thrown away at the end
    const gifs = new Set();
    for (const p of profiles) {
      const full = await call("GET", `profiles/${p.id}`);
      for (const b of Object.values(full.controls ?? {})) if (/\.gif$/i.test(b.look?.image ?? "")) gifs.add(b.look.image);
    }
    const list = [...gifs];
    if (list.length === 0) return report("soak", false, "no animated pictures to show: pass --profile <an animated profile>");
    const controls = { dial: { action: "core.pet", params: { name: "Mochi", press: "pet" }, look: { label: "Mochi" } } };
    for (let i = 0; i < 15; i++) controls[`keys.${i}`] = { action: "", look: { label: "", image: list[i % list.length], fit: "fill" } };
    const made = await call("POST", "profiles", { name: "Release gate soak", draft: true, controls });
    draft = made.profile?.id;
    await call("POST", `profiles/${draft}/try`);
    tried = `Release gate soak (${list.length} animations, Mochi)`;
  }
  console.log(`soak: ${minutes} min, a sample every ${every} s, ${tried ? `${tried} shown` : "the active profile"}`);
  const rows = [];
  try {
    let last = sample(p);
    let lastAt = Date.now();
    const end = Date.now() + minutes * 60_000;
    while (Date.now() < end) {
      await sleep(every * 1000);
      const s = sample(p);
      const now = Date.now();
      rows.push({ t: (now - (end - minutes * 60_000)) / 60_000, mb: s.mb, cpu: (100 * (s.cpu - last.cpu)) / ((now - lastAt) / 1000) });
      last = s;
      lastAt = now;
      process.stdout.write(`  ${rows.at(-1).t.toFixed(1)} min  ${s.mb.toFixed(1)} MB  ${rows.at(-1).cpu.toFixed(1)} %\r`);
    }
  } finally {
    if (tried) await call("POST", "try/end").catch(() => {});
    if (draft) await call("POST", `drafts/${draft}/discard`).catch((e) => console.log(`the soak draft stays (throw it away in the Assistant): ${e.message}`));
  }
  console.log();
  const csv = path.join(root, "logs", `release-gate-${process.platform}-${new Date().toISOString().slice(0, 16).replace(/[:T]/g, "")}.csv`);
  fs.mkdirSync(path.dirname(csv), { recursive: true });
  fs.writeFileSync(csv, "minute,mb,cpu_pct\n" + rows.map((r) => `${r.t.toFixed(2)},${r.mb.toFixed(1)},${r.cpu.toFixed(2)}`).join("\n") + "\n");
  // growth: the slope of a line through the samples after the first minute (start-up settles)
  const later = rows.filter((r) => r.t >= 1);
  const n = later.length;
  const mt = later.reduce((s, r) => s + r.t, 0) / n;
  const mm = later.reduce((s, r) => s + r.mb, 0) / n;
  const slope = later.reduce((s, r) => s + (r.t - mt) * (r.mb - mm), 0) / later.reduce((s, r) => s + (r.t - mt) ** 2, 0);
  const peak = Math.max(...rows.map((r) => r.mb));
  const cpuAvg = rows.reduce((s, r) => s + r.cpu, 0) / rows.length;
  const cpuPeak = Math.max(...rows.map((r) => r.cpu));
  // what the deck really got: the engine notes it every 10 s while things move ("deck: keys ...")
  const logFile = win ? path.join(process.env.LOCALAPPDATA, "dev.hubdeck.app", "logs", "hubdeck.log") : path.join(os.homedir(), "Library", "Logs", "dev.hubdeck.app", "hubdeck.log");
  try {
    const notes = fs.readFileSync(logFile, "utf8").split(/\r?\n/).filter((l) => l.includes("] deck: ")).slice(-Math.ceil((minutes * 60) / 10));
    console.log(`deck while soaking (${notes.length} notes):`);
    for (const n of notes.slice(-6)) console.log("  " + n.split("] deck: ")[1]);
  } catch {
    console.log(`no log at ${logFile}`);
  }
  report("soak memory", slope <= LIMITS.growthMbPerMin && peak <= LIMITS.peakMb, `${rows[0].mb.toFixed(1)} -> ${rows.at(-1).mb.toFixed(1)} MB, growth ${slope.toFixed(2)} MB/min after the first minute (limit ${LIMITS.growthMbPerMin}), peak ${peak.toFixed(1)} MB (limit ${LIMITS.peakMb})`);
  report("soak CPU", cpuAvg <= LIMITS.cpuAvgPct, `average ${cpuAvg.toFixed(1)} %, peak ${cpuPeak.toFixed(1)} % of one core (limit ${LIMITS.cpuAvgPct} average); ${csv}`);
}

// --- 4. restarts ---------------------------------------------------------------------------
async function restartLoop() {
  const before = await attached();
  if (!before) return report("restarts", false, "the installed app does not answer on the local API");
  const times = [];
  for (let i = 1; i <= restarts; i++) {
    await api()("POST", "quit");
    let gone = false;
    for (let k = 0; k < 30 && !gone; k++) {
      await sleep(500);
      gone = !pid();
    }
    if (!gone) return report("restarts", false, `restart ${i}: the app did not quit within 15 s`);
    // the modules need a moment after the hand-back (macOS: 6 s settle, docs/KNOWLEDGE.md)
    await sleep(2000);
    const started = Date.now();
    start();
    let now = null;
    while (Date.now() - started < 45_000) {
      await sleep(1000);
      now = await attached();
      if (now && before.every((m) => now.includes(m))) break;
    }
    const missing = before.filter((m) => !(now ?? []).includes(m));
    if (missing.length) return report("restarts", false, `restart ${i}: ${missing.join(", ")} did not come back within 45 s`);
    times.push(((Date.now() - started) / 1000).toFixed(1));
  }
  report("restarts", true, `${restarts} restarts, ${before.join(", ")} back each time (${times.join(" s, ")} s)`);
}

// --- run -----------------------------------------------------------------------------------
if (!win && !mac) {
  console.error("the release gate runs on Windows and macOS");
  process.exit(2);
}
console.log(`release gate on ${win ? "Windows" : "macOS"}, ${new Date().toLocaleString()}`);
if (!skip.has("audit")) audits();
if (!skip.has("sizes")) sizes();
if (!skip.has("soak")) await soak().catch((e) => report("soak", false, e.message));
if (!skip.has("restarts")) await restartLoop().catch((e) => report("restarts", false, e.message));
const failed = results.filter((r) => !r.ok);
console.log(failed.length ? `\nFAILED: ${failed.map((r) => r.step).join(", ")} (do not release)` : "\nPASSED");
process.exit(failed.length ? 1 : 0);
