// Before a panel is replaced by a new one (owner's "3 a", 2026-10-28: the key styles of the Living deck
// panel fell out of the Deck look mockups and he had to ask "to zniknęło?"): every text the old panel
// shows (its i18n keys) must have a place in the new one, or be on the list of things he cut.
//   node scripts/ui/feature-parity.mjs <old file>[:<function>] <new file> [--cut key,key]
// Prints the keys of the old one the new one lacks; exits 1 when any are left.
import fs from "node:fs";

const [oldArg, newFile, ...rest] = process.argv.slice(2);
if (!oldArg || !newFile) {
  console.log("usage: node scripts/ui/feature-parity.mjs <old file>[:<function>] <new file> [--cut key,key]");
  process.exit(2);
}
const cut = new Set((rest[rest.indexOf("--cut") + 1] ?? "").split(",").filter(Boolean));
const [oldFile, fn] = oldArg.split(":");
let oldText = fs.readFileSync(oldFile, "utf8");
if (fn) {
  // only that component: from its declaration to the next top-level function
  const at = oldText.search(new RegExp(`function ${fn}\\b`));
  if (at < 0) throw new Error(`no function ${fn} in ${oldFile}`);
  const next = oldText.slice(at + 10).search(/\n(export )?function [A-Z]/);
  oldText = oldText.slice(at, next < 0 ? undefined : at + 10 + next);
}
const keys = (text) => new Set([...text.matchAll(/\bt\(\s*[`"']([a-zA-Z0-9_.\-${}]+)[`"']/g)].map((m) => m[1]));
const had = keys(oldText);
const has = keys(fs.readFileSync(newFile, "utf8"));
const missing = [...had].filter((k) => !has.has(k) && !cut.has(k)).sort();
console.log(`${had.size} texts in the old one, ${has.size} in the new one`);
for (const k of missing) console.log(`MISSING  ${k}`);
console.log(missing.length ? `${missing.length} texts of the old one have no place in the new one` : "every text of the old one has a place");
process.exit(missing.length ? 1 : 0);
