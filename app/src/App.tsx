import { ConfirmHost } from "./components/ConfirmHost";
import { startTransition, useCallback, useEffect, useRef, useState } from "react";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "./styles.css";
import "./theme";
import { useDeck, type ControlRef } from "./deck";
import type { LeftTab } from "./components/Panels";
import { api, saverEnd, triedPresets, useEngine, useSaverShown } from "./engine";
import { DeckView } from "./components/DeckView";
import { Inspector } from "./components/Inspector";
import type { SettingsSection } from "./components/Panels";
import { PluginsTab } from "./components/PluginsTab";
import { ArrangePanel, Banner, ControlMenu, BaseTurnPicker, EdgePanel, ShareArrangement, ForAppPanel, GalleryPanel, GamesPanel, LeftPanel, PageBar, PluginFileSheet, ProfilePanel, SettingsPanel, Sheet, TopBar } from "./components/Panels";
import { listen } from "@tauri-apps/api/event";
import { Builder } from "./components/Builder";
import { Tour } from "./components/Tour";
import { Guide, guideSeen } from "./components/Guide";
import { askAssistant, useSetUp } from "./components/Assistant";
import { Icon } from "./components/Icon";
import { CalibrationPanel } from "./components/Calibration";
import { detachedKinds, placeModules, placementKey, usePlacements } from "./layout";
import { clearIdentify, identifyModule } from "./identify";
import { useEditing } from "./editing";
import { BUILT_IN_LANGUAGES, setLanguage, storedLanguageFile, t, useLanguage, type Key } from "./i18n";
import { demoParam } from "./demo/params";

// `?demo&panel=profile|settings|plugins|calibrate` opens a side panel at start (screenshots)
const DEMO_PANEL = demoParam("panel");
// `&select=keys.6` (or `dial`, `knobs.1`...) selects that control at start
const DEMO_SELECT = demoParam("select");

export default function App() {
  const { lang } = useLanguage();
  // a language file picked last time: read it once (the window is in English until then)
  useEffect(() => {
    const code = storedLanguageFile();
    if (code && BUILT_IN_LANGUAGES[code]) BUILT_IN_LANGUAGES[code].load().then((file) => setLanguage(code, file), () => {});
    else if (code && "__TAURI_INTERNALS__" in window) api.languageFile(code).then((file) => setLanguage(code, file), () => {});
  }, []);
  // the deck shows some words itself (the dial gallery): in the window's language
  useEffect(() => {
    if ("__TAURI_INTERNALS__" in window) api.languageSet(lang).catch(() => {});
  }, [lang]);
  const live = useDeck();
  const view = useEngine();
  const saverOn = !!useSaverShown();
  // less movement: the setting, taken from the system the first time
  const reducedMotion = view.state?.reducedMotion;
  useEffect(() => {
    if (!view.state || !("__TAURI_INTERNALS__" in window)) return;
    if (reducedMotion == null && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) api.reducedMotionSet(true).catch(() => {});
    document.documentElement.classList.toggle("reduce-motion", reducedMotion === true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reducedMotion, !!view.state]);
  const [selected, setSelected] = useState<ControlRef | null>(null);
  const [arranging, setArranging] = useState(false);
  const [profileOpen, setProfileOpen] = useState(DEMO_PANEL === "profile");
  const [calibrating, setCalibrating] = useState(DEMO_PANEL === "calibrate");
  const [gamesOpen, setGamesOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(DEMO_PANEL === "settings" || DEMO_PANEL === "plugins");
  const [settingsSection, setSettingsSection] = useState<SettingsSection>(DEMO_PANEL === "plugins" ? "plugins" : "general");
  const [pluginsOpen, setPluginsOpen] = useState(DEMO_PANEL === "catalog");
  // "Use it" in the Plugins tab: the deck, with the library open at that plugin's actions
  const [focusPlugin, setFocusPlugin] = useState<{ id: string; at: number } | null>(null);
  const [galleryOpen, setGalleryOpen] = useState(DEMO_PANEL === "gallery");
  const [newRequest, setNewRequest] = useState(0);
  // Get started is open when the app opens (owner, 2026-10-14); once set up, Profiles is
  // (docs/FLOWS.md difference 3), unless the user picked a tab already
  const [leftTab, setLeftTab] = useState<LeftTab>(DEMO_PANEL === "profiles" ? "profiles" : "assistant");
  const setUp = useSetUp(view);
  const tabPicked = useRef(false);
  useEffect(() => {
    if (setUp && !tabPicked.current && !DEMO_PANEL) setLeftTab("profiles");
  }, [setUp]);
  const [forAppOpen, setForAppOpen] = useState(DEMO_PANEL === "forapp");
  // "Edge" under the drawing: this profile's lights (owner's pick 3a, 2026-10-16)
  const [edgeOpen, setEdgeOpen] = useState(false);
  // a profile just made for an app: the inspector suggests next steps until something is picked
  const [fresh, setFresh] = useState<string | null>(DEMO_PANEL === "fresh" ? "demo-obs" : null);
  const { saved, save } = usePlacements();
  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);
  const notify = useCallback((text: string) => {
    setToast(text);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2500);
  }, []);
  const { dropAction, moveKey, openControl, copy, paste, clear, isSet, canPaste, opensPage } = useEditing(view, selected, notify);
  // a ready-made profile tried and left without Add (Back, or another profile picked) is thrown away
  const tried = useRef<string | null>(null);
  useEffect(() => {
    const now = view.state?.trying ?? null;
    const was = tried.current;
    tried.current = now;
    if (was && was !== now && triedPresets.has(was)) {
      triedPresets.delete(was);
      if (view.state?.profiles.some((p) => p.id === was && p.draft)) api.settleDraft(was, false).catch(() => {});
    }
  }, [view.state?.trying, view.state?.profiles]);
  // a control's right-click menu (owner's 1a, 2026-10-20)
  const [ctlMenu, setCtlMenu] = useState<{ c: ControlRef; x: number; y: number } | null>(null);
  // a control picked in the window is to be edited: whatever covers the inspector goes away
  const showInspector = () => {
    setProfileOpen(false);
    setSettingsOpen(false);
    setGamesOpen(false);
    setGalleryOpen(false);
    setForAppOpen(false);
  };
  useEffect(() => {
    if (!DEMO_SELECT || selected) return;
    const [kind, index] = DEMO_SELECT.split(".");
    const m = live.deck.modules.find((x) => x.kind === kind);
    if (m) setSelected({ module: m.id, kind: m.kind, index: Number(index ?? 0) } as ControlRef);
  }, [live.deck.modules, selected]);
  // module kinds the user has (attached and owned), so presets come adapted to them
  const kinds = [...new Set([...live.deck.modules.filter((m) => m.kind !== "base").map((m) => m.kind), ...detachedKinds(live.deck.modules, saved)])];

  // "Build my deck step by step" (layout A): a page of its own
  const [builderOpen, setBuilderOpen] = useState(DEMO_PANEL === "builder");
  // the tour (show, then ask, then build): a page of its own
  const [tourOpen, setTourOpen] = useState(DEMO_PANEL === "tour");
  // the first start goes straight into the tour, from its desk check (owner's pick 1a,
  // 2026-10-14); Edit layout from there comes back to it when done
  // where things are, after "Use as my home deck" (owner's pick 2a): once, over the main window
  const [guideOpen, setGuideOpen] = useState(DEMO_PANEL === "guide");
  const startGuide = () => {
    if (guideSeen()) return;
    // a key is picked so the inspector shows its editor with "What should it do?"
    const keys = live.deck.modules.find((m) => m.kind === "keys");
    if (keys) setSelected({ module: keys.id, kind: keys.kind, index: 0 } as ControlRef);
    showInspector();
    setGuideOpen(true);
  };
  // plugin files opened with HubDeck (a double click, Open with) or dropped on the window: one at a
  // time, each asked about before it installs
  const [pluginFiles, setPluginFiles] = useState<string[]>(DEMO_PANEL === "pluginfile" ? ["C:\Users\you\Downloads\com.fredemmott.audiooutputswitch.streamDeckPlugin"] : []);
  const [dropping, setDropping] = useState(false);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    const add = (files: string[]) => files.length > 0 && setPluginFiles((now) => [...now, ...files.filter((f) => !now.includes(f))]);
    const take = () => api.sdOpened().then(add, () => {});
    take();
    const none = () => () => {};
    const offOpened = listen("engine://plugin-file", take).catch(none);
    const isPlugin = (p: string) => /\.(streamdeckplugin|zip)$/i.test(p);
    // a plugin file dropped on the window, through the window's own drag and drop: the system's
    // (Tauri's dragDropEnabled) stops actions being dragged onto keys on Windows (owner found it,
    // 2026-10-18), so the file's bytes go to the engine, which keeps them in a temp file
    const files = (e: DragEvent) => e.dataTransfer?.types.includes("Files") ?? false;
    let depth = 0;
    const enter = (e: DragEvent) => {
      if (!files(e)) return;
      depth++;
      setDropping(true);
    };
    const over = (e: DragEvent) => {
      if (files(e)) e.preventDefault();
    };
    const leave = (e: DragEvent) => {
      if (files(e) && --depth <= 0) (depth = 0), setDropping(false);
    };
    const drop = (e: DragEvent) => {
      if (!files(e)) return;
      e.preventDefault();
      depth = 0;
      setDropping(false);
      for (const f of [...(e.dataTransfer?.files ?? [])].filter((f) => isPlugin(f.name))) {
        f.arrayBuffer().then((b) => api.sdDropFile(f.name, new Uint8Array(b))).then((path) => add([path]), () => {});
      }
    };
    window.addEventListener("dragenter", enter);
    window.addEventListener("dragover", over);
    window.addEventListener("dragleave", leave);
    window.addEventListener("drop", drop);
    const offDrop = Promise.resolve(() => {
      window.removeEventListener("dragenter", enter);
      window.removeEventListener("dragover", over);
      window.removeEventListener("dragleave", leave);
      window.removeEventListener("drop", drop);
    });
    return () => {
      offOpened.then((f) => f());
      offDrop.then((f) => f());
    };
  }, []);
  const setupPending = !!view.state && !view.state.onboarded;
  const tourShown = (tourOpen || setupPending) && !arranging && !builderOpen;
  // leaving arrange mode confirms every module as it is shown, touched or not
  const toggleArrange = () => {
    if (arranging) {
      for (const p of placeModules(live.deck.modules, saved)) {
        const key = placementKey(p.module);
        if (key && p.source !== "user") save(key, { col: p.col, row: p.row, rotation: p.rotation });
      }
    }
    setArranging(!arranging);
  };

  // while arranging, mark every control on the hardware (numbers, arrow, LED colors);
  // when arranging ends the engine draws the profile again
  const moduleIds = live.deck.modules.map((m) => m.id).join(",");
  useEffect(() => {
    if (!arranging) {
      // LEDs back to the default effect; keys and dial are redrawn by the engine
      live.deck.modules.filter((m) => m.kind === "knobs" || m.kind === "faders").forEach((m) => clearIdentify(m).catch(() => {}));
      if ("__TAURI_INTERNALS__" in window) api.redraw().catch(() => {});
      return;
    }
    live.deck.modules.forEach((m) => identifyModule(m).catch((e) => console.error("identify", m.id, e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [arranging, moduleIds]);

  // touching a control on the hardware selects it, so you can find a key by pressing it
  useEffect(() => {
    if (live.lastTouched) {
      const touched = live.lastTouched;
      // after the deck's drawing: the control's panel is heavy and the first move of a fader
      // waited for it (2026-10-15)
      startTransition(() => {
        setSelected(touched);
        setProfileOpen(false);
      });
    }
  }, [live.lastTouched]);

  // "Show" on the made-by-itself banner: the profile list was often open already, so the click
  // looked dead (owner, 2026-10-22); bring the first new profile into view and light it once
  const showProfiles = (ids: string[]) => {
    setLeftTab("profiles");
    window.setTimeout(() => {
      const row = ids.map((id) => document.querySelector<HTMLElement>(`.prow[data-profile="${CSS.escape(id)}"]`)).find(Boolean);
      if (!row) return;
      row.scrollIntoView({ block: "center", behavior: "smooth" });
      row.classList.remove("is-shown");
      void row.offsetWidth; // restart the glow on a second click
      row.classList.add("is-shown");
    }, 50);
  };

  return (
    <div className="app">
      <TopBar
        kinds={kinds}
        newRequest={newRequest}
        live={live}
        view={view}
        onOpenProfile={() => (setProfileOpen((o) => !o), setSettingsOpen(false), setGalleryOpen(false))}
        onOpenGallery={() => (setGalleryOpen(true), setSettingsOpen(false), setProfileOpen(false))}
        onOpenForApp={() => (setForAppOpen(true), setGalleryOpen(false), setSettingsOpen(false), setProfileOpen(false))}
        onOpenAssistant={() => setLeftTab("assistant")}
        onOpenTour={() => (setTourOpen(true), setSettingsOpen(false), setGalleryOpen(false), setForAppOpen(false), setProfileOpen(false))}
        onOpenSettings={() => (setSettingsOpen((o) => !o), setPluginsOpen(false), setGamesOpen(false), setGalleryOpen(false), setForAppOpen(false))}
        onOpenPlugins={() => (setPluginsOpen((o) => !o), setSettingsOpen(false), setGamesOpen(false), setGalleryOpen(false), setForAppOpen(false))}
        onOpenGames={() => (setGamesOpen((o) => !o), setSettingsOpen(false))}
      />
      <Banner
        live={live}
        view={view}
        onOpenProfiles={showProfiles}
        onOpenConnections={() => (setSettingsSection("connections"), setSettingsOpen(true))}
      />
      {tourShown ? (
        <main className="workspace">
          <Tour
            live={live}
            view={view}
            saved={saved}
            onPlace={save}
            deskCheck={setupPending}
            onArrange={toggleArrange}
            onClose={(home) => (setTourOpen(false), home && startGuide())}
            onBuild={() => (setTourOpen(false), setBuilderOpen(true))}
            onAsk={(message) => (setTourOpen(false), setLeftTab("assistant"), askAssistant(message))}
          />
        </main>
      ) : builderOpen ? (
        <main className="workspace">
          <Builder live={live} view={view} saved={saved} onPlace={save} onClose={() => setBuilderOpen(false)} />
        </main>
      ) : pluginsOpen ? (
        <main className="workspace">
          <PluginsTab onClose={() => setPluginsOpen(false)} onInstalled={() => {}} onUse={(id) => (setPluginsOpen(false), setSelected(null), setFocusPlugin({ id, at: Date.now() }))} />
        </main>
      ) : settingsOpen && !calibrating ? (
        <main className="workspace">
          <SettingsPanel live={live} view={view} section={settingsSection} onSection={setSettingsSection} onClose={() => setSettingsOpen(false)} onCalibrate={() => (setCalibrating(true), setSettingsOpen(false))} onTour={() => (setTourOpen(true), setSettingsOpen(false))} onBuild={() => (setBuilderOpen(true), setSettingsOpen(false))} onBrowsePlugins={() => (setPluginsOpen(true), setSettingsOpen(false))} />
        </main>
      ) : (
      <main className={`workspace${arranging ? " is-arranging" : ""}`}>
        <LeftPanel view={view} tab={leftTab} onTab={(tab) => ((tabPicked.current = true), setLeftTab(tab))} onBuild={() => setBuilderOpen(true)} onTour={() => (setTourOpen(true), setSettingsOpen(false))} onNewProfile={() => setNewRequest((n) => n + 1)} onOpenProfile={() => (setProfileOpen(true), setSettingsOpen(false), setGalleryOpen(false), setForAppOpen(false))} />
        <section className="center">
          <div className="stage-bar">
            {!arranging && live.deck.modules.some((m) => m.kind === "keys") && <PageBar view={view} />}
            <span className="spacer" />
            {/* a draft the assistant or the user put on the deck for a try: say so, and the way back */}
            {view.state?.trying && (
              <span className="chip is-trying" role="status">
                <Icon name="play" />
                {view.state.comparing
                  ? t("assistant.comparingBar", { n: view.state.comparing[0], count: view.state.comparing[1] })
                  : t("assistant.tryingBar", { name: view.state.profiles.find((p) => p.id === view.state?.trying)?.name ?? "" })}
                {triedPresets.has(view.state.trying) && (
                  <button className="link-btn" onClick={() => {
                      // out of the tried set first: the end of the try must not throw it away
                      const id = view.state!.trying!;
                      triedPresets.delete(id);
                      api.settleDraft(id, true).catch((e) => notify(String(e)));
                    }}>
                    {t("stage.addProfile")}
                  </button>
                )}
                <button className="link-btn" onClick={() => api.tryProfile(null)}>
                  {t("assistant.back")}
                </button>
              </span>
            )}
            {/* the deck shows its screensaver; the window shows the keys, says so, and can end it */}
            {saverOn && (
              <span className="chip" role="status">
                <Icon name="sparkles" />
                {t("stage.saverOn")}
                <button className="link-btn" onClick={() => saverEnd()}>
                  {t("stage.saverEnd")}
                </button>
              </span>
            )}
            {live.deck.modules.length > 0 && (
              <button className={`chip layout-chip${arranging ? " is-active" : ""}`} onClick={toggleArrange} title={t("stage.editLayoutHelp")}>
                <Icon name={arranging ? "check" : "layout-grid"} />
                {arranging ? t("stage.doneLayout") : t("stage.editLayout")}
              </button>
            )}
          </div>
          <div className="deck-area">
          <DeckView
            live={live}
            selected={selected}
            onSelect={(c) => {
              setSelected(c);
              showInspector();
              // a click on the deck in the window is a touch: the screensaver ends
              if (saverOn) saverEnd();
            }}
            saved={saved}
            onPlace={save}
            arranging={arranging}
            onEdge={() => setEdgeOpen(true)}
            onDeselect={() => setSelected(null)}
            profile={view.profile}
            page={view.state?.page ?? null}
            onDropAction={(c, action) => {
              setSelected(c);
              showInspector();
              dropAction(c, action);
            }}
            onMoveKey={(from, to) => {
              moveKey(from, to);
              setSelected(to);
            }}
            onOpenControl={openControl}
            onControlMenu={(c, x, y) => setCtlMenu({ c, x, y })}
          />
          {ctlMenu && (
            <ControlMenu
              title={`${t(`control.${ctlMenu.c.kind}` as Key)}${ctlMenu.c.kind === "dial" ? "" : ` ${ctlMenu.c.index + 1}`}`}
              x={ctlMenu.x}
              y={ctlMenu.y}
              isSet={isSet(ctlMenu.c)}
              canPaste={canPaste()}
              opensPage={opensPage(ctlMenu.c)}
              onCopy={() => copy(ctlMenu.c)}
              onPaste={() => paste(ctlMenu.c)}
              onClear={() => clear(ctlMenu.c)}
              onOpenPage={() => openControl(ctlMenu.c)}
              onClose={() => setCtlMenu(null)}
            />
          )}
          {arranging && <BaseTurnPicker live={live} />}
          {arranging && <ShareArrangement live={live} saved={saved} />}
          </div>
          {/* while arranging the panel on the right says what to check */}
          {!arranging && <p className="hintbar">{t("app.hint")}</p>}
          {toast && (
            <div className="toast" role="status">
              {toast}
            </div>
          )}
        </section>
        {calibrating ? (
          <CalibrationPanel onClose={() => setCalibrating(false)} />
        ) : arranging ? (
          <ArrangePanel onDone={toggleArrange} live={live} saved={saved} onPlace={save} />
        ) : (
          <Inspector live={live} selected={selected} view={view} fresh={!!fresh && fresh === view.state?.active} onDismissFresh={() => setFresh(null)} onOpenPlugins={() => (setPluginsOpen(true), setSettingsOpen(false), setGamesOpen(false), setGalleryOpen(false), setForAppOpen(false))} focusPlugin={focusPlugin} />
        )}
      </main>
      )}
      {/* panels open over the window, so a touch on a control still shows its editor (docs/FLOWS.md, difference 4) */}
      {!tourShown && !arranging && !calibrating && (forAppOpen || galleryOpen || gamesOpen || profileOpen || edgeOpen) && (
        <Sheet onClose={() => (setForAppOpen(false), setGalleryOpen(false), setGamesOpen(false), setProfileOpen(false), setEdgeOpen(false))}>
          {edgeOpen ? (
            <EdgePanel view={view} onClose={() => setEdgeOpen(false)} onAll={() => (setEdgeOpen(false), setSettingsSection("lights"), setSettingsOpen(true))} />
          ) : forAppOpen ? (
            <ForAppPanel view={view} kinds={kinds} onClose={() => setForAppOpen(false)} onMade={(id) => (setForAppOpen(false), setFresh(id), setSelected(null))} />
          ) : galleryOpen ? (
            <GalleryPanel live={live} saved={saved} onClose={() => setGalleryOpen(false)} />
          ) : gamesOpen ? (
            <GamesPanel view={view} onClose={() => setGamesOpen(false)} />
          ) : (
            <ProfilePanel view={view} onClose={() => setProfileOpen(false)} />
          )}
        </Sheet>
      )}
      {guideOpen && !tourShown && <Guide onClose={() => setGuideOpen(false)} />}
      <ConfirmHost />
      {pluginFiles.length > 0 && <PluginFileSheet key={pluginFiles[0]} path={pluginFiles[0]} onClose={() => setPluginFiles((now) => now.slice(1))} />}
      {dropping && (
        <div className="drop-overlay" aria-hidden="true">
          <Icon name="plug" />
          {t("plugins.dropHere")}
        </div>
      )}
    </div>
  );
}
