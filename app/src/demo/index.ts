// Browser demo: `pnpm dev`, then open http://localhost:1420/?demo (or ?demo=welcome for the
// first-run screen). The engine is mocked with the real action catalog, icons, presets and key
// pictures from `fixture.json` (written by `cargo test -p hubdeck dump_demo_fixture -- --ignored`)
// and the owner's modules, so the UI can be designed and screenshotted without hardware.
// `&touch=keys.1` (or dial, knobs.0...) presses that control once after start, which selects it.
// `&lang=pl`: the window in Polish.
import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { DeckSnapshot } from "../deck";
import type { Binding, EngineState, Profile } from "../engine";
import fixture from "./fixture.json";
import { setLanguage } from "../i18n";
import { demoParam } from "./params";

// `&banner=paused,dial,error`: the deck paused, a module not responding, an action error
const banners = (demoParam("banner") ?? "").split(",");

/** A sample gallery made of fixture profiles (the real one: docs/SHARING.md). */
const GALLERY_BASE = "https://raw.githubusercontent.com/Przemotar/hubdeck-profiles/main";
const GALLERY = [
  { id: "demo-obs", name: "OBS streaming", author: "Przemotar", description: "Scenes, record, stream and replay on the keys with live state; mic level on the dial; the OBS mixer on the faders.", kinds: ["keys", "dial", "faders"], tags: ["streaming", "obs"], preview: null, package: `${GALLERY_BASE}/obs-streaming/obs-streaming.hubdeck`, version: "1.0" },
  { id: fixture.profiles[0].id, name: fixture.profiles[0].name, author: "Przemotar", description: "Media, copy and paste, app volumes on the faders, scrolling on the rollers.", kinds: ["keys", "faders", "rollers"], tags: ["everyday"], preview: null, package: `${GALLERY_BASE}/everyday/everyday.hubdeck`, version: "1.0" },
  // by id, not by place in the fixture: the description must belong to the profile it names
  { id: "demo-photoshop", name: fixture.profiles.find((p) => p.id === "demo-photoshop")?.name ?? "Photoshop", author: "a HubDeck user", description: "Tools on the keys, brush size on the dial, zoom and history on the knobs.", kinds: ["keys", "dial", "knobs"], tags: ["design", "adobe"], preview: null, package: `${GALLERY_BASE}/photoshop/photoshop.hubdeck`, version: "0.9" },
];

const UPDATE = {
  version: "0.2.0",
  notes: "New look: Pro studio window and the deck drawn like the real kit.\nMy modules: set up modules you own before attaching them.\nStreamlabs, Twitch and a soundboard key.",
  date: "2026-10-20 10:00:00 +00:00",
};

const profiles = new Map<string, Profile>((fixture.profiles as unknown as Profile[]).map((p) => [p.id, p]));

// the tour in the browser demo: the parts of a deck for the demo's kit (screenshots, flows)
const tour = { draft: "demo-everyday", focus: "games", kit: "dialKnobs", mac: false, call: "discord", music: "Spotify", games: 12, rollers: true, stops: [] as { id: string; keep: boolean; look: number; looks: number; controls: string[]; tryControl: string | null; tasks?: string[]; faces?: string[] }[] };
function tourView(focus: string) {
  const faders = demoParam("kit") === "faders";
  tour.focus = focus;
  tour.kit = faders ? "faders" : "dialKnobs";
  const part = (id: string, looks: number, tryControl: string | null) => ({ id, keep: true, look: 0, looks, controls: tryControl ? [tryControl] : [], tryControl });
  tour.stops = [
    part("volume", 2, faders ? "faders.4" : "knobs.2"),
    part(focus === "games" ? "games" : focus === "streaming" ? "streaming" : focus === "calls" ? "work" : focus === "music" ? "player" : "extras", 1, "keys.14"),
    part("mute", 1, "keys.5"),
    part("music", faders ? 1 : 2, faders ? "keys.8" : "dial"),
    part("apps", 1, "keys.0"),
    { ...part("gestures", 1, "keys.5"), tasks: ["keys.5:hold", faders ? "keys.6:double" : "knobs.2:double", ...(faders ? [] : ["dial:hold"])] },
    ...(faders ? [] : [{ ...part("dial", 8, "dial"), faces: ["nowPlaying", "pet", "clock", "pomodoro", "weather", "timer", "cpu", "volume"] }]),
    part("superAnimations", 1, "keys.9"),
    part("superSaver", 1, null),
    part("superRainbow", 1, null),
  ];
  return tour;
}

// `&kit=faders`: the kit with faders instead of the dial and the knobs (screenshots of both kits)
const FADERS_KIT = demoParam("kit") === "faders";

const deck: DeckSnapshot = {
  modules: [
    { id: "keys:DEMO-KEYS", kind: "keys", serial: "MHKD15DEMO0001", firmware: "02.00.01.00", slot: "2-3" },
    { id: "rollers:DEMO-ROLLERS", kind: "rollers", serial: "MHRR02DEMO0001", firmware: "01.00.03.01", slot: "3-3" },
    ...(FADERS_KIT
      ? [{ id: "faders:DEMO-FADERS", kind: "faders" as const, serial: "MHFD05DEMO0001", firmware: "01.00.05.00", slot: "1-1-3" }]
      : [
          { id: "dial:DEMO-DIAL", kind: "dial" as const, serial: "MHDD01DEMO0001", firmware: "01.00.04.00", slot: "1-3-4" },
          { id: "knobs:DEMO-KNOBS", kind: "knobs" as const, serial: "MHKB03DEMO0001", firmware: "01.00.05.00", slot: "1-4-3" },
        ]),
  ],
  orientation: "horizontal",
  turn: 270,
  turnOverride: null,
  unresponsive: banners.includes("dial") ? ["dial"] : [],
  paused: banners.includes("paused"),
};

// the owner's arrangement (2026-10-11): base turned 270 degrees on the desk
const placements: Record<string, { col: number; row: number; rotation: number }> = {
  "dial@1-3-4": { col: 0, row: 4, rotation: 90 },
  "keys@2-3": { col: 0, row: 0, rotation: 90 },
  "knobs@1-4-3": { col: 2, row: 4, rotation: 270 },
  "rollers@3-3": { col: 3, row: 0, rotation: 270 },
  // seen on the base before, detached now: on the shelf of owned modules
  "faders@1-1-3": { col: 0, row: 4, rotation: 0 },
};

let active = demoParam("profile") ?? "demo-obs";
let page: string | null = null;
let beforeTour: string | null = null;
let onboarded = new URLSearchParams(location.search).get("demo") !== "welcome" && demoParam("panel") !== "welcome";

// "Ready for your apps": profiles HubDeck made for installed apps, switching in only once switched on
const ready = new Map<string, boolean>();
for (const [id, name, exe] of [["demo-ready-firefox", "Firefox", "firefox.exe"], ["demo-ready-steam", "Steam", "steam.exe"], ["demo-ready-whatsapp", "WhatsApp", "WhatsApp.exe"]]) {
  profiles.set(id, { ...(fixture.profiles[0] as unknown as Profile), id, name, apps: [exe], inBar: false });
  ready.set(id, false);
}

const state = (): EngineState => ({
  profiles: [...profiles.values()].map((p) => ({ id: p.id, name: p.name, apps: p.apps, inBar: p.inBar ?? p.apps.length === 0, ready: ready.has(p.id), switchIn: ready.get(p.id) ?? true })),
  active,
  fallback: fixture.profiles[0].id,
  home: fixture.profiles[0].id,
  autoApp: active === "demo-obs" ? "obs64.exe" : null,
  pinned: false,
  autoSwitch: true,
  foreground: "obs64.exe",
  page,
  onboarded,
  playing: false,
});

/** A plain key picture for looks the fixture does not have (edited in the demo). */
function fallbackSvg(screen: string, look: { label?: string; bg?: string; fg?: string }) {
  const size = screen === "dial" ? 240 : 96;
  const label = (look.label ?? "").replace(/[<&>]/g, "");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}"><rect width="100%" height="100%" fill="${look.bg ?? "#26232e"}"/><text x="50%" y="55%" text-anchor="middle" font-family="IBM Plex Sans" font-size="${size / 6}" fill="${look.fg ?? "#fff"}">${label}</text></svg>`;
}

/** JSON with the keys in order, as the fixture writes them (the look's keys come in any order). */
const sortedJson = (v: unknown) => JSON.stringify(v, (_k, x) => (x && typeof x === "object" && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => (a < b ? -1 : 1))) : x));

const changed = () => void emit("engine://state", state());

const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
  engine_state: () => state(),
  deck_snapshot: () => deck,
  placements_get: () => placements,
  placement_set: ({ key, placement }) => {
    if (placement) placements[key as string] = placement as (typeof placements)[string];
    else delete placements[key as string];
  },
  actions_list: () => fixture.catalog,
  icons_list: () => fixture.icons,
  // the real catalog, test presets included
  presets_list: () => fixture.presets,
  macro_record_start: () => null,
  macro_record_stop: () => [
    { action: "core.hotkey", params: { keys: "Ctrl+C", mode: "normal", hold: 30 }, delay: 0 },
    { action: "core.hotkey", params: { keys: "Alt+Tab", mode: "normal", hold: 30 }, delay: 420 },
    { action: "core.hotkey", params: { keys: "Ctrl+V", mode: "normal", hold: 30 }, delay: 310 },
  ],
  profile_get: ({ id }) => profiles.get(id as string),
  profile_activate: ({ id }) => {
    active = id as string;
    changed();
  },
  profile_delete: ({ id }) => {
    profiles.delete(id as string);
    changed();
  },
  profile_create: ({ name, from }) => {
    const base = from ? profiles.get(from as string) : undefined;
    const p = { ...(base ? structuredClone(base) : { version: 1, apps: [], controls: {} }), id: `demo-${Date.now()}`, name: name as string } as Profile;
    profiles.set(p.id, p);
    changed();
    return p;
  },
  profile_save: ({ profile }) => {
    profiles.set((profile as Profile).id, profile as Profile);
    changed();
  },
  binding_set: ({ profile, control, binding }) => {
    const p = profiles.get(profile as string);
    if (!p) return;
    if (binding) p.controls[control as string] = binding as Binding;
    else delete p.controls[control as string];
    changed();
  },
  preview_svg: ({ screen, look }) => (fixture.previews as Record<string, string>)[`${screen}|${sortedJson(look)}`] ?? fallbackSvg(screen as string, look as never),
  screens_get: () => ({ brightness: 100, blankAfterMinutes: 0, blankWhenLocked: true }),
  integrations_get: () => ({ obsUrl: "ws://127.0.0.1:4455", obsPasswordSet: true, haUrl: "http://homeassistant.local:8123", haTokenSet: true, streamlabsUrl: "ws://127.0.0.1:59650", streamlabsTokenSet: false }),
  plugins_list: () => [
    { id: "com.example.weather", name: "Weather", version: "1.2.0", actions: 3, status: "running", connected: true, restarts: 0, permissions: ["network"], cpu: 0.4, memoryMb: 18 },
    { id: "com.example.pomodoro", name: "Pomodoro", version: "0.9.1", actions: 2, status: "exited (code 1)", connected: false, restarts: 2, permissions: [], cpu: null, memoryMb: null },
  ],
  backups_list: () => [
    { id: "b3", when: "2026-10-11 09:12", reason: "daily", profiles: 7 },
    { id: "b2", when: "2026-10-10 22:40", reason: "before import", profiles: 6 },
    { id: "b1", when: "2026-10-10 09:05", reason: "daily", profiles: 6 },
  ],
  calibration_start: () => ({ geometry: { x0: 0, y0: 0, dx: 100, dy: 99, w: 79, h: 75 }, selected: 1 }),
  calibration_set: () => null,
  calibration_stop: () => null,
  twitch_status: () => [false, false],
  "plugin:app|version": () => "0.1.0",
  "plugin:opener|open_url": () => null,
  update_settings_get: () => [true, true],
  update_settings_set: () => null,
  lights_get: () => ({
    style: "rainbow",
    color: "#8b7cf6",
    brightness: 100,
    react: { mic: { on: true, color: "#ff2020" }, live: { on: true, color: "#ff2020" }, agents: { on: true, color: "#ff8a00" }, timer: { on: true, color: "#ffffff" }, call: { on: true, color: "#ff2020" }, levels: true },
  }),
  lights_set: () => null,
  profile_check: () =>
    demoParam("check")
      ? [
          { control: "keys.0", message: String.raw`Program not found: C:\Tools\scene-helper.exe` },
          { control: "dial:hold", message: "Sound file not found: alert.wav" },
          { control: "knobs.1", message: "Audio device not found: USB Headset" },
        ]
      : [],
  installed_list: () => ({
    apps: ["Blender", "Visual Studio Code", "Google Chrome", "Discord", "Spotify", "Notion", "Figma", "Slack", "VLC media player"].map((name) => ({ name, target: name, exe: `${name.split(" ")[0]}.exe`, source: "startMenu" })),
    games: [],
  }),
  installed_picture: () => "",
  profile_pin: () => null,
  update_check: () => UPDATE,
  update_install: () => null,
  diagnostics_save: () => null,
  engine_redraw: () => null,
  apps_list: () => [
    { exe: "obs64.exe", path: "", title: "OBS 31.0.2 - Profile: Untitled", lastSeen: Date.now(), running: true, icon: "" },
    { exe: "Discord.exe", path: "", title: "Discord", lastSeen: Date.now() - 60_000, running: true, icon: "" },
    { exe: "Photoshop.exe", path: "", title: "Adobe Photoshop", lastSeen: Date.now() - 3_600_000, running: false, icon: "" },
  ],
  profile_for_app: ({ exe, name }) => {
    const base = profiles.get("demo-everyday")!;
    const p = { ...structuredClone(base), id: `demo-app-${Date.now()}`, name: name as string, apps: [exe as string] } as Profile;
    profiles.set(p.id, p);
    changed();
    return p;
  },
  claude_status: () => false,
  // `&leds=hub`: HubDeck's own rainbow on the knobs and the edge, as on the owner's deck
  leds_get: () =>
    demoParam("leds") === "hub"
      ? {
          "knobs:DEMO-KNOBS": [0, 1, 2].map(() => ({ color: "", effect: "hubRainbow" })),
        }
      : {},
  language_set: () => null,
  dial_gallery: () => (fixture as unknown as { dialGallery?: unknown[] }).dialGallery ?? [],
  auto_seen: () => null,
  auto_set: () => null,
  profile_fill: () => 0,
  suggestions: ({ control }) =>
    String(control).startsWith("keys")
      ? [
          { action: "core.hotkey", params: { keys: "Ctrl+T" }, look: { label: "New tab", icon: "square", bg: "#1f2a33" } },
          { action: "core.hotkey", params: { keys: "Ctrl+Shift+T" }, look: { label: "Reopen tab", icon: "refresh-cw", bg: "#1f2a33" } },
          { action: "core.hotkey", params: { keys: "Ctrl+F" }, look: { label: "Find", icon: "search", bg: "#1f2a33" } },
          { action: "core.media", params: { command: "playPause" }, look: { label: "Play", icon: "play" } },
          { action: "core.mic", params: { mode: "toggle" }, look: { label: "Mic", icon: "mic" } },
          { action: "core.hotkey", params: { keys: "Win+Shift+S" }, look: { label: "Snip", icon: "camera" } },
          { action: "core.clock", params: {}, look: { label: "Clock", icon: "clock" } },
          { action: "core.hotkey", params: { keys: "Win+L" }, look: { label: "Lock", icon: "lock" } },
        ]
      : [
          { action: "core.volume", params: { target: "system" }, look: { label: "System", icon: "volume-2" } },
          { action: "core.volume", params: { target: "app", app: "Discord.exe" }, look: { label: "Discord", icon: "headphones" } },
          { action: "core.volume", params: { target: "mic" }, look: { label: "Mic level", icon: "mic" } },
          { action: "core.brightness", params: { monitor: "all" }, look: { label: "Brightness", icon: "sun" } },
        ],
  mcp_info: () => ({
    exe: "C:\\Program Files\\HubDeck\\hubdeck.exe",
    desktop: false,
    codeCli: true,
    codeCommand: 'claude mcp add --scope user hubdeck -- "C:\\Program Files\\HubDeck\\hubdeck.exe" --mcp',
    snippet: '{ "mcpServers": { "hubdeck": { "command": "C:\\\\Program Files\\\\HubDeck\\\\hubdeck.exe", "args": ["--mcp"] } } }',
  }),
  profile_adapt: () => [{ label: "Volume", from: "dial", to: ["knobs.0"] }],
  // a sample pack only when asked for (`&icons=pack:neon-shapes`), so other screenshots show no fake pack
  live_looks: () => ({ profile: "", looks: {} }),
  builder_options: (args) =>
    args.kind === "dial"
      ? [
          { id: "nowPlayingSeek", group: "Music", name: "What's playing", does: "Turn: move in the song · press: play or pause", source: "Built in, no setup", setup: false, binding: { action: "core.nowPlaying", params: { turn: "seek" }, look: { icon: "music" } }, svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 240"><rect width="240" height="240" fill="#2a1a3a"/><text x="120" y="130" fill="#fff" font-size="28" text-anchor="middle" font-family="sans-serif">Song</text></svg>' },
          { id: "volume", group: "One dial, many jobs", name: "Volume, big", does: "Turn: system volume · press: mute", source: "Built in, no setup", setup: false, binding: { action: "core.volume", params: { target: "system" }, look: { icon: "volume-2" } }, svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 240"><rect width="240" height="240" fill="#1d1b2b"/><text x="120" y="135" fill="#fff" font-size="44" text-anchor="middle" font-family="sans-serif">45%</text></svg>' },
          { id: "spotifySong", group: "Music", name: "Song with its cover", does: "Turn: move in the song · hold and turn: songs · press: play or pause", source: "Essentials for Spotify", setup: true, binding: { action: "x/y", params: {}, look: {} }, svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 240"><rect width="240" height="240" fill="#7a2b8f"/></svg>' },
        ]
      : [
          { id: "volume", group: "One dial, many jobs", name: "System volume", does: "Turn: volume · press: mute", source: "Built in, no setup", setup: false, binding: { action: "core.volume", params: {}, look: { icon: "volume-2" } }, svg: null },
          { id: "tracks", group: "Music", name: "Songs", does: "Turn: next or previous song · press: play or pause", source: "Built in, no setup", setup: false, binding: { action: "core.nowPlaying", params: { turn: "tracks" }, look: { icon: "music" } }, svg: null },
        ],
  // the deck assistant: `&assistant=none` (no Claude Code) or `signin` (not signed in)
  assistant_info: () => {
    const how = demoParam("assistant");
    return {
      cli: how === "none" ? null : "/usr/local/bin/claude",
      version: "2.1.0",
      loggedIn: how !== "signin" && how !== "none",
      authMethod: "claude.ai",
      plan: "max",
      email: "you@example.com",
      model: null,
      session: null,
      contextUsed: 0,
      contextWindow: 0,
      folder: "",
      command: "claude",
      rules: "",
      chosenModel: null,
      chosenEffort: null,
      defaultModel: "claude-opus-5-5",
      defaultEffort: null,
      power: false,
    };
  },
  assistant_scan: () => {
    const deck = "- Deck modules attached now (build for these): keys, dial, knobs, rollers\n- Owned but not attached now: faders\n- Profiles the user has now: Everyday desk, OBS streaming";
    return {
      used: [["Spotify", 6.2, 4], ["Visual Studio Code", 12.5, 5]],
      since: "2026-10-12 16:30",
      known: ["Discord", "Figma", "Spotify", "Visual Studio Code"],
      modules: ["keys", "dial", "knobs", "rollers"],
      detached: ["faders"],
      profiles: ["Everyday desk", "OBS streaming"],
      text: `What HubDeck found on this computer (the user saw this before it was sent):\n- Apps used most (time in front, counted since 2026-10-12 16:30): Visual Studio Code (12.5 h on 5 days), Spotify (6.2 h on 4 days)\n- Installed apps HubDeck has checked shortcuts or a preset for: Discord, Figma, Spotify, Visual Studio Code\n${deck}`,
      deckText: `What HubDeck found on this computer (the user saw this before it was sent):\n- Apps used most: the user chose not to send them\n${deck}`,
    };
  },
  assistant_send: () => {
    setTimeout(() => void emit("assistant://event", { kind: "text", text: "Here are two ideas for the dial (demo answer).", choices: [], ideas: [] }), 300);
    setTimeout(() => void emit("assistant://event", { kind: "done" }), 400);
    return null;
  },
  profile_switch_in: ({ id, on }) => {
    if (ready.has(id as string)) ready.set(id as string, !!on);
    changed();
  },
  draft_settle: (a) => (a.keep ? { id: a.id, replaced: null } : null),
  builder_draft: () => ({ ...fixture.profiles[0], id: "demo-builder-draft", name: "My deck" }),
  onboarding_done: () => {
    onboarded = true;
    changed();
  },
  iconpacks_list: () => (demoParam("icons")?.startsWith("pack:") ? [{ id: "neon-shapes", name: "Neon shapes (sample)", author: "Demo", count: 12 }] : []),
  iconpack_icons: () =>
    ["#7c5cff", "#22d3ee", "#f472b6", "#ff9f43", "#4ade80", "#facc15", "#60a5fa", "#f87171", "#a78bfa", "#34d399", "#fb923c", "#e879f9"].map((c, i) => ({
      file: `icons/${String(i).padStart(4, "0")}.svg`,
      name: `Shape ${i + 1}`,
      tags: [],
      uri: `data:image/svg+xml;utf8,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 72 72"><rect width="72" height="72" fill="#111"/><circle cx="36" cy="36" r="${14 + (i % 4) * 4}" fill="none" stroke="${c}" stroke-width="5"/></svg>`)}`,
    })),
  iconpack_use: () => "demo.png",
  claude_connect: ({ on }) => Boolean(on),
  tour_suggest: () => ({ focus: "games", games: 12, obs: true, call: "discord", music: "Spotify", kit: FADERS_KIT ? "faders" : "dialKnobs", mac: false }),
  // the tour's deck: Everyday desk stands in for it on the drawing; Exit and Throw away go back
  tour_start: ({ focus }) => {
    const made = () => {
      beforeTour ??= active;
      active = "demo-everyday";
      changed();
      return tourView(focus as string);
    };
    // `&slowtour`: as long as a real first tour takes, to see the making steps
    return demoParam("slowtour") !== undefined && demoParam("slowtour") !== null ? new Promise((done) => setTimeout(() => done(made()), 4000)) : made();
  },
  tour_set: ({ stop, keep, look }) => {
    const s = tour.stops.find((x) => x.id === stop);
    if (s && keep !== null && keep !== undefined) s.keep = keep as boolean;
    if (s && look !== null && look !== undefined) s.look = look as number;
    return tour;
  },
  tour_sound: () => null,
  tour_saver: () => null,
  tour_highlight: () => null,
  // key pages: made, renamed and shown like the engine does
  page_create: ({ profile, name }) => {
    const p = profiles.get(profile as string);
    if (!p) return null;
    const made = { id: `page-${Date.now().toString(36)}`, name: name as string };
    p.pages = [...(p.pages ?? []), made];
    changed();
    return made;
  },
  page_rename: ({ profile, page: id, name }) => {
    const p = profiles.get(profile as string);
    const found = p?.pages?.find((x) => x.id === id);
    if (found) found.name = name as string;
    changed();
  },
  page_set: ({ page: id }) => {
    page = (id as string | null) ?? null;
    changed();
  },
  // `&nudge=unused`: the next-day card about a control of the tour's deck nobody touched
  nudge_get: () => (demoParam("nudge") === "unused" ? { subject: "unused:knobs.2", kind: "unused", exe: "knobs.2", name: "Music", hours: 0, deck: "My deck · dial and knobs", since: "2026-10-13" } : null),
  presses_counting: () => false,
  reduced_motion_set: () => null,
  tour_finish: ({ keep }) => {
    if (!keep && beforeTour) active = beforeTour;
    beforeTour = null;
    changed();
    return keep ? tour.draft : null;
  },
  tour_exit: () => {
    if (beforeTour) active = beforeTour;
    beforeTour = null;
    changed();
  },
  sd_catalog: () => [
    { id: "me.amankhanna.oadiscord", name: "Discord", author: "nekename", description: "Mute, deafen and voice channels in the Discord desktop app", repository: "https://github.com/OpenActionPlugins/discord", icon: `data:image/svg+xml;utf8,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 36 36"><rect width="36" height="36" rx="8" fill="#2a2a33"/><circle cx="18" cy="18" r="8" fill="#8b7cf6"/></svg>')}`, aiAssisted: false },
    { id: "com.example.hue", name: "Philips Hue", author: "someone", description: "Turn Hue lights and scenes on and off, set brightness with a dial", repository: "https://github.com/example/hue", icon: `data:image/svg+xml;utf8,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 36 36"><rect width="36" height="36" rx="8" fill="#2a2a33"/><circle cx="18" cy="18" r="8" fill="#8b7cf6"/></svg>')}`, aiAssisted: false },
    { id: "com.example.clock", name: "World clock", author: "someone else", description: "Time zones on the keys", repository: "https://github.com/example/clock", icon: `data:image/svg+xml;utf8,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 36 36"><rect width="36" height="36" rx="8" fill="#2a2a33"/><circle cx="18" cy="18" r="8" fill="#8b7cf6"/></svg>')}`, aiAssisted: true },
  ],
  sd_catalog_install: ({ id }) => id,
  gallery_list: () => {
    if (demoParam("gallery") === "offline") throw new Error("offline");
    return GALLERY;
  },
  gallery_add: ({ package: url }) => {
    const entry = GALLERY.find((g) => g.package === url);
    const source = entry && profiles.get(entry.id);
    if (!source) throw new Error("not in the gallery");
    const copy = { ...structuredClone(source), id: `gallery-${Date.now()}`, name: `${source.name} (shared)` };
    profiles.set(copy.id, copy);
    changed();
    return copy;
  },
  profiles_dir_get: () => [String.raw`C:\Users\you\AppData\Roaming\HubDeck\profiles`, false],
  spotify_status: () => ["", false],
  midi_ports: () => [],
  audio_devices: () => [[], []],
};

export function installDemo() {
  // `&lang=pl` / `&lang=en`: the window language (remembered like the Settings choice)
  const lang = demoParam("lang");
  if (lang === "pl" || lang === "en") setLanguage(lang);
  mockWindows("main");
  mockIPC(
    (cmd, args) => {
      const handler = handlers[cmd];
      if (!handler) console.info("demo: no mock for", cmd, args);
      return handler?.((args ?? {}) as Record<string, unknown>) ?? null;
    },
    { shouldMockEvents: true },
  );
  if (banners.includes("update")) setTimeout(() => void emit("update://available", UPDATE), 500);
  // screenshot scripts stand in for the deck: `demoEmit("tour://gesture", "dial:turn")`
  Object.assign(window, { demoEmit: (event: string, payload: unknown) => emit(event, payload) });
  if (banners.includes("error")) {
    const fail = () => void emit("engine://error", { control: "keys.2", message: "OBS is not running (ws://127.0.0.1:4455)" });
    setTimeout(fail, 500);
    setInterval(fail, 4000);
  }
  // a multi-action on key 5's hold and modes on knob 1, to show their editors
  const obs = profiles.get("demo-obs");
  if (obs) {
    obs.controls["keys.4:hold"] = {
      action: "core.multi",
      look: {},
      params: {
        steps: [
          { action: "core.hotkey", params: { keys: "Ctrl+Shift+F13", mode: "normal", hold: 30 }, delay: 0 },
          { action: "core.media", params: { command: "playPause" }, delay: 250, when: "obsRecording" },
        ],
      },
    };
    obs.controls["knobs.1"] = { action: "core.volume", look: { label: "Game", led: "#22d3ee", ledEffect: "breathing", ledSpeed: 40 }, params: { target: "app", app: "game.exe", step: 2, popup: "show" } };
    obs.controls["knobs.0"] = {
      action: "core.modes",
      look: { label: "Mixer" },
      params: {
        modes: [
          { label: "Game", action: "core.volume", params: { target: "app", app: "game.exe", step: 2, popup: "show" } },
          { label: "Discord", action: "core.volume", params: { target: "app", app: "Discord.exe", step: 2, popup: "show" } },
        ],
      },
    };
  }
  const touch = new URLSearchParams(location.search).get("touch");
  if (touch) {
    const [kind, n] = touch.split(".");
    const module = deck.modules.find((m) => m.kind === kind)?.id;
    const index = Number(n ?? 0);
    if (module)
      setTimeout(() => {
        if (kind === "keys") void emit("deck://event", { type: "keyDown", module, index }).then(() => emit("deck://event", { type: "keyUp", module, index }));
        else if (kind === "dial") void emit("deck://event", { type: "dial", module, delta: 1 });
        else if (kind === "knobs" || kind === "rollers") void emit("deck://event", { type: kind === "knobs" ? "knob" : "roller", module, index, delta: 1 });
      }, 600);
  }
}
