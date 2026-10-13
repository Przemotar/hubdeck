import { useEffect, useRef } from "react";
import type { ControlRef } from "./deck";
import { actionName, t } from "./i18n";
import { actionFits, api, controlId, newBinding, type Binding, type EngineView, type Gesture, type Profile } from "./engine";

const GESTURES: Gesture[] = ["tap", "hold", "double"];
const HISTORY = 50;

/** Typing in a field keeps the browser's own copy, paste and undo. */
const inField = (e: KeyboardEvent) => {
  const t = e.target as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
};

/**
 * Editing comfort for the deck view: an action dropped on a control, Ctrl+C / Ctrl+V of a
 * control's bindings (press, hold and double press), Delete to clear it, Ctrl+Z to undo the last
 * change to the profile.
 */
export function useEditing(view: EngineView, selected: ControlRef | null, notify: (text: string) => void) {
  const clipboard = useRef<{ bindings: Partial<Record<Gesture, Binding>>; from: string } | null>(null);
  const history = useRef<Profile[]>([]);
  const last = useRef<Profile | null>(null);
  const undoing = useRef(false);

  // every new version of the same profile pushes the one before it, unless it came from an undo
  useEffect(() => {
    const p = view.profile;
    if (p && last.current && last.current.id === p.id && JSON.stringify(last.current) !== JSON.stringify(p)) {
      if (!undoing.current) history.current = [...history.current, last.current].slice(-HISTORY);
      undoing.current = false;
    }
    if (p && last.current?.id !== p.id) history.current = [];
    last.current = p;
  }, [view.profile]);

  const page = view.state?.page ?? null;

  const dropAction = (c: ControlRef, actionId: string) => {
    const def = view.catalog?.actions.find((a) => a.id === actionId);
    if (!def || !view.profile) return;
    if (!actionFits(def, c.kind)) return notify(t("edit.notOn", { action: actionName(def), control: t(`on.${c.kind}`) }));
    api.setBinding(view.profile.id, controlId(c.kind, c.index, page), newBinding(def)).catch((e) => notify(String(e)));
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (inField(e) || !view.profile) return;
      const profile = view.profile;
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      if (mod && key === "z") {
        const prev = history.current.pop();
        if (!prev) return notify(t("edit.nothingToUndo"));
        e.preventDefault();
        undoing.current = true;
        api.saveProfile(prev).then(() => notify(t("edit.undone")), (err) => notify(String(err)));
        return;
      }
      if (!selected) return;
      const ids = Object.fromEntries(GESTURES.map((g) => [g, controlId(selected.kind, selected.index, page, g)])) as Record<Gesture, string>;
      if (mod && key === "c") {
        const bindings: Partial<Record<Gesture, Binding>> = {};
        for (const g of GESTURES) if (profile.controls[ids[g]]) bindings[g] = profile.controls[ids[g]];
        if (!bindings.tap && !bindings.hold && !bindings.double) return notify(t("edit.empty"));
        e.preventDefault();
        clipboard.current = { bindings, from: selected.kind };
        notify(t("edit.copied"));
      } else if (mod && key === "v" && clipboard.current) {
        e.preventDefault();
        const controls = { ...profile.controls };
        let skipped = 0;
        for (const g of GESTURES) {
          const b = clipboard.current.bindings[g];
          const def = b && view.catalog?.actions.find((a) => a.id === b.action);
          // a press gesture needs a pressable control; the tap must fit the control kind
          if (!b || !def || (g === "tap" ? !actionFits(def, selected.kind) : !(def.inputs.includes("press") && actionFits({ ...def, inputs: ["press"] }, selected.kind)))) {
            if (b) skipped++;
            continue;
          }
          controls[ids[g]] = structuredClone(b);
        }
        api.saveProfile({ ...profile, controls }).then(() => notify(skipped ? t("edit.pastedPartly") : t("edit.pasted")), (err) => notify(String(err)));
      } else if ((key === "delete" || key === "backspace") && !mod) {
        if (!GESTURES.some((g) => profile.controls[ids[g]])) return;
        e.preventDefault();
        const controls = { ...profile.controls };
        for (const g of GESTURES) delete controls[ids[g]];
        api.saveProfile({ ...profile, controls }).then(() => notify(t("edit.cleared")), (err) => notify(String(err)));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view.profile, view.catalog, selected, page, notify]);

  return { dropAction };
}
