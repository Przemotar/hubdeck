import { invoke } from "@tauri-apps/api/core";
import type { ModuleInfo } from "./deck";
import { t } from "./i18n";

/**
 * Identify mode: while arranging, every control is marked the same way on the hardware and in the
 * drawing, so the user can match orientation. Colors differ in lightness, not only hue.
 */
export const KNOB_COLORS = ["#ff7a1a", "#2f8fff", "#f2f2f2"];
export const FADER_COLORS = ["#ff7a1a", "#2f8fff", "#f2f2f2", "#ff3fb4", "#ffd400"];

const hexToRgb = (hex: string): [number, number, number] => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as [number, number, number];

async function jpegOf(canvas: HTMLCanvasElement): Promise<number[]> {
  const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("toBlob failed"))), "image/jpeg", 0.9));
  return Array.from(new Uint8Array(await blob.arrayBuffer()));
}

function canvas(size: number) {
  const c = document.createElement("canvas");
  c.width = c.height = size;
  const g = c.getContext("2d")!;
  g.fillStyle = "#000";
  g.fillRect(0, 0, size, size);
  return { c, g };
}

/** Key tile: big number; a notch at the top edge marks the module's own "up". */
function keyTile(n: number) {
  const { c, g } = canvas(72);
  g.fillStyle = "#2a2733";
  g.fillRect(3, 3, 66, 66);
  g.fillStyle = "#c4b5fd";
  g.fillRect(28, 3, 16, 5);
  g.fillStyle = "#ffffff";
  g.font = "600 34px 'IBM Plex Sans', sans-serif";
  g.textAlign = "center";
  g.textBaseline = "middle";
  g.fillText(String(n), 36, 40);
  return c;
}

/** Dial screen: an arrow pointing to the module's own "up". */
function dialArrow() {
  const { c, g } = canvas(240);
  g.fillStyle = "#c4b5fd";
  g.beginPath();
  g.moveTo(120, 30);
  g.lineTo(170, 100);
  g.lineTo(138, 100);
  g.lineTo(138, 170);
  g.lineTo(102, 170);
  g.lineTo(102, 100);
  g.lineTo(70, 100);
  g.closePath();
  g.fill();
  g.fillStyle = "#ffffff";
  g.font = "600 26px 'IBM Plex Sans', sans-serif";
  g.textAlign = "center";
  g.fillText(t("deck.top"), 120, 210);
  return c;
}

const inTauri = () => "__TAURI_INTERNALS__" in window;

export async function identifyModule(m: ModuleInfo) {
  if (!inTauri()) return;
  switch (m.kind) {
    case "keys":
      for (let i = 0; i < 15; i++) {
        await invoke("draw_jpeg", { module: m.id, screen: "key", index: i, jpeg: await jpegOf(keyTile(i + 1)) });
      }
      break;
    case "dial":
      await invoke("draw_jpeg", { module: m.id, screen: "dial", index: 0, jpeg: await jpegOf(dialArrow()) });
      break;
    case "knobs":
      await invoke("set_leds", { module: m.id, colors: KNOB_COLORS.map(hexToRgb) });
      break;
    case "faders":
      await invoke("set_leds", { module: m.id, colors: FADER_COLORS.map(hexToRgb) });
      break;
  }
}

export async function clearIdentify(m: ModuleInfo) {
  if (!inTauri()) return;
  switch (m.kind) {
    case "keys":
    case "dial":
      await invoke("fill_screen", { module: m.id, rgb: [0, 0, 0] });
      break;
    case "knobs":
    case "faders":
      await invoke("set_leds", { module: m.id, colors: null });
      break;
  }
}
