# Sharing profiles

A HubDeck profile can travel as one file, and profiles can be shared through a public gallery
that the app browses (Profile tabs > "+" > Browse shared profiles).

## The package: `.hubdeck`

A `.hubdeck` file is a zip archive:

```
profile.json          the profile, exactly as HubDeck stores it
images/<name>         every picture the profile uses
```

- `profile.json` is the profile format of `app/src-tauri/src/engine/profile.rs` (`Profile`,
  `version` 1). Its `id` does not matter: an import always adds the profile under a new id and never
  overwrites one.
- `images/` holds every picture the profile refers to: key looks on all pages and gestures
  (`look.image`), the look while on (`lookOn.image`) and the full-panel picture
  (`panel: { kind: "picture", image }`). Names are the ones in `profile.json`: letters, digits, `.`,
  `-` and `_`, no folders. PNG, JPG, GIF (animated too), WebP and SVG.
- On import each picture is stored under a name made from its content, so two different pictures
  with the same name never clash; the profile's looks are updated to the new names. A picture that
  is missing or cannot be read is dropped from its look (the key keeps its icon and label).
- Limits: 15 MB per picture, 100 MB unpacked, 1000 files. Anything else in the zip is ignored.

Export: Profile panel > Export, file type "HubDeck profile with its pictures". The plain JSON file
type ("Settings only") is still there for editing by hand; it has no pictures.

Import: Profile panel > Import takes `.hubdeck`, `.json` (HubDeck) and `.mhz` (the official app's
export).

Sounds of the Play sound action and programs of Open app, file or website are paths on the
author's computer; they are not packed. After importing, check such keys.

## The gallery

The gallery is the public repository `Przemotar/hubdeck-profiles`. The app reads

```
https://raw.githubusercontent.com/Przemotar/hubdeck-profiles/main/index.json
```

with a 5 second timeout, off the UI thread. When it cannot be reached the panel says "The gallery
is not online yet". Packages are downloaded only from that repository
(`https://raw.githubusercontent.com/Przemotar/hubdeck-profiles/...` or
`https://github.com/Przemotar/hubdeck-profiles/...`).

### `index.json`

```json
{
  "version": 1,
  "profiles": [
    {
      "id": "obs-streaming",
      "name": "OBS streaming",
      "author": "Przemotar",
      "description": "Scenes, record, stream and replay on the keys with live state; mic level on the dial.",
      "kinds": ["keys", "dial", "faders"],
      "tags": ["streaming", "obs"],
      "preview": "https://raw.githubusercontent.com/Przemotar/hubdeck-profiles/main/obs-streaming/preview.png",
      "package": "https://raw.githubusercontent.com/Przemotar/hubdeck-profiles/main/obs-streaming/obs-streaming.hubdeck",
      "version": "1.0"
    }
  ]
}
```

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | Unique, lowercase, `-` between words; also the folder name |
| `name` | yes | Shown on the card |
| `author` | no | Your name or GitHub handle |
| `description` | no | One or two sentences: what the profile is for, what is where |
| `kinds` | no | Modules it uses: `keys`, `dial`, `knobs`, `faders`, `rollers`. The card greys the ones the user does not have |
| `tags` | no | Short words for searching: `streaming`, `photo`, `music` |
| `preview` | no | A picture of the deck with the profile (PNG or JPG, about 800 x 450) |
| `package` | yes | The `.hubdeck` file in the repository |
| `version` | no | Your version of the profile, e.g. `1.1` |

Unknown fields are ignored, so the format can grow.

## Submitting a profile

1. In HubDeck, Profile panel > Export > "HubDeck profile with its pictures". Use a name without
   private details (no personal paths, accounts or tokens; HubDeck never puts passwords or tokens in
   a profile).
2. Optional: a screenshot of the deck with the profile as `preview.png`.
3. Fork `Przemotar/hubdeck-profiles`, add a folder `<id>/` with `<id>.hubdeck` (and `preview.png`),
   and add an entry to `index.json`.
4. Open a pull request saying what the profile does and which apps it is for.

Reviewers check that the profile imports, that its pictures are your own or free to share, and that
no key runs a program or command that is not obvious from its label.
