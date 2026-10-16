import { invoke } from "@tauri-apps/api/core";
import type { Imported } from "./engine";

/** One profile in the gallery's index.json; mirrors `GalleryEntry` in src-tauri/src/engine/share.rs. */
export interface GalleryEntry {
  id: string;
  name: string;
  author: string;
  description: string;
  /** Module kinds it uses. */
  kinds: string[];
  tags: string[];
  preview: string | null;
  /** URL of the .hubdeck package. */
  package: string;
  version: string | null;
}

export const share = {
  /** The gallery's profiles; rejects when it cannot be reached. */
  gallery: () => invoke<GalleryEntry[]>("gallery_list"),
  /** Download a package from the gallery and add it as a new profile. */
  add: (pkg: string) => invoke<Imported>("gallery_add", { package: pkg }),
  /** Write the diagnostics file and show it (as the tray item does). */
  saveDiagnostics: () => invoke("diagnostics_save"),
  diagnosticsBrief: () => invoke<string>("diagnostics_brief"),
};

/** Extension of a profile package (profile and its pictures in one file). */
export const PACKAGE_EXT = "hubdeck";
