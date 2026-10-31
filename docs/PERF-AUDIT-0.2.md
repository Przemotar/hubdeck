# Performance of the deck and the app: audit before 0.2.0

Asked by the owner on 2026-10-27 (session 34): "dokładniejszy audyt wydajności i optymalizacji
zarówno samego decku jak i apki". Done in the night session 35, built on the frame cost debugger
(his "ok" to its sketch). Picture quality was never traded (the JPEG settings stay fixed,
`PROTOCOL.md`); every change was measured before and after.

## How it is measured

- `scripts/ui/theme-cost.mjs [seconds]`: a test copy with his profiles (virtual modules, the window
  open and mirroring the deck at 20 pictures a second) puts each of the 17 themes on, waits 15 s for
  the window to settle (the first theme measured right after start read 50-90 % CPU), then reads the
  frame cost (`frame_cost`: each layer's ms, the keys, the JPEG) and HubDeck's CPU (`deck_rate`)
  once a second. USB is not in it: the release gate's soak measures that on the real deck.
- The frame cost debugger in the Living deck panel (the same numbers, live, on his deck).
- `node scripts/release-gate.mjs --minutes 2 --skip audit,sizes,restarts` on the installed app.

## What a picture costs (session 35, before -> after)

| Theme | Layers before | Layers after | JPEG | CPU after (test copy, window open) |
|---|---|---|---|---|
| Halloween: haunted night | 9.0 ms | 7.1 ms | 2.8 ms | 23.3 % |
| Halloween: lanterns | 9.7 ms | 4.8 ms | 3.2 ms | 23.8 % |
| Halloween: spooky keys | 13.7 ms | 4.6 ms | 2.9 ms | 21.4 % (45 % before) |
| Poland: golden autumn | 8.0 ms | 5.4 ms | 3.1 ms | 22.7 % |
| Poland: mushroom hunt | 6.1 ms | 4.8 ms | 2.7 ms | 21.8 % |
| the twelve single-layer themes | 2.2-4.1 ms | 1.9-3.4 ms | 2.7-3.8 ms | 16-24 % |

The release gate on the installed build 7 (2 min, alone): soak 11.0 % CPU (9.9 % on build 6; 12.0
and 13.8 % before the chain), keys 16.1 pictures a second, memory flat, the window 126 frames a
second.

Every theme holds the deck's 24-25 pictures a second. A picture has 41.7 ms; the heaviest now
takes about 12 ms of HubDeck's time (layers, keys, JPEG).

## Done

1. **Neighbouring GPU layers as one chain** (`gpu::draw_chain`): the panel went up to the card and
   came back for every layer (~1.1-1.5 ms each even for a light one); now once for a run of layers
   on the card. One frame in 24 still draws them one by one, so the debugger knows each one's own
   share; `a_chain_draws_as_one_by_one` checks the two pictures are identical to the byte (a
   difference would blink once a second).
2. **The halo touches only its pixels**: worked out once per content as a list of the pixels it
   changes, not a pass over all 130 560 every frame (0.71 -> 0.60 ms).
3. **The load bar tells the truth**: it follows the measured time (F2 of `docs/UX-AUDIT-0.2.md`).

## Next, by gain

1. **Fog (`mist`) is the heaviest single layer** (2.0-4.1 ms) because it draws twice a frame: its
   far banks behind the icons (`under`) and a thin near bank in front of them (`over`), each a trip
   of the panel to the card and back; its noise is light (27 lookups a pixel). With the chain, its
   `under` already rides with its neighbours; next, its `over` with the other layers' `over` passes
   (Halloween spooky has five) once those sit next to each other in the drawing order.
2. **The halo is still 0.6 ms a frame on average**: it is worked out again whenever the keys'
   content changes, and his active profile has live keys (Mochi `core.pet`, the CPU reading,
   HWiNFO readings; no animated looks, checked in his profile). Not checked yet: how often that is
   per second (a counter in the frame cost). Then: work it out at a quarter of the size (it is a
   7 px blur) or only round the keys whose content changed.
3. **JPEG for 15 keys**: 2.7-4.1 ms a picture at quality 98 (fixed). The keys' JPEGs are independent:
   made on two or three threads, the picture is ready sooner (no change to the bytes or quality).
4. **A CPU layer in the middle breaks the chain** (bats, embers in "haunted night"): drawing the CPU
   layers before or after the GPU run where the drawing order allows it would keep one chain.
5. The window's WebView2 memory at 20 pictures a second (450-800 MB on the gate's test copy, flat):
   measure with the window closed and open on his deck; the mirror's pictures are already decoded
   before they go in (session 34).
