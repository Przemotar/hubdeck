import { useSyncExternalStore } from "react";
import { demoParam } from "./demo/params";

/**
 * Light or dark look. "system" follows the OS (`prefers-color-scheme`); the choice is a UI
 * preference kept in localStorage. Tokens: styles.css (`:root` dark, `[data-theme="light"]` light).
 */
export type Theme = "system" | "dark" | "light";

const KEY = "hubdeck.theme";
const listeners = new Set<() => void>();

function load(): Theme {
  // `?demo&theme=light` for screenshots
  const demo = demoParam("theme");
  if (demo === "dark" || demo === "light" || demo === "system") return demo;
  try {
    const v = localStorage.getItem(KEY);
    if (v === "dark" || v === "light" || v === "system") return v;
  } catch {
    // storage unavailable: the default
  }
  return "dark";
}

let theme: Theme = load();

function apply() {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}
apply();

export function setTheme(next: Theme) {
  theme = next;
  try {
    localStorage.setItem(KEY, next);
  } catch {
    // not kept, still applied
  }
  apply();
  listeners.forEach((l) => l());
}

export function useTheme(): Theme {
  return useSyncExternalStore(
    (l) => (listeners.add(l), () => listeners.delete(l)),
    () => theme,
  );
}
