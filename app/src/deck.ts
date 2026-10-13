import { useEffect, useReducer } from "react";
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

function reduce(state: LiveState, ev: DeckEvent): LiveState {
  switch (ev.type) {
    case "snapshot":
      return { ...state, deck: ev.deck };
    case "keyDown":
    case "keyUp": {
      const pressedKeys = { ...state.pressedKeys, [controlKey(ev.module, ev.index)]: ev.type === "keyDown" };
      const lastTouched = ev.type === "keyDown" ? { module: ev.module, kind: kindOf(state, ev.module), index: ev.index } : state.lastTouched;
      return { ...state, pressedKeys, lastTouched };
    }
    case "fader":
      return {
        ...state,
        faders: { ...state.faders, [controlKey(ev.module, ev.index)]: ev.value },
        lastTouched: { module: ev.module, kind: "faders", index: ev.index },
      };
    case "knob":
    case "roller": {
      const key = controlKey(ev.module, ev.index);
      return {
        ...state,
        counters: { ...state.counters, [key]: (state.counters[key] ?? 0) + ev.delta },
        pulses: { ...state.pulses, [key]: (state.pulses[key] ?? 0) + 1 },
        lastTouched: { module: ev.module, kind: ev.type === "knob" ? "knobs" : "rollers", index: ev.index },
      };
    }
    case "knobDown":
    case "knobUp":
      return {
        ...state,
        pressedKeys: { ...state.pressedKeys, [controlKey(ev.module, ev.index)]: ev.type === "knobDown" },
        lastTouched: { module: ev.module, kind: "knobs", index: ev.index },
      };
    case "dial": {
      const key = controlKey(ev.module, 0);
      return {
        ...state,
        counters: { ...state.counters, [key]: (state.counters[key] ?? 0) + ev.delta },
        lastTouched: { module: ev.module, kind: "dial", index: 0 },
      };
    }
    case "dialDown":
    case "dialUp":
      return {
        ...state,
        pressedKeys: { ...state.pressedKeys, [controlKey(ev.module, 0)]: ev.type === "dialDown" },
        lastTouched: { module: ev.module, kind: "dial", index: 0 },
      };
  }
}

/** Snapshot once, then follow `deck://event`. Works in a plain browser too (empty deck). */
export function useDeck(): LiveState {
  const [state, dispatch] = useReducer(reduce, empty);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen<DeckEvent>("deck://event", (e) => dispatch(e.payload)).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    invoke<DeckSnapshot>("deck_snapshot").then((deck) => dispatch({ type: "snapshot", deck }));
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
  return state;
}
