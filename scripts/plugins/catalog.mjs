#!/usr/bin/env node
// HubDeck's plugin catalog in one file (owner's pick A, 2026-10-18: "jedna zakładka z sortowaniem,
// podaniem źródła, popularnością"): every free Stream Deck plugin that installs in HubDeck with one
// click, from three sources, with what the Plugins tab sorts and filters by.
//   - Elgato Marketplace's 300 most downloaded: only those whose authors publish an open file
//     (matched by hand, `elgatoOpen` in app/src-tauri/src/engine/sdcatalog.json); the rank, the
//     systems, the category and dial support come from Elgato's list.
//   - the OpenAction Marketplace's catalogue (open-source plugins on GitHub).
//   - GitHub repositories with the topics streamdeck-plugin, stream-deck-plugin or
//     elgato-stream-deck that publish a .streamDeckPlugin in a release and changed in the last two
//     years (forks and archived ones left out).
// Every GitHub repository adds its stars and last change; its manifest.json the plugin id, the
// systems, dial support and an icon. Plugins only Elgato's app installs are left out (owner,
// 2026-10-18: "pokażmy te które są dostępne i tyle").
//
//   GITHUB_TOKEN=... node scripts/plugins/catalog.mjs [out.json]    # default: site/plugins.json
//
// Without GITHUB_TOKEN it uses `gh auth token`. The Pages workflow runs it every day and publishes
// the file with the site (przemotar.github.io/hubdeck/plugins.json); the app reads that file and
// falls back to the two live lists when it cannot.
import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const OUT = path.resolve(process.argv[2] ?? path.join(ROOT, "site", "plugins.json"));
const DATA = JSON.parse(fs.readFileSync(path.join(ROOT, "app/src-tauri/src/engine/sdcatalog.json"), "utf8"));
const OPENACTION = "https://openactionapi.github.io/plugins/catalogue.json";
const OA_ICONS = "https://openactionapi.github.io/plugins/icons/";
const ELGATO_LIST = "https://mp-gateway.elgato.com/products?extension=plugins&ordering=-download_count&limit=100";
const ELGATO_PAGE = "https://marketplace.elgato.com/product/";
const ELGATO_TOP = 300;
const TOPICS = ["streamdeck-plugin", "stream-deck-plugin", "elgato-stream-deck"];
const FRESH_DAYS = 730;
const TOKEN = process.env.GITHUB_TOKEN || execSync("gh auth token", { encoding: "utf8" }).trim();

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function get(url, { json = true, auth = false } = {}) {
  for (let attempt = 0; ; attempt++) {
    const res = await fetch(url, { headers: { "User-Agent": "hubdeck-catalog", ...(auth ? { Authorization: `Bearer ${TOKEN}` } : {}) } });
    if (res.ok) return json ? res.json() : res.text();
    if (res.status === 404) return null;
    if (attempt >= 3) throw new Error(`${url}: HTTP ${res.status}`);
    await sleep(2000 * (attempt + 1));
  }
}
async function graphql(query) {
  for (let attempt = 0; ; attempt++) {
    const res = await fetch("https://api.github.com/graphql", { method: "POST", headers: { Authorization: `Bearer ${TOKEN}`, "User-Agent": "hubdeck-catalog" }, body: JSON.stringify({ query }) });
    const body = await res.json().catch(() => ({}));
    if (res.ok && body.data) return body.data;
    if (attempt >= 3) throw new Error(`GitHub GraphQL: HTTP ${res.status} ${JSON.stringify(body.errors ?? body).slice(0, 300)}`);
    await sleep(3000 * (attempt + 1));
  }
}

/** Whether a download address still gives a file (a HEAD, else the first byte). */
async function answers(url) {
  for (const method of ["HEAD", "GET"]) {
    const res = await fetch(url, { method, redirect: "follow", headers: { "User-Agent": "hubdeck-catalog", ...(method === "GET" ? { Range: "bytes=0-0" } : {}) } }).catch(() => null);
    if (res?.ok || res?.status === 206) return true;
    if (res && res.status !== 405 && res.status !== 403) return false;
  }
  return false;
}

/** `owner/repo` of a GitHub address, lower case for matching. */
const repoOf = (url) => {
  const m = /^https:\/\/github\.com\/([^/]+)\/([^/#?]+)/i.exec(url ?? "");
  return m ? `${m[1]}/${m[2].replace(/\.git$/i, "")}` : null;
};
const key = (repo) => repo.toLowerCase();

/** A plain first paragraph of a description, at most 220 letters (as sdcatalog.rs does). */
function firstParagraph(text) {
  const para = (text ?? "").split(/\r?\n/).map((l) => l.trim()).find((l) => l) ?? "";
  const plain = para.replace(/[*_#`]/g, "").trim();
  return plain.length > 220 ? `${plain.slice(0, 220).trimEnd()}…` : plain;
}

// The categories the tab filters by: Elgato's own names where it has the plugin, else a guess from
// the plugin's name, description and manifest category.
const CATEGORIES = [
  ["Music", /spotify|music|ytmd|youtube music|apple music|tidal|deezer|vlc|winamp|foobar|now playing|cider|plexamp|ampdeck|feishin|mediamonkey|last\.?fm|\bmidi\b|reaper|dorico|stem player|media (info|player)/i],
  ["Communication", /discord|teams|zoom|google meet|slack|teamspeak|mumble|webex|mattermost|workplace/i],
  ["Audio", /audio|volume|sound|mic\b|microphone|mute|voicemeeter|goxlr|wave ?link|mixer|totalmix|headset|speaker|sonar|\brm1\b/i],
  ["Streaming", /\bobs\b|stream ?labs|twitch|youtube|\bkick\b|tiktok|live ?stream|streaming|streamer|restream|meld|vtube|warudo|vmix|kairos|roland v-|\bdhd\b|\bscenes?\b|recording|meme/i],
  ["Smart home", /home ?assistant|homey|hubitat|openhab|smartthings|homekit|tuya|shelly|domoticz|smart home|switchbot|sensibo|tesla|roku|universal devices|mqtt|bluetooth|ir blaster|rocrail/i],
  ["Lighting", /hue|govee|nanoleaf|lifx|wled|light|lamp|led\b|key light|razer chroma|litra/i],
  ["Monitoring", /cpu|gpu|ram\b|memory|hwinfo|sensor|temperature|system (info|vitals|monitor)|uptime|ping|network|battery|speed ?test|hardware monitor|pi-?hole|nagios|\baws\b|openstack|azure|my ip|nightscout|credits|usage|fan profile/i],
  ["Developer", /vs ?code|visual studio|jetbrains|xcode|git\b|github|docker|devops|terminal|ssh|script|powershell|api request|websocket|http|web requests?|fetch|\budp\b|\btcp\b|\bosc\b|unity|godot|sample|hammerspoon|shell|\bai\b|claude|codex|copilot|coding|opencode|agent/i],
  ["Gaming", /game|sim ?hub|flight|racing|irac|elite dangerous|star citizen|msfs|x-plane|\bsteam\b|xbox|playstation|\bdcs\b|ffxiv|fivem|destiny|minecraft|dota|league|bakkes|forza|guild wars|helldivers|cookie|whack|dddice|vatis|vex /i],
  ["Productivity", /timer|stopwatch|pomodoro|clock|calendar|ical\b|alarm|reminder|todo|notes?\b|obsidian|clipboard|window|launcher|macro|hotkey|keyboard|app switch|productiv|toggl|time track|harvest|jira|youtrack|wakatime|keepass|bitwarden|awake|amphetamine|magnifier|file ?explorer|finder|linkwarden|clicker|airplane mode|random text/i],
];
// OpenDeck's plugins that add support for another maker's deck: no actions for a MasterHUB
const DEVICE_DRIVER = /unofficial (opendeck )?plugin (for|to use|adding)|adding device support|device support for/i;
const ELGATO_CATEGORY = { Audio: "Audio", Music: "Music", Video: "Streaming", Social: "Streaming", Engagement: "Streaming", Streaming: "Streaming", Lighting: "Lighting", "Smart Home": "Smart home", Monitoring: "Monitoring", Utilities: "Productivity", Productivity: "Productivity", Gaming: "Gaming", Artistic: "Productivity", Developer: "Developer", Communication: "Communication" };
// "Stream Deck" is in most descriptions: not a sign of streaming
const guessCategory = (...texts) => {
  const plain = texts.filter(Boolean).map((t) => t.replace(/stream ?deck|stream ?dock/gi, ""));
  return CATEGORIES.find(([, re]) => plain.some((t) => re.test(t)))?.[0] ?? "Other";
};

async function openAction() {
  const raw = await get(OPENACTION);
  return Object.entries(raw)
    .filter(([, r]) => r?.name?.trim())
    .map(([id, r]) => ({ id, name: r.name.trim(), author: r.author ?? "", description: r.description ?? "", repository: r.repository ?? "", downloadUrl: r.download_url || undefined, aiAssisted: !!r.flags?.ai_assisted, icon: `${OA_ICONS}${id}.png` }));
}

async function elgatoTop() {
  const all = [];
  for (let offset = 0; offset < ELGATO_TOP; offset += 100) {
    const page = await get(`${ELGATO_LIST}&offset=${offset}`).catch((e) => (console.warn(`Elgato's list: ${e.message}`), null));
    if (!page?.results?.length) break;
    all.push(...page.results);
  }
  return all.map((e, i) => {
    const tags = Object.fromEntries((e.tagsets ?? []).map((t) => [t.name, t.tags ?? []]));
    return {
      elgatoId: e.id,
      rank: i + 1,
      name: e.name,
      author: e.org_name ?? "",
      page: e.slug ? `${ELGATO_PAGE}${e.slug}` : undefined,
      description: firstParagraph(e.description),
      icon: e.app_icon_cdn ?? "",
      systems: (tags.OS ?? []).map((s) => (/mac/i.test(s) ? "macOS" : /win/i.test(s) ? "Windows" : s)),
      category: ELGATO_CATEGORY[(tags.Type ?? [])[0]] ?? undefined,
      dial: (tags["Dial support"] ?? [])[0] === "Yes" ? true : undefined,
      free: !(e.variants ?? []).some((v) => v.is_free === false),
    };
  });
}

async function topicRepos() {
  const repos = new Map();
  for (const topic of TOPICS) {
    for (let page = 1; page <= 10; page++) {
      const res = await get(`https://api.github.com/search/repositories?q=topic:${topic}&per_page=100&page=${page}`, { auth: true });
      for (const r of res?.items ?? []) repos.set(key(r.full_name), r.full_name);
      if (!res || res.items.length < 100) break;
      await sleep(2500);
    }
  }
  return [...repos.values()];
}

/** Stars, last change and release files of many repositories, 50 a query. */
async function repoFacts(repos) {
  const facts = new Map();
  for (let i = 0; i < repos.length; i += 50) {
    const batch = repos.slice(i, i + 50);
    const fields = batch
      .map((r, j) => {
        const [owner, name] = r.split("/");
        return `r${j}: repository(owner: ${JSON.stringify(owner)}, name: ${JSON.stringify(name)}) { nameWithOwner stargazerCount pushedAt isArchived isFork defaultBranchRef { name } releases(first: 5, orderBy: {field: CREATED_AT, direction: DESC}) { nodes { isDraft releaseAssets(first: 30) { nodes { name } } } } }`;
      })
      .join("\n");
    const data = await graphql(`{ ${fields} }`).catch(async () => {
      // a missing repository fails the whole query: ask one by one
      const one = {};
      for (const [j, r] of batch.entries()) {
        const [owner, name] = r.split("/");
        const d = await graphql(`{ r0: repository(owner: ${JSON.stringify(owner)}, name: ${JSON.stringify(name)}) { nameWithOwner stargazerCount pushedAt isArchived isFork defaultBranchRef { name } releases(first: 5, orderBy: {field: CREATED_AT, direction: DESC}) { nodes { isDraft releaseAssets(first: 30) { nodes { name } } } } } }`).catch(() => null);
        one[`r${j}`] = d?.r0 ?? null;
      }
      return one;
    });
    batch.forEach((r, j) => {
      const d = data[`r${j}`];
      if (!d) return;
      const files = d.releases.nodes.filter((n) => !n.isDraft).flatMap((n) => n.releaseAssets.nodes.map((a) => a.name));
      facts.set(key(r), { repo: d.nameWithOwner, stars: d.stargazerCount, updated: d.pushedAt?.slice(0, 10), archived: d.isArchived, fork: d.isFork, branch: d.defaultBranchRef?.name, pluginFile: files.some((f) => /\.streamdeckplugin$/i.test(f)), zipFile: files.some((f) => /\.zip$/i.test(f)) });
    });
    process.stdout.write(`\rrepositories ${Math.min(i + 50, repos.length)}/${repos.length}`);
  }
  process.stdout.write("\n");
  return facts;
}

/** The plugin's manifest from the repository: id, name, author, systems, dial, icon, category. */
async function manifestOf(fact) {
  if (!fact.branch) return null;
  const tree = await get(`https://api.github.com/repos/${fact.repo}/git/trees/${encodeURIComponent(fact.branch)}?recursive=1`, { auth: true }).catch(() => null);
  const paths = (tree?.tree ?? []).map((t) => t.path);
  const manifests = paths.filter((p) => /\.sdPlugin\/manifest\.json$/i.test(p) && !/node_modules|\/dist\/.*\/dist\//i.test(p)).sort((a, b) => a.length - b.length);
  if (!manifests.length) return null;
  const at = manifests[0];
  const text = await get(`https://raw.githubusercontent.com/${fact.repo}/${encodeURIComponent(fact.branch)}/${at.split("/").map(encodeURIComponent).join("/")}`, { json: false }).catch(() => null);
  if (!text) return null;
  let m;
  try {
    m = JSON.parse(text.replace(/^﻿/, ""));
  } catch {
    return null;
  }
  const dir = at.slice(0, at.lastIndexOf("/"));
  const icon = typeof m.Icon === "string" ? [`${m.Icon}@2x.png`, `${m.Icon}.png`, `${m.Icon}.svg`].map((f) => `${dir}/${f}`.replace(/\/\.\//g, "/")).find((f) => paths.includes(f)) : undefined;
  return {
    id: (typeof m.UUID === "string" && m.UUID) || dir.split("/").pop().replace(/\.sdPlugin$/i, ""),
    name: m.Name,
    author: m.Author,
    description: m.Description,
    category: m.Category,
    systems: (m.OS ?? []).map((o) => (o.Platform === "mac" ? "macOS" : o.Platform === "windows" ? "Windows" : o.Platform)).filter(Boolean),
    dial: (m.Actions ?? []).some((a) => (a.Controllers ?? []).includes("Encoder")),
    icon: icon ? `https://raw.githubusercontent.com/${fact.repo}/${encodeURIComponent(fact.branch)}/${icon.split("/").map(encodeURIComponent).join("/")}` : undefined,
  };
}

async function main() {
  const started = Date.now();
  const [oa, elgato, topics] = await Promise.all([openAction(), elgatoTop(), topicRepos()]);
  console.log(`OpenAction ${oa.length}, Elgato ${elgato.length}, GitHub topics ${topics.length}`);

  const openByElgato = new Map(DATA.elgatoOpen.map(([e, id, repo]) => [e, { id, repo }]));
  const repos = new Map();
  for (const p of oa) {
    const r = repoOf(p.repository);
    if (r) repos.set(key(r), r);
  }
  for (const { repo } of openByElgato.values()) if (repo) repos.set(key(repo), repo);
  for (const r of topics) repos.set(key(r), r);
  const facts = await repoFacts([...repos.values()]);

  const entries = new Map();
  const add = (e) => entries.set(e.id, { ...entries.get(e.id), ...Object.fromEntries(Object.entries(e).filter(([, v]) => v !== undefined && v !== "")) });
  for (const p of oa) add({ ...p, source: "openaction" });

  // Elgato's open plugins: the catalog's entry gains Elgato's rank and facts, or one comes in
  for (const e of elgato) {
    const open = openByElgato.get(e.elgatoId);
    if (!open || !e.free) continue;
    const base = entries.get(open.id);
    add({
      id: open.id,
      name: base?.name ?? e.name,
      author: base?.author ?? e.author,
      description: base?.description || e.description,
      repository: base?.repository ?? (open.repo ? `https://github.com/${open.repo}` : undefined),
      icon: e.icon || base?.icon,
      source: "elgato",
      popular: e.rank,
      elgato: e.page,
      systems: e.systems.length ? e.systems : undefined,
      category: e.category,
      dial: e.dial,
    });
  }

  // GitHub repositories: facts for all, new plugins from the topics
  const known = new Set([...entries.values()].map((e) => repoOf(e.repository)).filter(Boolean).map(key));
  const cutoff = new Date(Date.now() - FRESH_DAYS * 864e5).toISOString().slice(0, 10);
  const candidates = topics.map((r) => facts.get(key(r))).filter((f) => f && !known.has(key(f.repo)) && f.pluginFile && !f.fork && !f.archived && f.updated >= cutoff);
  const ids = new Set(entries.keys());
  let i = 0;
  for (const f of candidates) {
    const m = await manifestOf(f);
    process.stdout.write(`\rmanifests ${++i}/${candidates.length}`);
    if (!m?.id || !m.name || ids.has(m.id)) continue;
    ids.add(m.id);
    add({ id: m.id, name: m.name, author: m.author ?? f.repo.split("/")[0], description: firstParagraph(m.description), repository: `https://github.com/${f.repo}`, icon: m.icon, source: "github", systems: m.systems.length ? m.systems : undefined, dial: m.dial || undefined, category: guessCategory(m.name, m.category, m.description) });
  }
  process.stdout.write("\n");

  // every entry: stars, last change, tried, a category; missing systems and dial from the manifest
  const list = [];
  const dropped = [];
  for (const e of entries.values()) {
    const f = facts.get(key(repoOf(e.repository) ?? ""));
    // only what installs (owner, 2026-10-18: "jak ktoś gdzieś podaje repo to musisz sprawdzać czy
    // jeszcze istnieje"; YouTube Ticker's repository was gone and its Install said "http status:
    // 404"): a download address that answers, else a repository that exists and has
    // a plugin file in a release (OpenAction's own ship a .zip, which the app's pick_asset takes)
    // (an archived repository still installs: Audio Switcher, Audio Mute, IFTTT were tried)
    const installs = e.downloadUrl ? await answers(e.downloadUrl) : !!f && (f.pluginFile || f.zipFile);
    if (!installs) {
      dropped.push(`${e.name} (${e.downloadUrl ? "download gone" : !f ? "repository gone" : "no plugin file"})`);
      continue;
    }
    if (DEVICE_DRIVER.test(e.description ?? "")) continue;
    // installs but cannot run outside Elgato's app (checked in HubDeck; sdcatalog.json wontRun)
    if (DATA.wontRun?.[e.id]) {
      dropped.push(`${e.name} (${DATA.wontRun[e.id]})`);
      continue;
    }
    if (f) Object.assign(e, { stars: f.stars, updated: f.updated });
    if (!e.systems && f && e.source !== "github") {
      const m = await manifestOf(f);
      if (m) Object.assign(e, { systems: m.systems.length ? m.systems : undefined, dial: e.dial ?? (m.dial || undefined), icon: e.icon ?? m.icon, mcat: m.category });
    }
    e.category ??= guessCategory(e.name, e.mcat, e.description);
    delete e.mcat;
    e.tried = DATA.tried[e.id];
    list.push(e);
  }
  console.log(`left out, nothing to install or will not run: ${dropped.length}${dropped.length ? `: ${dropped.join(", ")}` : ""}`);
  list.sort((a, b) => (a.popular ?? 1e9) - (b.popular ?? 1e9) || (b.stars ?? 0) - (a.stars ?? 0) || a.name.localeCompare(b.name));
  const out = { version: 1, generated: new Date().toISOString(), sources: { elgato: list.filter((e) => e.source === "elgato").length, openaction: list.filter((e) => e.source === "openaction").length, github: list.filter((e) => e.source === "github").length }, plugins: list };
  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.writeFileSync(OUT, JSON.stringify(out));
  console.log(`${list.length} plugins (${JSON.stringify(out.sources)}) -> ${path.relative(ROOT, OUT)} in ${Math.round((Date.now() - started) / 1000)} s`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
