// Deck drawing (inline SVG) for the three HubDeck look variants.
// Geometry: base lying flat (6 x 4 cells, 100 units per cell): rollers top-left 4x1,
// keys below 4x3, knobs top-right 2x2, dial bottom-right 2x2.

const r2 = (n) => Math.round(n * 100) / 100;
const polar = (cx, cy, r, deg) => {
  const a = (deg * Math.PI) / 180;
  return [r2(cx + r * Math.cos(a)), r2(cy + r * Math.sin(a))];
};
const arc = (cx, cy, r, a0, a1) => {
  const [x0, y0] = polar(cx, cy, r, a0);
  const [x1, y1] = polar(cx, cy, r, a1);
  return `M${x0} ${y0}A${r} ${r} 0 ${a1 - a0 > 180 ? 1 : 0} 1 ${x1} ${y1}`;
};

export const FACES = [
  { i: "mic-off", l: "Mute mic", bg: "#b42a42", ic: "#fff", lc: "#ffe1e6" },
  { i: "monitor", l: "Gameplay", bg: "#7c5cff", ic: "#fff", lc: "#fff" },
  { i: "message-square", l: "Chatting", ic: "#a5b4fc" },
  { i: "clock", l: "BRB", ic: "#fbbf24" },
  { i: "circle-dot", l: "Record", ic: "#ff6b6b" },
  { i: "radio", l: "Go live", ic: "#f472b6" },
  { i: "rocket", l: "Replay", ic: "#67e8f9" },
  { i: "music", l: "Spotify", bg: "#0d2a19", ic: "#1ed760", lc: "#bff5d2" },
  { i: "heart", l: "Like", ic: "#1ed760" },
  { i: "volume-2", l: "Volume", ic: "#e4e4ea" },
  { i: "play", l: "Play", ic: "#e4e4ea" },
  { i: "skip-forward", l: "Next", ic: "#e4e4ea" },
  { i: "globe", l: "Open", ic: "#93c5fd" },
  { i: "lightbulb", l: "Desk light", bg: "#2b2110", ic: "#fcd34d", lc: "#fde9b0" },
  { i: "folder", l: "Audio page", ic: "#c4b5fd" },
];

export const STYLES = {
  a: {
    vb: "-28 -28 656 496",
    board: ["#222129", "#15151a"], well: "#09090c", mod: ["#1b1a21", "#111116"], hl: 0.12,
    glow: 0.8, glowW: 9, blur: 13, halo: 0, pool: 0,
    metal: ["#4b4a54", "#1d1c22", "#5e5d67", "#18171c", "#3c3b44"],
    bezel: false, noise: false, keyFace: "#17161d",
    shadow: [16, 18, 0.65, "#000"],
    tip: ["#25242d", "#ededf2", "#a3a2b1", "#3a3944"], acc: "#8b7cf6", font: "inherit",
  },
  b: {
    vb: "-26 -26 652 492",
    board: ["#25242c", "#121116"], well: "#060608", mod: ["#1d1c23", "#0e0e12"], hl: 0.16,
    glow: 1, glowW: 14, blur: 19, inner: 0.35, halo: 0.34, pool: 0.55,
    metal: ["#9a99a3", "#3a3941", "#c9c8d1", "#2c2b32", "#7d7c86"],
    bezel: true, noise: true, sheen: true, brushed: true, keyFace: "#141319",
    shadow: [22, 22, 0.8, "#000"],
    tip: ["#1f1d29", "#f3f2f8", "#a9a6bb", "#3b3850"], acc: "#9d8cff", font: "inherit",
  },
  c: {
    vb: "-28 -28 656 496",
    board: ["#2b2a31", "#1a191f"], well: "#0b0b0e", mod: ["#1f1e25", "#121117"], hl: 0.15,
    glow: 0.7, glowW: 10, blur: 18, inner: 0.2, halo: 0.22, pool: 0,
    metal: ["#8d8c96", "#34333b", "#bdbcc5", "#2a2930", "#6f6e78"],
    bezel: true, noise: false, brushed: true, keyFace: "#18171e",
    shadow: [22, 24, 0.3, "#2a2342"],
    tip: ["#ffffff", "#17171c", "#626273", "#e2e2ea"], acc: "#6e56ff", font: "inherit",
  },
};

export function deck(p, s) {
  const id = (n) => `${p}-${n}`;
  const u = (n) => `url(#${id(n)})`;
  const out = [];
  const [sdy, ssd, sop, scol] = s.shadow;
  const knobs = [
    { n: 2, x: 452, y: 62, c: "#22d3ee", a: -58 },
    { n: 1, x: 548, y: 62, c: "#ff9f43", a: 34 },
    { n: 3, x: 500, y: 141, c: "#f472b6", a: -12 },
  ];
  out.push(`<svg class="deck" viewBox="${s.vb}" role="img" aria-label="Deck: rollers, 15 keys, three knobs and the dial on the base">`);
  out.push(`<defs>
<linearGradient id="${id("edge")}" gradientUnits="userSpaceOnUse" x1="-20" y1="-20" x2="620" y2="420"><stop offset="0" stop-color="#7c5cff"/><stop offset=".35" stop-color="#b892ff"/><stop offset=".62" stop-color="#8b5cf6"/><stop offset="1" stop-color="#d48cff"/></linearGradient>
<filter id="${id("blur")}" x="-25%" y="-25%" width="150%" height="150%"><feGaussianBlur stdDeviation="${s.blur}"/></filter>
<filter id="${id("pblur")}" x="-20%" y="-300%" width="140%" height="700%"><feGaussianBlur stdDeviation="18"/></filter>
<filter id="${id("soft")}" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="3"/></filter>
<filter id="${id("sh")}" x="-20%" y="-20%" width="140%" height="160%"><feDropShadow dx="0" dy="${sdy}" stdDeviation="${ssd}" flood-color="${scol}" flood-opacity="${sop}"/></filter>
<filter id="${id("tsh")}" x="-20%" y="-30%" width="140%" height="190%"><feDropShadow dx="0" dy="4" stdDeviation="5" flood-color="#000" flood-opacity=".35"/></filter>
<linearGradient id="${id("board")}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${s.board[0]}"/><stop offset="1" stop-color="${s.board[1]}"/></linearGradient>
<linearGradient id="${id("mod")}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${s.mod[0]}"/><stop offset="1" stop-color="${s.mod[1]}"/></linearGradient>
<linearGradient id="${id("hl")}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="${s.hl}"/><stop offset=".3" stop-color="#fff" stop-opacity="${r2(s.hl / 4)}"/><stop offset="1" stop-color="#fff" stop-opacity=".015"/></linearGradient>
<linearGradient id="${id("glass")}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="${s.bezel ? 0.2 : 0.13}"/><stop offset=".44" stop-color="#fff" stop-opacity=".03"/><stop offset=".5" stop-color="#fff" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity=".3"/></linearGradient>
<linearGradient id="${id("cyl")}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#040405"/><stop offset=".2" stop-color="#2c2b33"/><stop offset=".4" stop-color="#72717d"/><stop offset=".56" stop-color="#3b3a43"/><stop offset="1" stop-color="#040405"/></linearGradient>
<linearGradient id="${id("rend")}" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#000" stop-opacity=".75"/><stop offset=".14" stop-color="#000" stop-opacity="0"/><stop offset=".86" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity=".75"/></linearGradient>
<linearGradient id="${id("sheen")}" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#fff" stop-opacity=".09"/><stop offset=".45" stop-color="#fff" stop-opacity="0"/></linearGradient>
<radialGradient id="${id("knob")}" cx=".38" cy=".3" r=".8"><stop offset="0" stop-color="#55545f"/><stop offset=".55" stop-color="#26252d"/><stop offset="1" stop-color="#0f0f13"/></radialGradient>
<radialGradient id="${id("cap")}" cx=".42" cy=".36" r=".72"><stop offset="0" stop-color="#34333c"/><stop offset="1" stop-color="#17161c"/></radialGradient>
<linearGradient id="${id("metal")}" x1="0" y1="0" x2="1" y2="1">${s.metal.map((c, i) => `<stop offset="${r2(i / (s.metal.length - 1))}" stop-color="${c}"/>`).join("")}</linearGradient>
<radialGradient id="${id("scr")}" cx=".5" cy=".4" r=".62"><stop offset="0" stop-color="#1a1524"/><stop offset="1" stop-color="#050408"/></radialGradient>
<linearGradient id="${id("arc")}" gradientUnits="userSpaceOnUse" x1="455" y1="340" x2="515" y2="250"><stop offset="0" stop-color="#6d4dff"/><stop offset="1" stop-color="#d48cff"/></linearGradient>
<radialGradient id="${id("halo")}"><stop offset="0" stop-color="#8b5cf6" stop-opacity="${s.halo}"/><stop offset=".55" stop-color="#8b5cf6" stop-opacity="${r2(s.halo * 0.35)}"/><stop offset="1" stop-color="#8b5cf6" stop-opacity="0"/></radialGradient>
${knobs.map((k) => `<radialGradient id="${id("k" + k.n)}"><stop offset=".6" stop-color="${k.c}" stop-opacity=".42"/><stop offset="1" stop-color="${k.c}" stop-opacity="0"/></radialGradient>`).join("")}
${s.noise ? `<filter id="${id("noise")}" x="0" y="0" width="100%" height="100%"><feTurbulence type="fractalNoise" baseFrequency=".9" numOctaves="2" stitchTiles="stitch"/><feColorMatrix values="0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 .55 0"/><feComposite in2="SourceAlpha" operator="in"/></filter>` : ""}
</defs>`);

  // light that the edge throws on the desk
  if (s.halo) out.push(`<ellipse cx="300" cy="200" rx="400" ry="300" fill="${u("halo")}"/>`);
  if (s.pool) out.push(`<ellipse cx="300" cy="440" rx="320" ry="22" fill="#8b5cf6" opacity="${s.pool}" filter="${u("pblur")}"/>`);
  // base: glow, body, light bar
  out.push(`<rect class="edge-glow" x="-20" y="-20" width="640" height="440" rx="30" fill="none" stroke="${u("edge")}" stroke-width="${s.glowW}" opacity="${s.glow}" filter="${u("blur")}"/>`);
  out.push(`<rect x="-20" y="-20" width="640" height="440" rx="30" fill="${u("board")}" filter="${u("sh")}"/>`);
  if (s.noise) out.push(`<rect x="-20" y="-20" width="640" height="440" rx="30" fill="#000" filter="${u("noise")}" opacity=".07"/>`);
  out.push(`<rect x="-18.5" y="-18.5" width="637" height="437" rx="28.5" fill="none" stroke="${u("edge")}" stroke-width="3"/>`);
  out.push(`<rect x="-18.5" y="-18.5" width="637" height="437" rx="28.5" fill="none" stroke="#fff" stroke-opacity=".45" stroke-width=".7"/>`);
  if (s.inner) out.push(`<rect x="-13" y="-13" width="626" height="426" rx="24" fill="none" stroke="${u("edge")}" stroke-width="5" opacity="${s.inner}" filter="${u("soft")}"/>`);
  out.push(`<rect x="-7" y="-7" width="614" height="414" rx="19" fill="${s.well}"/>`);

  const mod = (x, y, w, h) =>
    `<rect x="${x + 4}" y="${y + 4}" width="${w - 8}" height="${h - 8}" rx="14" fill="${u("mod")}"/><rect x="${x + 4.5}" y="${y + 4.5}" width="${w - 9}" height="${h - 9}" rx="13.5" fill="none" stroke="${u("hl")}"/>`;

  // rollers
  out.push(mod(0, 0, 400, 100));
  for (const cx of [102, 298]) {
    out.push(`<rect x="${cx - 86}" y="24" width="172" height="52" rx="15" fill="#050507" stroke="#000" stroke-opacity=".6"/>`);
    out.push(`<rect x="${cx - 86}" y="24.5" width="172" height="52" rx="15" fill="none" stroke="#fff" stroke-opacity=".05"/>`);
    out.push(`<rect x="${cx - 78}" y="33" width="156" height="34" rx="7" fill="${u("cyl")}"/>`);
    const lines = [];
    for (let i = 1; i < 12; i++) {
      const y = r2(50 + 17 * Math.sin(-Math.PI / 2 + (i * Math.PI) / 12));
      lines.push(`M${cx - 76} ${y}H${cx + 76}`);
    }
    out.push(`<path d="${lines.join("")}" stroke="#000" stroke-opacity=".55" stroke-width=".9"/>`);
    out.push(`<path d="M${cx - 76} 44.6H${cx + 76}" stroke="#fff" stroke-opacity=".16" stroke-width=".8"/>`);
    out.push(`<rect x="${cx - 78}" y="33" width="156" height="34" rx="7" fill="${u("rend")}"/>`);
  }

  // keys: one LCD behind 15 keys
  out.push(mod(0, 100, 400, 300));
  const KS = 64, KG = 10, X0 = 20, Y0 = 144;
  out.push(`<rect x="${X0 - 9}" y="${Y0 - 9}" width="${5 * KS + 4 * KG + 18}" height="${3 * KS + 2 * KG + 18}" rx="15" fill="#030304" stroke="#fff" stroke-opacity=".04"/>`);
  FACES.forEach((f, n) => {
    const x = X0 + (n % 5) * (KS + KG), y = Y0 + Math.floor(n / 5) * (KS + KG);
    const g = [`<g transform="translate(${x} ${y})">`];
    if (s.bezel) g.push(`<rect y="2.5" width="${KS}" height="${KS}" rx="11" fill="#000" opacity=".7"/>`);
    g.push(`<rect width="${KS}" height="${KS}" rx="10" fill="${f.bg ?? s.keyFace}"/>`);
    g.push(`<use href="#i-${f.i}" class="ki" x="21" y="11" width="22" height="22" style="color:${f.ic}"/>`);
    g.push(`<text x="32" y="51" text-anchor="middle" font-size="9.5" font-weight="500" fill="${f.lc ?? "#c9c8d3"}">${f.l}</text>`);
    g.push(`<rect width="${KS}" height="${KS}" rx="10" fill="${u("glass")}"/>`);
    g.push(`<rect x=".5" y=".5" width="${KS - 1}" height="${KS - 1}" rx="9.5" fill="none" stroke="#fff" stroke-opacity="${s.bezel ? 0.13 : 0.07}"/>`);
    if (s.bezel) g.push(`<path d="M10 1.2H54" stroke="#fff" stroke-opacity=".35" stroke-width=".8" stroke-linecap="round"/>`);
    g.push(`</g>`);
    out.push(g.join(""));
  });

  // knobs, each with its LED ring
  out.push(mod(400, 0, 200, 200));
  for (const k of knobs) {
    out.push(`<circle cx="${k.x}" cy="${k.y}" r="47" fill="${u("k" + k.n)}"/>`);
    out.push(`<circle cx="${k.x}" cy="${k.y}" r="36.5" fill="none" stroke="${k.c}" stroke-width="4" filter="${u("soft")}"/>`);
    out.push(`<circle cx="${k.x}" cy="${k.y}" r="36.5" fill="none" stroke="${k.c}" stroke-width="2.2"/>`);
    out.push(`<circle cx="${k.x}" cy="${k.y}" r="36.5" fill="none" stroke="#fff" stroke-opacity=".55" stroke-width=".6"/>`);
    out.push(`<circle cx="${k.x}" cy="${k.y}" r="33.5" fill="#060608"/>`);
    out.push(`<circle cx="${k.x}" cy="${k.y + 1.5}" r="30" fill="#000" opacity=".6"/>`);
    out.push(`<circle cx="${k.x}" cy="${k.y}" r="30" fill="${u("knob")}"/>`);
    out.push(`<circle cx="${k.x}" cy="${k.y}" r="28.6" fill="none" stroke="#000" stroke-opacity=".5" stroke-width="2.6" stroke-dasharray="1.1 1.9"/>`);
    out.push(`<circle cx="${k.x}" cy="${k.y}" r="22.5" fill="${u("cap")}" stroke="#fff" stroke-opacity=".09"/>`);
    out.push(`<path d="${arc(k.x, k.y, 19.5, 205, 262)}" fill="none" stroke="#fff" stroke-opacity=".22" stroke-width="1.6" stroke-linecap="round"/>`);
    out.push(`<line x1="${k.x}" y1="${k.y - 9}" x2="${k.x}" y2="${k.y - 19}" stroke="#fff" stroke-opacity=".9" stroke-width="2.6" stroke-linecap="round" transform="rotate(${k.a} ${k.x} ${k.y})"/>`);
  }

  // dial: turnable metal ring around a round 240x240 screen
  out.push(mod(400, 200, 200, 200));
  const cx = 500, cy = 300;
  out.push(`<circle cx="${cx}" cy="${cy + 3}" r="85" fill="#000" opacity=".55" filter="${u("soft")}"/>`);
  out.push(`<circle cx="${cx}" cy="${cy}" r="84" fill="${u("metal")}"/>`);
  out.push(`<circle cx="${cx}" cy="${cy}" r="82.4" fill="none" stroke="#000" stroke-opacity=".32" stroke-width="3.2" stroke-dasharray="1 1.5"/>`);
  out.push(`<circle cx="${cx}" cy="${cy}" r="83.5" fill="none" stroke="#fff" stroke-opacity=".22"/>`);
  if (s.brushed) out.push([71.5, 74, 76.5, 79, 81].map((r, i) => `<circle cx="${cx}" cy="${cy}" r="${r}" fill="none" stroke="${i % 2 ? "#000" : "#fff"}" stroke-opacity="${i % 2 ? 0.12 : 0.1}" stroke-width="1"/>`).join(""));
  out.push(`<circle cx="${cx}" cy="${cy}" r="69" fill="#050507" stroke="#000" stroke-opacity=".7" stroke-width="1.5"/>`);
  out.push(`<rect x="${cx - 1.5}" y="${cy - 82}" width="3" height="9" rx="1.5" fill="#fff" opacity=".75" transform="rotate(24 ${cx} ${cy})"/>`);
  out.push(`<circle cx="${cx}" cy="${cy}" r="64" fill="${u("scr")}"/>`);
  const ticks = [];
  for (let a = 135; a <= 405.1; a += 27) {
    const [x1, y1] = polar(cx, cy, 58.5, a), [x2, y2] = polar(cx, cy, 61, a);
    ticks.push(`M${x1} ${y1}L${x2} ${y2}`);
  }
  out.push(`<path d="${ticks.join("")}" stroke="#fff" stroke-opacity=".22" stroke-width="1" stroke-linecap="round"/>`);
  const end = 135 + 270 * 0.42;
  out.push(`<path d="${arc(cx, cy, 51, 135, 405)}" fill="none" stroke="#fff" stroke-opacity=".08" stroke-width="6" stroke-linecap="round"/>`);
  out.push(`<path d="${arc(cx, cy, 51, 135, end)}" fill="none" stroke="${u("arc")}" stroke-width="7" stroke-linecap="round" filter="${u("soft")}" opacity=".8"/>`);
  out.push(`<path d="${arc(cx, cy, 51, 135, end)}" fill="none" stroke="${u("arc")}" stroke-width="6" stroke-linecap="round"/>`);
  const [ex, ey] = polar(cx, cy, 51, end);
  out.push(`<circle cx="${ex}" cy="${ey}" r="4.2" fill="#fff"/>`);
  out.push(`<use href="#i-volume-2" class="ki" x="${cx - 8}" y="${cy - 34}" width="16" height="16" style="color:#c4b5fd"/>`);
  out.push(`<text x="${cx}" y="${cy + 12}" text-anchor="middle" fill="#fff" font-size="32" font-weight="600" letter-spacing="-1">42<tspan font-size="14" font-weight="500" dx="1" fill="#c9c8d3">%</tspan></text>`);
  out.push(`<text x="${cx}" y="${cy + 30}" text-anchor="middle" fill="#a3a2b1" font-size="8.5" font-weight="600" letter-spacing="1.6">VOLUME</text>`);
  out.push(`<ellipse cx="${cx - 18}" cy="${cy - 30}" rx="42" ry="22" fill="#fff" opacity=".035" transform="rotate(-28 ${cx - 18} ${cy - 30})"/>`);

  if (s.sheen) out.push(`<rect x="-20" y="-20" width="640" height="440" rx="30" fill="${u("sheen")}" pointer-events="none"/>`);
  // selected key and a hovered knob
  const sx = X0 + KS + KG, sy = Y0;
  out.push(`<rect x="${sx - 5}" y="${sy - 5}" width="${KS + 10}" height="${KS + 10}" rx="15" fill="none" stroke="${s.acc}" stroke-width="3" filter="${u("soft")}"/>`);
  out.push(`<rect x="${sx - 4.5}" y="${sy - 4.5}" width="${KS + 9}" height="${KS + 9}" rx="14.5" fill="none" stroke="${s.acc}" stroke-width="2"/>`);
  const k2 = knobs[0];
  out.push(`<circle cx="${k2.x}" cy="${k2.y}" r="43" fill="none" stroke="${s.tip[2]}" stroke-width="1.2" stroke-dasharray="3 3"/>`);
  const [tb, tf, tm, ts] = s.tip;
  out.push(`<g><rect x="206" y="44" width="194" height="36" rx="9" fill="${tb}" stroke="${ts}" filter="${u("tsh")}"/><path d="M400 56l7 6-7 6z" fill="${tb}"/><text x="219" y="66.5" font-size="11.5" fill="${tf}"><tspan font-weight="600">Knob 2</tspan><tspan dx="7" fill="${tm}">Discord volume</tspan><tspan dx="7" font-weight="600">70%</tspan></text></g>`);
  out.push(`</svg>`);
  return out.join("\n");
}
