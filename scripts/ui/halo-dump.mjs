// The keys' content of the owner's active page as a test copy draws it (his profiles, the window off
// screen), kept as raw RGBA for scripts/ui/halo-lab.py; one file per label plate.
//   node scripts/ui/halo-dump.mjs [plate ...]     # plates: hard (today), fade, shadow
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { screenClean } from "./screen.mjs";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const exe = path.join(root, "target", "test-build", "release", "hubdeck.exe");
const real = path.join(process.env.APPDATA, "dev.hubdeck.app");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const plates = process.argv.slice(2).length ? process.argv.slice(2) : ["hard"];
for (const plate of plates) {
  const dir = path.join(os.tmpdir(), "hubdeck-halo-dump");
  const dump = path.join(root, "logs", "halo-lab", `content-${plate}.rgba`);
  fs.mkdirSync(path.dirname(dump), { recursive: true });
  fs.rmSync(dump, { force: true });
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  for (const item of ["profiles", "images", "settings.json", "placements.json"]) if (fs.existsSync(path.join(real, item))) fs.cpSync(path.join(real, item), path.join(dir, item), { recursive: true });
  const app = spawn(exe, [], { env: { ...process.env, HUBDECK_AUTOMATION: dir, HUBDECK_DUMP_CONTENT: dump, HUBDECK_LABEL_PLATE: plate }, stdio: "ignore" });
  for (let i = 0; i < 40 && !fs.existsSync(dump); i++) await sleep(500);
  await sleep(3000);
  app.kill();
  spawn("taskkill", ["/F", "/T", "/PID", String(app.pid)]);
  await sleep(1500);
  console.log(`${plate}: ${fs.existsSync(dump) ? fs.statSync(dump).size + " bytes" : "no dump"}`, screenClean(dir)[1]);
}
