import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ModuleKind } from "./deck";
import { actionName } from "./i18n";

/** Mirrors src-tauri/src/engine/profile.rs. */
export interface Look {
  label?: string;
  icon?: string;
  bg?: string;
  fg?: string;
  /** The icon in its own color; absent = the text color. */
  iconColor?: string;
  /** What the control does and how to set it (the demo's `demo:<id>` are translated). */
  note?: string;
  /** How a picture is drawn: like an icon with the label under it, or over the whole key; absent = by the picture. */
  fit?: "icon" | "fill";
  /** LED of a knob or fader, `#rrggbb`. */
  led?: string;
  /** The LED's brightness follows the value (fader position, volume). */
  ledLevel?: boolean;
  /** How a key's background is drawn; absent = flat. */
  style?: KeyStyle;
  /** LED effect of a knob or fader (one of `LED_EFFECTS`); absent = static. */
  ledEffect?: string;
  /** Effect speed 0..100; one speed per module. */
  ledSpeed?: number;
  /** The user's own picture (name returned by `importImage`); replaces the icon. */
  image?: string;
}

/** One picture or a clock across all 15 keys. */
export type Metric = "cpu" | "ram" | "gpu" | "gpuTemp" | "vram";
/** Game mode is switched off for now; mirrors `GAMES_ENABLED` in src-tauri/src/engine/games.rs. */
export const GAMES_ENABLED = false;

export const METRICS: Metric[] = ["cpu", "ram", "gpu", "gpuTemp", "vram"];

/** One picture, a clock or a live graph across all 15 keys. */
export type PanelContent = { kind: "picture"; image: string } | { kind: "clock" } | { kind: "graph"; metric: Metric };

export interface Binding {
  action: string;
  params: Record<string, unknown>;
  look: Look;
  /** Turning fast counts each detent two or three times. */
  accel?: boolean;
  /** Look while the state is on (muted, recording, live, device on); absent = a red version. */
  lookOn?: Look;
  /** A Stream Deck dial action: a plain turn acts as a turn while pressed, and the other way round. */
  swapTurns?: boolean;
}

/** Actions whose keys show a real on/off state. Mirrors `state_of` in the engine. */
export const hasState = (action: string) =>
  ["core.mic", "core.volume", "core.audioDevice", "core.toggle", "spotify.like", "obs.record", "obs.stream", "obs.mute", "obs.scene", "obs.source", "obs.recordPause", "obs.virtualCam", "obs.replayBuffer", "obs.studioMode", "obs.sceneCollection", "obs.profile", "obs.filter", "streamlabs.record", "streamlabs.stream", "streamlabs.replayBuffer", "streamlabs.studioMode", "streamlabs.mute", "streamlabs.scene", "streamlabs.source", "twitch.chatMode", "twitch.shield", "ha.toggle"].includes(action) || action.includes("/");

/** One mode of a dial or knob (the modes action). */
export interface Mode {
  label: string;
  action: string;
  params: Record<string, unknown>;
}

export interface Page {
  id: string;
  name: string;
}

/** A function copied to a control the user has, after an import or "Adapt to my modules" (`presets::Moved`). */
export interface Moved {
  label: string;
  /** Control it is bound to in the profile; that binding stays. */
  from: string;
  /** Controls that do it now (one turnable control, or a "+" and "-" key). */
  to: string[];
}

/** An imported profile with what was copied to fit the user's modules. */
export type Imported = Profile & { moved: Moved[] };

export interface Profile {
  version: number;
  id: string;
  name: string;
  apps: string[];
  /** Control id -> binding; see `controlId` for page and gesture ids. */
  controls: Record<string, Binding>;
  /** Extra key pages (the main page is not listed). */
  pages?: Page[];
  /** Base edge light color while this profile is active; absent = the base's rainbow. */
  edge?: string;
  /** Effect of the base edge light (one of `LED_EFFECTS`); absent = static. */
  edgeEffect?: string;
  edgeSpeed?: number;
  /** Replaces the separate key faces; keys still act. */
  panel?: PanelContent;
  /** Color for the "profile color" light style; absent = one picked from the name. */
  color?: string;
  /** Shown as a tab in the top bar; absent = shown when the profile has no apps. */
  inBar?: boolean;
}

/** One reaction of the lights. */
export interface Reaction {
  on: boolean;
  color: string;
}

/** The deck's lights: one style, one brightness, reactions (Settings > Lights). */
export interface LightSettings {
  style: string;
  color: string;
  brightness: number;
  react: { mic: Reaction; live: Reaction; agents: Reaction; timer: Reaction; call: Reaction; levels: boolean };
  /** The rainbow is drawn by HubDeck (the window shows exactly what the deck does), not by each module. */
  hubRainbow?: boolean;
}

/** One step of a multi-action. */
export interface Step {
  action: string;
  params: Record<string, unknown>;
  /** Milliseconds to wait before this step. */
  delay: number;
  /** Run only when this holds; "" = always. Mirrors `condition_holds` in the engine. */
  when?: string;
}

export type KeyStyle = "flat" | "gradient" | "glass" | "outline";
export const KEY_STYLES: KeyStyle[] = ["flat", "gradient", "glass", "outline"];

/** Background and text color pairs offered in the look editor. */
export const BACKGROUNDS: [bg: string, fg: string][] = [
  ["#1f1d26", "#f0eef6"],
  ["#2b2733", "#f0eef6"],
  ["#1d2b3a", "#e6f0ff"],
  ["#1f3326", "#e6ffef"],
  ["#3a1f22", "#ffe6e8"],
  ["#33301f", "#fff6d6"],
  ["#2a1f33", "#f3e6ff"],
  ["#7c5cff", "#ffffff"],
  ["#2f6fed", "#ffffff"],
  ["#16a36a", "#ffffff"],
  ["#e5484d", "#ffffff"],
  ["#f5a524", "#1a1406"],
  ["#f0eef6", "#17171c"],
  ["#000000", "#ffffff"],
];

/** Hardware LED effects; mirrors `leds::EFFECTS` in src-tauri/src/engine/leds.rs. */
export const LED_EFFECTS = ["static", "breathing", "strobe", "blink", "cycling", "random", "wave", "water", "rainbow"] as const;

export const STEP_CONDITIONS = ["", "micMuted", "micOn", "soundMuted", "soundOn", "obsRecording", "obsNotRecording", "obsStreaming", "obsNotStreaming"] as const;

export interface ProfileSummary {
  id: string;
  name: string;
  apps: string[];
  /** Shown as a tab in the top bar (absent in old states: profiles without apps are). */
  inBar?: boolean;
  /** A draft of the deck assistant: not in the bar, not switched to, until picked. */
  draft?: boolean;
}

/** What the deck assistant's scan found and the text its first message sends. */
export interface AssistantScan {
  used: [string, number, number][];
  /** Owned but not attached now (the assistant builds for `modules`). */
  detached: string[];
  /** Since when the time in front is counted (local, `2026-10-12 16:30`). */
  since: string;
  recent: string[];
  installed: number;
  games: string[];
  modules: string[];
  profiles: string[];
  text: string;
}

/** What the deck assistant runs on (read on this computer, nothing sent). */
export interface AssistantInfo {
  cli: string | null;
  version: string | null;
  loggedIn: boolean;
  authMethod: string | null;
  plan: string | null;
  email: string | null;
  model: string | null;
  session: string | null;
  /** Tokens in the conversation's context and the model's window (0 before the first answer). */
  contextUsed: number;
  contextWindow: number;
  folder: string;
  command: string;
  rules: string;
  /** Picked in HubDeck; null = Claude Code's default. */
  chosenModel: string | null;
  chosenEffort: string | null;
  /** Claude Code's own defaults from its settings. */
  defaultModel: string | null;
  defaultEffort: string | null;
  /** More power: Claude Code's own tools, each command and web fetch asked in the window. */
  power: boolean;
}

/** What a running answer of the deck assistant reports. */
/** An idea the assistant proposes, shown as a card to pick. */
export interface Idea {
  title: string;
  app: string;
  pitch: string;
  extras: string[];
  /** A few sample keys to draw on the card. */
  keys: { label: string; icon: string }[];
  /** The whole deck sketched; HubDeck makes a draft of it. */
  deck?: unknown;
}

export type AssistantEvent =
  | { kind: "context"; used: number; window: number }
  | { kind: "text"; text: string; choices: string[]; ideas: Idea[] }
  /** A piece of the message being written; the `text` event replaces it. */
  | { kind: "delta"; text: string }
  /** An idea (propose_idea, or a quick sketch HubDeck made itself) with its draft. */
  | { kind: "idea"; idea: Idea; draft: string | null; dropped: string[] }
  /** Answers as buttons; with `multiple` the user ticks several (or all) and sends them together. */
  | { kind: "choices"; choices: string[]; multiple: boolean }
  | { kind: "remembered"; text: string }
  /** More power: Claude Code wants to run a command, fetch a page...; the user answers. */
  | { kind: "permission"; id: number; tool: string; detail: string }
  | { kind: "tool"; name: string }
  | { kind: "done" }
  | { kind: "error"; message: string };

/** A suggestion HubDeck made on its own (an app without a deck, a new kind of module). */
export interface Nudge {
  subject: string;
  kind: "app" | "module";
  exe: string;
  name: string;
  hours: number;
}

/** An app of the user's that HubDeck listens to. */
export interface AppSource {
  id: string;
  name: string;
  exe?: string;
  status: string;
  events?: string;
  actions?: { id: string; label: string; icon?: string; method?: string; url?: string; link?: string }[];
}
export interface AppItem {
  id: string;
  label?: string;
  state?: string;
  text?: string;
  value?: number;
}
export interface AppsState {
  apps: AppSource[];
  /** By name, like `launcher.1`. */
  states: Record<string, AppItem>;
}

/** What the assistant remembers about the user's taste (`taste.json`, on this computer). */
export interface Taste {
  notes: { text: string; by: "assistant" | "user"; at: string }[];
  history: { title: string; app: string; outcome: "kept" | "thrown"; at: string }[];
}

/** An app to link to a profile: open now or seen in front recently, with its icon. */
export interface AppInfo {
  exe: string;
  path: string;
  title: string;
  lastSeen: number;
  running: boolean;
  /** Small PNG data URI. */
  icon?: string;
  /** The program's own name ("Google Chrome"), when it has one. */
  name?: string;
}

export interface EngineState {
  profiles: ProfileSummary[];
  active: string | null;
  /** The home profile at start. */
  fallback: string | null;
  /** The profile the user picked by hand; app profiles give back to it. */
  home: string | null;
  /** The app whose own profile took over, e.g. `Photoshop.exe`; null while the home profile shows. */
  autoApp: string | null;
  /** "Keep this profile": no automatic switching. */
  pinned: boolean;
  autoSwitch: boolean;
  /** Profiles that made themselves and the user has not looked at yet (ids). */
  autoMade?: string[];
  /** Apps that connected themselves and the user has not looked at yet (their names). */
  appsJoined?: string[];
  /** Profiles make themselves for apps with a preset and for games. */
  autoProfiles?: boolean;
  /** The draft shown on the deck for a try. */
  trying?: string | null;
  /** Comparing drafts on the deck: [which one, of how many]. */
  comparing?: [number, number] | null;
  /** A call is on and the deck stays on its profile. */
  inCall?: boolean;
  /** During a call the deck shows the call app's profile. */
  callSwitch?: boolean;
  foreground: string;
  /** Key page shown on the deck; null = the main page. */
  page: string | null;
  /** The first-run welcome was dismissed. */
  onboarded: boolean;
  /** The deck is in game mode. */
  playing: boolean;
}

export interface GameSummary {
  id: string;
  name: string;
  about: string;
  icon: string;
  best: number | null;
}

export interface Screens {
  brightness: number;
  blankAfterMinutes: number;
  blankWhenLocked: boolean;
  /** How app icons show on keys: like an icon with the name, the whole key, the whole key with the name. */
  appIcons?: "icon" | "fill" | "fillLabel";
  /** The screensaver's effect ("" = none) and after how many idle minutes it starts. */
  saver?: string;
  saverAfterMinutes?: number;
}

export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  actions: number;
  status: string;
  connected: boolean;
  restarts: number;
  permissions: string[];
  cpu: number | null;
  memoryMb: number | null;
}

export interface PresetSummary {
  id: string;
  name: string;
  description: string;
  /** A profile for testing the hardware, listed apart. */
  test?: boolean;
  /** `start`, `streaming`, `photo`, `video`, `games`, `tools`, `showcase`, `test`. */
  group?: string;
}

export type InputKind = "press" | "turn" | "slide";

export type ParamDef = {
  key: string;
  label: string;
  default: unknown;
  help?: string;
  when?: [string, string[]];
} & (
  | { type: "text"; multiline: boolean }
  | { type: "target" }
  | { type: "hotkey" }
  | { type: "select"; options: [string, string][] }
  | { type: "number"; min: number; max: number; step: number }
  | { type: "profile" }
  | { type: "app" }
  | { type: "page" }
  | { type: "steps" }
  | { type: "modes" }
  | { type: "midiPort" }
  | { type: "audioDevice" }
  | { type: "game" }
  | { type: "secret" }
  | { type: "place" }
);

/** An app or game installed on this PC (`installed_list`). */
export interface InstalledItem {
  name: string;
  target: string;
  exe: string;
  source: string;
  cover?: string;
  icon?: string;
}

export interface ActionDef {
  id: string;
  name: string;
  group: string;
  description: string;
  inputs: InputKind[];
  params: ParamDef[];
  icon: string;
}

export interface Catalog {
  actions: ActionDef[];
  specialKeys: string[];
}

export interface IconDef {
  name: string;
  svg: string;
}

/** Which inputs a control produces. */
export const CONTROL_INPUTS: Record<ModuleKind, InputKind[]> = {
  base: [],
  keys: ["press"],
  dial: ["press", "turn"],
  knobs: ["press", "turn"],
  faders: ["slide"],
  rollers: ["turn"],
};

export const actionFits = (action: ActionDef, kind: ModuleKind) => action.inputs.some((i) => CONTROL_INPUTS[kind].includes(i));

export type Gesture = "tap" | "hold" | "double";

/**
 * Control id used in profiles: `keys.4`, `dial`, `knobs.1`. Keys on an extra page get the page
 * prefix (`p1a2b3c4d/keys.4`); a hold or double press gets a suffix (`keys.4:hold`). Mirrors
 * `profile::control_id` and `Gesture::suffix` in the engine.
 */
export function controlId(kind: ModuleKind, index: number, page: string | null = null, gesture: Gesture = "tap") {
  const base = kind === "dial" ? "dial" : `${kind}.${index}`;
  const paged = page && kind === "keys" ? `${page}/${base}` : base;
  return gesture === "tap" ? paged : `${paged}:${gesture}`;
}

/** Controls that can be pressed, and so have hold and double press gestures. */
export const pressable = (kind: ModuleKind) => kind === "keys" || kind === "knobs" || kind === "dial";

/** A binding for an action with its defaults and a sensible look. */
/** One look of the dial gallery: a real dial setting, its captions and a preview with sample values. */
export interface DialFace {
  id: string;
  caption: string;
  captionPl: string;
  binding: Binding;
  svg: string;
}

/** The actions most people use, shown first in the library and the action list. */
export const POPULAR_ACTIONS = ["core.hotkey", "core.launch", "core.media", "core.volume", "core.mic", "core.profile", "core.page", "core.text", "core.multi", "core.clock", "core.nowPlaying", "core.game"];

/** Params most people never change: folded under "More options" (`key`, or `action:key` for one action). */
export const ADVANCED_PARAMS = new Set(["step", "amount", "hold", "popup", "core.hotkey:mode", "core.run:folder", "core.run:window"]);

/** Params only a turning control uses (hidden on keys). */
export const TURN_PARAMS = new Set(["clockwise", "counterClockwise"]);

/** Shortcuts picked by name: label key, Windows keys, macOS keys (null = none there), icon. */
export const COMMON_SHORTCUTS: [string, string, string | null, string][] = [
  ["copy", "Ctrl+C", "Cmd+C", "copy"],
  ["paste", "Ctrl+V", "Cmd+V", "clipboard-paste"],
  ["cut", "Ctrl+X", "Cmd+X", "scissors"],
  ["undo", "Ctrl+Z", "Cmd+Z", "undo-2"],
  ["redo", "Ctrl+Y", "Cmd+Shift+Z", "redo-2"],
  ["save", "Ctrl+S", "Cmd+S", "save"],
  ["find", "Ctrl+F", "Cmd+F", "search"],
  ["selectAll", "Ctrl+A", "Cmd+A", "text-select"],
  ["newTab", "Ctrl+T", "Cmd+T", "plus"],
  ["closeTab", "Ctrl+W", "Cmd+W", "x"],
  ["reopenTab", "Ctrl+Shift+T", "Cmd+Shift+T", "rotate-ccw"],
  ["screenshot", "Win+Shift+S", "Cmd+Shift+4", "camera"],
  ["taskView", "Win+Tab", "Ctrl+Up", "layers"],
  ["desktop", "Win+D", "F11", "monitor"],
  ["lock", "Win+L", "Ctrl+Cmd+Q", "lock"],
  ["clipboard", "Win+V", null, "clipboard-list"],
  ["emoji", "Win+.", "Ctrl+Cmd+Space", "smile"],
  ["explorer", "Win+E", null, "folder"],
  ["gameBar", "Win+G", null, "gamepad-2"],
  ["clip", "Win+Alt+G", null, "video"],
];

export function newBinding(action: ActionDef): Binding {
  const params: Record<string, unknown> = {};
  for (const p of action.params) params[p.key] = p.default;
  return { action: action.id, params, look: { label: actionName(action).split(" ")[0], icon: action.icon } };
}

const inTauri = () => "__TAURI_INTERNALS__" in window;

/** One LED as the deck shows it now: `#rrggbb` and its effect (`static`, `breathing`, ..., `off`). */
export interface LedView {
  color: string;
  effect: string;
}

/** What the knob, fader and base LEDs show now, by module id; null = the module's own rainbow. */
export type LedsShown = Record<string, LedView[] | null>;

export const api = {
  state: () => invoke<EngineState>("engine_state"),
  leds: () => invoke<LedsShown>("leds_get"),
  redraw: () => invoke("engine_redraw"),
  profile: (id: string) => invoke<Profile>("profile_get", { id }),
  saveProfile: (profile: Profile) => invoke("profile_save", { profile }),
  createProfile: (name: string, from?: string) => invoke<Profile>("profile_create", { name, from: from ?? null }),
  deleteProfile: (id: string) => invoke("profile_delete", { id }),
  activate: (id: string) => invoke("profile_activate", { id }),
  autoSeen: () => invoke("auto_seen"),
  appsSeen: () => invoke("apps_seen"),
  callSwitchSet: (on: boolean) => invoke("call_switch_set", { on }),
  languageSet: (language: string) => invoke("language_set", { language }),
  dialGallery: () => invoke<DialFace[]>("dial_gallery"),
  aiPrompt: (name: string, exe: string, target: string, picture: string | null, language: string) => invoke<string>("ai_prompt", { name, exe, target, picture, language }),
  /** Runs Claude Code with HubDeck's tools only; rejects with "no-cli" when it is not installed. */
  aiBuild: (prompt: string) => invoke<string>("ai_build", { prompt }),
  suggestions: (profile: string, control: string) => invoke<Binding[]>("suggestions", { profile, control }),
  profileFill: (profile: string, page: string | null) => invoke<number>("profile_fill", { profile, page }),
  autoSet: (on: boolean) => invoke("auto_set", { on }),
  settings: (s: { autoSwitch?: boolean; fallback?: string }) => invoke("settings_set", { autoSwitch: s.autoSwitch ?? null, fallback: s.fallback ?? null }),
  setBinding: (profile: string, control: string, binding: Binding | null) => invoke("binding_set", { profile, control, binding }),
  catalog: () => invoke<Catalog>("actions_list"),
  icons: () => invoke<IconDef[]>("icons_list"),
  preview: (screen: "key" | "dial", look: Look) => invoke<string>("preview_svg", { screen, look }),
  exportProfile: (id: string, path: string) => invoke("profile_export", { id, path }),
  /** Import a file as a new profile, adapted to the user's modules; `moved` says what was copied where. */
  importProfile: (path: string) => invoke<Imported>("profile_import", { path }),
  /** "Adapt to my modules": what was copied where (empty: nothing had to move). */
  adaptProfile: (id: string) => invoke<Moved[]>("profile_adapt", { id }),
  setPage: (page: string | null) => invoke("page_set", { page }),
  importImage: (path: string) => invoke<string>("image_import", { path }),
  appIcon: (path: string) => invoke<string>("app_icon", { path }),
  pin: (pinned: boolean) => invoke("profile_pin", { pinned }),
  lights: () => invoke<LightSettings>("lights_get"),
  installed: (refresh: boolean) => invoke<{ apps: InstalledItem[]; games: InstalledItem[] }>("installed_list", { refresh }),
  installedPicture: (item: InstalledItem) => invoke<string>("installed_picture", { item }),
  /** A new profile for one app: its preset or a starter for its kind, adapted to `kinds`. */
  profileForApp: (exe: string, name: string, target: string, picture: string | null, kinds?: string[]) =>
    invoke<Profile>("profile_for_app", { exe, name, target, picture, kinds: kinds && kinds.length ? kinds : null }),
  checkProfile: (id: string) => invoke<{ control: string; message: string }[]>("profile_check", { id }),
  setLights: (lights: LightSettings) => invoke("lights_set", { lights }),
  apps: () => invoke<AppInfo[]>("apps_list"),
  presets: () => invoke<PresetSummary[]>("presets_list"),
  /** `kinds`: the module kinds the user has (attached and owned); the preset is adapted to them. */
  addPreset: (id: string, kinds?: string[]) => invoke<Profile>("preset_add", { id, kinds: kinds && kinds.length ? kinds : null }),
  macroRecordStart: () => invoke("macro_record_start"),
  /** The keys pressed since the start, as Hotkey steps with the pauses between them. */
  macroRecordStop: () => invoke<Step[]>("macro_record_stop"),
  onboardingDone: () => invoke("onboarding_done"),
  integrations: () =>
    invoke<{ obsUrl: string; obsPasswordSet: boolean; haUrl: string; haTokenSet: boolean; streamlabsUrl: string; streamlabsTokenSet: boolean }>("integrations_get"),
  midiPorts: () => invoke<string[]>("midi_ports"),
  audioDevices: () => invoke<[{ name: string; default: boolean }[], { name: string; default: boolean }[]]>("audio_devices"),
  plugins: () => invoke<PluginInfo[]>("plugins_list"),
  /** macOS: may HubDeck send keys and type (Accessibility)? */
  accessibilityOk: () => invoke<boolean>("accessibility_ok"),
  /** macOS: ask for Accessibility with the system's dialog. */
  accessibilityAsk: () => invoke<boolean>("accessibility_ask"),
  /** Install a Stream Deck plugin file; returns its id. */
  sdInstall: (path: string) => invoke<string>("sd_install", { path }),
  /** A Stream Deck action's settings page for a control, or null. */
  sdInspector: (control: string, action: string, settings: unknown) => invoke<string | null>("sd_inspector", { control, action, settings }),
  assistantScan: () => invoke<AssistantScan>("assistant_scan"),
  /** What the assistant runs on: Claude Code, the account and plan, the session, how it starts. */
  assistantInfo: (language: string) => invoke<AssistantInfo>("assistant_info", { language }),
  assistantSet: (model: string | null, effort: string | null) => invoke("assistant_set", { model, effort }),
  assistantSend: (message: string, scan: string | null, language: string) => invoke("assistant_send", { message, scan, language }),
  assistantStop: () => invoke("assistant_stop"),
  assistantReset: () => invoke("assistant_reset"),
  /** The personal tour as a draft, made at once; its id. */
  assistantTour: (language: string) => invoke<string>("assistant_tour", { language }),
  tasteGet: () => invoke<Taste>("taste_get"),
  /** The user's own apps HubDeck listens to, and what they report now (docs/APP-CONTRACT.md). */
  appsState: () => invoke<AppsState>("apps_state"),
  appConnect: (url: string) => invoke<AppSource>("app_connect", { url }),
  appRemove: (id: string) => invoke("app_remove", { id }),
  appContractPrompt: () => invoke<string>("app_contract_prompt"),
  assistantPermit: (id: number, choice: "once" | "talk" | "deny") => invoke("assistant_permit", { id, choice }),
  assistantPowerSet: (on: boolean) => invoke("assistant_power_set", { on }),
  nudgeGet: () => invoke<Nudge | null>("nudge_get"),
  nudgeAct: (subject: string, action: "later" | "never" | "off" | "done") => invoke("nudge_act", { subject, action }),
  nudgeEnable: (on: boolean) => invoke("nudge_enable", { on }),
  nudgeEnabled: () => invoke<boolean>("nudge_enabled"),
  /** One app's quick sketch as a draft (a suggestion's "Show me a sketch"); its id. */
  assistantSketchApp: (exe: string, name: string, language: string) => invoke<string>("assistant_sketch_app", { exe, name, language }),
  tasteSet: (notes: string[]) => invoke<Taste>("taste_set", { notes }),
  tasteForget: () => invoke<Taste>("taste_forget"),
  tryProfile: (id: string | null) => invoke("profile_try", { id }),
  /** Icons of linked programs also when they were not in front lately (game folders, covers). */
  appIcons: (exes: string[]) => invoke<Record<string, string | null>>("app_icons", { exes }),
  /** Drafts from the assistant's sketches: (title, draft id) of each made. */
  sketchesMake: (ideas: Idea[]) => invoke<[string, string][]>("sketches_make", { ideas }),
  /** Compare drafts on the deck (the dial flips them); null stops. */
  /** Put drafts on the deck to compare from the one at `at` (the dial flips them); null stops. */
  compareDrafts: (ids: string[] | null, at = 0) => invoke("compare_drafts", { ids, at }),
  settleDraft: (id: string, keep: boolean) => invoke("draft_settle", { id, keep }),
  /** A web or mail link in the system's browser or mail app (a test copy only logs it). */
  openLink: (url: string) => invoke("open_link", { url }),
  games: () => invoke<GameSummary[]>("games_list"),
  backups: () => invoke<{ id: string; when: string; reason: string; profiles: number }[]>("backups_list"),
  backupNow: () => invoke<string>("backup_now"),
  profilesDir: () => invoke<[string, boolean]>("profiles_dir_get"),
  setProfilesDir: (dir: string | null) => invoke("profiles_dir_set", { dir }),
  screens: () => invoke<Screens>("screens_get"),
  setScreens: (screens: Screens) => invoke("screens_set", { screens }),
  restoreBackup: (id: string) => invoke("backup_restore", { id }),
  playGames: (on: boolean) => invoke("games_play", { on }),
  wheel: () => invoke<string[]>("wheel_get"),
  setWheel: (choices: string[]) => invoke<string[]>("wheel_set", { choices }),
  openPluginsFolder: () => invoke("plugins_open_folder"),
  spotifyStatus: () => invoke<[string, boolean]>("spotify_status"),
  spotifyConnect: (clientId: string) => invoke("spotify_connect", { clientId }),
  spotifyDisconnect: () => invoke("spotify_disconnect"),
  setHomeAssistant: (url: string, token: string | null) => invoke("homeassistant_set", { url, token }),
  setStreamlabs: (url: string, token: string | null) => invoke("streamlabs_set", { url, token }),
  twitchStatus: () => invoke<[boolean, boolean]>("twitch_status"),
  twitchConnect: () => invoke("twitch_connect"),
  twitchDisconnect: () => invoke("twitch_disconnect"),
  setIntegrations: (obsUrl: string, obsPassword: string | null) => invoke("integrations_set", { obsUrl, obsPassword }),
  createPage: (profile: string, name: string) => invoke<Page>("page_create", { profile, name }),
  renamePage: (profile: string, page: string, name: string) => invoke("page_rename", { profile, page, name }),
  deletePage: (profile: string, page: string) => invoke("page_delete", { profile, page }),
  pauseDeck: (paused: boolean) => invoke("deck_pause", { paused }),
  setBaseTurn: (turn: number | null) => invoke("base_turn_set", { turn }),
  restartModule: (kind: string) => invoke("module_restart", { kind }),
};

export interface EngineView {
  state: EngineState | null;
  profile: Profile | null;
  catalog: Catalog | null;
  icons: IconDef[];
  error: { control: string; message: string } | null;
  reloadProfile: () => void;
}

/** Engine state, the active profile, the catalog and the latest action error. */
export function useEngine(): EngineView {
  const [state, setState] = useState<EngineState | null>(null);
  const [profile, setProfile] = useState<Profile | null>(null);
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  const [icons, setIcons] = useState<IconDef[]>([]);
  const [error, setError] = useState<EngineView["error"]>(null);
  const errorTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    if (!inTauri()) return;
    api.state().then(setState);
    api.catalog().then(setCatalog);
    api.icons().then(setIcons);
    const unlisten = [
      listen<EngineState>("engine://state", (e) => setState(e.payload)),
      // a plugin was installed: its actions join the library
      listen("engine://catalog", () => api.catalog().then(setCatalog)),
      listen<{ control: string; message: string }>("engine://error", (e) => {
        setError(e.payload);
        window.clearTimeout(errorTimer.current);
        errorTimer.current = window.setTimeout(() => setError(null), 6000);
      }),
    ];
    return () => unlisten.forEach((p) => p.then((u) => u()));
  }, []);

  const reloadProfile = useCallback(() => {
    if (state?.active) api.profile(state.active).then(setProfile).catch(() => setProfile(null));
  }, [state?.active]);

  // the active profile is re-read whenever the engine reports a change
  useEffect(() => {
    if (state?.active) api.profile(state.active).then(setProfile).catch(() => setProfile(null));
    else setProfile(null);
  }, [state]);

  return { state, profile, catalog, icons, error, reloadProfile };
}

/** How long HubDeck's rainbow takes to go round (the same as `leds::RAINBOW_MS`). */
export const RAINBOW_MS = 10_000;

/** HubDeck's rainbow color at a moment (ms since 1970), worked out as the engine does. */
export function hubRainbow(ms: number): string {
  const h = ((ms % RAINBOW_MS) / RAINBOW_MS) * 6;
  const x = Math.round((1 - Math.abs((h % 2) - 1)) * 255);
  const [r, g, b] = [[255, x, 0], [x, 255, 0], [0, 255, x], [0, x, 255], [x, 0, 255], [255, 0, x]][Math.min(5, Math.floor(h))];
  return `rgb(${r}, ${g}, ${b})`;
}

/** The LEDs as the engine last set them on the deck (null outside the app, e.g. the demo). */
export function useLeds(): LedsShown | null {
  const [leds, setLeds] = useState<LedsShown | null>(null);
  // HubDeck's rainbow: the same clock as the engine, so the drawing and the deck show one color
  const hub = !!leds && Object.values(leds).some((l) => l?.some((x) => x.effect === "hubRainbow"));
  useEffect(() => {
    if (!hub) return;
    // set on the deck drawings only: on the whole page it made WebKit restyle and repaint the
    // window ten times a second (hover lagged on macOS, 2026-10-13)
    // and it waits while the pointer moves: each step repaints the drawing's filters (~80 ms in
    // WebKit), which made hovering the panels lag; the deck itself keeps its rainbow
    let moved = 0;
    const onMove = () => (moved = Date.now());
    window.addEventListener("pointermove", onMove, { passive: true });
    const tick = () => {
      if (Date.now() - moved < 700) return;
      const color = hubRainbow(Date.now());
      document.querySelectorAll<HTMLElement>(".dk").forEach((el) => el.style.setProperty("--hub-rainbow", color));
    };
    tick();
    const timer = window.setInterval(tick, 100);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("pointermove", onMove);
    };
  }, [hub]);
  useEffect(() => {
    if (!inTauri()) return;
    api.leds().then(setLeds).catch(() => {});
    const un = listen<LedsShown>("engine://leds", (e) => setLeds(e.payload));
    return () => {
      un.then((u) => u());
    };
  }, []);
  return leds;
}

/** What plugins show on the active profile's controls right now (by control id): the deck drawing
 *  uses these over the saved looks. */
export type LiveLooks = { profile: string; looks: Record<string, Look> };
export function useLiveLooks(): LiveLooks {
  const [looks, setLooks] = useState<LiveLooks>({ profile: "", looks: {} });
  useEffect(() => {
    if (!inTauri()) return;
    invoke<LiveLooks | null>("live_looks").then((v) => v?.looks && setLooks(v)).catch(() => {});
    const un = listen<LiveLooks>("engine://looks", (e) => e.payload?.looks && setLooks(e.payload));
    return () => {
      un.then((u) => u());
    };
  }, []);
  return looks;
}

const previewCache = new Map<string, string>();
/** Changes when a setting that changes how looks are drawn changes, so previews are drawn again. */
let previewSalt = "";
export function setPreviewSalt(salt: string) {
  previewSalt = salt;
}

/** The exact SVG the hardware shows for a look (cached). */
export function usePreview(screen: "key" | "dial", look: Look | undefined): string | null {
  const key = look ? `${screen}:${previewSalt}:${JSON.stringify(look)}` : null;
  const [svg, setSvg] = useState<string | null>(key ? previewCache.get(key) ?? null : null);
  useEffect(() => {
    if (!key || !look || !inTauri()) {
      setSvg(null);
      return;
    }
    const cached = previewCache.get(key);
    if (cached) {
      setSvg(cached);
      return;
    }
    let live = true;
    api.preview(screen, look).then((s) => {
      previewCache.set(key, s);
      if (live) setSvg(s);
    });
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return svg;
}

export const svgDataUrl = (svg: string) => `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
