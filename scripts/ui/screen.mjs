// Did a test copy put anything on the owner's screen? The copy watches its own windows and logs
// `test copy: on the screen: <title>` for each one that came up on it (app/src-tauri/src/
// automation.rs `watch_screen`); every walk with a test copy ends with this check (owner's pick
// 1a, 2026-10-23, after a walk's question came up on his screen).
//   import { screenClean } from "./screen.mjs";
//   check("nothing came up on the owner's screen", ...screenClean(dir));
import fs from "node:fs";
import path from "node:path";

/** [ok, detail] for a test copy's data folder (`HUBDECK_AUTOMATION`). */
export function screenClean(dir) {
  const logs = path.join(dir, "logs");
  if (!fs.existsSync(logs)) return [false, `no log in ${logs}`];
  const seen = fs
    .readdirSync(logs)
    .filter((f) => f.endsWith(".log"))
    .flatMap((f) => fs.readFileSync(path.join(logs, f), "utf8").split("\n"))
    .filter((l) => l.includes("test copy: on the screen:"))
    .map((l) => l.split("test copy: on the screen:")[1].trim() || "(a window without a title)");
  return [seen.length === 0, seen.length ? seen.join(" | ") : "no window of the test copy on the screen"];
}
