// Does a Live value on a page opened from the deck start fetching? (Discussions #3, 8bitcanvascode,
// 2026-10-19: "When I navigate between pages using the physical MasterHUB device and open a page
// containing the Live Value action ... it just displays '...'".) A test copy of its own (virtual
// modules, no deck, never the focus), driven through the local API: a page whose first key is a
// Live value from a little web server here that counts its requests, opened by a key press, then
// left and opened again.
//   node scripts/ui/live-page.mjs [path to hubdeck.exe]    # default: the test build
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = process.argv[2] ?? path.join(root, "target", "test-build", "release", "hubdeck.exe");
const dir = path.join(os.tmpdir(), "hubdeck-live-page");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// the live value's source: every request counted
let hits = 0;
const server = http.createServer((_, res) => {
  hits++;
  res.writeHead(200, { "Content-Type": "application/json" });
  res.end(JSON.stringify({ v: 42 }));
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const url = `http://127.0.0.1:${server.address().port}/value`;

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });
for (const item of ["profiles", "images", "settings.json", "placements.json"]) {
  if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
}
const settings = JSON.parse(fs.readFileSync(path.join(dir, "settings.json"), "utf8"));
settings.autoSwitch = false;
fs.writeFileSync(path.join(dir, "settings.json"), JSON.stringify(settings, null, 2));
const profile = settings.active;

const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_CDP_PORT: "9473" }, stdio: "ignore" });
for (let i = 0; i < 60 && !fs.existsSync(path.join(dir, "api-http-port")); i++) await sleep(500);
await sleep(1500);
const api = (route, body) => {
  const token = fs.readFileSync(path.join(dir, "api-token"), "utf8").trim();
  const port = fs.readFileSync(path.join(dir, "api-http-port"), "utf8").trim();
  return fetch(`http://127.0.0.1:${port}/api/${route}`, { method: body ? "POST" : "GET", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined }).then((r) => r.json());
};
const press = async (index) => {
  await api("test/input", { kind: "keys", index, gesture: "press" });
  await sleep(300);
};
const page = async () => (await api("controls")).page ?? null;

const results = [];
const check = (name, ok, detail) => {
  results.push(ok);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? `: ${detail}` : ""}`);
};
try {
  console.log(`exe: ${exe}`);
  const pg = await api(`profiles/${profile}/pages`, { name: "Live test", id: "livetest" });
  // the main page's last key opens the page; the page's last key goes back; its first key is the live value
  await api(`profiles/${profile}/controls`, { control: "keys.14", binding: { action: "core.page", params: { page: pg.id }, look: { label: "Live test" } } });
  await api(`profiles/${profile}/controls`, { control: `${pg.id}/keys.14`, binding: { action: "core.page", params: { page: "" }, look: { label: "Back" } } });
  await api(`profiles/${profile}/controls`, { control: `${pg.id}/keys.0`, binding: { action: "core.live", params: { source: "url", url, path: "v", every: 5 }, look: {} } });
  await sleep(2000);
  check("nothing fetched while the page is not shown", hits === 0, `${hits} requests`);

  await press(14);
  const at = await page();
  check("the deck key opened the page", at === pg.id, String(at));
  await sleep(3000);
  const first = hits;
  check("the page's live value is fetched within 3 s", first >= 1, `${first} requests`);
  await sleep(6000);
  check("and fetched again on its 5 s beat", hits > first, `${hits} requests after 9 s`);

  await press(14);
  check("the page's back key went to the main page", (await page()) === null, String(await page()));
  await sleep(1000);
  const left = hits;
  await sleep(6000);
  check("not fetched once the page is left", hits === left, `${hits - left} requests in 6 s`);

  await press(14);
  await sleep(3000);
  check("fetched again when the page is opened again", hits > left, `${hits - left} requests`);
} finally {
  app.kill();
  server.close();
  await sleep(1000);
}
const passed = results.filter(Boolean).length;
console.log(`${passed}/${results.length}${passed === results.length ? "" : " FAILED"}`);
process.exit(passed === results.length ? 0 : 1);
