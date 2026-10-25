import { describe, expect, it } from "vitest";
import { withLook } from "./editing";
import type { Look } from "./engine";

describe("Paste look", () => {
  it("carries colors, the light and the style, keeps the label, icon and picture", () => {
    const to = { label: "Mute", icon: "mic", bg: "#000000", led: "#ff0000", image: "a.png" };
    const from: Look = { label: "Other", icon: "music", bg: "#123456", fg: "#ffffff", led: "#00ff00", ledEffect: "breathing", style: "glass" };
    expect(withLook(to, from)).toEqual({ label: "Mute", icon: "mic", image: "a.png", bg: "#123456", fg: "#ffffff", led: "#00ff00", ledEffect: "breathing", style: "glass" });
  });
  it("a color the copied control leaves unset is unset here too (its look, not a mix)", () => {
    expect(withLook({ bg: "#000000", iconColor: "#ff00ff" }, { fg: "#ffffff" })).toEqual({ fg: "#ffffff" });
  });
});
