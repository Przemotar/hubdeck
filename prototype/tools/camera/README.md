# Camera tools

A USB camera pointed at the deck (owner's PC, 2026-10-25/26; only when a task needs it and the owner
has set it up). OpenCV runs in a venv of its own, never the project's (`python -m venv camvenv`,
`camvenv/Scripts/pip install opencv-python`).

- `shot.py <out.jpg> <exposure>`: one photo, manual exposure (-8 at night).
- `film.py <out.mp4> <seconds> [key@second ...]`: a 30 fps film with presses sent through HubDeck's API.
- `timeline.py <film.avi>`: reads `crates/mh-device/examples/counter.rs` from a film, second by second.

See `docs/KNOWLEDGE.md` (the camera's exposure and focus) and `PROTOCOL.md` (what was measured with it).
