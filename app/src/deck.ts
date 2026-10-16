import { useEffect, useReducer } from "react";
import { deckTouched } from "./engine";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type ModuleKind = "base" | "keys" | "faders" | "knobs" | "rollers" | "dial";
export type Orientation = "horizontal" | "vertical";

export interface ModuleInfo {
  id: string;
  kind: ModuleKind;
  serial: string;
  firmware: string | null;
  /** Slot inside the base, e.g. "1-4-3"; null when the platform cannot tell. */
  slot: string | null;
}

export interface DeckSnapshot {
  modules: ModuleInfo[];
  orientation: Orientation | null;
  /** Clockwise degrees the base is turned on the desk: the user's choice, else from the sensor. */
  turn: number;
  /** The user's choice of `turn`; null = follow the sensor. */
  turnOverride: number | null;
  unresponsive: string[];
  /** Paused from the tray: keys and dial show their own idle animation. */
  paused: boolean;
  /** Where each fader stands now, by `controlKey`. */
  faders?: Record<string, number>;
  /** While no module is attached yet: ms since the hub began looking (start, resume). */
  lookingMs?: number | null;
}

/** Mirrors `UiEvent` in src-tauri/src/deck.rs. */
export type DeckEvent =
  | { type: "snapshot"; deck: DeckSnapshot }
  | { type: "keyDown" | "keyUp" | "knobDown" | "knobUp"; module: string; index: number }
  | { type: "fader"; module: string; index: number; value: number }
  | { type: "knob" | "roller"; module: string; index: number; delta: number }
  | { type: "dial"; module: string; delta: number }
  | { type: "dialDown" | "dialUp"; module: string };

/** A control on the deck, e.g. `{ module: "keys:MH…", kind: "keys", index: 5 }`. */
export interface ControlRef {
  module: string;
  kind: ModuleKind;
  index: number;
}

/** Live values shown in the mirror. Counters for endless controls are local, they start at 0. */
export interface LiveState {
  deck: DeckSnapshot;
  pressedKeys: Record<string, boolean>;
  faders: Record<string, number>;
  counters: Record<string, number>;
  /** Last control touched on the hardware; the inspector follows it. */
  lastTouched: ControlRef | null;
  /** Bumped on every roller notch so the UI can flash it. */
  pulses: Record<string, number>;
}

const empty: LiveState = {
  deck: { modules: [], orientation: null, turn: 0, turnOverride: null, unresponsive: [], paused: false },
  pressedKeys: {},
  faders: {},
  counters: {},
  lastTouched: null,
  pulses: {},
};

export const controlKey = (module: string, index: number) => `${module}#${index}`;

function kindOf(state: LiveState, module: string): ModuleKind {
  return state.deck.modules.find((m) => m.id === module)?.kind ?? (module.split(":")[0] as ModuleKind);
}

/** The control last touched, the same object while it is the same control: a fader reports dozens
 * of times a second and a new object each time re-selected it in the window (2026-10-15). */
function touched(state: LiveState, module: string, kind: ControlRef["kind"], index: number): ControlRef {
  const t = state.lastTouched;
  return t && t.module === module && t.kind === kind && t.index === index ? t : { module, kind, index };
}

function reduce(state: LiveState, ev: DeckEvent): LiveState {
  switch (ev.type) {
    case "snapshot":
      // the faders' places as the engine knows them; a newer move wins
      return { ...state, deck: ev.deck, faders: { ...(ev.deck.faders ?? {}), ...state.faders } };
    case "keyDown":
    case "keyUp": {
      const pressedKeys = { ...state.pressedKeys, [controlKey(ev.module, ev.index)]: ev.type === "keyDown" };
      const lastTouched = ev.type === "keyDown" ? touched(state, ev.module, kindOf(state, ev.module), ev.index) : state.lastTouched;
      return { ...state, pressedKeys, lastTouched };
    }
    case "fader":
      return {
        ...state,
        faders: { ...state.faders, [controlKey(ev.module, ev.index)]: ev.value },
        lastTouched: touched(state, ev.module, "faders", ev.index),
      };
    case "knob":
    case "roller": {
      const key = controlKey(ev.module, ev.index);
      return {
        ...state,
        counters: { ...state.counters, [key]: (state.counters[key] ?? 0) + ev.delta },
        pulses: { ...state.pulses, [key]: (state.pulses[key] ?? 0) + 1 },
        lastTouched: touched(state, ev.module, ev.type === "knob" ? "knobs" : "rollers", ev.index),
      };
    }
    case "knobDown":
    case "knobUp":
      return {
        ...state,
        pressedKeys: { ...state.pressedKeys, [controlKey(ev.module, ev.index)]: ev.type === "knobDown" },
        lastTouched: touched(state, ev.module, "knobs", ev.index),
      };
    case "dial": {
      const key = controlKey(ev.module, 0);
      return {
        ...state,
        counters: { ...state.counters, [key]: (state.counters[key] ?? 0) + ev.delta },
        lastTouched: touched(state, ev.module, "dial", 0),
      };
    }
    case "dialDown":
    case "dialUp":
      return {
        ...state,
        pressedKeys: { ...state.pressedKeys, [controlKey(ev.module, 0)]: ev.type === "dialDown" },
        lastTouched: touched(state, ev.module, "dial", 0),
      };
  }
}

/** Snapshot once, then follow `deck://event`. Works in a plain browser too (empty deck). */
/** Several deck events at once: applied in one go, so the window draws once per screen frame. */
function reduceAll(state: LiveState, events: DeckEvent[]): LiveState {
  return events.reduce(reduce, state);
}

export function useDeck(): LiveState {
  const [state, dispatch] = useReducer(reduceAll, empty);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    // a fader reports ~25 times a second and each report redrew the whole window (WebKit could
    // not keep up, 2026-10-15): events wait for the next screen frame and go in together
    let queued: DeckEvent[] = [];
    let frame = 0;
    const flush = () => {
      frame = 0;
      const batch = queued;
      queued = [];
      dispatch(batch);
    };
    listen<DeckEvent>("deck://event", (e) => {
      (window as unknown as { __deckEvent?: () => void }).__deckEvent?.();
      deckTouched();
      queued.push(e.payload);
      if (!frame) frame = requestAnimationFrame(flush);
    }).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    invoke<DeckSnapshot>("deck_snapshot").then((deck) => dispatch([{ type: "snapshot", deck }]));
    return () => {
      cancelled = true;
      if (frame) cancelAnimationFrame(frame);
      unlisten?.();
    };
  }, []);
  return state;
}
