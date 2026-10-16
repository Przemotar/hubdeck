// Finds window fields where the user types the name of something HubDeck could list instead (an
// app, an OBS or Streamlabs scene or source, a device, a Home Assistant entity, a profile...).
// Owner's rule (2026-10-15): "Tam gdzie mamy mieć coś z apek to powinna być jakaś lista albo jakiś
// wybór apek a nie miejsce na nazwę"; owner's pick 2a (2026-10-16): run before every release.
//
// Reads the action catalog the window gets (app/src/demo/fixture.json; make it fresh first with
// `cargo test -p hubdeck dump_demo_fixture -- --ignored`) and lists every typed field (`text`)
// whose key or label names a listable thing. Fields that are a list already have their own type
// (app, audioDevice, profile, page, named, midiPort, game).
//
//   node scripts/ui/inputs.mjs          # exit code 1 while a typed listable field is left
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const fixture = JSON.parse(fs.readFileSync(path.join(here, '../../app/src/demo/fixture.json'), 'utf8'));

// words that name something an app or service can enumerate
const LISTABLE = /\b(app|application|program|process|window|scene|source|input|transition|collection|profile|filter|entity|light|script|playlist|device|game|category|port|note)\b/i;
// typed on purpose: free text, not a name of something that exists (key: reason)
const FREE = {
  'twitch.chat:message': 'a chat message is free text',
  'twitch.title:title': 'a stream title is free text',
  'twitch.marker:description': 'free text',
  'core.text:text': 'free text',
  'core.pet:name': "the pet's name is the user's own",
};

const found = [];
for (const a of fixture.catalog.actions) {
  for (const p of a.params) {
    if (p.type !== 'text') continue;
    const id = `${a.id}:${p.key}`;
    if (FREE[id]) continue;
    if (LISTABLE.test(p.key) || LISTABLE.test(p.label)) found.push({ id, label: p.label, action: a.name });
  }
}

for (const f of found) console.log(`typed  ${f.id.padEnd(28)} ${f.action}: ${f.label}`);
console.log(`\n${found.length} typed field(s) that name something listable${found.length ? ' (make each a list, or add it to FREE with a reason)' : ''}`);
process.exit(found.length ? 1 : 0);
