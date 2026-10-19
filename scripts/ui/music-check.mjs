// Lights to the music on the installed app, checked with numbers (owner's rule, 2026-10-19: after
// any change to the lights, read each module's LEDs that should change, a few samples, and CPU
// over 60 s on his active profile). Samples GET /api/leds ten times a second while music plays:
// every module whose lights follow the music must really change, knobs and faders must stay under
// the rate their firmware takes (the faders dropped off the bus at 50 a second), CPU under 15 %.
// Start it a minute or more after HubDeck starts: the first minute (plugins, pictures) read 14.6 %
// where the same deck took 3.7 % later; its own sampling costs nothing measurable (3.8 % without).
//   node scripts/ui/music-check.mjs [--seconds 60]
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const opt = (name, def) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : def;
};
const seconds = Number(opt("seconds", "60"));
const win = process.platform === "win32";
const data = win ? path.join(process.env.APPDATA, "dev.hubdeck.app") : path.join(os.homedir(), "Library", "Application Support", "dev.hubdeck.app");
const port = fs.readFileSync(path.join(data, "api-http-port"), "utf8").trim();
const token = fs.readFileSync(path.join(data, "api-token"), "utf8").trim();
const api = async (p) => (await fetch(`http://127.0.0.1:${port}/api/${p}`, { headers: { Authorization: `Bearer ${token}` }, signal: AbortSignal.timeout(3000) })).json();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const LIMITS = { cpuPct: 15, knobsFadersPerSec: 30 };

/** CPU seconds HubDeck used so far (its biggest process: the guard is a small second one). */
function cpuSeconds() {
  if (win) {
    const out = execFileSync("powershell", ["-NoProfile", "-Command", "(Get-Process hubdeck | Sort-Object WorkingSet64 -Descending | Select-Object -First 1).TotalProcessorTime.TotalSeconds"], { encoding: "utf8" });
    return Number(out.trim().replace(",", "."));
  }
  const pid = execFileSync("pgrep", ["-x", "hubdeck"], { encoding: "utf8" }).trim().split("\n")[0];
  const parts = execFileSync("ps", ["-o", "time=", "-p", pid], { encoding: "utf8" }).trim().split(/[-:]/).map(Number);
  return parts.reverse().reduce((sum, v, i) => sum + v * [1, 60, 3600, 86400][i], 0);
}

const results = [];
const check = (what, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${what}${detail ? `: ${detail}` : ""}`);
};

const first = await api("leds");
const lights = await api("lights");
// the modules whose lights follow the music now: the window's view names the effect
const music = Object.entries(first.shown ?? {}).filter(([, v]) => (v ?? []).some((l) => l.effect === "music")).map(([id]) => id);
console.log(`music-check: ${seconds} s; following the music: ${music.join(", ") || "none"}; knobs ${lights.musicKnobs ? "on" : "off"}, faders ${lights.musicFaders ? "on" : "off"}`);
const colors = new Map();
const cpu0 = cpuSeconds();
const t0 = Date.now();
while (Date.now() - t0 < seconds * 1000) {
  const now = await api("leds");
  for (const [id, v] of Object.entries(now.sent ?? {})) {
    if (!colors.has(id)) colors.set(id, new Set());
    colors.get(id).add((v.colors ?? ["rainbow"]).join(","));
  }
  await sleep(100);
}
const secs = (Date.now() - t0) / 1000;
const cpu = (100 * (cpuSeconds() - cpu0)) / secs;
const last = await api("leds");
for (const [id, v] of Object.entries(last.sent ?? {})) {
  const perSec = (v.sends - (first.sent?.[id]?.sends ?? 0)) / secs;
  const kind = id.split(":")[0];
  const distinct = colors.get(id)?.size ?? 0;
  const detail = `${perSec.toFixed(1)} sends/s, ${distinct} different colors seen`;
  if (music.includes(id)) check(`${kind} follows the music`, distinct >= 3, `${detail}${distinct < 3 ? " (is music playing?)" : ""}`);
  else console.log(`      ${kind}: ${detail}`);
  if (kind === "knobs" || kind === "faders") check(`${kind} under ${LIMITS.knobsFadersPerSec}/s`, perSec <= LIMITS.knobsFadersPerSec, `${perSec.toFixed(1)}/s`);
}
check("CPU", cpu <= LIMITS.cpuPct, `${cpu.toFixed(1)} % of one core over ${secs.toFixed(0)} s (limit ${LIMITS.cpuPct})`);
console.log(results.every(Boolean) ? "\nPASSED" : "\nFAILED");
process.exitCode = results.every(Boolean) ? 0 : 1;
