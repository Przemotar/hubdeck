import { useEffect, useRef } from "react";
import { useQuestion } from "../confirm";
import { t } from "../i18n";

/** The question of `confirm.ts` over the window: Enter answers the red button, Escape cancels. */
export function ConfirmHost() {
  const q = useQuestion();
  const okRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!q) return;
    okRef.current?.focus();
    const on = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        q.answer(false);
      }
    };
    // before the sheet's own Escape, which would close the sheet under the question
    window.addEventListener("keydown", on, true);
    return () => window.removeEventListener("keydown", on, true);
  }, [q]);
  if (!q) return null;
  return (
    <div className="sheet-backdrop confirm-backdrop" onMouseDown={(e) => e.target === e.currentTarget && q.answer(false)}>
      <div className="confirm" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby={q.body ? "confirm-body" : undefined}>
        <h2 id="confirm-title">{q.title}</h2>
        {q.body && <p id="confirm-body">{q.body}</p>}
        {q.details && q.details.length > 0 && (
          <ul className="confirm-details">
            {q.details.map((d) => (
              <li key={d}>{d}</li>
            ))}
          </ul>
        )}
        <div className="confirm-foot">
          <button type="button" className="tool ghost" onClick={() => q.answer(false)}>
            {q.cancel ?? t("confirm.cancel")}
          </button>
          <button type="button" ref={okRef} className={`tool${q.danger ? " is-danger" : " primary"}`} onClick={() => q.answer(true)}>
            {q.ok}
          </button>
        </div>
      </div>
    </div>
  );
}
