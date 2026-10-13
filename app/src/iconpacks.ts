import { invoke } from "@tauri-apps/api/core";

/** An installed icon pack; mirrors `PackInfo` in src-tauri/src/engine/iconpacks.rs. */
export interface PackInfo {
  id: string;
  name: string;
  author: string;
  count: number;
}

/** One icon of a pack, with a small data URI to show it. */
export interface PackIcon {
  file: string;
  name: string;
  tags: string[];
  uri: string;
}

export const iconPacks = {
  list: () => invoke<PackInfo[]>("iconpacks_list"),
  icons: (id: string) => invoke<PackIcon[]>("iconpack_icons", { id }),
  /** Copy a pack icon into the pictures; returns the picture name for a look. */
  use: (pack: string, file: string) => invoke<string>("iconpack_use", { pack, file }),
  /** Install a pack from a folder, a zip or a Stream Deck `.streamDeckIconPack`. */
  install: (path: string) => invoke<PackInfo>("iconpack_import", { path }),
  remove: (id: string) => invoke("iconpack_remove", { id }),
};
