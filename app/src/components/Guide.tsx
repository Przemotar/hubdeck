// Where things are (owner's pick 2a, 2026-10-14): after "Use as my home deck" on the tour's end
// page, five places in the window, one at a time: a dark veil with a cut-out around the place and
// a small card beside it (step n of 4, a title, one sentence, Skip and Next/Done). Esc or Skip
// ends it; it shows once.
import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { t, useLanguage, type Key } from "../i18n";

/** The places, in order: what to light up (the first one found) and what to say. */
const STEPS: { target: string; title: Key; text: Key }[] = [
  { target: ".workspace > aside.inspector", title: "guide.inspector.title", text: "guide.inspector.text" },
  { target: "[data-guide='newDeck']", title: "guide.newDeck.title", text: "guide.newDeck.text" },
  { target: "[data-guide='profiles']", title: "guide.profiles.title", text: "guide.profiles.text" },
  { target: "[data-guide='edge']", title: "guide.edge.title", text: "guide.edge.text" },
];

const SEEN = "hubdeck.guideSeen";

/** The guide was shown (or skipped) before: it does not come again. */
export function guideSeen(): boolean {
  try {
    return localStorage.getItem(SEEN) === "1";
  } catch {
    return false;
  }
}

function markSeen() {
  try {
    localStorage.setItem(SEEN, "1");
  } catch {
    // private window or blocked storage: it may show once more, nothing else
  }
}

type Box = { top: number; left: number; width: number; height: number };

const PAD = 8;
const CARD_W = 300;
const GAP = 14;

/** Where the card goes: beside the place where there is room (right, left, below, above), kept in the window. */
function cardAt(box: Box, cardH: number): { top: number; left: number } {
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(v, hi));
  const top = clamp(box.top, 12, vh - cardH - 12);
  if (box.left + box.width + GAP + CARD_W <= vw - 12) return { top, left: box.left + box.width + GAP };
  if (box.left - GAP - CARD_W >= 12) return { top, left: box.left - GAP - CARD_W };
  const left = clamp(box.left + box.width / 2 - CARD_W / 2, 12, vw - CARD_W - 12);
  if (box.top + box.height + GAP + cardH <= vh - 12) return { top: box.top + box.height + GAP, left };
  return { top: Math.max(12, box.top - GAP - cardH), left };
}

export function Guide({ onClose }: { onClose: () => void }) {
  useLanguage();
  const [at, setAt] = useState(0);
  const [box, setBox] = useState<Box | null>(null);
  const [cardH, setCardH] = useState(160);
  const card = useRef<HTMLDivElement>(null);
  const next = useRef<HTMLButtonElement>(null);
  const step = STEPS[at];

  const end = () => {
    markSeen();
    onClose();
  };

  // the place's box, again when the window changes size (the panels move)
  useLayoutEffect(() => {
    const measure = () => {
      const el = document.querySelector(step.target);
      const r = el?.getBoundingClientRect();
      setBox(r && r.width > 0 ? { top: r.top - PAD, left: r.left - PAD, width: r.width + 2 * PAD, height: r.height + 2 * PAD } : null);
      if (card.current) setCardH(card.current.offsetHeight);
    };
    measure();
    // the window may still be laying out the place (the inspector opens with the key)
    const timer = window.setTimeout(measure, 120);
    window.addEventListener("resize", measure);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("resize", measure);
    };
  }, [step.target]);

  useEffect(() => {
    next.current?.focus();
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        end();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [at]);

  const last = at === STEPS.length - 1;
  const pos = box ? cardAt(box, cardH) : { top: window.innerHeight / 2 - cardH / 2, left: window.innerWidth / 2 - CARD_W / 2 };
  return (
    <div className="guide" role="dialog" aria-modal="true" aria-labelledby="guide-title">
      {/* the veil is the cut-out's shadow; without a place the whole window is veiled */}
      {box ? <div className="guide-spot" style={box} /> : <div className="guide-veil" />}
      <div ref={card} className="guide-card" style={{ ...pos, width: CARD_W }}>
        <span className="guide-step">{t("guide.step", { n: at + 1, total: STEPS.length })}</span>
        <b id="guide-title">{t(step.title)}</b>
        <p>{t(step.text)}</p>
        <div className="row guide-actions">
          {!last && (
            <button className="tool ghost small" onClick={end}>
              {t("guide.skip")}
            </button>
          )}
          <span className="spacer" />
          <button ref={next} className="tool primary small" onClick={() => (last ? end() : setAt(at + 1))}>
            {last ? t("guide.done") : t("guide.next")}
          </button>
        </div>
      </div>
    </div>
  );
}
