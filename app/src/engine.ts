import type { LanguagePack } from "./i18n";
import { useCallback, useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
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
  /** Where a key's label stands, over the icon as on Stream Deck; absent = under the icon. */
  labelPos?: "top" | "middle" | "bottom";
  /** A key label's size in key pixels (a key is 72); absent = fitted to the key. */
  labelSize?: number;
  /** A key icon's size; absent = as the label leaves room for. */
  iconSize?: "small" | "large";
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

/** A timer that ran out (`engine::Ringing`). */
export interface Ringing {
  profile: string;
  profileName: string;
  /** `keys.3`, `<page>/keys.3` or `dial`. */
  control: string;
  label: string | null;
  page: string | null;
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
/** An imported profile; `runs`: commands, web requests and scripts in it (asked before it is used). */
export type Imported = Profile & { moved: Moved[]; runs?: string[] };

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
  /** Lights to the music: how dark between beats and from how loud a part they light up, 0..100. */
  musicDim?: number;
  musicFrom?: number;
  /** "predict" (the default) or "react" (the first way, to compare). */
  musicMode?: string;
  /** How early a predicted beat flashes, ms; snares flash whiter. */
  musicLead?: number;
  musicWhite?: boolean;
  /** Knobs and faders flash with the edge too. */
  musicKnobs?: boolean;
  musicFaders?: boolean;
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
export const LED_EFFECTS = ["static", "breathing", "strobe", "blink", "cycling", "random", "wave", "water", "rainbow", "music"] as const;

export const STEP_CONDITIONS = ["", "micMuted", "micOn", "soundMuted", "soundOn", "obsRecording", "obsNotRecording", "obsStreaming", "obsNotStreaming"] as const;

export interface ProfileSummary {
  id: string;
  name: string;
  apps: string[];
  /** Shown as a tab in the top bar (absent in old states: profiles without apps are). */
  inBar?: boolean;
  /** A draft of the deck assistant: not in the bar, not switched to, until picked. */
  draft?: boolean;
  /** Made at once for an installed app ("Ready for your apps"). */
  ready?: boolean;
  /** Switches in when its app comes to the front (off for a ready profile until switched on). */
  switchIn?: boolean;
}

/** What the deck assistant's scan found and the text its first message sends. */
export interface AssistantScan {
  used: [string, number, number][];
  /** Since when the time in front is counted (local, `2026-10-12 16:30`). */
  since: string;
  /** Installed apps HubDeck has a shortcut pack or a preset for. */
  known: string[];
  modules: string[];
  /** Owned but not attached now (the assistant builds for `modules`). */
  detached: string[];
  profiles: string[];
  /** What goes with the first message: the apps used most, the deck, the taste notes. */
  text: string;
  /** The same without the app list ("Send without the app list"). */
  deckText: string;
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
  | { kind: "model"; model: string }
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
  kind: "app" | "module" | "unused";
  /** The app's program, the module kind, or the control (`knobs.2`). */
  exe: string;
  name: string;
  hours: number;
  /** `unused`: the deck's name and the day it was kept. */
  deck?: string;
  since?: string;
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
  /** Sends sound out now (macOS). */
  playing?: boolean;
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
  /** A draft that was on the deck for a try when HubDeck stopped: the window offers it back. */
  interruptedTry?: string | null;
  /** Comparing drafts on the deck: [which one, of how many]. */
  comparing?: [number, number] | null;
  /** Timers that ran out, wherever they are (another page or profile); the window can stop them. */
  ringing?: Ringing[];
  /** A call is on and the deck stays on its profile. */
  inCall?: boolean;
  /** During a call the deck shows the call app's profile. */
  callSwitch?: boolean;
  /** Less movement on the deck and in the window; null = never set (the system's setting is taken). */
  reducedMotion?: boolean | null;
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
  /** The font for text on the deck, a family on this computer ("" = HubDeck's own). */
  deckFont?: string;
  /** The screensaver's effect ("" = none) and after how many idle minutes it starts. */
  saver?: string;
  saverAfterMinutes?: number;
}

export type TourFocus = "games" | "streaming" | "calls" | "music" | "mixed";

/** One part of the deck the tour walks through. */
export interface TourStop {
  id: string;
  keep: boolean;
  look: number;
  looks: number;
  controls: string[];
  tryControl: string | null;
  /** "One control, many jobs": the gestures to try, in order (`keys.5:hold`). */
  tasks?: string[];
  /** "Your dial, many faces": the dial gallery's ids; `look` picks one. */
  faces?: string[];
}

export interface TourView {
  draft: string;
  focus: TourFocus;
  kit: "faders" | "dialKnobs" | "keys" | "none";
  mac: boolean;
  stops: TourStop[];
  call: "discord" | "teams" | "zoom" | null;
  music: string | null;
  games: number;
  rollers: boolean;
}

export interface TourSuggestion {
  focus: TourFocus;
  games: number;
  obs: boolean;
  call: "discord" | "teams" | "zoom" | null;
  music: string | null;
  kit: TourView["kit"];
  mac: boolean;
}

/** A plugin in the open-source catalog (Settings > Plugins > Catalog). */
export interface CatalogPlugin {
  id: string;
  name: string;
  author: string;
  description: string;
  repository: string;
  icon: string;
  aiAssisted: boolean;
  /** Checked in HubDeck on these systems ("Windows", "Windows, macOS"); null = not tried yet. */
  tried?: string | null;
  /** Its page on Elgato Marketplace, when it is there. */
  elgato?: string | null;
  /** Its place among Elgato Marketplace's most downloaded plugins (1 = most). */
  popular?: number | null;
  /** Where the catalog has it from. */
  source: "elgato" | "openaction" | "github" | string;
  /** Its GitHub repository's stars and last change (YYYY-MM-DD), when known. */
  stars?: number | null;
  updated?: string | null;
  /** The systems its manifest or Elgato names ("Windows", "macOS"); empty = not known. */
  systems: string[];
  /** One of the tab's categories ("Audio", "Music", "Streaming"...). */
  category?: string | null;
  /** It has actions for the dial and the knobs. */
  dial: boolean;
}

/** A Stream Deck plugin on this computer: in the Stream Deck app's folder, or a file opened with HubDeck. */
export interface LocalPlugin {
  id: string;
  name: string;
  author: string;
  version: string;
  /** Its icon as a data URI. */
  icon?: string | null;
  /** Why HubDeck cannot run it; null: it can. */
  problem?: string | null;
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
  /** Stopped on the Plugins page. */
  held: boolean;
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
  | { type: "picture" }
  | { type: "petFit" }
  | { type: "named"; what: string }
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

/** Ready-made profiles put on the deck for a try (draft ids): the trying bar offers Add for them,
 * and one left without Add is thrown away (owner's 1a, 2026-10-20). */
export const triedPresets = new Set<string>();

/** Plugins installed in this session (Plugins tab): their actions come first in the library,
 * marked New (owner's 1a, 2026-10-18: "Use it" after Install). */
const newPlugins: string[] = [];
const newPluginsListeners = new Set<() => void>();
export function markPluginNew(id: string) {
  if (!newPlugins.includes(id)) newPlugins.unshift(id);
  newPluginsListeners.forEach((l) => l());
}
export const newPluginIds = () => newPlugins;
export function onNewPlugins(l: () => void) {
  newPluginsListeners.add(l);
  return () => {
    newPluginsListeners.delete(l);
  };
}
/** The library group of a plugin's actions (its actions are `<plugin id>/<action>`). */
export const pluginGroup = (actions: ActionDef[], pluginId: string) => actions.find((a) => a.id.startsWith(`${pluginId}/`))?.group ?? null;

export interface Catalog {
  actions: ActionDef[];
  specialKeys: string[];
  /** Pictures of plugin groups in the library, by group name. */
  groupIcons?: Record<string, string>;
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
/** Whether a key on the deck opens `page`: a Go to page key for it anywhere, Next page on the page
 * before it or Previous page on the one after (owner's pick 1a, 2026-10-16: the window says so
 * when nothing does). The main page always counts as reachable: the deck starts on it. */
export function pageReachable(profile: { pages?: { id: string }[]; controls: Record<string, { action: string; params?: Record<string, unknown> }> }, page: string | null): boolean {
  if (page === null) return true;
  const order = [null, ...(profile.pages ?? []).map((p) => p.id)];
  const at = order.indexOf(page);
  if (at < 0) return true;
  const onPage = (p: string | null, action: string) => Array.from({ length: 15 }, (_, i) => profile.controls[controlId("keys", i, p)]).some((b) => b?.action === action);
  if (Object.values(profile.controls).some((b) => b.action === "core.page" && b.params?.page === page)) return true;
  return onPage(order[at - 1], "core.nextPage") || (at + 1 < order.length && onPage(order[at + 1], "core.previousPage"));
}

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
  /** What turning, a press, a double press and a hold do on this look. */
  gestures: { kind: "turn" | "press" | "double" | "hold"; what: string; whatPl: string }[];
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
  // a plugin's action (`<plugin>/<action>`) starts with no label, as in Stream Deck: it draws its
  // own picture, and a Stream Deck plugin gets the label as its title (owner's 1a, 2026-10-20)
  if (action.id.includes("/")) return { action: action.id, params, look: { icon: action.icon } };
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

/** The last time HubDeck ended without quitting (written by its guard process). */
export interface CrashReport {
  at: string;
  restarted: boolean;
  panic: string | null;
  log: string[];
}

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
  crashReport: () => invoke<CrashReport | null>("crash_report"),
  crashSeen: () => invoke("crash_seen"),
  timerStop: (profile: string, control: string) => invoke("timer_stop", { profile, control }),
  appsSeen: () => invoke("apps_seen"),
  callSwitchSet: (on: boolean) => invoke("call_switch_set", { on }),
  autostart: () => invoke<boolean>("autostart_get"),
  autostartSet: (on: boolean) => invoke<boolean>("autostart_set", { on }),
  reducedMotionSet: (on: boolean) => invoke("reduced_motion_set", { on }),
  pressesCounting: () => invoke<boolean>("presses_counting"),
  pressesCountSet: (on: boolean) => invoke("presses_count_set", { on }),
  languageSet: (language: string) => invoke("language_set", { language }),
  languages: () => invoke<{ code: string; name: string }[]>("languages_list"),
  languageFile: (code: string) => invoke<LanguagePack>("language_file", { code }),
  languageTemplate: (template: unknown) => invoke<string>("language_template", { template }),
  languagesOpenFolder: () => invoke("languages_open_folder"),
  dialGallery: () => invoke<DialFace[]>("dial_gallery"),
  aiPrompt: (name: string, exe: string, target: string, picture: string | null, language: string) => invoke<string>("ai_prompt", { name, exe, target, picture, language }),
  /** Runs Claude Code with HubDeck's tools only; rejects with "no-cli" when it is not installed. */
  aiBuild: (prompt: string) => invoke<string>("ai_build", { prompt }),
  suggestions: (profile: string, control: string) => invoke<Binding[]>("suggestions", { profile, control }),
  profileFill: (profile: string, page: string | null) => invoke<number>("profile_fill", { profile, page }),
  autoSet: (on: boolean) => invoke("auto_set", { on }),
  settings: (s: { autoSwitch?: boolean; fallback?: string }) => invoke("settings_set", { autoSwitch: s.autoSwitch ?? null, fallback: s.fallback ?? null }),
  setBinding: (profile: string, control: string, binding: Binding | null) => invoke("binding_set", { profile, control, binding }),
  /** Several controls at once, changed on the engine's copy of the profile (null clears one). */
  setBindings: (profile: string, bindings: Record<string, Binding | null>) => invoke("bindings_set", { profile, bindings }),
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
  imageThumb: (name: string) => invoke<string | null>("image_thumb", { name }),
  petPreview: (params: Record<string, unknown>) => invoke<string[]>("pet_preview", { params }),
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
  addPreset: (id: string, kinds?: string[], draft = false) => invoke<Profile>("preset_add", { id, kinds: kinds && kinds.length ? kinds : null, draft }),
  macroRecordStart: () => invoke("macro_record_start"),
  /** The keys pressed since the start, as Hotkey steps with the pauses between them. */
  macroRecordStop: () => invoke<Step[]>("macro_record_stop"),
  onboardingDone: () => invoke("onboarding_done"),
  integrations: () =>
    invoke<{ obsUrl: string; obsPasswordSet: boolean; haUrl: string; haTokenSet: boolean; streamlabsUrl: string; streamlabsTokenSet: boolean }>("integrations_get"),
  midiPorts: () => invoke<string[]>("midi_ports"),
  audioDevices: () => invoke<[{ id: string; name: string; default: boolean }[], { id: string; name: string; default: boolean }[]]>("audio_devices"),
  plugins: () => invoke<PluginInfo[]>("plugins_list"),
  pluginStop: (id: string) => invoke("plugin_stop", { id }),
  pluginStart: (id: string) => invoke("plugin_start", { id }),
  /** Stop a plugin, take its actions out and delete its folder. */
  pluginUninstall: (id: string) => invoke("plugin_uninstall", { id }),
  /** Where a plugin's actions stand in the profiles. */
  pluginUses: (id: string) => invoke<{ profile: string; pages: Profile["pages"]; control: string; label: string | null }[]>("plugin_uses", { id }),
  /** macOS: may HubDeck send keys and type (Accessibility)? */
  accessibilityOk: () => invoke<boolean>("accessibility_ok"),
  /** macOS: ask for Accessibility with the system's dialog. */
  accessibilityAsk: () => invoke<boolean>("accessibility_ask"),
  /** Install a Stream Deck plugin file; returns its id. */
  sdInstall: (path: string) => invoke<string>("sd_install", { path }),
  /** The catalog of open-source Stream Deck / OpenAction plugins (downloaded on request). */
  sdCatalog: () => invoke<CatalogPlugin[]>("sd_catalog"),
  /** Download a catalog plugin and install it; returns its id. */
  sdCatalogInstall: (id: string) => invoke<string>("sd_catalog_install", { id }),
  /** A plugin file dropped on the window: its bytes kept in a temp file; returns that file's path. */
  sdDropFile: (name: string, bytes: Uint8Array) => invoke<string>("sd_drop_file", bytes, { headers: { "x-name": encodeURIComponent(name) } }),
  /** The plugins Elgato's Stream Deck app installed on this computer. */
  sdStreamDeck: () => invoke<LocalPlugin[]>("sd_stream_deck"),
  /** Copy one of the Stream Deck app's plugins into HubDeck and start it; returns its id. */
  sdImport: (id: string) => invoke<string>("sd_import", { id }),
  /** What a plugin file holds, without installing it. */
  sdPeek: (path: string) => invoke<LocalPlugin>("sd_peek", { path }),
  /** Plugin files opened with HubDeck (a double click) since the window last asked. */
  sdOpened: () => invoke<string[]>("sd_opened"),
  /** The tour: what HubDeck suggests the deck is mostly for, from what it sees. */
  tourSuggest: (language: string) => invoke<TourSuggestion>("tour_suggest", { language }),
  tourDone: () => invoke<boolean>("tour_done"),
  /** Make the deck for this focus and show it on the deck (a draft). */
  tourStart: (focus: TourFocus, language: string) => invoke<TourView>("tour_start", { focus, language }),
  /** Keep or leave out a part, or pick its look; the deck follows. */
  tourSet: (stop: string, keep: boolean | null, look: number | null) => invoke<TourView>("tour_set", { stop, keep, look }),
  tourSound: (on: boolean) => invoke("tour_sound", { on }),
  tourSaver: (effect: string) => invoke("tour_saver", { effect }),
  /** Point at a control on the hardware (`keys.5`, `knobs.2`, `dial`); null stops it. */
  tourHighlight: (control: string | null) => invoke("tour_highlight", { control }),
  /** Keep the deck (and make it home) or throw it away; returns the profile id when kept. */
  tourFinish: (keep: boolean, home: boolean) => invoke<string | null>("tour_finish", { keep, home }),
  tourExit: () => invoke("tour_exit"),
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
  /** "Use this" (keep: on the deck as home; a new version of one of the user's decks replaces
   * it, `replaced` = its name) or throw the draft away. */
  /** "Switch in with the app" of a ready profile. */
  profileSwitchIn: (id: string, on: boolean) => invoke("profile_switch_in", { id, on }),
  settleDraft: (id: string, keep: boolean) => invoke<{ id: string; replaced: string | null } | null>("draft_settle", { id, keep }),
  /** A web or mail link in the system's browser or mail app (a test copy only logs it). */
  openLink: (url: string) => invoke("open_link", { url }),
  games: () => invoke<GameSummary[]>("games_list"),
  backups: () => invoke<{ id: string; when: string; reason: string; profiles: number }[]>("backups_list"),
  backupNow: () => invoke<string>("backup_now"),
  profilesDir: () => invoke<[string, boolean]>("profiles_dir_get"),
  setProfilesDir: (dir: string | null) => invoke("profiles_dir_set", { dir }),
  screens: () => invoke<Screens>("screens_get"),
  setScreens: (screens: Screens) => invoke("screens_set", { screens }),
  deckFonts: () => invoke<string[]>("deck_fonts"),
  restoreBackup: (id: string) => invoke("backup_restore", { id }),
  playGames: (on: boolean) => invoke("games_play", { on }),
  wheel: () => invoke<string[]>("wheel_get"),
  setWheel: (choices: string[]) => invoke<string[]>("wheel_set", { choices }),
  openPluginsFolder: () => invoke("plugins_open_folder"),
  spotifyStatus: () => invoke<[string, boolean]>("spotify_status"),
  musicTest: () => invoke("music_test"),
  spotifyConnect: (clientId: string) => invoke<{ name: string; premium: boolean | null }>("spotify_connect", { clientId }),
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
  addWayToPage: (profile: string, page: string) => invoke("page_add_way", { profile, page }),
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
    // the same while typing or scrolling (owner, 2026-10-14: typing in the assistant lagged)
    const tick = () => {
      if (userBusy()) return;
      const color = hubRainbow(Date.now());
      document.querySelectorAll<HTMLElement>(".dk").forEach((el) => el.style.setProperty("--hub-rainbow", color));
    };
    tick();
    const timer = window.setInterval(tick, 100);
    return () => window.clearInterval(timer);
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

interface DialFrame {
  profile: string | null;
  module: string;
  jpeg: string;
}

/** When the user last moved the pointer, typed or scrolled in the window (ms since 1970). Each
 * repaint of the deck drawing re-runs its filters (~80 ms in WebKit), so live pictures and the
 * rainbow wait while the user works (hovering lagged on macOS, 2026-10-13 and 2026-10-15). */
let lastInput = 0;
if (typeof window !== "undefined") {
  const onInput = () => (lastInput = Date.now());
  (["pointermove", "keydown", "wheel"] as const).forEach((type) => window.addEventListener(type, onInput, { passive: true }));
}
const INPUT_QUIET_MS = 1200;
/** The deck itself counts as the user working: turning and sliding redraw the drawing too. */
export function deckTouched() {
  lastInput = Date.now();
}
/** WebView2 (Windows) draws the deck at 160 frames a second with the pictures coming in (release
 * gate, 2026-10-16): there nothing waits, or the screensaver and GIFs stood still while the
 * pointer moved over the window (the owner, 2026-10-16). */
const waitsForUser = typeof navigator === "undefined" || !navigator.userAgent.includes("Edg/");
function userBusy(): boolean {
  return waitsForUser && Date.now() - lastInput < INPUT_QUIET_MS;
}

/** A live JPEG from the engine as an object URL, the previous one released. As data URIs each
 * frame (16 a second) stayed in WebKit's memory: the window's page grew to 930 MB in 17 minutes
 * and went black (2026-10-15). While the user works the newest frame waits. Through a channel, not
 * an event: as events the page grew ~19 MB a minute while the deck moved (2026-10-16). */
function useLiveJpeg<F extends { jpeg: string }>(which: "keys" | "dial"): { frame: F; url: string } | null {
  const [shown, setShown] = useState<{ frame: F; url: string } | null>(null);
  useEffect(() => {
    if (!inTauri()) return;
    let pending: F | null = null;
    let current: string | null = null;
    let alive = true;
    const show = (f: F) => {
      (window as unknown as { __mirrorFrame?: () => void }).__mirrorFrame?.();
      const bytes = Uint8Array.from(atob(f.jpeg), (c) => c.charCodeAt(0));
      const url = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
      const old = current;
      current = url;
      setShown({ frame: f, url });
      // the old picture goes once the new one is in the page
      if (old) window.setTimeout(() => URL.revokeObjectURL(old), 1000);
    };
    const offer = (f: F) => {
      if (!alive) return;
      if (userBusy()) pending = f;
      else show(f);
    };
    const timer = window.setInterval(() => {
      if (pending && !userBusy()) {
        const f = pending;
        pending = null;
        show(f);
      }
    }, 250);
    const channel = new Channel<F>();
    channel.onmessage = offer;
    invoke<F | null>("mirror_listen", { which, channel }).then((f) => f && offer(f)).catch(() => {});
    return () => {
      alive = false;
      window.clearInterval(timer);
      invoke("mirror_unlisten", { id: channel.id }).catch(() => {});
      if (current) URL.revokeObjectURL(current);
    };
  }, [which]);
  return shown;
}

/** The screensaver on the deck's keys now, for the window to draw it itself (src/saver.ts); null
 * while none runs. */
/** End the screensaver on the deck, as a touch of the deck does. */
export const saverEnd = () => (inTauri() ? invoke("saver_end").catch(() => {}) : Promise.resolve());

export function useSaverShown(): { effect: string; color: string } | null {
  const [shown, setShown] = useState<{ effect: string; color: string } | null>(null);
  useEffect(() => {
    if (!inTauri()) return;
    invoke<{ effect: string; color: string } | null>("saver_shown").then(setShown).catch(() => {});
    const un = listen<{ effect: string; color: string } | null>("engine://saver", (e) => setShown(e.payload));
    return () => {
      un.then((u) => u());
    };
  }, []);
  return shown;
}

/** A clock in seconds that ticks `fps` times a second while `on`. */
export function useTicker(on: boolean, fps: number): number {
  const [t, setT] = useState(0);
  useEffect(() => {
    if (!on) return;
    const start = performance.now();
    const timer = window.setInterval(() => setT((performance.now() - start) / 1000), 1000 / fps);
    return () => window.clearInterval(timer);
  }, [on, fps]);
  return t;
}

/** The dial's picture exactly as the hardware shows it (live values, covers, animations), for the
 * profile on the deck; null when another profile is shown or nothing came yet. */
export function useDialFrame(profile: string | undefined, module: string): string | null {
  const shown = useLiveJpeg<DialFrame>("dial");
  if (!shown || !profile || shown.frame.profile !== profile || shown.frame.module !== module) return null;
  return shown.url;
}

interface KeysFrame {
  profile: string | null;
  page: string | null;
  module: string;
  jpeg: string;
  rects: [number, number, number, number][];
}

/** The key panel's picture exactly as the hardware shows it, with each key's window in it, for the
 * profile and page on the deck; null otherwise. */
export function useKeysFrame(profile: string | undefined, page: string | null | undefined, module: string): { src: string; rects: KeysFrame["rects"] } | null {
  const shown = useLiveJpeg<KeysFrame>("keys");
  if (!shown || !profile || shown.frame.profile !== profile || (shown.frame.page ?? null) !== (page ?? null) || shown.frame.module !== module) return null;
  return { src: shown.url, rects: shown.frame.rects };
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
    api.preview(screen, look).then((raw) => {
      const s = inWindow(raw);
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

export const svgDataUrl = (svg: string) => `data:image/svg+xml;utf8,${encodeURIComponent(inWindow(svg))}`;

/** A deck drawing for the window: the deck's own font is the only one the renderer has, the window
 * may not have it at all, and without a fallback the browser drew its serif (2026-10-14). */
export const inWindow = (svg: string) => svg.replace(/font-family="IBM Plex Sans"/g, 'font-family="IBM Plex Sans, system-ui, Segoe UI, Helvetica Neue, Arial, sans-serif"');
