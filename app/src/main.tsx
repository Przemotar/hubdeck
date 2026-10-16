import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

// the page's start and its errors go to HubDeck's log: the release window has no inspector
// (2026-10-15: a black window on macOS left no trace)
if ("__TAURI_INTERNALS__" in window) {
  const send = (error: boolean, message: string) => invoke("window_log", { error, message }).catch(() => {});
  window.addEventListener("error", (e) => send(true, `${e.message} at ${e.filename}:${e.lineno}:${e.colno} ${e.error?.stack ?? ""}`));
  window.addEventListener("unhandledrejection", (e) => send(true, `unhandled rejection: ${e.reason?.stack ?? e.reason}`));
  // how smoothly the window keeps up while the deck moves: every 5 s with input or with the
  // deck's pictures coming in (the screensaver, GIFs: the owner's slow window of 2026-10-16 had no
  // input, so it logged nothing), the deck events and pictures that came, the frames WebKit drew,
  // and the slowest frame (owner, 2026-10-15: faders lagged)
  const perf = { events: 0, mirror: 0, frames: 0, slow: 0, worst: 0, last: performance.now(), since: performance.now() };
  const probe = window as unknown as { __deckEvent: () => void; __mirrorFrame: () => void };
  probe.__deckEvent = () => perf.events++;
  probe.__mirrorFrame = () => perf.mirror++;
  const frame = (t: number) => {
    const gap = t - perf.last;
    perf.last = t;
    // a gap over a second is a hidden or idle window, not a slow frame
    if (perf.events + perf.mirror > 0 && gap < 1000) {
      perf.frames++;
      if (gap > 50) perf.slow++;
      perf.worst = Math.max(perf.worst, gap);
    }
    if (t - perf.since >= 5000) {
      if (perf.events + perf.mirror > 0) {
        const s = (t - perf.since) / 1000;
        send(false, `smoothness: deck events ${(perf.events / s).toFixed(1)}/s, deck pictures ${(perf.mirror / s).toFixed(1)}/s, frames ${(perf.frames / s).toFixed(1)}/s, ${perf.slow} over 50 ms, slowest ${perf.worst.toFixed(0)} ms`);
      }
      Object.assign(perf, { events: 0, mirror: 0, frames: 0, slow: 0, worst: 0, since: t });
    }
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  send(false, `page started (${navigator.userAgent.match(/AppleWebKit\/[\d.]+|Edg\/[\d.]+/)?.[0] ?? "?"})`);
}

/** A drawing error shows a way back instead of an empty window (a black window on macOS,
 * 2026-10-15); the error itself reaches the log through the window's error handler. */
class Recover extends React.Component<{ children: React.ReactNode }, { error: string | null }> {
  state = { error: null as string | null };
  static getDerivedStateFromError(e: unknown) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
  componentDidCatch(e: unknown) {
    window.dispatchEvent(new ErrorEvent("error", { message: `drawing failed: ${e instanceof Error ? `${e.message} ${e.stack ?? ""}` : String(e)}` }));
  }
  render() {
    if (this.state.error === null) return this.props.children;
    return (
      <div style={{ padding: 32, fontFamily: "system-ui, sans-serif", color: "#e8e6f0", background: "#0c0b10", height: "100vh", boxSizing: "border-box" }}>
        <h2 style={{ marginTop: 0 }}>Something in the window went wrong</h2>
        <p style={{ opacity: 0.7 }}>The deck keeps working. The error is in HubDeck's log.</p>
        <p style={{ opacity: 0.5, fontSize: 12 }}>{this.state.error}</p>
        <button onClick={() => location.reload()} style={{ padding: "8px 16px", borderRadius: 8, border: 0, background: "#8b5cf6", color: "#fff", cursor: "pointer" }}>
          Reload the window
        </button>
      </div>
    );
  }
}

const render = () =>
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <Recover>
        <App />
      </Recover>
    </React.StrictMode>,
  );

// `?demo` in a plain browser: a mocked engine with sample modules and profiles (src/demo)
if (!("__TAURI_INTERNALS__" in window) && new URLSearchParams(location.search).has("demo")) {
  import("./demo").then((demo) => {
    demo.installDemo();
    render();
  });
} else {
  render();
}
