// Pure logic of the settings window, tested without Tauri: control ids, module placement, texts.
// Run with `pnpm test` in app/.

import { describe, expect, it } from "vitest";
import { controlId, hasState, hubRainbow, pageReachable, pressable, RAINBOW_MS } from "./engine";
import { clampToGrid, detachedKinds, keysOfKind, ownedKey, placeModules, rotatedSize } from "./layout";
import { actionName, BUILT_IN_LANGUAGES, languageTemplate, localizeAction, localizeError, localizeGame, localizePreset, setLanguage, specialKeyName, t } from "./i18n";
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
    // a module may hang over the base's edge but keeps a cell on it
    expect(clampToGrid(3, 5, 4, 3)).toEqual([3, 5]);
    expect(clampToGrid(-9, 9, 4, 3)).toEqual([-3, 5]);
    expect(clampToGrid(0, -2, 3, 4)).toEqual([0, -2]);
  });

  it("puts a module without a known slot somewhere free", () => {
    const placed = placeModules([mod("dial", null), mod("knobs", null)], {});
    expect(placed).toHaveLength(2);
    const [a, b] = placed;
    const overlap = a.col < b.col + b.cols && b.col < a.col + a.cols && a.row < b.row + b.rows && b.row < a.row + a.rows;
    expect(overlap).toBe(false);
  });

  it("places a module in a connector the official app leaves out, hanging over the edge (#4)", () => {
    // keys in 1-1-1 next to faders, knobs and the dial, as on Zendeman's deck: no guess on top of the faders
    const placed = placeModules([mod("keys", "1-1-1"), mod("faders", "2-3"), mod("knobs", "1-3-4"), mod("dial", "1-4-1")], {});
    const keys = placed.find((p) => p.module.kind === "keys")!;
    expect([keys.col, keys.row, keys.rotation, keys.source]).toEqual([2, 0, 90, "table"]);
    expect(keys.col + keys.cols).toBe(5);
    // every module in every connector has a place
    const kinds: ModuleInfo["kind"][] = ["keys", "faders", "knobs", "dial", "rollers"];
    const slots = ["1-1-1", "1-1-2", "1-1-3", "1-1-4", "1-2-1", "1-2-2", "1-2-3", "1-2-4", "1-3-1", "1-3-2", "1-3-3", "1-3-4", "1-4-1", "1-4-2", "1-4-3", "2-1", "2-2", "2-3", "2-4", "3-1", "3-2", "3-3", "4-1", "4-2", "4-3", "4-4"];
    for (const kind of kinds) for (const slot of slots) expect(placeModules([mod(kind, slot)], {})[0].source, `${kind}@${slot}`).toBe("table");
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

  it("uses a language file of the user's own, English for what it leaves out (Discussions #3)", () => {
    const ja = {
      name: "日本語",
      strings: { "topbar.settings": "設定", "topbar.modules": { other: "{count} 個のモジュール" }, "topbar.plugins": 42 },
      actions: { "core.nextPage": { name: "次のページ" } },
      groups: { Media: "メディア" },
      presets: { cs2: { name: "CS2 日本語" } },
      games: { mole: { name: "モグラ" } },
      keys: { PlayPause: "再生/一時停止" },
    };
    setLanguage("ja", ja);
    expect(t("topbar.settings")).toBe("設定");
    expect(t("topbar.modules", { count: 1 })).toBe("1 個のモジュール");
    expect(t("topbar.plugins")).toBe("Plugins");
    expect(t("topbar.calibrate")).toBe("Calibrate keys");
    expect(actionName({ id: "core.nextPage", name: "Next page" })).toBe("次のページ");
    expect(actionName({ id: "core.previousPage", name: "Previous page" })).toBe("Previous page");
    expect(localizeAction({ id: "x", name: "X", group: "Media", description: "d", inputs: [], params: [], icon: "" }).group).toBe("メディア");
    expect(localizePreset({ id: "cs2", name: "CS2", description: "e" } as never).name).toBe("CS2 日本語");
    expect(localizeGame("mole", "name", "Mole")).toBe("モグラ");
    expect(specialKeyName("PlayPause")).toBe("再生/一時停止");
    expect(localizeError("this plugin is encrypted for Elgato's Stream Deck app; other apps cannot read it")).toContain("encrypted");
    setLanguage("ja");
    expect(t("topbar.settings")).toBe("Settings");
    setLanguage("en");
  });

  it("has Japanese for every English text, with the same {words}", async () => {
    const ja = await BUILT_IN_LANGUAGES.ja.load();
    const en = languageTemplate([], [], []).strings as Record<string, unknown>;
    const text = (v: unknown) => (typeof v === "string" ? v : ((v as { other?: string })?.other ?? ""));
    const words = (v: unknown) => [...text(v).matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",");
    const missing = Object.keys(en).filter((k) => !(k in ja.strings));
    expect(missing, "add these to app/src/locales/ja.json").toEqual([]);
    const differ = Object.keys(en).filter((k) => words(en[k]) !== words(ja.strings[k]));
    expect(differ).toEqual([]);
    setLanguage("ja", ja);
    expect(t("topbar.settings")).toBe("設定");
    expect(t("topbar.modules", { count: 3 })).toBe("3 個のモジュール");
    setLanguage("en");
  });

  it("makes a language file with every English text", () => {
    const file = languageTemplate([{ id: "core.nextPage", name: "Next page", group: "HubDeck", description: "d", inputs: [], params: [{ key: "k", label: "L", type: "select", options: [["a", "A"]] } as never], icon: "" }], [], [{ id: "mole", name: "Mole", about: "a" }]);
    expect(file.strings["topbar.settings"]).toBe("Settings");
    expect(file.actions?.["core.nextPage"]).toEqual({ name: "Next page", description: "d", params: { k: { label: "L", options: { a: "A" } } } });
    expect(file.groups).toEqual({ HubDeck: "HubDeck" });
    expect(file.games?.mole?.name).toBe("Mole");
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

describe("a page nothing on the deck opens (owner's pick 1a, 2026-10-16)", () => {
  const b = (action: string, params: Record<string, unknown> = {}) => ({ action, params });
  const pages = [{ id: "p1" }, { id: "p2" }];
  it("counts Next on the page before, Previous on the one after and Go to page anywhere", () => {
    expect(pageReachable({ pages, controls: {} }, null)).toBe(true);
    expect(pageReachable({ pages, controls: {} }, "p1")).toBe(false);
    expect(pageReachable({ pages, controls: { "keys.14": b("core.nextPage") } }, "p1")).toBe(true);
    expect(pageReachable({ pages, controls: { "keys.14": b("core.nextPage") } }, "p2")).toBe(false);
    expect(pageReachable({ pages, controls: { "p2/keys.10": b("core.previousPage") } }, "p1")).toBe(true);
    expect(pageReachable({ pages, controls: { "p1/keys.3": b("core.page", { page: "p2" }) } }, "p2")).toBe(true);
  });
});

describe("questions before something that cannot be undone", () => {
  // window.confirm in HubDeck's window returns a promise an `if` takes as yes (a profile was
  // deleted without a question, 2026-10-18): confirmed() from confirm.ts waits for the answer
  it("never uses window.confirm", () => {
    const sources = import.meta.glob(["./**/*.{ts,tsx}", "!./demo/**", "!./**/*.test.ts"], { query: "?raw", import: "default", eager: true }) as Record<string, string>;
    expect(Object.keys(sources).length).toBeGreaterThan(10);
    expect(Object.entries(sources).filter(([, text]) => text.includes("window.confirm(")).map(([file]) => file)).toEqual([]);
  });
});
