#!/usr/bin/env node
// Is the public README ready? (owner's pick 2a, 2026-10-16: "Na pewno readme jest dopieszczone?",
// after my "ready" a second look still found a dead link and an inside word.) Checks the exported
// README (../hubdeck, what the public sees; run scripts/export-public.sh first):
//   1. every relative link, picture and #anchor exists there
//   2. every web link answers (Reddit blocks tools: 403 is fine; release downloads may 404 before
//      a release is published: a warning)
//   3. GitHub renders it with nothing left raw (through `gh api markdown`)
//   4. no word the window no longer uses (STALE)
//   5. the window's screenshots are not older than the last change of the window's components
//
//   node scripts/readme-check.mjs [path to the exported repo, default ../hubdeck]
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..");
const repo = path.resolve(process.argv[2] || path.join(root, "..", "hubdeck"));
const file = path.join(repo, "README.md");
// names the window dropped (sessions 14-15): keep this list as names change
const STALE = [/\bActions tab\b/i, /\bNew deck\b/, /\binspector\b/i, /\baction library\b/i, /Settings > Help and about fills/i, /\bkey editor\b/i];
const SCREENSHOTS = ["docs/images/hubdeck-main.png", "docs/images/hubdeck-tour.png", "docs/images/hubdeck-settings.png"];

const fails = [];
const warns = [];
const md = fs.readFileSync(file, "utf8");

// 1. relative links, pictures and anchors
const slug = (h) => h.toLowerCase().replace(/[^a-z0-9 -]/g, "").trim().replace(/ /g, "-");
const anchors = new Set([...md.matchAll(/^#+ (.+)$/gm)].map((m) => slug(m[1])));
const links = [...md.matchAll(/\]\(([^)\s]+)\)|(?:src|href)="([^"]+)"/g)].map((m) => m[1] || m[2]);
const web = new Set();
for (const l of links) {
  if (/^https?:/.test(l)) web.add(l);
  else if (l.startsWith("#")) {
    if (!anchors.has(l.slice(1))) fails.push(`no heading for ${l}`);
  } else if (!l.startsWith("mailto:") && !fs.existsSync(path.join(repo, l.split("#")[0]))) fails.push(`missing in the public repo: ${l}`);
}

// 2. web links
for (const u of web) {
  let code = 0;
  try {
    code = (await fetch(u, { method: "GET", redirect: "follow", headers: { "User-Agent": "Mozilla/5.0", Range: "bytes=0-1023" }, signal: AbortSignal.timeout(15000) })).status;
  } catch {
    code = 0;
  }
  if (code >= 200 && code < 400) continue;
  if (code === 403 && /redd\.it|reddit\.com/.test(u)) continue;
  if (code === 404 && /releases\/latest\/download/.test(u)) warns.push(`${u}: 404 (no published release yet?)`);
  else fails.push(`${u}: ${code || "no answer"}`);
}

// 3. GitHub's render
try {
  const html = execFileSync("gh", ["api", "-X", "POST", "markdown", "-f", "mode=gfm", "-f", "context=Przemotar/hubdeck", "-F", `text=@${file}`], { encoding: "utf8", maxBuffer: 1 << 26 });
  const raw = (html.match(/\*\*[A-Za-z]/g) || []).length + (html.match(/\|\s*---/g) || []).length;
  if (raw) fails.push(`${raw} bits of markdown left raw in GitHub's render`);
} catch (e) {
  warns.push(`render not checked (gh api markdown): ${e.message.split("\n")[0]}`);
}

// 4. words the window no longer uses
for (const re of STALE) {
  const m = md.match(re);
  if (m) fails.push(`outdated word: "${m[0]}"`);
}

// 5. screenshots against the window's last change
const lastDate = (args) => {
  try {
    return Number(execFileSync("git", ["-C", root, "log", "-1", "--format=%ct", "--", ...args], { encoding: "utf8" }).trim()) || 0;
  } catch {
    return 0;
  }
};
const windowChanged = lastDate(["app/src/components", "app/src/App.tsx", "app/src/styles.css"]);
for (const shot of SCREENSHOTS) {
  const at = lastDate([shot]);
  if (at && windowChanged - at > 3 * 86400) warns.push(`${shot} is ${Math.round((windowChanged - at) / 86400)} days older than the window: node scripts/ui/readme-shots.mjs`);
}

for (const w of warns) console.log(`WARN  ${w}`);
for (const f of fails) console.log(`FAIL  ${f}`);
console.log(fails.length ? `README not ready: ${fails.length} problem(s)` : `README ready: ${links.length} links, ${web.size} web links, ${anchors.size} headings checked`);
process.exit(fails.length ? 1 : 0);
