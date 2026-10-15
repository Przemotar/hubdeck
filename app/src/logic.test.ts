// Pure logic of the settings window, tested without Tauri: control ids, module placement, texts.
// Run with `pnpm test` in app/.

import { describe, expect, it } from "vitest";
import { controlId, hasState, hubRainbow, pressable, RAINBOW_MS } from "./engine";
import { clampToGrid, detachedKinds, keysOfKind, ownedKey, placeModules, rotatedSize } from "./layout";
import { localizeError, localizeGame, setLanguage, specialKeyName, t } from "./i18n";
import type { ModuleInfo } from "./deck";

const mod = (kind: ModuleInfo["kind"], slot: string | null): ModuleInfo => ({ id: `${kind}:x`, kind, serial: "x", firmware: null, slot });

describe("control ids (mirror profile::control_id and Gesture::suffix)", () => {
  it("adds the page only to keys and the gesture as a suffix", () => {
    expect(controlId("keys", 4)).toBe("keys.4");
    expect(controlId("keys", 4, "p1")).toBe("p1/keys.4");
    expect(controlId("dial", 0, "p1")).toBe("dial");
    expect(controlId("knobs", 1, "p1", "hold")).toBe("knobs.1:hold");
    expect(controlId("keys", 2, "p1", "double")).toBe("p1/keys.2:double");
  });

  it("knows which controls press and which actions have a state", () => {
    expect(pressable("keys") && pressable("dial") && pressable("knobs")).toBe(true);
    expect(pressable("faders") || pressable("rollers")).toBe(false);
    expect(hasState("core.mic")).toBe(true);
    expect(hasState("com.example.plugin/count")).toBe(true);
    expect(hasState("core.launch")).toBe(false);
  });
});

describe("module placement", () => {
  it("keeps a saved placement and fits the grid", () => {
    const placed = placeModules([mod("base", null), mod("keys", "2-3")], { "keys@2-3": { col: 0, row: 3, rotation: 0 } });
    expect(placed).toHaveLength(1);
    expect(placed[0]).toMatchObject({ col: 0, row: 3, rotation: 0, source: "user" });
  });

  it("turns footprints and clamps positions", () => {
    expect(rotatedSize("keys", 90)).toEqual([3, 4]);
    expect(rotatedSize("keys", 180)).toEqual([4, 3]);
    expect(clampToGrid(3, 5, 4, 3)).toEqual([0, 3]);
  });

  it("puts a module without a known slot somewhere free", () => {
    const placed = placeModules([mod("dial", null), mod("knobs", null)], {});
    expect(placed).toHaveLength(2);
    const [a, b] = placed;
    const overlap = a.col < b.col + b.cols && b.col < a.col + a.cols && a.row < b.row + b.rows && b.row < a.row + a.rows;
    expect(overlap).toBe(false);
  });
});

describe("texts", () => {
  it("uses English unless Polish is picked, with Polish plurals", () => {
    setLanguage("en");
    expect(t("topbar.modules", { count: 1 })).toBe("1 module");
    expect(t("topbar.modules", { count: 3 })).toBe("3 modules");
    setLanguage("pl");
    expect(t("topbar.modules", { count: 1 })).toBe("1 moduł");
    expect(t("topbar.modules", { count: 2 })).toBe("2 moduły");
    expect(t("topbar.modules", { count: 5 })).toBe("5 modułów");
    expect(t("topbar.modules", { count: 12 })).toBe("12 modułów");
    expect(t("topbar.modules", { count: 22 })).toBe("22 moduły");
    expect(localizeGame("mole", "name", "Mole")).toBe("Krety");
    expect(localizeGame("unknown", "name", "Unknown")).toBe("Unknown");
    setLanguage("en");
  });

  it("names the special keys for people, in both languages", () => {
    setLanguage("en");
    expect(specialKeyName("PlayPause")).toBe("Play/Pause");
    expect(specialKeyName("NumSubtract")).toBe("Num −");
    expect(specialKeyName("Num5")).toBe("Num 5");
    expect(specialKeyName("F13")).toBe("F13");
    setLanguage("pl");
    expect(specialKeyName("PrevTrack")).toBe("Poprzedni utwór");
    // no Polish of its own: the English name
    expect(specialKeyName("NumAdd")).toBe("Num +");
    setLanguage("en");
  });
});

describe("owned modules that are not attached", () => {
  const at = { col: 0, row: 0, rotation: 0 as const };
  it("lists remembered and owned kinds without an attached module, once per kind", () => {
    const saved = { "faders@1-1-3": at, "keys@2-3": at, "keys@1-1-1": at, [ownedKey("dial")]: at };
    expect(detachedKinds([mod("keys", "2-3")], saved)).toEqual(["faders", "dial"]);
    expect(detachedKinds([], saved)).toEqual(["keys", "faders", "dial"]);
    expect(keysOfKind(saved, "keys").sort()).toEqual(["keys@1-1-1", "keys@2-3"]);
    expect(ownedKey("rollers")).toBe("rollers@owned");
  });
});

describe("engine messages in Polish", () => {
  it("translates known messages and keeps the rest", () => {
    setLanguage("pl");
    expect(localizeError('cannot open "notes.txt": not found')).toBe("Nie można otworzyć „notes.txt”: not found");
    expect(localizeError("stopped (exit code: 1)")).toBe("zatrzymana (kod 1)");
    expect(localizeError("Install vJoy to use the deck as a joystick: https://x")).toBe("Zainstaluj vJoy, aby używać decka jako joysticka: https://x");
    expect(localizeError("something new")).toBe("something new");
    setLanguage("en");
    expect(localizeError("pick a game first")).toBe("pick a game first");
  });
});


describe("HubDeck's rainbow", () => {
  it("goes round with the clock like the engine's (leds::rainbow_rgb)", () => {
    expect(hubRainbow(0)).toBe("rgb(255, 0, 0)");
    expect(hubRainbow(RAINBOW_MS / 3)).toBe("rgb(0, 255, 0)");
    expect(hubRainbow(RAINBOW_MS)).toBe("rgb(255, 0, 0)");
  });
});

import { readable } from "./components/DeckView";

describe("captions of dimmed lights", () => {
  it("keep the hue at full brightness", () => {
    expect(readable("#0a1a33")).toBe("#5b9bff");
    const lifted = readable("#081a10")!;
    expect(parseInt(lifted.slice(1, 3), 16) + parseInt(lifted.slice(3, 5), 16) + parseInt(lifted.slice(5, 7), 16)).toBeGreaterThan(300);
    expect(readable("#05100a")).toBeUndefined(); // nearly off: the caption keeps its own light color
    expect(readable("#000000")).toBeUndefined();
    expect(readable(null)).toBeUndefined();
  });
});
