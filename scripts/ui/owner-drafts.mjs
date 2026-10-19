// The assistant on the owner's own data (owner, 2026-10-19: "porobic podemnie jakies profile i przy
// okazji szukac co trzeba poprawic albo co dodac do agenta do profili"). A test copy with a fresh
// copy of his data (his apps, his time in front of them, his modules; no drafts), never the focus:
// Get started > the assistant, his words in the box, then every draft it makes is shown on the
// deck (Try) and screenshotted, with what the profile check finds. Nothing reaches his real data.
//   node scripts/ui/owner-drafts.mjs ["what to ask"]    # screenshots and report in scripts/ui/owner/
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const message = process.argv[2] ?? "Make me profiles for the apps I use most, so the deck helps in my daily work.";
const PORT = 9475;
const dir = path.join(process.env.TEMP, "hubdeck-owner-drafts");
const out = path.join(root, "scripts", "ui", "owner");
fs.mkdirSync(out, { recursive: true });
const ps = (...a) => execFileSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(root, "scripts", "automation.ps1"), "-Dir", dir, "-CdpPort", String(PORT), ...a], { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

ps("-Stop");
ps("-Fresh");
ps("-Stop");
// his data as it is, without the drafts of other talks
const sp = path.join(dir, "settings.json");
const settings = JSON.parse(fs.readFileSync(sp, "utf8"));
Object.assign(settings, { drafts: [], language: "en" });
fs.writeFileSync(sp, JSON.stringify(settings, null, 2));
ps();

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
page.setDefaultTimeout(30000);
await page.setViewportSize({ width: 1440, height: 900 });
const report = { message, at: new Date().toISOString(), text: [], drafts: [], error: null };
try {
  await page.locator('.lib-tabs [role="tab"]', { hasText: /Get started/ }).click();
  await page.fill("#as-know", message);
  await page.locator(".as-intro .row .tool").click();
  await page.locator(".as-consent").waitFor();
  const first = page.locator(".as-consent textarea");
  if (await first.count()) await first.fill(message);
  await page.screenshot({ path: path.join(out, "0-consent.png") });
  const t0 = Date.now();
  await page.locator(".as-consent .tool.primary").first().click();
  for (;;) {
    const s = await page.evaluate(() => ({
      busy: !!document.querySelector(".as-work.is-live"),
      err: [...document.querySelectorAll(".as-msg.is-error")].map((e) => e.textContent).join(" | "),
    }));
    if (s.err) {
      report.error = s.err;
      break;
    }
    if (!s.busy && Date.now() - t0 > 3000) break;
    if (Date.now() - t0 > 400000) {
      report.error = "timeout";
      break;
    }
    await sleep(500);
  }
  report.seconds = Math.round((Date.now() - t0) / 1000);
  report.text = await page.evaluate(() => [...document.querySelectorAll(".as-msg.is-assistant:not(.is-cards) p")].map((x) => x.textContent));
  await page.screenshot({ path: path.join(out, "1-talk.png") });
  const drafts = await page.evaluate(async () => {
    const inv = window.__TAURI_INTERNALS__.invoke;
    const st = await inv("engine_state");
    const res = [];
    for (const d of st.profiles.filter((x) => x.draft)) {
      const prof = await inv("profile_get", { id: d.id }).catch(() => null);
      const issues = await inv("profile_check", { id: d.id }).catch(() => []);
      const card = [...document.querySelectorAll(".as-sketch")].find((c) => c.querySelector(".as-draft-name")?.textContent === d.name);
      res.push({
        id: d.id,
        name: d.name,
        about: card?.querySelector(".as-sketch-about")?.textContent ?? "",
        apps: prof?.apps ?? [],
        controls: Object.entries(prof?.controls ?? {}).map(([c, b]) => `${c}: ${b.action} ${JSON.stringify(b.params ?? {}).slice(0, 80)} [${b.look?.label ?? ""}${b.look?.icon ? " icon " + b.look.icon : ""}${b.look?.image ? " image" : ""}]`),
        issues: issues.map((i) => i.message),
      });
    }
    return res;
  });
  report.drafts = drafts;
  // each draft on the deck drawing, as Try shows it
  for (const [i, d] of drafts.entries()) {
    const card = page.locator(".as-sketch", { has: page.locator(".as-draft-name", { hasText: d.name }) }).first();
    await card.scrollIntoViewIfNeeded().catch(() => {});
    await card.click().catch(() => {});
    await sleep(1500);
    await page.screenshot({ path: path.join(out, `draft-${i + 1}.png`) });
  }
} catch (e) {
  report.error = String(e);
  await page.screenshot({ path: path.join(out, "error.png") }).catch(() => {});
} finally {
  fs.writeFileSync(path.join(out, "report.json"), JSON.stringify(report, null, 2));
  await browser.close().catch(() => {});
  ps("-Stop");
}
console.log(JSON.stringify({ seconds: report.seconds, error: report.error, drafts: report.drafts.map((d) => `${d.name}: ${d.controls.length} controls, ${d.issues.length} issues`) }, null, 1));
