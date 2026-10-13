import { useCallback, useEffect, useRef, useState } from "react";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "./styles.css";
import "./theme";
import { useDeck, type ControlRef } from "./deck";
import type { LeftTab } from "./components/Panels";
import { api, useEngine } from "./engine";
import { DeckView } from "./components/DeckView";
import { Inspector } from "./components/Inspector";
import type { SettingsSection } from "./components/Panels";
import { ArrangePanel, Banner, BaseTurnPicker, ShareArrangement, ForAppPanel, GalleryPanel, GamesPanel, LeftPanel, PageBar, ProfilePanel, SettingsPanel, TopBar, Welcome } from "./components/Panels";
import { Builder } from "./components/Builder";
import { Icon } from "./components/Icon";
import { CalibrationPanel } from "./components/Calibration";
import { detachedKinds, placeModules, placementKey, usePlacements } from "./layout";
import { clearIdentify, identifyModule } from "./identify";
import { useEditing } from "./editing";
import { t, useLanguage } from "./i18n";
import { demoParam } from "./demo/params";

// `?demo&panel=profile|settings|plugins|calibrate` opens a side panel at start (screenshots)
const DEMO_PANEL = demoParam("panel");
// `&select=keys.6` (or `dial`, `knobs.1`...) selects that control at start
const DEMO_SELECT = demoParam("select");

export default function App() {
  const { lang } = useLanguage();
  // the deck shows some words itself (the dial gallery): in the window's language
  useEffect(() => {
    if ("__TAURI_INTERNALS__" in window) api.languageSet(lang).catch(() => {});
  }, [lang]);
  const live = useDeck();
  const view = useEngine();
  const [selected, setSelected] = useState<ControlRef | null>(null);
  const [arranging, setArranging] = useState(false);
  const [profileOpen, setProfileOpen] = useState(DEMO_PANEL === "profile");
  const [calibrating, setCalibrating] = useState(DEMO_PANEL === "calibrate");
  const [gamesOpen, setGamesOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(DEMO_PANEL === "settings" || DEMO_PANEL === "plugins");
  const [settingsSection, setSettingsSection] = useState<SettingsSection>(DEMO_PANEL === "plugins" ? "plugins" : "general");
  const [galleryOpen, setGalleryOpen] = useState(DEMO_PANEL === "gallery");
  const [newRequest, setNewRequest] = useState(0);
  const [leftTab, setLeftTab] = useState<LeftTab>(DEMO_PANEL === "profiles" ? "profiles" : "actions");
  const [forAppOpen, setForAppOpen] = useState(DEMO_PANEL === "forapp");
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
  const { dropAction } = useEditing(view, selected, notify);
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

  // leaving arrange mode confirms every module as it is shown, touched or not
  // the first start's setup page; a step that works on the deck steps away from it for a while
  const [setupAway, setSetupAway] = useState(false);
  // "Build my deck step by step" (layout A): a page of its own
  const [builderOpen, setBuilderOpen] = useState(DEMO_PANEL === "builder");
  const setupPending = !!view.state && !view.state.onboarded;
  const setupShown = setupPending && !setupAway && !arranging && !calibrating && !settingsOpen;
  const awayFromSetup = (then: () => void) => (setSetupAway(true), then());
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
      setSelected(live.lastTouched);
      setProfileOpen(false);
    }
  }, [live.lastTouched]);

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
        onOpenProfiles={() => setLeftTab((tab) => (tab === "profiles" ? "actions" : "profiles"))}
        onOpenAssistant={() => setLeftTab("assistant")}
        onOpenSettings={() => (setSettingsOpen((o) => !o), setGamesOpen(false), setGalleryOpen(false), setForAppOpen(false))}
        onOpenPlugins={() => (setSettingsSection("plugins"), setSettingsOpen(true), setGamesOpen(false), setGalleryOpen(false), setForAppOpen(false))}
        onOpenGames={() => (setGamesOpen((o) => !o), setSettingsOpen(false))}
      />
      <Banner
        live={live}
        view={view}
        onOpenProfiles={() => setLeftTab("profiles")}
        onOpenConnections={() => (setSettingsSection("connections"), setSettingsOpen(true))}
      />
      {builderOpen ? (
        <main className="workspace">
          <Builder live={live} view={view} saved={saved} onPlace={save} onClose={() => setBuilderOpen(false)} />
        </main>
      ) : setupShown ? (
        <main className="workspace">
          <Welcome
            live={live}
            onArrange={() => awayFromSetup(toggleArrange)}
            onCalibrate={() => awayFromSetup(() => setCalibrating(true))}
            saved={saved}
            onPlace={save}
            kinds={kinds}
            onForApp={() => awayFromSetup(() => setForAppOpen(true))}
            onBrowse={() => awayFromSetup(() => setNewRequest((n) => n + 1))}
          />
        </main>
      ) : settingsOpen && !calibrating ? (
        <main className="workspace">
          <SettingsPanel live={live} view={view} section={settingsSection} onSection={setSettingsSection} onClose={() => setSettingsOpen(false)} onCalibrate={() => (setCalibrating(true), setSettingsOpen(false))} />
        </main>
      ) : (
      <main className={`workspace${arranging ? " is-arranging" : ""}`}>
        <LeftPanel view={view} selected={selected} tab={leftTab} onTab={setLeftTab} onBuild={() => setBuilderOpen(true)} onNewProfile={() => setNewRequest((n) => n + 1)} onOpenProfile={() => (setProfileOpen(true), setSettingsOpen(false), setGalleryOpen(false), setForAppOpen(false))} />
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
                <button className="link-btn" onClick={() => api.tryProfile(null)}>
                  {t("assistant.back")}
                </button>
              </span>
            )}
            {/* set-up steps done on the deck: the way back to the setup page */}
            {setupPending && setupAway && !arranging && (
              <button className="chip is-trying" onClick={() => setSetupAway(false)}>
                <Icon name="chevron-left" />
                {t("welcome.backToSetup")}
              </button>
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
              // picking a control in the window is to give it something: the actions come back
              // (the assistant's talk stays: a key of a draft being tried is fixed next to it)
              setLeftTab((tab) => (tab === "assistant" ? tab : "actions"));
            }}
            saved={saved}
            onPlace={save}
            arranging={arranging}
            profile={view.profile}
            page={view.state?.page ?? null}
            onDropAction={(c, action) => {
              setSelected(c);
              showInspector();
              dropAction(c, action);
            }}
          />
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
          <ArrangePanel onDone={toggleArrange} />
        ) : forAppOpen ? (
          <ForAppPanel view={view} kinds={kinds} onClose={() => setForAppOpen(false)} onMade={(id) => (setForAppOpen(false), setFresh(id), setSelected(null))} />
        ) : galleryOpen ? (
          <GalleryPanel live={live} saved={saved} onClose={() => setGalleryOpen(false)} />
        ) : gamesOpen ? (
          <GamesPanel view={view} onClose={() => setGamesOpen(false)} />
        ) : profileOpen ? (
          <ProfilePanel view={view} onClose={() => setProfileOpen(false)} />
        ) : (
          <Inspector live={live} selected={selected} view={view} fresh={!!fresh && fresh === view.state?.active} onDismissFresh={() => setFresh(null)} />
        )}
      </main>
      )}
    </div>
  );
}
