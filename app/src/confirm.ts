import { useSyncExternalStore } from "react";
import * as dialog from "@tauri-apps/plugin-dialog";
import { t } from "./i18n";

/**
 * Questions before something that cannot be undone, asked in HubDeck's own window (owner's pick
 * 1b, 2026-10-23, https://claude.ai/artifact/GCr9VTUJX1bD4CseYo9h1d: like Discord, Spotify and VS
 * Code's custom dialogs, in the window's look, the button named for what it does). Not
 * `window.confirm`: in HubDeck's window it goes to a dialog command the plugin no longer has and
 * returns a promise, which an `if` takes as yes; the profile was deleted without a question
 * (2026-10-18). `ConfirmHost` shows them, one at a time.
 */
export interface Question {
  title: string;
  body?: string;
  /** What it touches, one line each (the keys that use a plugin). */
  details?: string[];
  ok: string;
  cancel?: string;
  /** The answer deletes or removes something: the OK button is red. */
  danger?: boolean;
}
type Asked = Question & { answer: (yes: boolean) => void };

let queue: Asked[] = [];
const listeners = new Set<() => void>();
const changed = () => listeners.forEach((l) => l());

/** Asks in the window and waits for the answer (Cancel, Escape or a click beside it: no). */
export function askInWindow(q: Question): Promise<boolean> {
  return new Promise((resolve) => {
    const asked: Asked = {
      ...q,
      answer: (yes) => {
        queue = queue.filter((a) => a !== asked);
        changed();
        resolve(yes);
      },
    };
    queue = [...queue, asked];
    changed();
  });
}

/** A question as one sentence, with a red Delete. */
export function confirmed(message: string, ok: string = t("confirm.delete")): Promise<boolean> {
  return askInWindow({ title: message, ok, danger: true });
}

/** The question shown now, for `ConfirmHost`. */
export function useQuestion(): Asked | null {
  return useSyncExternalStore(
    (l) => (listeners.add(l), () => listeners.delete(l)),
    () => queue[0] ?? null,
  );
}

/**
 * The system's file dialogs. A test copy (`HUBDECK_AUTOMATION`, its window off screen) never shows
 * one: a dialog would come up on the owner's screen and take his focus (2026-10-23, a walk's
 * Uninstall question did, when it was still the system's). There the engine's start script sets
 * `__HUBDECK_TEST__` and a file dialog picks nothing.
 */
const testCopy = () => typeof window !== "undefined" && "__HUBDECK_TEST__" in window;

export const open: typeof dialog.open = ((options) => (testCopy() ? Promise.resolve(null) : dialog.open(options))) as typeof dialog.open;

export const save: typeof dialog.save = (options) => (testCopy() ? Promise.resolve(null) : dialog.save(options));
