import fs from "node:fs";
import path from "node:path";
import { deck, STYLES } from "./gen-deck.mjs";

const OUT = path.join(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Z]:)/, "$1")), process.env.ONLY ? "only.html" : "variants.html");

// ---------- icons (Lucide, ISC) ----------
const P = (...d) => d.map((x) => `<path d="${x}"/>`).join("");
const ICONS = {
  "app-window": `<rect x="2" y="4" width="20" height="16" rx="2"/>` + P("M10 4v4", "M2 8h20", "M6 4v4"),
  keyboard: P("M10 8h.01", "M12 12h.01", "M14 8h.01", "M16 12h.01", "M18 8h.01", "M6 8h.01", "M7 16h10", "M8 12h.01") + `<rect width="20" height="16" x="2" y="4" rx="2"/>`,
  play: P("M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"),
  "volume-2": P("M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z", "M16 9a5 5 0 0 1 0 6", "M19.364 18.364a9 9 0 0 0 0-12.728"),
  mic: P("M12 19v3", "M19 10v2a7 7 0 0 1-14 0v-2") + `<rect x="9" y="2" width="6" height="13" rx="3"/>`,
  "mic-off": P("M12 19v3", "M15 9.34V5a3 3 0 0 0-5.68-1.33", "M16.95 16.95A7 7 0 0 1 5 12v-2", "M18.89 13.23A7 7 0 0 0 19 12v-2", "m2 2 20 20", "M9 9v3a3 3 0 0 0 5.12 2.12"),
  music: P("M9 18V5l12-2v13") + `<circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>`,
  monitor: `<rect width="20" height="14" x="2" y="3" rx="2"/>` + P("M8 21h8", "M12 17v4"),
  sun: `<circle cx="12" cy="12" r="4"/>` + P("M12 2v2", "M12 20v2", "m4.93 4.93 1.41 1.41", "m17.66 17.66 1.41 1.41", "M2 12h2", "M20 12h2", "m6.34 17.66-1.41 1.41", "m19.07 4.93-1.41 1.41"),
  "circle-dot": `<circle cx="12" cy="12" r="1"/><circle cx="12" cy="12" r="10"/>`,
  radio: P("M16.247 7.761a6 6 0 0 1 0 8.478", "M19.075 4.933a10 10 0 0 1 0 14.134", "M4.925 19.067a10 10 0 0 1 0-14.134", "M7.753 16.239a6 6 0 0 1 0-8.478") + `<circle cx="12" cy="12" r="2"/>`,
  rocket: P("M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5", "M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09", "M9 12a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.4 22.4 0 0 1-4 2z", "M9 12H4s.55-3.03 2-4c1.62-1.08 5 .05 5 .05"),
  heart: P("M2 9.5a5.5 5.5 0 0 1 9.591-3.676.56.56 0 0 0 .818 0A5.49 5.49 0 0 1 22 9.5c0 2.29-1.5 4-3 5.5l-5.492 5.313a2 2 0 0 1-3 .019L5 15c-1.5-1.5-3-3.2-3-5.5"),
  lightbulb: P("M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5", "M9 18h6", "M10 22h4"),
  terminal: P("M12 19h8", "m4 17 6-6-6-6"),
  headphones: P("M3 14h3a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a9 9 0 0 1 18 0v7a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3"),
  folder: P("M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"),
  search: P("m21 21-4.34-4.34") + `<circle cx="11" cy="11" r="8"/>`,
  type: P("M12 4v16", "M4 7V5a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v2", "M9 20h6"),
  "arrow-up-down": P("m21 16-4 4-4-4", "M17 20V4", "m3 8 4-4 4 4", "M7 4v16"),
  "skip-forward": P("M21 4v16", "M6.029 4.285A2 2 0 0 0 3 6v12a2 2 0 0 0 3.029 1.715l9.997-5.998a2 2 0 0 0 .003-3.432z"),
  "message-square": P("M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z"),
  clock: `<circle cx="12" cy="12" r="10"/>` + P("M12 6v6l4 2"),
  globe: `<circle cx="12" cy="12" r="10"/>` + P("M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20", "M2 12h20"),
  camera: P("M13.997 4a2 2 0 0 1 1.76 1.05l.486.9A2 2 0 0 0 18.003 7H20a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2h1.997a2 2 0 0 0 1.759-1.048l.489-.904A2 2 0 0 1 10.004 4z") + `<circle cx="12" cy="13" r="3"/>`,
  check: P("M20 6 9 17l-5-5"),
  x: P("M18 6 6 18", "m6 6 12 12"),
  plus: P("M5 12h14", "M12 5v14"),
  "chevron-down": P("m6 9 6 6 6-6"),
  plug: P("M12 22v-5", "M9 8V2", "M15 8V2", "M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z"),
  crosshair: `<circle cx="12" cy="12" r="10"/>` + P("M22 12h-4", "M6 12H2", "M12 6V2", "M12 22v-4"),
  "layout-grid": `<rect width="7" height="7" x="3" y="3" rx="1"/><rect width="7" height="7" x="14" y="3" rx="1"/><rect width="7" height="7" x="14" y="14" rx="1"/><rect width="7" height="7" x="3" y="14" rx="1"/>`,
  image: `<rect width="18" height="18" x="3" y="3" rx="2"/><circle cx="9" cy="9" r="2"/>` + P("m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"),
  upload: P("M12 3v12", "m17 8-5-5-5 5", "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"),
  alert: P("m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3", "M12 9v4", "M12 17h.01"),
  grip: `<circle cx="9" cy="6" r="1"/><circle cx="9" cy="12" r="1"/><circle cx="9" cy="18" r="1"/><circle cx="15" cy="6" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="15" cy="18" r="1"/>`,
  "rotate-cw": P("M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8", "M21 3v5h-5"),
  pointer: P("M14 4.1 12 6", "m5.1 8-2.9-.8", "m6 12-1.9 2", "M7.2 2.2 8 5.1", "M9.037 9.69a.498.498 0 0 1 .653-.653l11 4.5a.5.5 0 0 1-.074.949l-4.349 1.041a1 1 0 0 0-.74.739l-1.04 4.35a.5.5 0 0 1-.95.074z"),
  layers: P("M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z", "M2 12a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 12", "M2 17a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 17"),
  info: `<circle cx="12" cy="12" r="10"/>` + P("M12 16v-4", "M12 8h.01"),
  pause: `<rect x="14" y="3" width="5" height="18" rx="1"/><rect x="5" y="3" width="5" height="18" rx="1"/>`,
};
const sprite = `<svg width="0" height="0" style="position:absolute" aria-hidden="true"><defs>${Object.entries(ICONS)
  .map(([k, v]) => `<symbol id="i-${k}" viewBox="0 0 24 24">${v}</symbol>`)
  .join("")}</defs></svg>`;
const ic = (n, cls = "") => `<svg class="i ${cls}" aria-hidden="true"><use href="#i-${n}"/></svg>`;

const logo = `<svg class="logo" viewBox="0 0 64 64" aria-hidden="true"><rect x="2" y="2" width="60" height="60" rx="14" fill="#8b7cf6"/><circle cx="25" cy="32" r="14" fill="none" stroke="#fff" stroke-width="5"/><line x1="25" y1="32" x2="25" y2="22" stroke="#fff" stroke-width="5" stroke-linecap="round"/><rect x="44" y="12" width="10" height="10" rx="2.5" fill="#fff"/><rect x="44" y="27" width="10" height="10" rx="2.5" fill="#fff"/><rect x="44" y="42" width="10" height="10" rx="2.5" fill="#fff"/></svg>`;

// ---------- content ----------
const HUE = { sys: "#7aa2ff", aud: "#2dd4bf", med: "#f472b6", obs: "#b9b8c6", spo: "#1ed760", ha: "#38bdf8", midi: "#fbbf24", hub: "#a78bfa" };
const LIB = [
  { g: "System", h: "sys", items: [["app-window", "Open app, file or website"], ["keyboard", "Hotkey", "hov"], ["type", "Type text"], ["terminal", "Run command"], ["sun", "Monitor brightness"], ["monitor", "System monitor"], ["arrow-up-down", "Scroll", "off"]] },
  { g: "Audio", h: "aud", items: [["volume-2", "Volume"], ["mic", "Microphone mute"], ["headphones", "Audio device"]] },
  { g: "Media", h: "med", n: 2 },
  { g: "OBS", h: "obs", items: [["monitor", "Scene", "act"], ["circle-dot", "Record"], ["radio", "Stream"], ["rocket", "Save replay"], ["mic", "Mute source"], ["volume-2", "Source volume", "off"], ["layers", "Show / hide source"]] },
  { g: "Spotify", h: "spo", n: 3 },
  { g: "Home Assistant", h: "ha", n: 4 },
  { g: "MIDI", h: "midi", n: 2 },
  { g: "HubDeck", h: "hub", n: 6 },
];
const GRID_ICONS = ["monitor", "mic", "mic-off", "volume-2", "play", "music", "radio", "circle-dot", "rocket", "heart", "globe", "lightbulb", "folder", "clock", "message-square", "camera", "sun", "terminal"];

function library() {
  const groups = LIB.map((grp) => {
    const n = grp.items ? grp.items.length : grp.n;
    const head = `<div class="gh${grp.items ? "" : " closed"}">${ic("chevron-down")}<span>${grp.g}</span><span class="n">${n}</span></div>`;
    if (!grp.items) return head;
    const items = grp.items
      .map(([i, name, st = ""]) => {
        const tail =
          st === "act" ? `<span class="tail on">${ic("check")}Key 2</span>` : st === "off" ? `<span class="tail">turn only</span>` : st === "hov" ? `<span class="tail grip">${ic("grip")}</span>` : "";
        return `<div class="it ${st}"><span class="ic" style="--h:${HUE[grp.h]}">${ic(i)}</span><span class="nm">${name}</span>${tail}</div>`;
      })
      .join("");
    return head + items;
  }).join("");
  return `<aside class="lib">
  <div class="lib-head">
    <div class="lib-title">Actions<span>38</span></div>
    <div class="inp search">${ic("search")}<span class="ph">Search actions</span><span class="kbd">Ctrl K</span></div>
    <p class="help">Drag an action onto a control, or click one to give it to Key 2.</p>
  </div>
  <div class="groups">${groups}</div>
  <div class="lib-foot">${ic("plug")}More actions come with plugins.</div>
</aside>`;
}

function topbar(v) {
  return `<header class="top">
  <div class="brand">${logo}<span>HubDeck</span></div>
  <nav class="profiles" aria-label="Profiles">
    <button class="ptab">Everyday</button>
    <button class="ptab on"><span class="auto" title="Switches here for obs64.exe"></span>Streaming</button>
    <button class="ptab">Photo</button>
    <button class="ptab">Gaming</button>
  </nav>
  <button class="btn ico gho add" aria-label="New profile">${ic("plus")}</button>
  <div class="spacer"></div>
  <div class="fg">${ic("app-window")}<span>In front</span><b>obs64.exe</b></div>
  <div class="sep"></div>
  <div class="tools">
    <button class="btn gho">${ic("plug")}<span>Plugins</span></button>
    <button class="btn gho">${ic("crosshair")}<span>Calibrate keys</span></button>
    <button class="btn gho${v === "b" ? " hov" : ""}">${ic("layout-grid")}<span>Arrange modules</span>${v === "b" ? `<em class="tip">Arrange modules</em>` : ""}</button>
  </div>
  <div class="sep"></div>
  <div class="status"><span class="dot"></span>4 modules</div>
  <button class="btn gho lang">${ic("globe")}EN</button>
  <div class="wc" aria-hidden="true"><span><svg viewBox="0 0 10 10"><path d="M0 5h10"/></svg></span><span><svg viewBox="0 0 10 10"><rect x=".5" y=".5" width="9" height="9"/></svg></span><span><svg viewBox="0 0 10 10"><path d="M0 0l10 10M10 0 0 10"/></svg></span></div>
</header>`;
}

function stage(v) {
  const live =
    v === "b"
      ? `<div class="live"><span><i style="--c:#a78bfa"></i>Edge<b>Violet breathe</b></span><span><i style="--c:#c084fc"></i>Dial<b>Volume 42%</b></span><span><i style="--c:#22d3ee"></i>Knob 2<b>Discord 70%</b></span><span><i style="--c:#ff9f43"></i>Knob 1<b>Game 55%</b></span></div>`
      : "";
  return `<section class="stage">
  <div class="sbar">
    <span class="sbl">Key pages</span>
    <div class="seg pages"><button class="on">Main</button><button>Scenes</button><button>Audio</button></div>
    <button class="btn ico gho" aria-label="New page">${ic("plus")}</button>
    <div class="spacer"></div>
    <span class="chip">${ic("rotate-cw")}Lying flat · 270°</span>
  </div>
  <div class="deckwrap">${deck(v, STYLES[v])}</div>
  ${live}
  <div class="hintbar">${ic("pointer")}<span>Press, turn or slide anything on the deck to select it.</span><span class="keys"><span class="kbd">Ctrl C</span><span class="kbd">Ctrl V</span> copy <span class="kbd">Del</span> clear <span class="kbd">Ctrl Z</span> undo</span></div>
</section>`;
}

function keyPreview(cls = "") {
  return `<div class="kprev ${cls}">${ic("monitor")}<span>Gameplay</span></div>`;
}

function inspector(v) {
  const icons = GRID_ICONS.map((n, i) => `<span class="${i === 0 ? "on" : ""}">${ic(n)}</span>`).join("");
  return `<aside class="insp">
  <div class="ih">
    <div class="ih-text">
      <div class="crumb">${ic("layout-grid")}15-key module<span class="dotsep"></span>Main page</div>
      <h2>Key 2</h2>
    </div>
    ${keyPreview()}
  </div>
  <div class="isec">
    <div class="seg gest"><button class="on">Press</button><button>Hold<span class="set" title="set"></span></button><button>Double press</button></div>
    <div class="fld">
      <label class="lbl">Action</label>
      <div class="sel act"><span class="ic" style="--h:${HUE.obs}">${ic("monitor")}</span><span>Scene</span><span class="tag">OBS</span>${ic("chevron-down", "chev")}</div>
      <p class="help">Switches OBS to a scene. The key turns green while that scene is live.</p>
    </div>
    <div class="fld">
      <label class="lbl">Scene</label>
      <div class="sel"><span>Gameplay</span><span class="state"><i></i>Live now</span>${ic("chevron-down", "chev")}</div>
    </div>
  </div>
  <div class="isec look">
    <div class="st">Look</div>
    <div class="fld">
      <label class="lbl">Label</label>
      <div class="inp focus"><span>Gameplay</span><span class="caret"></span><span class="count">8 / 14</span></div>
    </div>
    <div class="row">
      <div class="fld grow"><label class="lbl">Background</label><div class="inp clr"><span class="sw" style="background:#7c5cff"></span><span class="mono">#7C5CFF</span></div></div>
      <div class="fld grow"><label class="lbl">Text</label><div class="inp clr"><span class="sw" style="background:#ffffff"></span><span class="mono">#FFFFFF</span></div></div>
    </div>
    <div class="fld">
      <label class="lbl">Picture</label>
      <div class="pic"><button class="btn sec">${ic("upload")}Choose picture</button><span class="help">PNG, JPG, GIF, WebP or SVG</span></div>
    </div>
    <div class="fld">
      <label class="lbl">Icon</label>
      <div class="igrid">${icons}</div>
    </div>
    <label class="trow"><span><b>Own look while on</b><span class="help">Without it the key turns green while live.</span></span><span class="tg"></span></label>
  </div>
  <div class="isec dev">
    <dl class="facts"><dt>Serial</dt><dd class="mono">K15-2310-00A7</dd><dt>Firmware</dt><dd class="mono">1.0.7</dd></dl>
  </div>
</aside>`;
}

function windowMock(v) {
  return `<div class="frame"><div class="win v${v}">${topbar(v)}<main class="ws">${library()}${stage(v)}${inspector(v)}</main></div></div>`;
}

function kit(v) {
  return `<div class="kit v${v}">
  <div class="cc"><h4>Buttons</h4><div class="wrap">
    <button class="btn pri">${ic("check")}Save</button><button class="btn sec">Duplicate</button><button class="btn gho hov">Cancel</button><button class="btn dan">Delete</button><button class="btn sec ico" aria-label="Add">${ic("plus")}</button><button class="btn pri dis">Restore</button>
  </div></div>
  <div class="cc"><h4>Select, open</h4>
    <div class="sel open"><span class="ic" style="--h:${HUE.aud}">${ic("mic")}</span><span>Microphone mute</span><span class="tag">Audio</span>${ic("chevron-down", "chev")}</div>
    <div class="menu"><div class="mg">Audio</div><div class="mi">${ic("volume-2")}Volume</div><div class="mi on">${ic("mic")}Microphone mute${ic("check", "ok")}</div><div class="mi">${ic("headphones")}Audio device</div><div class="mg">Media</div><div class="mi">${ic("play")}Media control</div></div>
  </div>
  <div class="cc"><h4>Inputs</h4>
    <div class="fld"><label class="lbl">Profile name</label><div class="inp focus"><span>Streaming</span><span class="caret"></span></div></div>
    <div class="fld"><label class="lbl">Keys</label><div class="inp"><span class="ph">e.g. Ctrl+Shift+F13</span><span class="kbd rec">Record</span></div></div>
  </div>
  <div class="cc"><h4>Tabs</h4>
    <div class="seg gest"><button class="on">Press</button><button>Hold<span class="set"></span></button><button>Double press</button></div>
    <div class="seg pages"><button>Main</button><button class="on">Scenes</button><button>Audio</button></div>
  </div>
  <div class="cc"><h4>Toggles</h4>
    <label class="trow"><span><b>Turn screens off while locked</b></span><span class="tg on"></span></label>
    <label class="trow"><span><b>Speed up when turned fast</b></span><span class="tg"></span></label>
    <label class="crow"><span class="cb on">${ic("check")}</span>Switch profiles with the app in front</label>
  </div>
  <div class="cc"><h4>Banner</h4>
    <div class="ban">${ic("alert")}<div><b>Dial module is not responding.</b><span>Restart it, or pull it off the base and put it back.</span><button class="btn sec sm">Restart module</button></div></div>
    <div class="ban info">${ic("pause")}<div><span>The deck is paused. Keys and dial show their own animation.</span><button class="btn gho sm">Resume</button></div></div>
  </div>
  <div class="cc"><h4>Empty state</h4>
    <div class="empty"><span class="eic">${ic("pointer")}</span><b>Nothing selected</b><span>Click a control, or press, turn or slide it on the deck, to set what it does.</span></div>
  </div>
  <div class="cc"><h4>Status, tooltip, toast</h4>
    <div class="chips"><span class="pill"><i></i>4 modules</span><span class="pill acc">Hold set</span><span class="pill warn"><i style="--c:var(--warn)"></i>Not installed</span><span class="pill"><i style="--c:#22d3ee"></i>Knob 2</span></div>
    <div class="ttip"><b>Knob 2</b><span>Discord volume</span><b>70%</b></div>
    <div class="chips"><span class="kbd">Ctrl</span><span class="kbd">Shift</span><span class="kbd">F13</span></div>
    <div class="toasts"><div class="toast">${ic("check")}Copied</div><div class="toast">Cleared<span class="kbd">Ctrl Z</span>brings it back</div></div>
  </div>
</div>`;
}

// ---------- CSS ----------
const css = fs.readFileSync(new URL("./variants.css", import.meta.url), "utf8");

const VARIANTS = [
  {
    v: "a",
    name: "Pro studio",
    sub: "Grafit: ciemne, warstwowe, precyzyjne",
    notes: [
      ["Idea", "Narzędzie klasy Figma / Logic: trzy poziomy ciemnej szarości z lekkim fioletowym odcieniem, panele oddzielone linią 1 px, akcent tylko tam, gdzie jest stan (zaznaczenie, fokus, aktywna karta). Deck półrealistyczny, czytelny jak schemat, z umiarkowaną poświatą krawędzi."],
      ["Tokeny", "<code>--bg0 #0d0d11</code> scena · <code>--bg1 #131318</code> panele · <code>--bg2 #1a1a21</code> pola · <code>--bg3 #23232c</code> hover/aktywne · <code>--ln rgba(255,255,255,.065)</code> · <code>--tx #ededf2 / --tx2 #a3a2b1 / --tx3 #6c6b7b</code> · <code>--acc #8b7cf6</code> · <code>--ok #3ecf8e</code> · <code>--warn #f5a524</code> · <code>--err #f0506e</code>"],
      ["Font", "IBM Plex Sans 400/500/600 + IBM Plex Mono 400/500. Aplikacja już je ma (@fontsource), nic nowego do dołączenia."],
      ["Dlaczego profesjonalnie", "Jedna wysokość kontrolek (32 px), stała skala pisma 11/12/13/18, kolor niesie tylko informację (kolor grupy akcji, stan), siatka kropek na scenie jak w edytorach graficznych. Nic nie krzyczy, wszystko jest wyrównane."],
    ],
  },
  {
    v: "b",
    name: "Hardware-forward",
    sub: "Showroom: deck jako bohater, ciche panele",
    notes: [
      ["Idea", "Deck jest zdjęciem produktu: szklane klawisze z grubością, szczotkowany metalowy pierścień dialu, radełkowane gałki, wałki z cieniowaniem walca, krawędź świecąca na biurko. Panele unoszą się nad sceną jak szkło (blur), narzędzia u góry tylko jako ikony z podpowiedzią. Pod deckiem pasek „na żywo” z kolorami LED."],
      ["Tokeny", "<code>--bg0</code> gradient sceny <code>#1d1830 → #070709</code> · <code>--bg1 rgba(18,17,25,.62)</code> + <code>backdrop-filter: blur(18px)</code> · <code>--ln rgba(255,255,255,.07)</code> · <code>--tx #f3f2f8 / --tx2 #aaa7bb / --tx3 #6f6c82</code> · <code>--acc #9d8cff</code> · <code>--acc2 #c084fc</code> · promień paneli 18 px"],
      ["Font", "Barlow 400/500/600 (interfejs, techniczny charakter jak napisy na sprzęcie) + Barlow Semi Condensed 600 (nagłówki sekcji, wersaliki) + JetBrains Mono 500 (wartości). Do dołączenia: @fontsource/barlow, @fontsource/barlow-semi-condensed, @fontsource/jetbrains-mono."],
      ["Dlaczego profesjonalnie", "Tak prezentują się aplikacje producentów sprzętu z wyższej półki: produkt w świetle, interfejs schodzi na drugi plan. Najmocniejszy efekt „wow”, ale najwięcej pracy i najcięższy rysunek (filtry, blur)."],
    ],
  },
  {
    v: "c",
    name: "Daylight",
    sub: "Jasny, przestronny, miękkie powierzchnie",
    notes: [
      ["Idea", "W duchu Apple / Linear: jasne tło, białe panele bez ciężkich ramek, oddech między sekcjami, profile jako przełącznik segmentowy. Ciemny deck leży na jasnym biurku z miękkim cieniem, a fioletowa krawędź rzuca lekką liliową poświatę. Najlepiej czyta się za dnia; w trybie ciemnym zostałby wariant A."],
      ["Tokeny", "<code>--bg0 #f3f3f6</code> scena · <code>--bg1 #ffffff</code> panele · <code>--bg2 #f7f7f9</code> pola · <code>--bg3 #efeff3</code> hover · <code>--ln #e7e7ee</code> · <code>--tx #17171c / --tx2 #5c5c6b / --tx3 #9696a6</code> · <code>--acc #6e56ff</code> · <code>--accbg #f0edff</code> · cienie <code>0 1px 2px rgba(20,20,40,.06)</code>"],
      ["Font", "Instrument Sans 400/500/600 (nowoczesny grotesk, węższy i cieplejszy od systemowego) + IBM Plex Mono 400/500. Do dołączenia: @fontsource/instrument-sans."],
      ["Dlaczego profesjonalnie", "Dużo światła i równych odstępów (siatka 4/8 px), hierarchia przez wagę i szarości zamiast ramek, jeden akcent. Wygląda jak natywna aplikacja systemowa; dobrze wypada na zrzutach w README."],
    ],
  },
];

const sizes = {};
for (const x of VARIANTS) sizes[x.v] = Buffer.byteLength(deck(x.v, STYLES[x.v]));

const page = `<title>HubDeck Look Variants</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Barlow:wght@400;500;600;700&family=Barlow+Semi+Condensed:wght@500;600&family=IBM+Plex+Mono:wght@400;500&family=IBM+Plex+Sans:wght@400;500;600&family=Instrument+Sans:wght@400;500;600&family=JetBrains+Mono:wght@500&display=swap">
<style>
${css}
</style>
${sprite}
<header class="pg-head">
  <div class="pg-title">${logo}<div><h1>HubDeck: trzy kierunki wyglądu</h1><p>Główne okno 1440 × 900 z tym samym układem „studio” (biblioteka · deck · inspektor) i trzema stylami. Pod każdym oknem zestaw komponentów i krótki opis. Wybierz jeden; można też łączyć (np. deck z B w oknie A).</p></div></div>
  <nav class="pg-nav" aria-label="Warianty">
    ${VARIANTS.map((x) => `<a href="#${x.v}"><b>${x.v.toUpperCase()}</b>${x.name}</a>`).join("")}
    <button id="zoom" class="pg-zoom" type="button" aria-pressed="false">Rozmiar 100%</button>
  </nav>
</header>
<main class="pg-main">
${VARIANTS.filter((x) => !process.env.ONLY || x.v === process.env.ONLY).map(
  (x) => `<section class="pg-var" id="${x.v}">
  <div class="pg-vhead"><span class="pg-letter">${x.v.toUpperCase()}</span><div><h2>${x.name}</h2><p>${x.sub}</p></div></div>
  ${windowMock(x.v)}
  <h3 class="pg-sub">Komponenty</h3>
  ${kit(x.v)}
  <dl class="pg-notes">${x.notes.map(([k, t]) => `<div><dt>${k}</dt><dd>${t}</dd></div>`).join("")}<div><dt>Rysunek decku</dt><dd>Inline SVG, ${(sizes[x.v] / 1024).toFixed(1)} KB (ikony klawiszy z jednego wspólnego sprite'a Lucide).</dd></div></dl>
</section>`
).join("\n")}
</main>
<script>
(function () {
  var root = document.documentElement;
  root.classList.add("js");
  function fit() {
    var actual = root.classList.contains("actual");
    document.querySelectorAll(".frame").forEach(function (f) {
      var w = f.querySelector(".win");
      if (actual) { w.style.transform = ""; f.style.height = ""; return; }
      var s = Math.min(1, f.clientWidth / 1440);
      w.style.transform = "scale(" + s + ")";
      f.style.height = Math.round(900 * s) + "px";
    });
  }
  var b = document.getElementById("zoom");
  b.addEventListener("click", function () {
    var on = root.classList.toggle("actual");
    b.setAttribute("aria-pressed", on ? "true" : "false");
    b.textContent = on ? "Dopasuj do ekranu" : "Rozmiar 100%";
    fit();
  });
  if (window.ResizeObserver) new ResizeObserver(fit).observe(document.querySelector(".pg-main"));
  window.addEventListener("resize", fit);
  fit();
})();
</script>
`;

fs.writeFileSync(OUT, page);
console.log(OUT, Buffer.byteLength(page), sizes);
