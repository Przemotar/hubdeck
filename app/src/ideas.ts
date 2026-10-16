import type { Binding } from "./engine";

/**
 * Keys other deck owners use most outside streaming, ready to put on a key in one click (owner's
 * pick, 2026-10-16; `docs/PAIN-POINTS.md` RD8: "What is the most useful Stream Deck button you have
 * that has nothing to do with streaming?", r/elgato redd.it/1wh69jh, its 100 comments read
 * 2026-10-16, ordered by how many people named one). Per system where the keys differ.
 */
export interface Idea {
  id: string;
  /** Why people like it, shown as the tooltip. */
  why: string;
  binding: Binding;
  /** Put on the key's hold, not its press (shutting down by accident). */
  hold?: boolean;
}

const IS_MAC = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");

const key = (action: string, params: Record<string, unknown>, label: string, icon: string): Binding => ({ action, params, look: { label, icon } }) as Binding;
const hotkey = (win: string, mac: string, label: string, icon: string) => key("core.hotkey", { keys: IS_MAC ? mac : win }, label, icon);
const run = (win: [string, string], mac: [string, string], label: string, icon: string) => key("core.run", { command: IS_MAC ? mac[0] : win[0], args: IS_MAC ? mac[1] : win[1], window: "hidden" }, label, icon);

export const IDEAS: Idea[] = [
  { id: "audioSwap", why: "Headphones and speakers in one press: the most named key in the thread.", binding: key("core.audioDevice", { kind: "output", device: "" }, "Audio out", "headphones") },
  { id: "plainPaste", why: "Paste without the source's fonts and colors.", binding: hotkey("Ctrl+Shift+V", "Cmd+Alt+Shift+V", "Plain paste", "clipboard-paste") },
  { id: "dateTime", why: "Types today's date and the time where you write.", binding: key("core.text", { text: "{date} {time}" }, "Date", "calendar") },
  { id: "snip", why: "A screenshot of an area to the clipboard.", binding: hotkey("Win+Shift+S", "Cmd+Ctrl+Shift+4", "Snip", "scissors") },
  { id: "mute", why: "Your microphone on and off in any call app.", binding: key("core.mic", { mode: "toggle" }, "Mic", "mic-off") },
  { id: "lock", why: "Lock the computer when you step away.", binding: hotkey("Win+L", "Cmd+Ctrl+Q", "Lock", "lock") },
  { id: "desktop", why: "Every window out of the way (and back).", binding: hotkey("Win+D", "F11", "Desktop", "app-window") },
  { id: "screens", why: "Extend the desktop to a second screen, or show the same on both.", binding: run(["DisplaySwitch.exe", "/extend"], ["", ""], "Extend", "monitor") },
  { id: "cpuTemp", why: "The CPU temperature on the key, live.", binding: key("core.monitor", { metric: "cpuTemp", press: "open" }, "CPU", "thermometer") },
  { id: "gpuTemp", why: "The GPU temperature on the key, live.", binding: key("core.monitor", { metric: "gpuTemp", press: "open" }, "GPU", "cpu") },
  { id: "sleep", why: "Sleep, on a hold so it is never pressed by accident.", binding: run(["rundll32.exe", "powrprof.dll,SetSuspendState 0,1,0"], ["pmset", "sleepnow"], "Sleep", "moon"), hold: true },
  { id: "shutdown", why: "Shut down, on a hold so it is never pressed by accident.", binding: run(["shutdown.exe", "/s /t 0"], ["osascript", '-e "tell app \\"System Events\\" to shut down"'], "Shut down", "power"), hold: true },
  { id: "restart", why: "Restart, on a hold so it is never pressed by accident.", binding: run(["shutdown.exe", "/r /t 0"], ["osascript", '-e "tell app \\"System Events\\" to restart"'], "Restart", "rotate-ccw"), hold: true },
].filter((i) => !(IS_MAC && i.id === "screens"));

/** Polish: the key's label and why (the Polish window puts Polish words on the key). */
const PL: Record<string, [string, string]> = {
  audioSwap: ["Dźwięk", "Słuchawki i głośniki jednym naciśnięciem: najczęściej wymieniany klawisz w wątku."],
  plainPaste: ["Wklej czysto", "Wklejenie bez czcionek i kolorów źródła."],
  dateTime: ["Data", "Wpisuje dzisiejszą datę i godzinę tam, gdzie piszesz."],
  snip: ["Wycinek", "Zrzut zaznaczonego obszaru do schowka."],
  mute: ["Mikrofon", "Mikrofon wł./wył. w każdej aplikacji do rozmów."],
  lock: ["Zablokuj", "Blokuje komputer, gdy odchodzisz."],
  desktop: ["Pulpit", "Wszystkie okna na bok (i z powrotem)."],
  screens: ["Rozszerz", "Pulpit na drugi ekran albo to samo na obu."],
  cpuTemp: ["CPU", "Temperatura procesora na klawiszu, na żywo."],
  gpuTemp: ["GPU", "Temperatura karty graficznej na klawiszu, na żywo."],
  sleep: ["Uśpij", "Uśpienie, na przytrzymaniu, żeby nigdy nie nacisnąć przypadkiem."],
  shutdown: ["Wyłącz", "Wyłączenie, na przytrzymaniu, żeby nigdy nie nacisnąć przypadkiem."],
  restart: ["Restart", "Ponowne uruchomienie, na przytrzymaniu, żeby nigdy nie nacisnąć przypadkiem."],
};

/** The ideas in the window's language. */
export function ideasIn(lang: string): Idea[] {
  if (lang !== "pl") return IDEAS;
  return IDEAS.map((i) => (PL[i.id] ? { ...i, why: PL[i.id][1], binding: { ...i.binding, look: { ...i.binding.look, label: PL[i.id][0] } } } : i));
}
