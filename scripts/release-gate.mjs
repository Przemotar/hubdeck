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
//   5. window (Windows): a test copy shows the busiest deck with the screensaver for
//      --window-seconds (150); its page's smoothness and the WebView2 memory
//   6. calls: the calls to other services during the soak, by host, from the app's log (`calls in
//      the last minute:`); fails above --calls-per-hour (720, one every 5 s) for any host (owner's
//      pick 3a, 2026-10-22: Spotify was asked every 3 s)
//
//   node scripts/release-gate.mjs [--profile <name or id>, default: a draft with every GIF and Mochi] [--minutes 5] [--every 5] [--restarts 3]
//                                 [--skip audit,sizes,readme,soak,calls,restarts,window] [--window-seconds 150] [--calls-per-hour 720]
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
  callsPerHour: Number(opt("calls-per-hour", "720")),
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
// --- calls to other services ---------------------------------------------------------------
// The app writes `calls in the last minute: <host> N, ...` once a minute (engine/calls.rs); the
// lines inside the soak are added up per host and turned into calls an hour.
function callsStage() {
  if (!soakWindow) return report("calls", false, "no soak ran: the calls are counted during the soak");
  const logDir = win ? path.join(process.env.LOCALAPPDATA, "dev.hubdeck.app", "logs") : path.join(os.homedir(), "Library", "Logs", "dev.hubdeck.app");
  const files = fs.existsSync(logDir) ? fs.readdirSync(logDir).filter((f) => f.endsWith(".log")).map((f) => path.join(logDir, f)) : [];
  const [from, to] = soakWindow;
  const hosts = {};
  let lines = 0;
  for (const file of files) {
    if (fs.statSync(file).mtimeMs < from) continue;
    for (const line of fs.readFileSync(file, "utf8").split("\n")) {
      const m = line.match(/^\[(\d{4}-\d{2}-\d{2})\]\[(\d{2}:\d{2}:\d{2})\].*calls in the last minute: (.*)$/);
      if (!m) continue;
      const at = new Date(`${m[1]}T${m[2]}`).getTime(); // the log's local time
      if (at < from || at > to + 60_000) continue;
      lines++;
      for (const part of m[3].split(", ")) {
        const [host, n] = part.trim().split(" ");
        if (host && Number(n)) hosts[host] = (hosts[host] ?? 0) + Number(n);
      }
    }
  }
  const hours = (to - from) / 3_600_000;
  const rows = Object.entries(hosts).map(([host, n]) => [host, Math.round(n / hours)]).sort((a, b) => b[1] - a[1]);
  const over = rows.filter(([, n]) => n > LIMITS.callsPerHour);
  const text = rows.length ? rows.map(([h, n]) => `${h} ${n}/h`).join(", ") : `no calls to other services (${lines} log lines)`;
  report("calls", over.length === 0, `${text}; limit ${LIMITS.callsPerHour}/h a host${over.length ? `; over: ${over.map(([h]) => h).join(", ")}` : ""}`);
}

let soakWindow = null;

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
    soakWindow = [end - minutes * 60_000, end];
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

// --- 5. window -----------------------------------------------------------------------------
// The window while the deck moves (owner's pick 2a, 2026-10-16: he kept finding the slow window
// himself): a test copy (scripts/automation.ps1: its own data, virtual modules, the window
// off-screen, never the focus) shows the busiest deck with the screensaver for --window-seconds;
// its page reports every 5 s how smoothly it drew (`window: smoothness`) and the WebView2
// processes' memory is read at the start and the end. Windows only (the test copy is Windows').
// growth: MB a minute after the first minute (as events the deck pictures grew it ~19-27 MB a
// minute; through a channel 0-3, 2026-10-16)
const WINDOW_LIMITS = { framesPerSec: 30, slowShare: 0.05, webviewGrowthMbPerMin: 5 };

function webviewMb(appPid) {
  // the WebView2 processes started by the test copy (the browser process and its children)
  const out = execFileSync("powershell", ["-NoProfile", "-Command", `$all = Get-CimInstance Win32_Process; $ids = @(${appPid}); do { $n = $ids.Count; $ids = @($ids + ($all | Where-Object { $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId } | ForEach-Object { $_.ProcessId })) } while ($ids.Count -gt $n); ($all | Where-Object { $ids -contains $_.ProcessId -and $_.Name -eq 'msedgewebview2.exe' } | ForEach-Object { (Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue).PrivateMemorySize64 } | Measure-Object -Sum).Sum`], { encoding: "utf8" }).trim();
  return Number(out || 0) / 1048576;
}

// The page's garbage collected through DevTools before a memory sample, and its JavaScript heap
// then: WebView2's memory goes up and down by 40 MB with when it collects, so 150 s gave PASS and
// FAIL for the same code (2026-10-18); after a collection what stays is what the page keeps.
async function collectGarbage(cdpPort) {
  try {
    const pages = await (await fetch(`http://127.0.0.1:${cdpPort}/json`, { signal: AbortSignal.timeout(3000) })).json();
    const target = pages.find((p) => p.type === "page" && /localhost:1460|tauri\.localhost/.test(p.url));
    if (!target) return null;
    const ws = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((ok, bad) => ((ws.onopen = ok), (ws.onerror = bad)));
    let id = 0;
    const send = (method) =>
      new Promise((ok) => {
        const me = ++id;
        ws.addEventListener("message", function on(e) {
          const m = JSON.parse(e.data);
          if (m.id === me) (ws.removeEventListener("message", on), ok(m.result));
        });
        ws.send(JSON.stringify({ id: me, method }));
      });
    await send("HeapProfiler.collectGarbage");
    const heap = await send("Runtime.getHeapUsage");
    ws.close();
    return heap ? heap.usedSize / 1048576 : null;
  } catch {
    return null;
  }
}

async function windowStage() {
  if (!win) return report("window", true, "skipped: the test copy runs on Windows only");
  const seconds = Number(opt("window-seconds", "150"));
  // --window-show gifs | saver | both (default): what moves, to tell their costs apart
  const show = opt("window-show", "both");
  const dir = path.join(process.env.TEMP, "hubdeck-gate-window");
  const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", "9450", ...a], { encoding: "utf8" });
  ps("-Stop");
  // the test copy is the debug build: build it with the code being released
  execFileSync("cargo", ["build", "-p", "hubdeck"], { cwd: root, stdio: "inherit" });
  ps("-Fresh");
  try {
    const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
    let port = null;
    for (let i = 0; i < 40 && !port; i++) {
      try {
        port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
      } catch {
        await sleep(500);
      }
    }
    const call = async (method, route, body) => {
      const res = await fetch(`http://127.0.0.1:${port}/api/${route}`, { method, headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined, signal: AbortSignal.timeout(5000) });
      if (!res.ok) throw new Error(`${method} /api/${route}: ${res.status} ${await res.text()}`);
      return res.json();
    };
    const appPid = Number(fs.readFileSync(path.join(dir, "automation.pid"), "utf8").trim().split(/\s+/)[0]);
    // the busiest deck, as in the soak: the user's GIFs on the keys, Mochi on the dial; key 0
    // starts the screensaver (12 s a press)
    const { profiles } = await call("GET", "profiles");
    const gifs = new Set();
    for (const p of profiles) {
      const full = await call("GET", `profiles/${p.id}`);
      for (const b of Object.values(full.controls ?? {})) if (/\.gif$/i.test(b.look?.image ?? "")) gifs.add(b.look.image);
    }
    const list = [...gifs];
    const controls = { dial: { action: "core.pet", params: { name: "Mochi", press: "pet" }, look: { label: "Mochi" } }, "keys.0": { action: "core.saver", params: { effect: "rain" }, look: { label: "Saver" } } };
    for (let i = 1; i < 15 && list.length && show !== "saver"; i++) controls[`keys.${i}`] = { action: "", look: { label: "", image: list[i % list.length], fit: "fill" } };
    const made = await call("POST", "profiles", { name: "Release gate window", controls });
    await call("POST", `profiles/${made.profile.id}/activate`);
    await sleep(5000); // the page settles
    const logFile = path.join(dir, "logs", "hubdeck.log");
    const from = fs.readFileSync(logFile, "utf8").split(/\r?\n/).length;
    const memStart = webviewMb(appPid);
    console.log(`window: ${seconds} s on the test copy, ${list.length} animations, Mochi, the screensaver; WebView2 ${memStart.toFixed(1)} MB`);
    const end = Date.now() + seconds * 1000;
    const mem = [];
    while (Date.now() < end) {
      if (show !== "gifs") await call("POST", "test/input", { kind: "keys", index: 0, gesture: "press", value: 1 });
      await sleep(Math.min(10_000, Math.max(0, end - Date.now())));
      const at = seconds - (end - Date.now()) / 1000;
      const heap = await collectGarbage(9450);
      await sleep(500);
      mem.push({ at, mb: webviewMb(appPid), heap });
      console.log(`  ${at.toFixed(0)} s  WebView2 ${mem.at(-1).mb.toFixed(1)} MB${heap != null ? `, page heap ${heap.toFixed(1)} MB` : ""}`);
    }
    const memEnd = webviewMb(appPid);
    const lines = fs.readFileSync(logFile, "utf8").split(/\r?\n/).slice(from).filter((l) => l.includes("] window: ") || l.includes("[ERROR]"));
    const errors = lines.filter((l) => l.includes("[ERROR]") && l.includes("window: "));
    const smooth = lines
      .map((l) => l.match(/smoothness: deck events ([\d.]+)\/s, deck pictures ([\d.]+)\/s, frames ([\d.]+)\/s, (\d+) over 50 ms, slowest (\d+) ms/))
      .filter(Boolean)
      .map((m) => ({ pictures: Number(m[2]), frames: Number(m[3]), slow: Number(m[4]), worst: Number(m[5]) }));
    for (const l of lines.slice(-4)) console.log("  " + l.split("] window: ")[1]);
    if (smooth.length === 0) return report("window", false, `no smoothness lines in ${seconds} s (the page did not draw, or got no deck pictures)`);
    const frames = smooth.reduce((s, x) => s + x.frames, 0) / smooth.length;
    const pictures = smooth.reduce((s, x) => s + x.pictures, 0) / smooth.length;
    const slowShare = smooth.reduce((s, x) => s + x.slow, 0) / Math.max(1, smooth.reduce((s, x) => s + x.frames * 5, 0));
    const worst = Math.max(...smooth.map((x) => x.worst));
    const settled = mem.filter((m) => m.at >= 60);
    // the slope through every sample after the first minute, not the last minus the first: one
    // sample high or low in the sawtooth moved that by 10 MB/min (2026-10-18)
    const slope = (pts) => {
      if (pts.length < 2) return 0;
      const mx = pts.reduce((a, p) => a + p.x, 0) / pts.length;
      const my = pts.reduce((a, p) => a + p.y, 0) / pts.length;
      const sxx = pts.reduce((a, p) => a + (p.x - mx) ** 2, 0);
      return sxx ? (pts.reduce((a, p) => a + (p.x - mx) * (p.y - my), 0) / sxx) * 60 : 0;
    };
    const growth = slope(settled.map((m) => ({ x: m.at, y: m.mb })));
    const heaps = settled.filter((m) => m.heap != null);
    const heapGrowth = slope(heaps.map((m) => ({ x: m.at, y: m.heap })));
    const ok = errors.length === 0 && frames >= WINDOW_LIMITS.framesPerSec && slowShare <= WINDOW_LIMITS.slowShare && growth <= WINDOW_LIMITS.webviewGrowthMbPerMin;
    report("window", ok, `frames ${frames.toFixed(1)}/s (limit ${WINDOW_LIMITS.framesPerSec}), ${(slowShare * 100).toFixed(1)} % over 50 ms (limit ${WINDOW_LIMITS.slowShare * 100}), slowest ${worst} ms, deck pictures ${pictures.toFixed(1)}/s, WebView2 ${memStart.toFixed(1)} -> ${memEnd.toFixed(1)} MB, growth ${growth.toFixed(1)} MB/min after the first minute (limit ${WINDOW_LIMITS.webviewGrowthMbPerMin}${settled.length < 2 ? "; run --window-seconds 120 or more to measure it" : ""})${heaps.length >= 2 ? `, page heap ${heaps[0].heap.toFixed(1)} -> ${heaps.at(-1).heap.toFixed(1)} MB (${heapGrowth.toFixed(2)} MB/min)` : ""}, ${errors.length} window errors${errors.length ? ": " + errors[0].split("] window: ")[1]?.slice(0, 200) : ""}`);
  } finally {
    ps("-Stop");
  }
}

// --- run -----------------------------------------------------------------------------------
if (!win && !mac) {
  console.error("the release gate runs on Windows and macOS");
  process.exit(2);
}
console.log(`release gate on ${win ? "Windows" : "macOS"}, ${new Date().toLocaleString()}`);
if (!skip.has("audit")) audits();
if (!skip.has("sizes")) sizes();
if (!skip.has("readme")) {
  // the public README (owner's pick 2a, 2026-10-16): run on the export in ../hubdeck
  try {
    execFileSync("node", [path.join(root, "scripts", "readme-check.mjs")], { stdio: "inherit" });
    report("readme", true, "links, render, words and screenshots checked");
  } catch {
    report("readme", false, "see the lines above (node scripts/readme-check.mjs)");
  }
}
if (!skip.has("soak")) await soak().catch((e) => report("soak", false, e.message));
if (!skip.has("calls")) callsStage();
if (!skip.has("restarts")) await restartLoop().catch((e) => report("restarts", false, e.message));
if (!skip.has("window")) await windowStage().catch((e) => report("window", false, e.message));
const failed = results.filter((r) => !r.ok);
console.log(failed.length ? `\nFAILED: ${failed.map((r) => r.step).join(", ")} (do not release)` : "\nPASSED");
process.exit(failed.length ? 1 : 0);
