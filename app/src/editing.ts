import { useEffect, useRef } from "react";
import type { ControlRef } from "./deck";
import { actionName, t } from "./i18n";
import { actionFits, api, controlId, newBinding, type Binding, type EngineView, type Gesture, type Look, type Profile } from "./engine";

const GESTURES: Gesture[] = ["tap", "hold", "double"];
const HISTORY = 50;

/** What "Paste look" carries: colors, the LED and its effect, the key's style; not the label,
 * icon or picture (issue #10, killa8boll: "Copy Color" / "Paste Color"; owner's pick 1a). */
const LOOK_FIELDS = ["bg", "fg", "iconColor", "led", "ledLevel", "ledEffect", "ledSpeed", "style"] as const;

/** `to` with the look fields of `from` (a field `from` leaves unset is unset on the result too). */
export function withLook(to: Look, from: Look): Look {
  const out: Look = { ...to };
  for (const f of LOOK_FIELDS) {
    if (from[f] === undefined) delete out[f];
    else (out as Record<string, unknown>)[f] = from[f];
  }
  return out;
}

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

  // the key handler reads what is selected and shown at the moment of the key press: a handler
  // made at an earlier render copied the key selected before, or pasted over a change the window
  // had not heard of yet (8bitcanvascode, Discussions #3, 2026-10-19)
  const now = useRef({ view, selected, page, notify });
  now.current = { view, selected, page, notify };

  const gestureIds = (c: ControlRef, page: string | null) => Object.fromEntries(GESTURES.map((g) => [g, controlId(c.kind, c.index, page, g)])) as Record<Gesture, string>;

  /** Does the control do anything (press, hold or double press)? */
  const isSet = (c: ControlRef) => {
    const { view, page } = now.current;
    const ids = gestureIds(c, page);
    return !!view.profile && GESTURES.some((g) => view.profile?.controls[ids[g]]);
  };
  const canPaste = () => !!clipboard.current;

  /** Copy a control's press, hold and double press; false when it does nothing. */
  const copy = (c: ControlRef) => {
    const { view, page, notify } = now.current;
    const profile = view.profile;
    if (!profile) return false;
    const ids = gestureIds(c, page);
    const bindings: Partial<Record<Gesture, Binding>> = {};
    for (const g of GESTURES) if (profile.controls[ids[g]]) bindings[g] = profile.controls[ids[g]];
    if (!bindings.tap && !bindings.hold && !bindings.double) {
      notify(t("edit.empty"));
      return false;
    }
    clipboard.current = { bindings, from: c.kind };
    notify(t("edit.copied"));
    return true;
  };

  /** Paste what was copied; the parts that do not fit the control are left out. */
  const paste = (c: ControlRef) => {
    const { view, page, notify } = now.current;
    const profile = view.profile;
    if (!profile || !clipboard.current) return;
    const ids = gestureIds(c, page);
    const changes: Record<string, Binding | null> = {};
    let skipped = 0;
    for (const g of GESTURES) {
      const b = clipboard.current.bindings[g];
      const def = b && view.catalog?.actions.find((a) => a.id === b.action);
      // a press gesture needs a pressable control; the tap must fit the control kind
      if (!b || !def || (g === "tap" ? !actionFits(def, c.kind) : !(def.inputs.includes("press") && actionFits({ ...def, inputs: ["press"] }, c.kind)))) {
        if (b) skipped++;
        continue;
      }
      changes[ids[g]] = structuredClone(b);
    }
    api.setBindings(profile.id, changes).then(() => notify(skipped ? t("edit.pastedPartly") : t("edit.pasted")), (err) => notify(String(err)));
  };

  /** Paste only the look of what was copied onto the control's press: its action stays. */
  const pasteLook = (c: ControlRef) => {
    const { view, page, notify } = now.current;
    const profile = view.profile;
    const from = clipboard.current?.bindings.tap ?? clipboard.current?.bindings.hold ?? clipboard.current?.bindings.double;
    if (!profile || !from) return;
    const id = controlId(c.kind, c.index, page);
    const to = profile.controls[id];
    if (!to) return notify(t("edit.lookNeedsAction"));
    const next: Binding = { ...to, look: withLook(to.look ?? {}, from.look ?? {}) };
    // the "on" look follows only where both have one (a new one would lose the control's own red)
    if (to.lookOn && from.lookOn) next.lookOn = withLook(to.lookOn, from.lookOn);
    api.setBindings(profile.id, { [id]: next }).then(() => notify(t("edit.lookPasted")), (err) => notify(String(err)));
  };

  /** Clear a control's press, hold and double press (Ctrl+Z brings them back). */
  const clear = (c: ControlRef) => {
    const { view, page, notify } = now.current;
    const profile = view.profile;
    if (!profile || !isSet(c)) return;
    const changes: Record<string, Binding | null> = Object.fromEntries(GESTURES.map((g) => [gestureIds(c, page)[g], null]));
    api.setBindings(profile.id, changes).then(() => notify(t("edit.cleared")), (err) => notify(String(err)));
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const { view, selected, notify } = now.current;
      if (inField(e) || !view.profile) return;
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
      if (mod && key === "c") {
        if (isSet(selected)) e.preventDefault();
        copy(selected);
      } else if (mod && e.shiftKey && key === "v" && clipboard.current) {
        e.preventDefault();
        pasteLook(selected);
      } else if (mod && key === "v" && clipboard.current) {
        e.preventDefault();
        paste(selected);
      } else if ((key === "delete" || key === "backspace") && !mod) {
        if (!isSet(selected)) return;
        e.preventDefault();
        clear(selected);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** A key dragged onto another: its press, hold and double press go there, and that key's come
   * back in its place (a swap; a move when the other was empty). */
  const moveKey = (from: ControlRef, to: ControlRef) => {
    const { view, page, notify } = now.current;
    const profile = view.profile;
    if (!profile || from.kind !== to.kind) return;
    const changes: Record<string, Binding | null> = {};
    let swapped = false;
    for (const g of GESTURES) {
      const [a, b] = [controlId(from.kind, from.index, page, g), controlId(to.kind, to.index, page, g)];
      const [ba, bb] = [profile.controls[a], profile.controls[b]];
      if (!ba && !bb) continue;
      if (bb) swapped = true;
      changes[b] = ba ?? null;
      changes[a] = bb ?? null;
    }
    if (Object.keys(changes).length === 0) return;
    api.setBindings(profile.id, changes).then(() => notify(t(swapped ? "edit.swapped" : "edit.moved")), (err) => notify(String(err)));
  };

  /** A double click on a page key shows its page, here and on the deck. */
  const openControl = (c: ControlRef) => {
    const { view, page } = now.current;
    const profile = view.profile;
    const b = profile?.controls[controlId(c.kind, c.index, page)];
    if (!profile || !b) return;
    const all = [null, ...(profile.pages ?? []).map((p) => p.id)];
    const at = Math.max(0, all.indexOf(page));
    const to =
      b.action === "core.page" ? (typeof b.params?.page === "string" && b.params.page ? b.params.page : null)
      : b.action === "core.nextPage" ? all[(at + 1) % all.length]
      : b.action === "core.previousPage" ? all[(at + all.length - 1) % all.length]
      : undefined;
    if (to === undefined || to === page) return;
    api.setPage(to).catch((err) => now.current.notify(String(err)));
  };

  /** Is it a key that opens a page (a double click or Open page in its menu opens it)? */
  const opensPage = (c: ControlRef) => {
    const { view, page } = now.current;
    const a = view.profile?.controls[controlId(c.kind, c.index, page)]?.action;
    return a === "core.page" || a === "core.nextPage" || a === "core.previousPage";
  };

  return { dropAction, moveKey, openControl, copy, paste, pasteLook, clear, isSet, canPaste, opensPage };
}
