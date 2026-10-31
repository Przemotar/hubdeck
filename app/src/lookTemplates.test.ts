import { describe, expect, it } from "vitest";
import { BUILT_IN, finish, looksLike, templateOf, withFinish, withKind, withTemplate } from "./lookTemplates";

describe("look templates", () => {
  const key = { label: "Lubię to", icon: "heart", badge: "3", labelPos: "top" as const, bg: "#111111", ring: "#ff0000" };

  it("a template changes how the key looks and keeps its label, icon, badge and label place", () => {
    const neon = BUILT_IN.find((b) => b.id === "neon")!.look;
    const out = withTemplate(key, neon);
    expect(out).toMatchObject({ label: "Lubię to", icon: "heart", badge: "3", labelPos: "top", style: "gradient", bg2: "#e0457b", gloss: "glass" });
    // what the template leaves unset goes: no frame from the key before
    expect(out.ring).toBeUndefined();
    expect(looksLike(out, neon)).toBe(true);
    expect(looksLike(key, neon)).toBe(false);
  });

  it("a saved template keeps only the look", () => {
    expect(templateOf({ ...key, style: "gradient", angle: 90 })).toEqual({ bg: "#111111", ring: "#ff0000", style: "gradient", angle: 90 });
  });

  it("the old glass style reads as a flat key with the glass finish and stays glass on a change of kind", () => {
    const old = { bg: "#3060c0", style: "glass" as const };
    expect(finish(old)).toBe("glass");
    expect(withKind(old, "gradient")).toMatchObject({ style: "gradient", gloss: "glass" });
    expect(withKind(old, "flat")).toMatchObject({ style: undefined, gloss: "glass" });
    expect(withFinish(old, "matte")).toMatchObject({ style: undefined, gloss: "matte" });
    expect(withFinish({ style: "gradient" as const, gloss: "glass" as const }, "none")).toMatchObject({ style: "gradient", gloss: undefined });
  });
});
