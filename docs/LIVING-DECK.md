# The living deck

How HubDeck's keys, dial and lights become one living, styled surface: whole key styles, effects
that run across the deck, reactions to presses and to what happens on the computer, themes for
seasons and places, and themes made from a description. A record and an idea bank,
kept across sessions. It does not steer how a preset is made: that is only the skill
`.claude/skills/living-deck-preset/` (owner, 2026-10-29, the clean sheet).

Started 2026-10-25/26 (session 31) on the owner's words: the patterns looked "średnio, jakby
niedopracowane"; "te przyciski muszą być topowej jakości"; then "rozkminiłbym jeszcze inne style -
ale takie top quality … wykorzystać potęgę opusa 5.5 … rzeczy, które do zrobienia ręcznie …
byłyby bardzo ciężkie albo nieopłacalne … Chcę mieć w hubdecku topowe rozwiązania, wyprzedające
znacznie konkurencję", and "Nie musimy tego robić szybko - chcę żebyś to ogarnął na spokojnie,
dokładnie i porządnie". Also: "może nawet będzie można napisać do cooler mastera i pokazać im co
zrobiłem".

## The aim

Top quality in every part, measured on the real deck, clearly ahead of Stream Deck and its
premium packs (`ARCHITECTURE.md` > The quality aim). A look is ready only after the camera, pixel
tests, CPU and window numbers, and the owner's pick; the morning after each stage the owner gets
a **blind test**: films of our effects mixed with premium pack previews, unlabelled, and he picks
which look better (his "to też", 2026-10-26).

## Status (keep current)

| Part | State | Where |
|---|---|---|
| Skins: film, glitch, neon, glass (`look.skin`) | engine, pixel test, API, window (Edit look > Templates > Styles; a page at once in the Living deck panel, with Undo); owner: "Style przycisków - super!" | `render.rs` `skinned`, `skin_layers`; test `skins_keep_the_content_on_the_key` |
| Skin motions: film, glitch, flicker, sparks | engine | `render.rs` `key_motion` |
| Deck-wide canvas (fog, rain, snow, fireflies, embers) and bats | engine, tests, window (the Living deck chip under the deck, its panel in the inspector's place); owner's test 2026-10-26: rain, snow, ripple kept; fog, fireflies, embers under pictures, bats redone the same day (session 32) | `canvas.rs`, `Profile::effects`, drawn between the keys' backgrounds and content (`Renderer::panel_pixmap_with`) |
| Reactions (ripple, embers from a press) | engine; `POST /api/effects/press` for apps and films | `canvas.rs` `Ripple` |
| Halloween skin `pumpkin` | engine, pixel test | `render.rs` |
| Full quality while moving, only changed keys sent | engine (per-key squares measured with the camera) | `render::changed_keys`, `key_slot`; `PROTOCOL.md` |
| Icon materials (relief, gold, chrome) | measured in a bench | |
| Themes | idea | |
| Theme from a description (AI) | idea | |
| Whole device (LEDs, dial) in one effect | idea | |
| Camera check of the real deck | working (`docs/KNOWLEDGE.md`, USB camera); only when a task needs it and the owner set it up (his word, 2026-10-26) | |
| The content mask (what of each pixel is icon, label, picture) | engine: the content layer drawn on its own, its alpha given to every effect (`Inputs::mask`) | `render.rs` `panel_pixmap_with`, `canvas.rs` `light_content` |

Pages shown to the owner: the four skins beside premium packs
https://claude.ai/artifact/Th53JXqZtF8NbWuTcgetb2 ; step 0 research and measurements
https://claude.ai/artifact/7XT9YxfgcNnkM8r1otc5PP ; the first sketches he rejected (patterns A/B/C)
https://claude.ai/artifact/2jdYPpZGX4cZ48EfXfhdtG .

## What others do (checked 2026-10-25)

- **Stream Deck**: per-icon packs made by hand on the Marketplace (Matinee, Glitch, Embers,
  Luminous, Horror 2 with "a dreadful apparition that appears on your keys from time to time");
  a static wallpaper cut across keys (Key Creator's Wallpaper Mode); a screensaver from an image
  or a GIF since 6.6, on some models only; Marketplace screensavers. Nothing live across keys,
  nothing that reacts beyond a key's own image.
- **Deck Threads** (a Stream Deck plugin): motions Wave, Orbit, Scan, Sparkle, Breathe, Sweep,
  Pulse, and it had to rate-limit them because animated keys flooded Stream Deck's render queue
  and froze the device. Motion is a problem there; HubDeck's engine budgets it from the start.
- **Wallpaper Engine** (live desktop wallpapers): a catalogue of effects (Water Ripple, Cursor
  Ripple, fluid simulation, Chromatic Aberration, Film Grain, VHS, Fire, God Rays, Water
  Caustics, Refraction, Depth Parallax), a particle editor, audio response, user properties,
  sharing on the Steam Workshop. The model to borrow from: effects as blocks with a few settings.
- **Razer Chroma**: ripple and reactive effects from a pressed key, effects as layers whose order
  matters; colours of LEDs only.
- **MasterHUB reviews**: praise the modularity and that the software knows how modules sit and
  turn; criticise rough software and a hard start. Nobody has shown the MasterHUB as one living
  surface.

## Facts that shape the design (measured)

- **The deck can stand upright** (owner, 2026-10-29: "niektórzy mogą mieć ustawione te 15 klawiszy
  w pionie"): the drawing turn is 0/90/180/270 (base sensor plus a manual turn, `PROTOCOL.md` >
  Orientation), and at 90/270 the user sees 3 columns x 5 rows. From the code (2026-10-29, not yet
  seen on the deck): particles follow it (`canvas.rs` `down_for`, tests at all four turns), but no
  scene shader reads `f.down`; every scene is composed for the landscape 480x272 only, so upright it
  lies on its side. Every preset needs a portrait composition and a portrait 1:1 check.

- The key panel is **one 480x272 screen** under 15 key windows (`render.rs` `PANEL_W/H`,
  `panel_tiles`): an effect can run across keys as one picture.
- The device takes **~32 frames a second** on the keys and the dial (`PROTOCOL.md`, 2026-10-10);
  HubDeck sends 16. Whether the screen shows every frame is not checked by eye yet (camera).
- **CPU, the whole key screen, release build** (2026-10-25): wave field with refraction 2.2 ms a
  frame (3.6 % of a core at 16/s), 60 lens raindrops 0.13 ms, icons relit from a distance field
  0.17 ms (field once 2 ms), noise aurora 3.9 ms, domain-warped noise fire 15.6 ms on one thread
  and 4.7 ms on four, the panel's JPEG 1.1 ms. **No GPU needed**; multi-octave noise at half
  resolution. HubDeck today: 9.4 % average on the busiest deck (limit 15 %).
- **Skins** cost on the first draw (film 3.9, glitch 1.6, glass 2.5, neon ~7 ms a key; a plain
  key 0.28), not in motion (0.08-0.5 ms a frame for the moving layer).
- **What the real screen shows** (camera, 2026-10-26): backgrounds darker than ~#24 are black
  (#000, #0e0e10 and the house #1b1a20 look the same); icons and labels on a dark key need
  ~#70 or more (#3a3a40 almost gone, #55555e dim). Subtle dark textures and gradients vanish.
- The modules' **physical layout** is known (`placements.json`: column, row, rotation of each
  module), so an effect can travel through the device as it stands on the desk.
- The music's **beat** is already heard (`beat.rs`, WASAPI loopback), for lights today.

## Idea bank

Each idea: what it is, why it is worth it, variants to explore, how it is checked. Decisions go
in "Decisions" below with the owner's words.

### A. Icons as lit objects (no 3D modelling)
From any icon (line icons, app logos, emoji) a distance field gives a height; lit with a moving
light it becomes gold, chrome, glass with refraction, an enamel pin, clay, carved wood, a neon
tube. Pack makers do this by hand per icon in 3D tools; HubDeck can do it for every app the user
has. Variants: light fixed / slowly sweeping / following the dial's turn / time of day; materials
as above. Check: the relief bench (0.17 ms a frame), pixel tests, camera.

### B. The whole deck as one live picture
Effects across keys: rain on glass with drops crossing keys, a ripple from the pressed key to
its neighbours, aurora or smoke flowing, fire rising from a hot CPU key, snow settling on the top
edges of the key windows, fog. Variants: each effect's speed, strength, palette; 16 or 32 fps.
Check: bench numbers, camera films on the real deck.

### C. A theme from a description (AI)
The user writes "like a Fallout terminal" or "gold on black marble"; the assistant composes a
theme from the engine's blocks as data (no outside code runs), the engine checks readability,
contrast and the CPU budget and shows three variants on the user's own keys. Also the way new
seasonal themes are made and shared (theme files, like `.streamDeckIconPack`).

### D. Effects that carry meaning
A CPU key whose fire grows with the load; a muted microphone's key in a red ripple; the Spotify
key breathing the cover's colours; in Counter-Strike 2 the deck pulsing red at low health and
ticking with a planted bomb (Valve's Game State Integration, official; CS2 details not checked).
Decoration that says something stays; pure decoration tires.

### E. The whole device as one space
A ripple from a key runs across the key screen, leaves through the base's LED edge, rings the
knobs and ends on the dial, following `placements.json`. Neither Stream Deck (screens only) nor
Razer (LEDs only) can do this.

### F. Themes that follow context
Themes per profile or per app (with auto-profiles): a calm studio in Photoshop, fire in a game,
the cover's colours in Spotify; by date (the Halloween week), by time of day.

### G. Seasonal and place themes
- Halloween: icons lit from inside like a carved pumpkin, fog drifting across keys, a bat now
  and then, a press throws embers, LEDs flicker like candles, a pumpkin on the dial.
- Christmas: snow across the deck settling on the keys' top edges, icons as glass baubles,
  LEDs twinkling like fairy lights.
- Japan: ink-wash icons, sakura petals, a press makes rings like a koi pond.
- Poland: frames and backgrounds from wycinanki (folk paper cut-outs), a folk palette; the real
  craft, not a stereotype.

### I. Surprises from ordinary use (owner, 2026-10-26) — decided: not doing ("1 c")
The owner: a bat (or other visitor) reacts to a press; after ten such meetings, or now and then, a
bonus effect across the whole screen; "to by było śmieszne", but maybe annoying, maybe a switch, and
"ludzie będą klikać w rzeczy sobie na deku i to może trochę być słabe". The catch: every press on a
deck does something real, so a reward must never invite pressing keys for fun. Proposal (session
32): surprises only as a side effect of ordinary use (a bat hangs on a key the user presses anyway
and flies off; a day's count of such meetings now and then sets off a short bonus, e.g. a colony
across the deck), at most once an hour, about 2 s, icons readable; never during a call, a game,
the screensaver or with Reduce motion; a "Surprises" switch in the Living deck panel; play on
purpose only on a page whose keys have no actions ("Catch the bat" on the Fun page, as Mochi is a
toy on the dial). Others: Stream Deck has none (a "Horror 2" pack shows an apparition now and then,
not interactive); Razer Chroma reacts to presses with light, no rewards.

### Not doing
- Pictures from an outside image generator inside the app (a service, a cost, data sent): all
  looks are computed locally from code.
- Motion all the time as the default.
- A full layer editor first: a gallery of themes with one click, then sliders, then the editor.

### The moonshot "Living World" (session 37, his `/boost:moonshot`, 2026-10-29; no answer yet)
One persistent miniature world behind the deck instead of presets: the real sun, weather and season
for his place, continuity (snow that fell at night lies in the morning), places as presets and the
dial to travel between them (A); a compositor that lays the scene round his icons and turns icons
into objects of the scene (B); film quality from code (light baked locally at install, a measured
display profile) (C); an AI art director running the method above as a machine, scoring its own
1:1 frames and learning his taste from his picks (D); the whole device and the room's lights
through official APIs (E); a world that grows, his Claude Code sessions as lit windows (F: against
his "»żywe z danych« bym nie robił", said so); an SDK and a workshop (G). Page:
https://claude.ai/artifact/VjiCgprN2ALU1Y5uG1hErH .

### More ideas (from the talks of session 31, not yet sorted into a stage's work)
- Screensavers like Elgato's, but live: any canvas effect as the screensaver, the dial included.
- A camera-measured display profile: the panel's real tone curve corrects every picture before
  it is sent, so dark gradients and textures survive (game changer 2; the owner has not picked it).
- The deck as a film for the owner's letter to Cooler Master and for the README.
- A creature or inhabitants per theme (bats, petals, fireflies); the pet stays on the dial.
- Sounds per theme: not proposed (the deck has no speaker; PC sound is the user's).

## The plan after session 33 (owner, 2026-10-26: "WOW, SUPER POMYSŁY! CHCĘ ZOBACZYĆ JE WSZYSTKIE!")

His words that set it: "Zostawiam Cię samego! Spisz to wszystko dokładnie a potem działaj
autonomicznie non stop … Wszystko dodawaj w tych opcjach living deck (chyba że coś nie będzie
pasowało i będzie wymagało innego miejsca). Nie spiesz się. Rób wszystko dokładnie i najwyższej
jakości i wydajności." Earlier the same day: A1 (sending only the keys' squares, the strip at a
key's edge standing still) is **not done** ("nie możemy robić tak że widać jakieś ucięcia … Chcę
maksymalną optymalizację przy maksymalnej jakości"); Claude agents on the deck: no ("nie chcę mieć
agentów claude na decku"); the Mac: later.

**What makes this deck different, and what the block effects build on**: the 15 keys are windows
onto **one** screen (480x272), with hidden strips (21-24 px) under the plastic frames between
them, and each window's place is now known to the pixel (`PROTOCOL.md` > Keys: where a per-key
image lands). On a Stream Deck each key is its own screen: none of the block effects can be done
there (it offers a wallpaper cut across keys only, see What others do).

Order (each step measured before the owner sees it: frame sheets at 24 a second, cost a frame,
bytes, the jitter test, CPU over 60 s, a camera film on the deck; 2-3 variants in an artifact
where a look is chosen; nothing goes in the panel that is not top quality):

1. **His test of the panel** (2026-10-26): keys jumped a pixel with fog and lamp: **fixed**
   (key squares to the pixel, camera-checked on his deck 16:03-16:05); the night sky (the moon
   hidden behind icons, stars that seemed to move too fast: calmer twinkle, the moon where the
   top row is emptiest, a rare shooting star); the lamp's shadow moving in whole pixels (smooth,
   sub-pixel); fog out of the panel until it is rewritten (step 6).
2. **A2**: CPU under 15 % (the gate at 16:06: 18.4 % average): embers on the GPU, all GPU layers
   in one round trip, the 18.7 %/18.4 % explained.
3. **A3** flashes by the backlight (no picture), **A4** the key colour command as an instant
   answer to a press.
4. **B5** icons as objects (gold, chrome, glass, relief under the lamp): Firefox first, filmed.
5. **K. Effects built on the blocks** (all six, his "chcę zobaczyć je wszystkie"):
   - **K1 Glass blocks**: each key a thick glass brick: whatever lies behind (sky, water, aurora)
     refracts in each brick on its own, with a lit bevelled edge and a reflection; the lamp's
     light goes through the glass.
   - **K2 Frames that cast shadows, and light through the gaps**: the real plastic frames cast a
     shadow into each key that moves with the lamp, so the picture looks set under the key; light
     from behind the frames falls in shafts between the blocks.
   - **K3 Fifteen windows onto one world, with depth**: an aquarium, a forest, a city at night
     behind the keys; things disappear behind a frame for exactly as long as the gap takes; near
     things move faster than far ones (parallax); turning the dial looks round the scene.
   - **K4 Blocks with physics**: a pressed key sinks and its neighbours answer like springs; a
     page change as the 15 keys turning like cards in a cascade, in perspective and lit (may need
     an "On a page change" category).
   - **K5 Light hopping from block to block**: a ball of light bouncing between keys, lighting
     the icons it passes, leaving into the base edge's LEDs and the knobs (meets C9).
   - **K6 Liquid in the blocks**: each key a glass jar; its level can show data (CPU, volume); a
     press makes it slosh (meets D11; the data source is a list, never typed).
6. **B6-B8 on the GPU**: real fire and smoke; **fog from scratch with depth** (banks passing in
   front of and behind the icons; back in the panel only after the owner's verdict on a film);
   aurora, neon rain, caustics.
7. **More layers** (each must be top quality and "wow", or it is dropped by me before he sees
   it): frost growing in crystals that melts under a finger; a holographic foil shifting with the
   dial's turn; a lava lamp whose blobs cross the blocks; ink spreading in water.
8. **The cover's colours** (his "ok dodajmy taką opcję w live decku"): the now-playing cover's
   colours become the living deck's tint (an option in the panel), and "Lubię to" gets its own
   reaction (a heart spreading over the keys).
9. **The pixel tests** (his "ale jak?", answered 2026-10-26): (a) the camera measurement as one
   command (`examples/align.rs` plus its analysis: per key "on" or "a pixel to the right"), run
   after a hardware or camera change; (b) automatic, without a camera, in every build: the
   measured key table kept, and on every effect the icons' edges must not move between frames
   (the jitter test that would have caught the lamp's stepping shadow).
10. Then (owner, 2026-10-26 19:35: "C10 … tego nie róbmy. Możemy motywy wrzucić wcześniej? Przed
    C9?"): **themes** (Halloween with pumpkins and real spookiness, Christmas, Japan, Poland; a
    restatement and a sketch first, built on his ok), then C9 (the LEDs in the same effect), D11
    (effects from data), E12-E14 (moving previews, saved sets, a live load bar), the community
    gallery (stage 5), the release with Discord. **C10 (the dial's screen) not done.** The Mac later.

Where things go: the Living deck panel's categories (Across the keys, On a press, Now and then,
Behind the icons, Light on the icons, Finish); a new category only where none fits (a page
change). Each new effect gets its tile, its settings, its cost on the load bar, a walk on the
test copy with his profiles, a pixel test and a camera film.

## Themes and spreading presses (owner, 2026-10-26 19:40: "1 a, tak super pomysł … po jednej rundzie ideacji na każdy z tych motywów … każdy z tych efektów musi być na maksa dopieszczony … wow efekt (ale nie kosztem wydajności)"; "efekty naciśnięcia … które rozchodzą się na pozostałe przyciski … zaskocz mnie")

A theme is one click in the Living deck panel: it switches on its layers with its palette (they
show as switched-on tiles below and stay adjustable); the owner's key styles stay. Each theme's
round of ideas, the picks marked ✓ (built in this order: Halloween before 31.10, the presses,
Christmas, Japan, Poland):

- **Halloween**: ✓ jack-o'-lanterns on the keys' floors, flickering like candles, the pressed key's
  flaring (his "tak super pomysł"); ✓ eyes in the dark that open in empty corners, blink, and look
  at the key just pressed; ✓ a ghost drifting behind the windows, gone behind each frame, its glow
  on the icons' edges; ✓ a spider letting itself down on a thread into a key, climbing back when
  that key is pressed; ✓ cobwebs in the windows' corners with a drop of dew catching the light;
  the violet night sky with the full moon, low fog, bats, embers on a press. Also thought of: a
  haunted skyline on the bottom row (covered by his pictures), violet lightning (a flash).
- **Christmas**: ✓ a garland of bulbs along each row, twinkling in turn, a press sending a chase of
  light out from that key; ✓ snow settling on the icons, a press shaking it off that key; ✓ the warm
  glow of a fire from below; ✓ a sleigh crossing the moon, gone behind the frames; frost in the
  corners. Not: a present that appears (surprises are not done, his "1 c").
- **Japan**: ✓ cherry petals falling, settling on icons, a press blowing them off in a gust out
  over the other keys; ✓ koi in a pond behind the icons that gather at the pressed key as if fed;
  ✓ paper lanterns glowing behind the windows, swaying; ✓ an ukiyo-e moon with thin bands of cloud.
- **Poland (golden autumn)**: ✓ maple and oak leaves tumbling as they fall (turning in 3D), settling,
  a press blowing them out; ✓ "babie lato": gossamer threads drifting, glinting in the low sun;
  ✓ low golden light through the frames; ✓ storks crossing in a V, south; morning mist low.

Presses that spread to the other keys (the pressed key is under the finger, so the effect has to
travel), all in "On a press":
- ✓ **Domino**: the cards turn in a wave out from the pressed key (the page-turn's cards, from a
  press);
- ✓ **Chain lightning**: an arc leaps from the pressed key to three or four others, each flashing
  as it is struck (a flashing effect: the warning);
- ✓ **Firework**: a rocket rises from the key and bursts over the deck, sparks falling over the
  other keys;
- ✓ **Shockwave**: a ring of bent glass runs out from the key, bending the icons as it passes, its
  edge splitting into colours;
- ✓ **Magnet**: every other icon leans toward the pressed key and springs back;
- **Colour bloom**: the pressed key's colour floods outward key by key (thought of; built if the
  five above leave room).

## The plan after session 32 (owner's "1 a", 2026-10-26: "Super pomysły")

Built so far: the effects in six categories switched on and off (owner: categories, not a stack of
layers), the engine draws them in a fixed order; GPU layers (water, lamp, night sky, tiles); 22 a
second on a snowy deck; the key screen shows at most ~24 different pictures a second (camera).
In this order, each one measured (frame sheets, cost, bytes, the deck's picture, camera when
needed) before the owner looks:

**A. The engine: smoothness and cost** (every later effect needs the headroom)
1. ~~The last 5 % to 24 a second: with effects on, send only the keys' squares, not the strips
   round them (16 % fewer bytes on snow, measured).~~ Not done (owner, 2026-10-30: "Nie robimy tego wysylania samych kwadratow").
2. CPU back under 15 %: fog and embers on the GPU (2.2 and 2.4 ms a frame on the CPU now), and all
   GPU layers in one round trip to the card instead of one each.
3. Flashes by the screen's backlight (lightning, pulses): no picture sent; 4-24 ms a change
   (camera, 2026-10-26).
4. An instant answer to a press by the key colour command (14 ms, no JPEG) before the full
   picture arrives.

**B. New effects on the GPU** (what the CPU cannot afford)
5. Icons as objects: gold, chrome, glass, relief, catching the lamp's light. No deck on the
   market has it. First one icon (Firefox) in three materials, filmed, for the owner's verdict.
6. Real fire and smoke instead of a glow.
7. Fog with depth: banks passing in front of and behind the icons.
8. Aurora, neon rain, caustics (a still pool-floor light).

**C. The whole device in one effect**
9. The edge and knob LEDs in the same field: the ripple runs from a key onto the base's edge, the
   lamp lights the knobs.
10. The dial's round screen with the same effects (fog over the cover, water).

**D. Effects that mean something**
11. An effect's option tied to data: fire grows with the CPU, water moves to the music's beat,
    frost on the keys while the microphone is muted, the sky following the time of day.

**E. The window**
12. Moving previews on the tiles, drawn by the engine.
13. Saved sets ("My evening") and a Random button.
14. The load bar from a live measurement instead of a table.

Then themes (Halloween with pumpkins and real spookiness, Christmas, Japan, Poland) built from
these blocks; the first Halloween attempt (fog, fire, wisps over plain keys) was rejected: "nie
czuć tutaj klimatu halloween. Spodziewałem się jakiś dyń, spooky".

## Architecture: built so no later stage is blocked

The owner, 2026-10-26: "musisz to mądrze zaplanować, żeby przygotować architekturę i systemy tak,
żeby te kolejne rzeczy dało się robić, a nie żeby jakimś ruchem zablokować resztę możliwości".
Every stage builds on these seven parts; none of them may be cut short by a later shortcut.

1. **One effect interface for every kind of maker.** An effect gets the same inputs and draws into
   the same surface whether it is a built-in Rust effect (stage 1), a theme's block, or a shader
   program written by AI (stage 4, game changer 1). A shader runtime then plugs in without a
   redesign, and the two are compared on the same effect.
2. **Device space.** Effects think in the device's physical layout (`placements.json`: modules,
   columns, rows, rotations): the key panel first, then the dial and the LEDs sample the same
   field, so "a ripple runs from the keys through the base's edge to the dial" is a placement,
   not new code. Stage 1 implements the key panel, but already in these coordinates (and the
   panel's rotation).
3. **Named inputs, the same for all effects** (like ISF's inputs): time, presses (control,
   place, time), the beat and loudness, metrics (CPU, GPU, temperatures), now-playing colours,
   a game's state, the time of day, the content mask (where icons and labels are). New inputs
   are added, never renamed.
4. **A layer compositor with a fixed order** (content, key finish, deck canvas, reactions,
   inhabitants, device) in the renderer; effects get the content mask so they can go round it,
   light it or refract at its edges; keys with a full picture take part (a rim light, refraction
   at the edges), not only keys with a background.
5. **Looks and themes as versioned data** (JSON): an effect's id and its parameters, a palette,
   finishes per control, with each parameter's type, range and default declared so the window
   builds its sliders by itself (like ISF's JSON header). A built-in effect and a shader file are
   referenced the same way; themes become files that can be shared.
6. **Quality and cost.** **Full picture quality always** (owner, 2026-10-26: "nie możemy pogarszać
   jakości … musimy optymalizować tak, żeby nie tracić jakości"): no lower JPEG while effects run;
   the speed comes from drawing less (tiles kept, effects at their own resolution, the content
   layer cached), never from a worse picture of the content. Each effect has a measured cost; the
   engine keeps a CPU budget per machine (the Mac too, owner: "przetestujemy to też na Macu") by
   lowering the effect's own resolution or rate, never the content's.
7. **Safety and checks.** Effects that flash carry a warning in the window (owner: "dodamy
   ostrzeżenia przy tych trybach") and a flash limiter (WCAG's three flashes a second);
   reduced motion stills them. Every look and effect, built-in or AI-made, goes through the same
   checks: pixel tests (content readable, the brightness rule: background >= ~#24 to read as
   colour, content >= ~#70), the CPU and the window's memory measured (owner: "trzeba będzie
   zrobić pomiar"), the camera on the real deck.

## Stages

Each stage ends with films from the camera, numbers and the owner's picks; the next starts from
this document.

1. **Foundation and the first wow** (the night of 2026-10-26): the effect interface, the layer
   compositor with the content mask, device space for the key panel, full JPEG quality while
   moving (measured: the send time and the frame rate stay), the ripple from a press (across
   picture keys too), Halloween on a trial profile, three more effects polished to the end (rain
   on glass, snow settling on the keys, fireflies or embers), the flash limiter and the warning
   flag, the window's memory and smoothness measured, the install's way back tested before the
   first install; at the end, if the night allows, a fair test of the shader approach: the same
   ripple both ways, and "your icons as gold under water, the ripple bending light on them".
2. **The whole device and themes**: the dial and the LEDs in the same field; the theme format as
   data; Halloween, Christmas, Japan, Poland; the theme picker in the window (and the four skins'
   picker, the owner's "1 a"); where a theme is kept and what it does to the user's own looks
   (owner's call); the display profile from the camera if he wants it.
3. **Meaning and context**: icons as lit objects (materials); effects that carry information
   (CPU fire, a muted microphone, the cover's colours, Counter-Strike 2's state); themes by
   profile, app, date and time of day; screensavers from effects.
4. **Effects written by AI**: the shader runtime behind the same interface (wgpu: size and
   stability measured), the theme from a description (blocks or a program), validated by the same
   checks; compared with stage 1's effects on possibilities, customisation, wow and quality.
5. **Sharing and reach** (owner, 2026-10-26: "super pomysł! … zachęcać community do działania i
   dzielenia się … miejsce gdzie zbieramy te motywy zrobione przez ludzi, inni mogą oceniać,
   pobierać"): a community theme gallery: people publish their themes (made by hand or from a
   description), others browse with live previews, rate and install in one click; themes are
   data (and checked shader files), so installing one runs no outside code; the same checks as
   built-in looks (readability, CPU, flashes) before a theme is listed; where it lives (a GitHub
   repository with submissions as pull requests and a gallery page, or a small service) and
   moderation are open questions. Then theme files and a gallery, an editor (gallery first, sliders, then
   layers), the Mac's gate and quality levels, blind tests, the film and the letter to Cooler
   Master.

## The hostile critique (2026-10-26) and the answers

| Found | Answer |
|---|---|
| Too much for one night, the "almost done" the owner rejected | stages above; the night is stage 1 |
| The install's way back never run | tested on the test copy before the first install |
| Moving pictures go at a lower JPEG quality, so labels blur under a constant effect | full quality always (owner); measure the send time |
| Half of the owner's deck is full pictures where an effect under content is invisible | effects light and refract picture keys at their edges |
| A shader test at the night's end would be unfair | the same ripple both ways, equal time, or it moves to stage 4 |
| A 30 fps camera cannot plainly show 32 fps on the screen | a frame counter judged statistically (how often a number repeats) |
| Numbers from one strong PC | a CPU budget per machine; tested on the Mac too (owner) |
| Flashing effects can harm | a warning in the window and a flash limiter (owner) |
| Themes' storage and their effect on own looks undecided | the owner decides in stage 2 |
| The window pays for every frame (WebView2 grew 45 MB a minute with the screensaver drawn there) | measured with every effect (owner) |
| "Wow" judged by me after I got the patterns wrong | the camera, and a blind test with the owner |

## Decisions (owner's words, dates)
- 2026-10-26 (session 32, after testing the 09:28 build): "deszcz jest bardzo fajny, śnieg super
  i fajnie się osadza na przyciskach (… czy nie da się tak zrobić żeby faktycznie osiadał na ikonach
  od góry?)", "Ripple super efekt - to jest wow!", "Style przycisków - super!"; to redo: fog "dosyć
  słaby bo nie widać go i z nim przyciski jakoś tak drżą", fireflies "chaotyczne trochę i średnio
  wygląda", embers under keys with their own black pictures did not show, bats "tanio" without real
  behaviour or variety; then "Może się nawarstwiać bardziej" (snow).

- 2026-10-26: a theme is kept in the profile (each profile may have its own); the user's own key
  styles stay and the theme adds the background, effects and lights ("1a").
- 2026-10-26: a blind test after each stage ("to też"); with context left, go on with the other
  ideas to explore.

- 2026-10-26: no lower picture quality, ever: optimise instead ("nie możemy pogarszać jakości");
  test on the Mac too; warnings on flashing modes; measure the window; write down and group all
  ideas into stages, and plan the architecture so later stages are never blocked.

- 2026-10-25: four skins to build into the app ("1 a"); "fajne, szczególnie ten film i glitch".
- 2026-10-25: all three samples A, B and C ("1 c"); step 0 and 1 first ("1 a").
- 2026-10-25: 105 ms for drawing a page of neon keys is acceptable ("i tak nie przełącza się jak
  się już ustawi"), unless switching redraws effects.
- 2026-10-25: a USB camera looks at the deck so looks are checked on the real screen ("1 a").
- 2026-10-26: the night's order: the ripple from a press and Halloween on the real deck first, then
  many effects, each polished ("1 a i c - ale opcja C też musi być dopracowana mega dobrze"); the
  original plan (effects computed in the engine on the CPU), and at the end a test of game changer 1
  (the look as a shader program written by AI, one field over the whole device incl. LEDs, files
  like ISF) to compare which gives more: possibilities, customisation, wow, quality.
- 2026-10-26: overnight: explore, build, test and polish; ready solutions in the morning; write
  everything down so the work can go on over many sessions.

## Open questions

- Does the key screen show all 32 frames a second? (camera film of a frame counter)
- A creature walking between keys: against the 2026-10-14 pick (pet on the dial only); only as a
  theme's inhabitant if the owner wants it.
- Themes as shareable files: format and where people find them (GitHub, a gallery page).
- The dial and the LEDs in themes: what each theme does there.

## Journal

- 2026-10-25/26, session 31: skins built in the engine; step 0 (bench, research, camera);
  this document; overnight work on the deck canvas, reactions, materials and themes.
- 2026-10-26, session 32: the window's place for it (the Living deck chip and panel, styles in Edit
  look); after the owner's test: the content mask; snow settles on the flat tops of icons (8 px wide
  or more, so not on letters) and builds for half a minute; fog as big smooth banks computed at a
  quarter size (the fine noise made the keys seem to tremble); fireflies slow with flashes and waves;
  embers light pictures that cover the glow, sparks in front; bats drawn and flown anew (wings with
  bones and scallops, flaps and glides, colonies, one that hangs from a key until pressed). Costs
  (release, whole panel): fog 3.3 ms a frame, embers 2.5, ripple 2.8, rain, snow, fireflies under
  0.3 ms.
- 2026-10-26, session 32: **the shader comparison** (`prototype/gpu-bench`, outside the app's
  workspace; RTX 3060 Ti, Vulkan through wgpu 25): the engine's ripple CPU 5.5 ms a frame vs GPU
  0.54 ms with the panel up and back; a heavy ripple (full-size field, 48 steps, caustics) CPU 9.1
  ms vs GPU 1.5 ms; the pictures equal within 1 of 255; GPU start-up 0.7-1 s, pipelines 3-70 ms. At
  16 frames a second for 30 s the card stayed at its idle ~21 W in 7 of 9 samples (2 at 44 W, its
  boost); running flat out it boosts to 44 W. So the GPU is ten times faster and costs little power
  at the deck's pace, but the deck's limit today is the bytes sent at quality 98, not drawing: the
  GPU pays off for effects the CPU cannot afford (full-size water, light on icons, many particles,
  effects written as shaders). Not checked: the Mac (Metal), laptops' integrated GPUs, what wgpu
  adds to HubDeck's size (the bench's whole exe is 5.7 MB), a lost device.
