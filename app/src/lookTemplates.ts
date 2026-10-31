import type { Look } from "./engine";

/**
 * Key looks of the icon editor (owner's "1 a", 2026-10-25: the editor in the key's Look panel with
 * templates as its first tab). A template is how a key looks; the key keeps its own label, icon,
 * picture and badge text.
 */
export const TEMPLATE_FIELDS = ["bg", "bg2", "angle", "fg", "iconColor", "style", "gloss", "labelEffect", "ring", "badgeColor", "pattern", "patternOpacity", "motion", "font", "skin"] as const;

export interface LookTemplate {
  name: string;
  look: Look;
}

const HOUSE_BG = "#1b1a20";
const HOUSE_LABEL = "#e6e4ee";

/** The built-in templates; their names are translated (`template.<id>`). */
export const BUILT_IN: { id: string; look: Look }[] = [
  { id: "neon", look: { style: "gradient", bg: "#6a3fd8", bg2: "#e0457b", angle: 135, gloss: "glass", fg: "#ffffff" } },
  { id: "forest", look: { style: "gradient", bg: "#0f766e", bg2: "#22c55e", angle: 160, fg: "#ffffff" } },
  { id: "ember", look: { style: "gradient", bg: "#f59e0b", bg2: "#c2410c", angle: 200, fg: "#ffffff" } },
  { id: "night", look: { style: "gradient", bg: "#1e3a8a", bg2: "#0b1020", angle: 180, gloss: "glass", fg: "#e6ebff" } },
  { id: "frame", look: { bg: HOUSE_BG, fg: HOUSE_LABEL, ring: "#f5c542" } },
  { id: "matte", look: { bg: "#3060c0", gloss: "matte", fg: "#ffffff" } },
];

/** `to` with the template's look: every template field set as in `template` or taken off. */
export function withTemplate(to: Look, template: Look): Look {
  const out: Look = { ...to };
  for (const f of TEMPLATE_FIELDS) {
    if (template[f] === undefined) delete out[f];
    else (out as Record<string, unknown>)[f] = template[f];
  }
  return out;
}

/** The template part of a look (to save it as the user's own). */
export function templateOf(look: Look): Look {
  const out: Look = {};
  for (const f of TEMPLATE_FIELDS) if (look[f] !== undefined) (out as Record<string, unknown>)[f] = look[f];
  return out;
}

/** Whether a key already looks like the template. */
export function looksLike(look: Look, template: Look): boolean {
  return TEMPLATE_FIELDS.every((f) => (look[f] ?? undefined) === (template[f] ?? undefined));
}

/** The background kind the editor shows: the old `glass` style is a flat key with the glass finish. */
export function backgroundKind(look: Look): "flat" | "gradient" | "outline" {
  return look.style === "gradient" ? "gradient" : look.style === "outline" ? "outline" : "flat";
}

export function finish(look: Look): "none" | "glass" | "matte" {
  return look.gloss ?? (look.style === "glass" ? "glass" : "none");
}

/** A look with another background kind; the old `glass` style becomes the glass finish. */
export function withKind(look: Look, kind: "flat" | "gradient" | "outline"): Look {
  const gloss = finish(look);
  return { ...look, style: kind === "flat" ? undefined : kind, gloss: gloss === "none" ? undefined : gloss };
}

export function withFinish(look: Look, f: "none" | "glass" | "matte"): Look {
  return { ...look, style: look.style === "glass" ? undefined : look.style, gloss: f === "none" ? undefined : f };
}
