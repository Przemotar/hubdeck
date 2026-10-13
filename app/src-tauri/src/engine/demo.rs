//! "Discover HubDeck" (owner, 2026-10-12): a tour on the deck itself. Seven tabs (key pages) show
//! what HubDeck can do; the bottom-right key goes to the next tab, the top-left key names the
//! tab. Every key works (safely) and carries a note (`look.note`, `demo:<id>`) that the window
//! shows when the key is pressed or picked: what it does and how to set it. The dial is the dial
//! gallery; knobs, faders and rollers do their usual jobs with lit LEDs. Labels come in the
//! window's language; the keys use the house look, except on the Looks tab.

use serde_json::{Value, json};

use super::house::{self, hue};
use super::profile::{Binding, Look, Page, Profile};

/// Our own animations for the demo (`prototype/tools/gen_demo_gifs.py`), copied into the images
/// folder when the demo is added.
const ANIMATIONS: [(&str, &[u8]); 16] = [
    ("aurora.gif", include_bytes!("../../resources/demo/aurora.gif")),
    ("bars.gif", include_bytes!("../../resources/demo/bars.gif")),
    ("dial-aurora.gif", include_bytes!("../../resources/demo/dial-aurora.gif")),
    ("dial-orbit.gif", include_bytes!("../../resources/demo/dial-orbit.gif")),
    ("dial-sweep.gif", include_bytes!("../../resources/demo/dial-sweep.gif")),
    ("dots.gif", include_bytes!("../../resources/demo/dots.gif")),
    ("fire.gif", include_bytes!("../../resources/demo/fire.gif")),
    ("heartbeat.gif", include_bytes!("../../resources/demo/heartbeat.gif")),
    ("lava.gif", include_bytes!("../../resources/demo/lava.gif")),
    ("orbit.gif", include_bytes!("../../resources/demo/orbit.gif")),
    ("ping.gif", include_bytes!("../../resources/demo/ping.gif")),
    ("rain.gif", include_bytes!("../../resources/demo/rain.gif")),
    ("rainbow.gif", include_bytes!("../../resources/demo/rainbow.gif")),
    ("scope.gif", include_bytes!("../../resources/demo/scope.gif")),
    ("spinner.gif", include_bytes!("../../resources/demo/spinner.gif")),
    ("typing.gif", include_bytes!("../../resources/demo/typing.gif")),
];

/// An animation of the demo as a picture name; None without an images folder (tests).
pub fn animation(name: &str) -> Option<String> {
    let (_, bytes) = ANIMATIONS.iter().find(|(n, _)| *n == name)?;
    super::images::import_bytes(bytes, "gif").ok()
}

/// The window's language, for the labels of a demo made now (and of every profile HubDeck makes,
/// see [`super::localize`]).
pub fn set_language(language: &str) {
    super::localize::set_language(language);
}

fn pl() -> bool {
    super::localize::polish()
}

fn tr(en: &str, pl_text: &str) -> String {
    if pl() { pl_text.into() } else { en.into() }
}

/// A key in the house look with its note.
fn key(action: &str, params: Value, label: (&str, &str), icon: &str, h: f32, note: &str) -> Binding {
    Binding {
        action: action.into(),
        params,
        look: Look {
            label: Some(tr(label.0, label.1)),
            icon: Some(icon.into()),
            bg: Some(house::BG.into()),
            fg: Some(house::LABEL.into()),
            icon_color: Some(house::icon_color(h)),
            note: Some(format!("demo:{note}")),
            ..Look::default()
        },
        accel: false,
        look_on: None,
        swap_turns: false,
    }
}

/// The tab's name on its first key (a press goes back to the first tab).
fn title(label: (&str, &str), icon: &str, note: &str) -> Binding {
    let mut b = key("core.page", json!({ "page": "" }), label, icon, hue::SHOW, note);
    b.look.bg = Some("#2b2540".into());
    b.look.fg = Some("#ffffff".into());
    b
}

fn next(to: &str) -> Binding {
    let mut b = key("core.page", json!({ "page": to }), ("Next →", "Dalej →"), "skip-forward", hue::NAV, "next");
    b.look.bg = Some("#2b2540".into());
    b
}

fn hotkey(keys: &str, label: (&str, &str), icon: &str, note: &str) -> Binding {
    key("core.hotkey", json!({ "keys": keys }), label, icon, hue::TOOL, note)
}

fn step(action: &str, params: Value, delay: u64) -> Value {
    json!({ "action": action, "params": params, "delay": delay })
}

/// The picture of a showcase art file, if the images folder is there.
fn art(name: &str) -> Option<String> {
    super::showcase::art(name)
}

fn put(p: &mut Profile, page: Option<&str>, keys: Vec<Binding>) {
    for (i, b) in keys.into_iter().enumerate() {
        let id = match page {
            Some(pg) => format!("{pg}/keys.{i}"),
            None => format!("keys.{i}"),
        };
        if !b.action.is_empty() || b.look.label.is_some() || b.look.image.is_some() {
            p.controls.insert(id, b);
        }
    }
}

pub fn discover() -> Profile {
    let mut p = Profile::new(&tr("Discover HubDeck", "Poznaj HubDeck"));
    p.color = Some("#8b7cf6".into());
    p.pages = vec![
        Page { id: "live".into(), name: tr("Live", "Na żywo") },
        Page { id: "looks".into(), name: tr("Looks", "Wygląd") },
        Page { id: "control".into(), name: tr("Control", "Sterowanie") },
        Page { id: "anim".into(), name: tr("Animations", "Animacje") },
        Page { id: "picture".into(), name: tr("Picture", "Obraz") },
        Page { id: "apps".into(), name: tr("Integrations", "Integracje") },
    ];
    let media = |command: &str, label: (&str, &str), icon: &str, note: &str| key("core.media", json!({ "command": command }), label, icon, hue::MEDIA, note);

    // 1. Basics (the main page)
    put(
        &mut p,
        None,
        vec![
            title(("Basics 1/7", "Podstawy 1/7"), "home", "page-basics"),
            key("core.launch", json!({"target": "notepad"}), ("Notepad", "Notatnik"), "app-window", hue::APP, "open-app"),
            key("core.launch", json!({"target": "https://github.com"}), ("Website", "Strona"), "globe", hue::APP, "open-web"),
            hotkey("Win+Shift+S", ("Snip", "Wycinek"), "camera", "shortcut"),
            key("core.text", json!({"text": "Hello from HubDeck!"}), ("Type text", "Wpisz tekst"), "type", hue::TOOL, "type-text"),
            media("previous", ("Prev", "Poprz."), "skip-back", "media"),
            media("playPause", ("Play", "Graj"), "play", "media"),
            media("next", ("Next", "Dalej"), "skip-forward", "media"),
            key("core.hotkey", json!({"keys": "VolumeMute"}), ("Mute", "Wycisz"), "volume-x", hue::SOUND, "mute"),
            key("core.mic", json!({"mode": "toggle"}), ("Mic", "Mikrofon"), "mic", hue::MIC, "mic"),
            key("core.launch", json!({"target": "calc"}), ("Calculator", "Kalkulator"), "app-window", hue::APP, "open-app"),
            hotkey("Win+Tab", ("Task view", "Widok zadań"), "layers", "shortcut"),
            hotkey("Win+.", ("Emoji", "Emoji"), "star", "shortcut"),
            hotkey("Win+V", ("Clipboard", "Schowek"), "list-ordered", "shortcut"),
            next("live"),
        ],
    );

    // 2. Live: things that change by themselves
    let live = |params: Value, label: (&str, &str), icon: &str, note: &str| key(super::live::ACTION, params, label, icon, hue::LIVE, note);
    let monitor = |metric: &str, label: (&str, &str), icon: &str| key(super::actions::MONITOR, json!({ "metric": metric }), label, icon, hue::LIVE, "monitor");
    put(
        &mut p,
        Some("live"),
        vec![
            title(("Live 2/7", "Na żywo 2/7"), "radio", "page-live"),
            key(super::actions::CLOCK, json!({"show": "time", "hours": "24"}), ("Clock", "Zegar"), "clock", hue::LIVE, "clock"),
            key(super::actions::CLOCK, json!({"show": "date", "hours": "24"}), ("Date", "Data"), "calendar", hue::LIVE, "clock"),
            key(super::actions::TIMER, json!({"minutes": 1, "seconds": 0, "press": "startPause", "sound": "on"}), ("1 min", "1 min"), "timer", hue::LIVE, "timer"),
            monitor("cpu", ("CPU", "CPU"), "zap"),
            monitor("ram", ("Memory", "Pamięć"), "layers"),
            monitor("gpu", ("GPU", "GPU"), "monitor"),
            monitor("gpuTemp", ("GPU temp", "Temp. GPU"), "sun"),
            key(super::actions::NOW_PLAYING, json!({}), ("Now playing", "Teraz grane"), "music", hue::MEDIA, "now-playing"),
            live(json!({"source": "weather", "place": "", "units": "metric"}), ("Weather", "Pogoda"), "sun", "weather"),
            live(json!({"source": "calendar", "url": ""}), ("Meeting", "Spotkanie"), "calendar", "calendar"),
            live(
                json!({"source": "url", "url": "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd", "path": "bitcoin.usd", "every": 60, "prefix": "$", "decimals": 0}),
                ("Bitcoin", "Bitcoin"),
                "rocket",
                "live-url",
            ),
            key(super::actions::TIMER, json!({"minutes": 25, "seconds": 0, "press": "startPause", "sound": "on"}), ("Focus 25", "Skupienie 25"), "timer", hue::LIVE, "timer"),
            Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
            next("looks"),
        ],
    );

    // 3. Looks: the same key drawn many ways (this tab leaves the house look on purpose)
    let styled = |style: &str, label: (&str, &str), bg: &str| {
        let mut b = key("core.media", json!({"command": "playPause"}), label, "play", hue::MEDIA, "style");
        b.look.style = Some(style.into());
        b.look.bg = Some(bg.into());
        b.look.icon_color = None;
        b.look.fg = Some("#ffffff".into());
        b
    };
    let mut gif = key("core.media", json!({"command": "playPause"}), ("Animation", "Animacja"), "music", hue::MEDIA, "animation");
    gif.look.image = art("equalizer.gif");
    let mut photo = key("", json!({}), ("Picture", "Obrazek"), "sun", hue::SHOW, "picture");
    photo.look.image = art("scene-game.png");
    let mut app_icon = key("core.launch", json!({"target": "notepad"}), ("App icon", "Ikona apki"), "app-window", hue::APP, "app-icon");
    app_icon.look.image = super::appicon::png(r"C:\Windows\System32\notepad.exe").ok().and_then(|png| super::images::import_bytes(&png, "png").ok());
    let mut two = key(
        super::actions::TOGGLE,
        json!({"first": [step("core.hotkey", json!({"keys": "VolumeMute"}), 0)], "second": [step("core.hotkey", json!({"keys": "VolumeMute"}), 0)]}),
        ("Sound on", "Dźwięk wł."),
        "volume-2",
        hue::SOUND,
        "two-looks",
    );
    two.look_on = Some(Look { label: Some(tr("Sound off", "Dźwięk wył.")), icon: Some("volume-x".into()), bg: Some("#7a1f24".into()), fg: Some("#ffffff".into()), ..Look::default() });
    let mut big = key("", json!({}), ("42", "42"), "", hue::SHOW, "text-only");
    big.look.icon = None;
    let mut game_icon = key("", json!({}), ("Game icon", "Ikona gry"), "game:crowned-skull", hue::GAME, "game-icon");
    game_icon.look.icon_color = Some(house::icon_color(45.0));
    let mut colors = key("", json!({}), ("Your colors", "Twoje kolory"), "heart", hue::MIC, "colors");
    colors.look.bg = Some("#0e3b43".into());
    colors.look.fg = Some("#fef3c7".into());
    colors.look.icon_color = Some("#f97316".into());
    put(
        &mut p,
        Some("looks"),
        vec![
            title(("Looks 3/7", "Wygląd 3/7"), "sparkles", "page-looks"),
            styled("flat", ("Flat", "Płaski"), "#3b2f6b"),
            styled("gradient", ("Gradient", "Gradient"), "#3b2f6b"),
            styled("glass", ("Glass", "Szkło"), "#3b2f6b"),
            styled("outline", ("Outline", "Obrys"), "#8b7cf6"),
            gif,
            photo,
            app_icon,
            two,
            key("", json!({}), ("A long label fits by itself", "Długi podpis sam się mieści"), "type", hue::SHOW, "long-label"),
            big,
            game_icon,
            colors,
            key("", json!({}), ("House look", "Styl HubDeck"), "star", hue::SHOW, "house"),
            next("control"),
        ],
    );

    // 4. Control: one key, more jobs
    let mut hold = key("core.media", json!({"command": "playPause"}), ("Tap · hold", "Dotknij · przytrzymaj"), "mouse-pointer-click", hue::MEDIA, "gestures");
    hold.look.label = Some(tr("Tap, hold, 2×", "Klik, trzymaj, 2×"));
    let lighting =
        |effect: &str, label: (&str, &str), color: &str| key("core.lighting", json!({"target": "all", "effect": effect, "color": color, "speed": 50}), label, "lightbulb", hue::SHOW, "lighting");
    put(
        &mut p,
        Some("control"),
        vec![
            title(("Control 4/7", "Sterowanie 4/7"), "settings", "page-control"),
            hold,
            key(
                super::actions::MULTI,
                json!({"steps": [step("core.launch", json!({"target": "notepad"}), 0), step("core.text", json!({"text": "A macro opened me and typed this."}), 1200)]}),
                ("Macro", "Makro"),
                "list-ordered",
                hue::TOOL,
                "macro",
            ),
            key("core.mic", json!({"mode": "pushToTalk"}), ("Push to talk", "Mów trzymając"), "mic", hue::MIC, "push-to-talk"),
            lighting("rainbow", ("Rainbow", "Tęcza"), "#ff2020"),
            lighting("breathing", ("Breathing", "Oddychanie"), "#8b7cf6"),
            lighting("water", ("Shimmer", "Migotanie"), "#22d3ee"),
            lighting("profile", ("Lights back", "Światła wróć"), "#ffffff"),
            key("core.deckBrightness", json!({"press": "darker", "step": 20}), ("Dimmer", "Ciemniej"), "moon", hue::SHOW, "deck-brightness"),
            key("core.deckBrightness", json!({"press": "brighter", "step": 20}), ("Brighter", "Jaśniej"), "sun", hue::SHOW, "deck-brightness"),
            key("core.profile", json!({"which": "next"}), ("Next profile", "Nast. profil"), "layers", hue::NAV, "profile"),
            key("core.page", json!({"page": ""}), ("To tab 1", "Do 1. zakładki"), "home", hue::NAV, "page"),
            Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
            Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
            next("anim"),
        ],
    );
    // the same key held and pressed twice
    p.controls.insert("control/keys.1:hold".into(), key("core.media", json!({"command": "next"}), ("Next", "Dalej"), "skip-forward", hue::MEDIA, "gestures"));
    p.controls.insert("control/keys.1:double".into(), key("core.media", json!({"command": "previous"}), ("Prev", "Poprz."), "skip-back", hue::MEDIA, "gestures"));

    // 5. Animations: animated pictures on keys, each at its own pace
    let moving = |file: &str, label: (&str, &str), action: &str, params: Value| {
        let mut b = key(action, params, label, "sparkles", hue::SHOW, "animation");
        b.look.image = animation(file);
        b
    };
    let quiet = |file: &str| {
        let mut b = moving(file, ("", ""), "", json!({}));
        b.look.label = None;
        b
    };
    put(
        &mut p,
        Some("anim"),
        vec![
            title(("Animations 5/7", "Animacje 5/7"), "sparkles", "page-anim"),
            moving("bars.gif", ("Music", "Muzyka"), "core.media", json!({"command": "playPause"})),
            moving("heartbeat.gif", ("Pulse", "Puls"), "", json!({})),
            quiet("ping.gif"),
            quiet("spinner.gif"),
            quiet("aurora.gif"),
            quiet("orbit.gif"),
            quiet("scope.gif"),
            quiet("lava.gif"),
            quiet("rainbow.gif"),
            quiet("fire.gif"),
            moving("typing.gif", ("Chat", "Czat"), "", json!({})),
            quiet("rain.gif"),
            quiet("dots.gif"),
            next("picture"),
        ],
    );

    // 6. Picture: one picture split over all 15 keys; the keys still work
    let sky: Vec<Binding> = (0..15)
        .map(|i| {
            let mut b = match i {
                0 => title(("Picture 6/7", "Obraz 6/7"), "moon", "page-picture"),
                14 => next("apps"),
                _ => key("", json!({}), ("", ""), "moon", hue::SHOW, "split-picture"),
            };
            if i != 0 && i != 14 {
                b.look.label = None;
                b.look.icon = None;
            }
            b.look.image = art(&format!("sky-{i}.png"));
            b
        })
        .collect();
    put(&mut p, Some("picture"), sky);

    // 7. Integrations: they need a sign-in or a running app; the note says how
    put(
        &mut p,
        Some("apps"),
        vec![
            title(("Integrations 7/7", "Integracje 7/7"), "zap", "page-apps"),
            key("obs.record", json!({}), ("OBS record", "OBS nagraj"), "video", hue::MIC, "obs"),
            key("obs.stream", json!({}), ("OBS live", "OBS na żywo"), "radio", hue::MIC, "obs"),
            key("spotify.like", json!({}), ("Like song", "Polub"), "heart", hue::MEDIA, "spotify"),
            key("ha.toggle", json!({"entity": ""}), ("Desk light", "Lampka"), "lightbulb", hue::TOOL, "home-assistant"),
            key(super::actions::AGENTS, json!({}), ("Claude Code", "Claude Code"), "bot", hue::SHOW, "claude"),
            key("core.launch", json!({"target": "steam://open/games"}), ("Steam", "Steam"), "gamepad-2", hue::GAME, "open-game"),
            Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
            Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
            Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
            Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
            Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
            Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
            Binding { action: String::new(), params: json!({}), look: Look::default(), accel: false, look_on: None, swap_turns: false },
            next(""),
        ],
    );

    // the dial is the gallery; knobs, faders and rollers do their usual jobs, lit
    let mut dial = key(super::gallery::ACTION, json!({}), ("Dial gallery", "Galeria diala"), "circle-dot", hue::SHOW, "dial-gallery");
    dial.look.label = None;
    p.controls.insert("dial".into(), dial);
    let lit = |target: &str, app: &str, label: (&str, &str), icon: &str, h: f32| {
        let mut b = key("core.volume", json!({"target": target, "app": app, "step": 2}), label, icon, h, "volume");
        b.look.led = Some(house::icon_color(h));
        b.look.led_level = true;
        b.accel = true;
        b
    };
    p.controls.insert("knobs.0".into(), lit("system", "", ("System", "System"), "volume-2", hue::SOUND));
    p.controls.insert("knobs.1".into(), lit("front", "", ("App in front", "Apka na wierzchu"), "volume-2", hue::APP));
    p.controls.insert("knobs.2".into(), lit("mic", "", ("Mic level", "Poziom mikrofonu"), "mic", hue::MIC));
    for (i, (target, app, label, icon, h)) in [
        ("system", "", ("System", "System"), "volume-2", hue::SOUND),
        ("front", "", ("App in front", "Apka na wierzchu"), "volume-2", hue::APP),
        ("app", "Discord.exe", ("Discord", "Discord"), "headphones", hue::MIC),
        ("app", "Spotify.exe", ("Music", "Muzyka"), "music", hue::MEDIA),
        ("app", "chrome.exe", ("Browser", "Przeglądarka"), "globe", hue::NAV),
    ]
    .into_iter()
    .enumerate()
    {
        p.controls.insert(format!("faders.{i}"), lit(target, app, label, icon, h));
    }
    p.controls.insert("rollers.0".into(), key("core.scroll", json!({"axis": "vertical", "amount": 3}), ("Scroll", "Przewijanie"), "arrow-up-down", hue::TOOL, "scroll"));
    p.controls.insert("rollers.1".into(), key("core.hotkey", json!({"keys": "", "clockwise": "Ctrl+=", "counterClockwise": "Ctrl+-"}), ("Zoom", "Zoom"), "search", hue::TOOL, "zoom"));
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tour_has_seven_tabs_that_go_round_and_notes_everywhere() {
        let p = discover();
        assert_eq!(p.pages.len(), 6, "six tabs besides the main one");
        let next_of = |page: &str| {
            let id = if page.is_empty() { "keys.14".to_string() } else { format!("{page}/keys.14") };
            p.controls[&id].params["page"].as_str().unwrap().to_string()
        };
        assert_eq!(next_of(""), "live");
        assert_eq!(next_of("live"), "looks");
        assert_eq!(next_of("control"), "anim");
        assert_eq!(next_of("anim"), "picture");
        assert_eq!(next_of("picture"), "apps");
        assert_eq!(next_of("apps"), "", "the last tab goes back to the first");
        for (id, b) in &p.controls {
            assert!(b.look.note.as_deref().is_some_and(|n| n.starts_with("demo:")), "{id} has no note");
            if let Some(icon) = &b.look.icon {
                assert!(super::super::render::icon_svg(icon).is_some(), "{id}: no icon {icon}");
            }
        }
        assert_eq!(p.controls["dial"].action, super::super::gallery::ACTION);
        assert!(p.controls.contains_key("control/keys.1:hold") && p.controls.contains_key("control/keys.1:double"));
    }
}
