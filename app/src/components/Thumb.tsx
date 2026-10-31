import { useMemo, useState } from "react";
import { Icon } from "./Icon";

/** A moving thumbnail (a still until the mouse rests on it, or when `moving`). */
export function Thumb({ id, moving }: { id: string; moving?: boolean }) {
  const [over, setOver] = useState(false);
  const [gone, setGone] = useState(false);
  const reduce = useMemo(() => matchMedia("(prefers-reduced-motion: reduce)").matches, []);
  const src = `/looks/${id}.${(over || moving) && !reduce ? "webp" : "jpg"}`;
  return (
    <span className="dl-thumb" onMouseEnter={() => setOver(true)} onMouseLeave={() => setOver(false)}>
      {!gone && <img src={src} alt="" width={240} height={137} onError={() => setGone(true)} />}
    </span>
  );
}

/** "None" (no look, no screensaver): the deck's 15 key windows as plain empty keys, so the choice reads as
 * "the keys as they are" (owner, 2026-10-30: "może nie jako czarne tło tylko jakoś lepiej to pokazać?"). */
export function NoneThumb({ label }: { label: string }) {
  // the word on it too: empty keys alone read as "black keys" (owner, 2026-10-30: "musi pokazywać
  // jednak brak albo off a nie czarne klawisze")
  return (
    <span className="dl-thumb is-none" aria-hidden="true">
      {Array.from({ length: 15 }, (_, i) => (
        <i key={i} />
      ))}
      <b>
        <Icon name="eye-off" />
        {label}
      </b>
    </span>
  );
}
