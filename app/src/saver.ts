// The screensaver drawn by the window itself, the same effects as the deck's
// (app/src-tauri/src/engine/saver.rs, keep the two alike): the window plays what moves on its own,
// as Stream Deck's window does, instead of taking a stream of the deck's pictures, which cost
// WebView2 320-860 MB (owner, 2026-10-16: "nie możemy tak samo zrobić?").

const W = 480;
const H = 272;
const TAU = Math.PI * 2;

/** The same steady pseudo-random number in 0..1 as saver.rs `rnd` (u32 wrapping arithmetic). */
export function rnd(i: number, salt: number): number {
  let x = (Math.imul(i, 2_654_435_761) ^ Math.imul(salt, 40_503)) >>> 0;
  x = (x ^ (x >>> 15)) >>> 0;
  x = Math.imul(x, 2_246_822_519) >>> 0;
  x = (x ^ (x >>> 13)) >>> 0;
  return (x % 10_000) / 10_000;
}

/** One frame of `effect` at `t` seconds in `color`, as the content of the 480x272 panel; null for
 * an unknown effect. */
export function saverBody(effect: string, t: number, color: string): string | null {
  let out = "";
  switch (effect) {
    case "rain": {
      const glyphs = ["0", "1", "7", "Z", "A", "X", "$", "#", "%", "@", "*", "+", "=", "[", "]", "?"];
      const cols = 24;
      const step = 16;
      for (let c = 0; c < cols; c++) {
        const speed = 40 + rnd(c, 1) * 70;
        const tail = 6 + Math.floor(rnd(c, 2) * 8);
        const head = (t * speed + rnd(c, 3) * (H + 200)) % (H + tail * step + 40);
        const x = ((c + 0.5) * W) / cols;
        for (let k = 0; k < tail; k++) {
          const y = head - k * step;
          if (y < 0 || y >= H + step) continue;
          const row = Math.floor(y / step);
          const g = glyphs[(Math.floor(rnd(c * 97 + row, 4) * 16) + (k === 0 ? Math.floor(t * 3) : 0)) % 16];
          const alpha = k === 0 ? 1 : 0.85 * (1 - k / tail);
          const fill = k === 0 ? "#e8ffe8" : color;
          out += `<text x="${x.toFixed(1)}" y="${y.toFixed(1)}" font-size="14" text-anchor="middle" fill="${fill}" fill-opacity="${alpha.toFixed(2)}" font-family="IBM Plex Sans, Segoe UI, sans-serif">${g === "<" ? "&lt;" : g}</text>`;
        }
      }
      return out;
    }
    case "stars": {
      for (let i = 0; i < 70; i++) {
        const [x, y] = [rnd(i, 5) * W, rnd(i, 6) * H];
        const r = 0.6 + rnd(i, 7) * 1.6;
        const a = 0.25 + 0.75 * (0.5 + 0.5 * Math.sin(t * (0.6 + rnd(i, 8) * 1.8) + rnd(i, 9) * TAU));
        out += `<circle cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="${r.toFixed(2)}" fill="#ffffff" fill-opacity="${a.toFixed(2)}"/>`;
      }
      const phase = t % 9;
      if (phase < 1) {
        const n = Math.floor(t / 9);
        const [x0, y0] = [rnd(n, 10) * W * 0.6, rnd(n, 11) * H * 0.4];
        const [x, y] = [x0 + phase * 220, y0 + phase * 90];
        out += `<line x1="${(x - 40).toFixed(1)}" y1="${(y - 16).toFixed(1)}" x2="${x.toFixed(1)}" y2="${y.toFixed(1)}" stroke="${color}" stroke-width="2" stroke-linecap="round" stroke-opacity="${(1 - phase).toFixed(2)}"/>`;
      }
      return out;
    }
    case "waves": {
      for (let layer = 0; layer < 3; layer++) {
        const amp = 18 + layer * 8;
        const base = H * (0.45 + layer * 0.17);
        const speed = 0.5 + layer * 0.25;
        let d = `M0 ${H}`;
        for (let x = 0; x <= W; x += 12) {
          const y = base + amp * Math.sin((x / W) * TAU * (1.2 + layer * 0.4) + t * speed);
          d += ` L${x.toFixed(0)} ${y.toFixed(1)}`;
        }
        d += ` L${W} ${H} Z`;
        out += `<path d="${d}" fill="${color}" fill-opacity="${(0.18 + layer * 0.12).toFixed(2)}"/>`;
      }
      return out;
    }
    default:
      return null;
  }
}
