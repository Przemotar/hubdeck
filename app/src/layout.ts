import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ModuleInfo, ModuleKind } from "./deck";
import slotTable from "./slots.generated.json";

const SLOT_TABLE = slotTable as unknown as Record<string, Placement & { verified?: boolean }>;

export const GRID_COLS = 4;
export const GRID_ROWS = 6;

/** Footprint on the 4x6 base grid (columns, rows) with the base standing portrait, unrotated. */
export const FOOTPRINT: Record<ModuleKind, [number, number]> = {
  base: [4, 6],
  keys: [4, 3],
  faders: [4, 2],
  knobs: [2, 2],
  rollers: [4, 1],
  dial: [2, 2],
};

export type Rotation = 0 | 90 | 180 | 270;

export interface Placement {
  col: number;
  row: number;
  rotation: Rotation;
}

export interface Placed extends Placement {
  module: ModuleInfo;
  /** Footprint after rotation. */
  cols: number;
  rows: number;
  /** Unrotated footprint, for drawing the module before rotating it. */
  baseCols: number;
  baseRows: number;
  /** Where the position came from: the user's arrangement, the built-in slot table, or a guess. */
  source: "user" | "table" | "guess";
  known: boolean;
}

export const placementKey = (m: ModuleInfo) => (m.slot ? `${m.kind}@${m.slot}` : null);

export function rotatedSize(kind: ModuleKind, rotation: Rotation): [number, number] {
  const [c, r] = FOOTPRINT[kind];
  return rotation === 90 || rotation === 270 ? [r, c] : [c, r];
}

export function clampToGrid(col: number, row: number, cols: number, rows: number): [number, number] {
  return [Math.max(0, Math.min(GRID_COLS - cols, col)), Math.max(0, Math.min(GRID_ROWS - rows, row))];
}

const ORDER: ModuleKind[] = ["keys", "faders", "dial", "knobs", "rollers"];

/** The user's arrangement first, then the built-in slot table; the rest go to the first free spot. */
export function placeModules(modules: ModuleInfo[], saved: Record<string, Placement>): Placed[] {
  const attached = modules
    .filter((m) => m.kind !== "base")
    .sort((a, b) => ORDER.indexOf(a.kind) - ORDER.indexOf(b.kind));
  const used = Array.from({ length: GRID_ROWS }, () => Array<boolean>(GRID_COLS).fill(false));
  const mark = (p: { col: number; row: number; cols: number; rows: number }) => {
    for (let r = p.row; r < p.row + p.rows; r++) for (let c = p.col; c < p.col + p.cols; c++) if (used[r]?.[c] !== undefined) used[r][c] = true;
  };
  const free = (col: number, row: number, cols: number, rows: number) => {
    for (let r = row; r < row + rows; r++) for (let c = col; c < col + cols; c++) if (used[r]?.[c] !== false) return false;
    return true;
  };

  const placed: Placed[] = [];
  const pending: ModuleInfo[] = [];
  for (const m of attached) {
    const key = placementKey(m);
    const user = key ? saved[key] : undefined;
    const table = key ? SLOT_TABLE[key] : undefined;
    const p = user ?? table;
    if (!p) {
      pending.push(m);
      continue;
    }
    const rotation = p.rotation as Rotation;
    const [cols, rows] = rotatedSize(m.kind, rotation);
    const [baseCols, baseRows] = FOOTPRINT[m.kind];
    const source: Placed["source"] = user ? "user" : "table";
    const item = { module: m, col: p.col, row: p.row, rotation, cols, rows, baseCols, baseRows, source, known: true };
    mark(item);
    placed.push(item);
  }
  for (const m of pending) {
    const [cols, rows] = FOOTPRINT[m.kind];
    let spot: [number, number] = [0, 0];
    search: for (let row = 0; row <= GRID_ROWS - rows; row++) {
      for (let col = 0; col <= GRID_COLS - cols; col++) {
        if (free(col, row, cols, rows)) {
          spot = [col, row];
          break search;
        }
      }
    }
    const item = { module: m, col: spot[0], row: spot[1], rotation: 0 as Rotation, cols, rows, baseCols: cols, baseRows: rows, source: "guess" as const, known: false };
    mark(item);
    placed.push(item);
  }
  return placed;
}

/** Slot of a placement entry for a module the user owns but has not attached yet. */
export const OWNED_SLOT = "owned";
export const ownedKey = (kind: ModuleKind) => `${kind}@${OWNED_SLOT}`;
/** Kinds of module that can be attached to the base. */
export const MODULE_KINDS: ModuleKind[] = ORDER;
/** Module id of a remembered module that is not attached; its controls are edited like attached ones. */
export const detachedId = (kind: ModuleKind) => `detached:${kind}`;
export const isDetached = (moduleId: string) => moduleId.startsWith("detached:");

/**
 * Modules the user owns that are not attached now: every placement entry (a module seen on the
 * base before, or one added by hand with slot `owned`) whose kind has no attached module.
 * Profiles bind controls by kind, so one per kind is enough.
 */
export function detachedKinds(modules: ModuleInfo[], saved: Record<string, Placement>): ModuleKind[] {
  const attached = new Set(modules.map((m) => m.kind));
  const remembered = new Set(Object.keys(saved).map((k) => k.split("@")[0] as ModuleKind));
  return ORDER.filter((k) => remembered.has(k) && !attached.has(k));
}

/** A stand-in for a detached module, so it can be drawn and its controls selected. */
export const detachedModule = (kind: ModuleKind): ModuleInfo => ({ id: detachedId(kind), kind, serial: "", firmware: null, slot: null });

/** Placement keys of every entry of a kind (to forget a module the user no longer owns). */
export const keysOfKind = (saved: Record<string, Placement>, kind: ModuleKind) => Object.keys(saved).filter((k) => k.split("@")[0] === kind);

const inTauri = () => "__TAURI_INTERNALS__" in window;

/** Saved placements plus a setter that persists through the backend. */
export function usePlacements() {
  const [saved, setSaved] = useState<Record<string, Placement>>({});
  useEffect(() => {
    if (inTauri()) invoke<Record<string, Placement>>("placements_get").then(setSaved);
  }, []);
  const save = useCallback((key: string, placement: Placement | null) => {
    setSaved((prev) => {
      const next = { ...prev };
      if (placement) next[key] = placement;
      else delete next[key];
      return next;
    });
    if (inTauri()) invoke("placement_set", { key, placement }).catch((e) => console.error("placement_set", e));
  }, []);
  return { saved, save };
}
