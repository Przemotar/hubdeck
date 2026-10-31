// The icon library like Stream Deck's (owner's "2 a", 2026-10-25, step 2): a Stream Deck icon pack
// opened with HubDeck (as a double click does: the file on the command line) goes in at once with
// a note; a search finds its icons by name and tag next to the built-in ones; a picked icon shows
// in Recent, the first tab next time; "More icons" names Elgato's Marketplace. A test copy of its
// own with the owner's profiles (virtual modules, its window off screen, never the focus).
//   node scripts/ui/icon-library.mjs [path to hubdeck.exe]    # default: the test build
import { chromium } from "playwright-core";
import { execFileSync, spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { clipped } from "./clipped.mjs";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-icon-library");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const shots = path.join(root, "logs", "icon-library");
const PORT = 9485;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
fs.mkdirSync(shots, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
delete settings.recentIcons;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const profile = settings.active;

// a small Stream Deck icon pack as Elgato documents it: manifest.json, icons.json, icons/
const packDir = path.join(os.tmpdir(), "hubdeck-icon-library-pack");
fs.rmSync(packDir, { recursive: true, force: true });
const inner = path.join(packDir, "com.test.rockets.sdIconPack");
fs.mkdirSync(path.join(inner, "icons"), { recursive: true });
fs.writeFileSync(path.join(inner, "manifest.json"), JSON.stringify({ Name: "Test rockets", Version: "1.0.0", Author: "HubDeck walk", Icon: "icon.png" }));
// a 1x1 PNG (enough for a picture the pack lists)
const onePixel = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==", "base64");
for (const name of ["rocket", "planet", "comet"]) fs.writeFileSync(path.join(inner, "icons", `${name}.png`), onePixel);
fs.writeFileSync(path.join(inner, "icons.json"), JSON.stringify([
  { path: "rocket.png", name: "Rocket", tags: ["space", "launch"] },
  { path: "planet.png", name: "Planet", tags: ["space"] },
  { path: "comet.png", name: "Comet", tags: ["tail"] },
]));
const zip = path.join(packDir, "pack.zip");
execFileSync("powershell", ["-NoProfile", "-Command", `Compress-Archive -Path '${inner}' -DestinationPath '${zip}'`]);
const packFile = path.join(packDir, "Test rockets.streamDeckIconPack");
fs.renameSync(zip, packFile);

const app = spawn(exe, [packFile], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
const look = async () => (await api(`profiles/${profile}`)).controls?.["keys.6"]?.look ?? {};

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
let browser;
try {
  console.log(`exe: ${exe}, profile ${profile}`);
  await api(`profiles/${profile}/controls`, { control: "keys.6", binding: { action: "core.hotkey", params: { keys: "Ctrl+Alt+F8", mode: "normal" }, look: { label: "Lubię to", icon: "heart" } } });
  await api(`profiles/${profile}/activate`, {});
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  let win;
  for (let i = 0; i < 40 && !win; i++) {
    win = browser.contexts()[0]?.pages().find((p) => /tauri.localhost|localhost:1460/.test(p.url()));
    if (!win) await sleep(500);
  }
  let errors = 0;
  win.on("pageerror", (e) => (errors++, console.log("page error:", e.message)));
  // the note comes as the window starts: watch for it from the first moment
  let noted = false;
  for (let i = 0; i < 40 && !noted; i++) {
    noted = await win.getByText(/Icon pack added: Test rockets/).first().isVisible().catch(() => false);
    if (!noted) await sleep(200);
  }
  await win.setViewportSize({ width: 1280, height: 800 });
  await sleep(1500);

  // the pack opened with HubDeck went in at once, with a note
  const installed = fs.existsSync(path.join(dir, "iconpacks")) ? fs.readdirSync(path.join(dir, "iconpacks")) : [];
  check("the opened .streamDeckIconPack is installed", installed.length === 1, installed.join(", "));
  check("the window says so", noted);

  const key = win.locator('g.dk-ctl[data-kind="keys"][data-index="6"]').first();
  const box = await key.locator(".dk-hov").first().boundingBox();
  await win.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await sleep(800);
  await win.getByRole("button", { name: "Change icon" }).first().click();
  await sleep(600);
  const pop = win.locator(".icon-popover");
  check("no Recent tab before anything was picked", (await pop.getByRole("tab", { name: "Recent" }).count()) === 0);
  check("the pack has its own tab", (await pop.getByRole("tab", { name: "Test rockets" }).count()) === 1);
  check("More icons names Elgato Marketplace", (await pop.getByRole("button", { name: /More icons \(Elgato Marketplace\)/ }).count()) === 1);

  // a search by tag finds the pack's icons next to the built-in ones
  await pop.getByRole("textbox", { name: "Search icons" }).fill("space");
  await sleep(1500);
  const inPacks = pop.getByRole("listbox", { name: "In your icon packs" }).getByRole("button");
  check("a search by tag finds the pack's two space icons", (await inPacks.count()) === 2, String(await inPacks.count()));
  await pop.screenshot({ path: path.join(shots, "1-search.png") });
  await pop.getByRole("textbox", { name: "Search icons" }).fill("rocket");
  await sleep(800);
  await pop.getByRole("listbox", { name: "In your icon packs" }).getByRole("button", { name: "Rocket" }).click();
  await sleep(1000);
  check("a pack icon from the search goes on the key as its picture", !!(await look()).image, JSON.stringify(await look()));

  // a built-in icon picked: it is in Recent, the first tab next time
  await win.getByRole("button", { name: "Remove picture" }).click().catch(() => {});
  await sleep(600);
  await win.getByRole("button", { name: "Change icon" }).first().click();
  await sleep(500);
  await pop.getByRole("tab", { name: "Emoji" }).click();
  await pop.getByRole("button", { name: "rocket launch" }).click();
  await sleep(900);
  check("the emoji is the icon", (await look()).icon === "emoji:🚀", (await look()).icon);
  const saved = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8")).recentIcons ?? [];
  check("it is kept as recent", saved[0] === "emoji:🚀", JSON.stringify(saved));
  // a key with a built-in icon opens the library on Recent (one with an emoji opens on Emoji)
  await api(`profiles/${profile}/controls`, { control: "keys.6", binding: { action: "core.hotkey", params: { keys: "Ctrl+Alt+F8", mode: "normal" }, look: { label: "Lubię to", icon: "heart" } } });
  await sleep(1200);
  await win.getByRole("button", { name: "Change icon" }).first().click();
  await sleep(900);
  check("Recent is the first tab and open", (await pop.getByRole("tab").first().textContent()) === "Recent" && (await pop.getByRole("tab", { name: "Recent" }).getAttribute("aria-selected")) === "true");
  check("Recent shows the rocket", (await pop.getByRole("listbox", { name: "Recent" }).getByRole("button", { name: "rocket launch" }).count()) === 1);
  await pop.screenshot({ path: path.join(shots, "2-recent.png") });
  const cut = (await clipped(win)).filter((x) => /Recent|Interface|Emoji|Games|Test rockets|More icons|In your icon packs/.test(x.text));
  check("nothing of the library cut", cut.length === 0, cut.map((x) => x.text).join(" | "));
  await win.keyboard.press("Escape");
  check("no page errors", errors === 0, `${errors}`);
} finally {
  await browser?.close().catch(() => {});
  app.kill();
  spawn("taskkill", ["/F", "/T", "/PID", String(app.pid)]);
  await sleep(1500);
}
check("nothing came up on the owner's screen", ...screenClean(dir));
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
