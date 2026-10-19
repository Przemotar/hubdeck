// Spotify's setup in four steps (owner's 1a, 2026-10-19: "zeby uzytkwonik mogl sobie sam ustawic
// to spotify i mial to dobrze wytlumaczone"). A test copy of the given program (no deck, never the
// focus, a data folder without a Spotify sign-in): Settings > Connections > Spotify, as a user
// opens it, in dark and light and at two window sizes. Clicks only what stays inside the copy:
// Open the dashboard (the test copy does not open the browser). Not clicked: Copy (it would
// overwrite the owner's clipboard) and Connect (it opens Spotify's sign-in in his browser).
//   node scripts/ui/spotify-setup.mjs [hubdeck.exe] [name]    # default: the test build, "after"
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const name = process.argv[3] ?? "after";
const dir = path.join(os.tmpdir(), "hubdeck-spotify-setup");
const out = path.join(root, "scripts", "ui", "spotify");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const PORT = 9474;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.mkdirSync(out, { recursive: true });

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
delete settings.spotifyClientId;
settings.language = "en";
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
const results = [];
const check = (what, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${what}${detail ? `: ${detail}` : ""}`);
};
try {
  let browser, page;
  for (let i = 0; i < 60 && !page; i++) {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`).catch(() => undefined);
    page = browser?.contexts()[0]?.pages().find((x) => /tauri.localhost|localhost:1460/.test(x.url()));
    if (!page) {
      await browser?.close().catch(() => {});
      await sleep(1000);
    }
  }
  if (!page) throw new Error("the test copy's window did not come up");
  page.setDefaultTimeout(20000);
  for (const theme of ["dark", "light"]) {
    await page.evaluate((t) => localStorage.setItem("hubdeck.theme", t), theme);
    for (const [w, h] of [
      [1440, 900],
      [1100, 760],
    ]) {
      await page.setViewportSize({ width: w, height: h });
      await page.reload();
      await page.getByRole("button", { name: /^Settings$/ }).first().click();
      await page.locator(".settings-nav-item", { hasText: "Connections" }).click();
      const spotify = page.locator("details.svc", { has: page.locator(".svc-name", { hasText: /^Spotify$/ }) });
      await spotify.locator("summary").click();
      await sleep(400);
      await spotify.scrollIntoViewIfNeeded();
      const shot = `${name}-${theme}-${w}`;
      await spotify.screenshot({ path: path.join(out, `${shot}.png`) });
      if (name !== "after") continue;
      const steps = await spotify.locator(".spotify-steps .step").count();
      const cut = (await clipped(page)).filter((c) => /Spotify|dashboard|Copy|Connect|Client/i.test(c.text));
      check(`${shot}: four steps, nothing cut`, steps === 4 && cut.length === 0, `${steps} steps${cut.length ? `, cut: ${JSON.stringify(cut)}` : ""}`);
      const connect = spotify.getByRole("button", { name: /Connect Spotify/ });
      check(`${shot}: Connect waits for a Client ID`, await connect.isDisabled());
      await spotify.locator("#spotify-id").fill("0123456789abcdef0123456789abcdef");
      check(`${shot}: a Client ID makes Connect clickable`, await connect.isEnabled());
      const uri = await spotify.locator(".spotify-uri").innerText();
      check(`${shot}: the redirect address is the one HubDeck listens on`, uri.trim() === "http://127.0.0.1:43123/callback", uri);
      await spotify.getByRole("button", { name: /Open the dashboard/ }).click();
      await sleep(300);
      const banner = await page.locator(".banner, .error-banner").count();
      check(`${shot}: Open the dashboard gives no error`, banner === 0);
    }
  }
  await page.evaluate(() => localStorage.setItem("hubdeck.theme", "dark"));
  await browser.close().catch(() => {});
} finally {
  app.kill();
  await sleep(800);
}
if (name === "after") {
  const passed = results.filter(Boolean).length;
  console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}; screenshots in ${out}`);
  process.exit(passed === results.length ? 0 : 1);
}
console.log(`screenshots in ${out}`);
