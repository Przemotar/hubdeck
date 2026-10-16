import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";

/** Mirrors `UpdateInfo` in src-tauri/src/update.rs. */
export interface UpdateInfo {
  version: string;
  notes: string;
  date: string | null;
}

const inTauri = () => "__TAURI_INTERNALS__" in window;

export const updates = {
  /** (updates set up in this build, check at start) */
  settings: () => invoke<[boolean, boolean]>("update_settings_get"),
  setCheckAtStart: (checkAtStart: boolean) => invoke("update_settings_set", { checkAtStart }),
  check: () => invoke<UpdateInfo | null>("update_check"),
  /** Downloads, installs and restarts; only on the user's request. */
  install: () => invoke("update_install"),
  /** The version before this one, to go back to (None: there is none). */
  previous: () => invoke<UpdateInfo | null>("update_previous"),
  rollback: (version: string) => invoke("update_rollback", { version }),
};

/** A newer version found by the check at start (`update://available`); dismissable. */
export function useUpdateAvailable(): [UpdateInfo | null, () => void] {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  useEffect(() => {
    if (!inTauri()) return;
    let un: (() => void) | undefined;
    let cancelled = false;
    // found before the window opened (it is made only when opened): ask for it
    invoke<UpdateInfo | null>("update_available").then((i) => !cancelled && i && setInfo(i), () => {});
    listen<UpdateInfo>("update://available", (e) => setInfo(e.payload)).then((u) => (cancelled ? u() : (un = u)));
    return () => {
      cancelled = true;
      un?.();
    };
  }, []);
  return [info, () => setInfo(null)];
}

/** The app's version, e.g. "0.1.0". */
export function useAppVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    if (inTauri()) getVersion().then(setVersion, () => {});
  }, []);
  return version;
}

/** "Windows", "macOS" or "Linux", for bug reports. */
export const osName = () => {
  const ua = navigator.userAgent;
  return ua.includes("Windows") ? "Windows" : ua.includes("Mac") ? "macOS" : ua.includes("Linux") ? "Linux" : "unknown system";
};
