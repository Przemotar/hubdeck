// A timer that ran out where it cannot be seen says so in the window, with Stop (owner, 2026-10-16:
// a timer on another page or profile only lit the base's edge). On a test copy of its own (virtual
// modules, never the focus): a 3 s timer on the first key of one profile, started, then another
// profile shown; the window must name the timer and its profile, and Stop must clear it.
//   node scripts/ui/timer-check.mjs           # screenshots in logs/timer-check-*.png
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const dir = path.join(os.tmpdir(), "hubdeck-timer-check");
const PORT = 9471;
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const shot = (page, name) => page.screenshot({ path: path.join(root, "logs", `timer-check-${name}.png`) });

ps("-Stop");
ps("-Fresh");
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
let page;
for (let i = 0; i < 40 && !page; i++) {
  page = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
  if (!page) await sleep(500);
}
const invoke = (cmd, args) => page.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args ?? {}]);
const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
const api = (route, body) => fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());

const results = [];
const check = (what, ok, note = "") => (results.push({ what, ok, note }), console.log(`${ok ? "ok  " : "FAIL"} ${what}${note ? ` (${note})` : ""}`));
let errors = 0;
page.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
try {
  await page.setViewportSize({ width: 1280, height: 800 });
  const state = await invoke("engine_state");
  const [a, b] = state.profiles.filter((p) => !p.draft);
  if (!a || !b) throw new Error("the test copy needs two profiles");
  await api(`profiles/${a.id}/activate`, {});
  await api(`profiles/${a.id}/controls`, { control: "keys.0", binding: { action: "core.timer", params: { minutes: 0, seconds: 3, sound: "off" }, look: { label: "Tea" } } });
  await sleep(500);
  await api("test/input", { kind: "keys", index: 0, gesture: "press" });
  await sleep(300);
  await api(`profiles/${b.id}/activate`, {});
  await sleep(4500);
  const ringing = (await invoke("engine_state")).ringing ?? [];
  check("the engine lists the timer that ran out", ringing.length === 1 && ringing[0].profile === a.id && ringing[0].control === "keys.0", JSON.stringify(ringing));
  const bar = page.locator(".banner", { hasText: "Tea ran out" });
  await bar.first().waitFor({ timeout: 3000 }).catch(() => {});
  const text = (await bar.first().textContent().catch(() => "")) ?? "";
  check("the window names it and its profile", text.includes(`profile ${a.name}`), text.trim());
  await shot(page, "1-ringing");
  await bar.first().getByRole("button", { name: "Stop" }).click();
  await sleep(1200);
  check("Stop clears it", ((await invoke("engine_state")).ringing ?? []).length === 0);
  check("the bar is gone", (await bar.count()) === 0);
  await shot(page, "2-stopped");
  check("no page errors", errors === 0, String(errors));
} finally {
  await browser.close().catch(() => {});
  ps("-Stop");
}
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\nFAILED: ${failed} of ${results.length}` : `\npassed ${results.length}/${results.length}`);
process.exit(failed ? 1 : 0);
