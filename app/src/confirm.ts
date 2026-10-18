import { confirm } from "@tauri-apps/plugin-dialog";

/**
 * Asks before something that cannot be undone and waits for the answer. Not `window.confirm`: in
 * HubDeck's window it goes to a dialog command the plugin no longer has and returns a promise, which
 * an `if` takes as yes; the profile was deleted without a question (2026-10-18).
 */
export function confirmed(message: string): Promise<boolean> {
  return confirm(message, { kind: "warning" }).catch(() => false);
}
