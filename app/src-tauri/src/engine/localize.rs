//! The words of the profiles HubDeck makes (presets, profiles for an app, the ones that make
//! themselves, suggestions) in the window's language. They are written in English in the code; a
//! profile made while the window is in Polish gets Polish names, page names and labels from
//! [`PL`]. Profiles already made are never renamed, and a word missing here stays English.

use std::sync::Mutex;

use serde_json::Value;

use super::profile::{Binding, Profile};

/// The window's language (`language_set`), for what is made now.
static LANGUAGE: Mutex<&'static str> = Mutex::new("en");

pub fn set_language(language: &str) {
    *LANGUAGE.lock().unwrap_or_else(|e| e.into_inner()) = if language == "pl" { "pl" } else { "en" };
}

pub fn polish() -> bool {
    *LANGUAGE.lock().unwrap_or_else(|e| e.into_inner()) == "pl"
}

/// English -> Polish for names and labels of what HubDeck makes.
const PL: &[(&str, &str)] = &[
    ("1 star", "1 gwiazdka"),
    ("2 stars", "2 gwiazdki"),
    ("3 stars", "3 gwiazdki"),
    ("4 stars", "4 gwiazdki"),
    ("5 stars", "5 gwiazdek"),
    ("A long label fits by itself", "Długi podpis sam się mieści"),
    ("Ability C", "Zdolność C"),
    ("Ability E", "Zdolność E"),
    ("Ability Q", "Zdolność Q"),
    ("Add edit", "Dodaj cięcie"),
    ("Address", "Adres"),
    ("Adjust", "Korekta"),
    ("Advancements", "Postępy"),
    ("Alchemy", "Alchemia"),
    ("All chat", "Czat ogólny"),
    ("Animation", "Animacja"),
    ("Animations", "Animacje"),
    ("Animations 5/7", "Animacje 5/7"),
    ("App", "Aplikacja"),
    ("App icon", "Ikona aplikacji"),
    ("App in front", "Aplikacja na wierzchu"),
    ("App volume", "Głośność aplikacji"),
    ("Arrange", "Rozmieść"),
    ("Artboard", "Obszar roboczy"),
    ("Axis RZ", "Oś RZ"),
    ("Axis X", "Oś X"),
    ("Axis Y", "Oś Y"),
    ("Back", "Wstecz"),
    ("Basics 1/7", "Podstawy 1/7"),
    ("Before/after", "Przed/po"),
    ("Blade", "Ostrze"),
    ("Bomb", "Bomba"),
    ("Break 5", "Przerwa 5"),
    ("Breathing", "Oddech"),
    ("Brighter", "Jaśniej"),
    ("Brightness", "Jasność"),
    ("Browser", "Przeglądarka"),
    ("Brush", "Pędzel"),
    ("Brush size", "Rozmiar pędzla"),
    ("Build", "Buduj"),
    ("Buy", "Kup"),
    ("Calculator", "Kalkulator"),
    ("Calm", "Spokój"),
    ("Camera", "Kamera"),
    ("Camera lock", "Blokada kamery"),
    ("Center", "Wyśrodkuj"),
    ("Character", "Postać"),
    ("Chat", "Czat"),
    ("Chatting", "Rozmowa"),
    ("Clip", "Klip"),
    ("Clip 30 s", "Klip 30 s"),
    ("Capture", "Nagraj ekran"),
    ("Clipboard", "Schowek"),
    ("Clock", "Zegar"),
    ("Clone", "Klonuj"),
    ("Close tab", "Zamknij kartę"),
    ("Control", "Sterowanie"),
    ("Control 4/7", "Sterowanie 4/7"),
    ("Copy", "Kopiuj"),
    ("Copy settings", "Kopiuj ustawienia"),
    ("Crop", "Kadruj"),
    ("Cycle", "Przełączaj"),
    ("Dashboard", "Panel"),
    ("Date", "Data"),
    ("Deafen", "Bez dźwięku"),
    ("Default colors", "Domyślne kolory"),
    ("Delete", "Usuń"),
    ("Desk light", "Lampka"),
    ("Desktop", "Pulpit"),
    ("Develop", "Wywołaj"),
    ("Dim", "Przyciemnij"),
    ("Dimmer", "Ciemniej"),
    ("Direct select", "Bezpośrednie"),
    ("Discord & meetings", "Discord i spotkania"),
    ("Discord deafen", "Discord bez dźwięku"),
    ("Discord mute", "Discord wycisz"),
    ("Discover HubDeck", "Poznaj HubDeck"),
    ("Disk free", "Wolne miejsce"),
    ("Drop", "Wyrzuć"),
    ("Duplicate", "Duplikuj"),
    ("Edit", "Edycja"),
    ("Edit points", "Punkty edycji"),
    ("Edit tool", "Narzędzie edycji"),
    ("Ellipse", "Elipsa"),
    ("Emote", "Emotka"),
    ("End turn", "Koniec tury"),
    ("Ending", "Koniec"),
    ("Eraser", "Gumka"),
    ("Evade", "Unik"),
    ("Everyday desk", "Codzienne biurko"),
    ("Export", "Eksport"),
    ("Eyedropper", "Pipeta"),
    ("Find", "Znajdź"),
    ("Flaps", "Klapy"),
    ("Flat", "Płaski"),
    ("Floor", "Podłoga"),
    ("Focus", "Skupienie"),
    ("Focus 25", "Skupienie 25"),
    ("Forward", "Dalej"),
    ("Frame", "Klatka"),
    ("Fullscreen", "Pełny ekran"),
    ("GPU temp", "Temp. GPU"),
    ("Game", "Gra"),
    ("Game action bar", "Pasek akcji w grach"),
    ("Game icon", "Ikona gry"),
    ("Game volume", "Głośność gry"),
    ("Gameplay", "Gra"),
    ("Glass", "Szkło"),
    ("Go live", "Na żywo"),
    ("Grenades", "Granaty"),
    ("Grid", "Siatka"),
    ("Group", "Grupuj"),
    ("Hardness", "Twardość"),
    ("Heal", "Leczenie"),
    ("Highlight", "Wyróżnij"),
    ("History", "Historia"),
    ("Home", "Dom"),
    ("House look", "Styl HubDecka"),
    ("In front", "Na wierzchu"),
    ("Inside", "Wewnątrz"),
    ("Inspect", "Zbadaj"),
    ("Integrations", "Integracje"),
    ("Integrations 7/7", "Integracje 7/7"),
    ("Inventory", "Ekwipunek"),
    ("Item 1", "Przedmiot 1"),
    ("Item 2", "Przedmiot 2"),
    ("Item 3", "Przedmiot 3"),
    ("Journal", "Dziennik"),
    ("Jump", "Skok"),
    ("Knife", "Nóż"),
    ("Library", "Biblioteka"),
    ("Lift", "Podnieś"),
    ("Lights back", "Światła z powrotem"),
    ("Like", "Polub"),
    ("Like song", "Polub utwór"),
    ("Liked", "Polubione"),
    ("Live", "Na żywo"),
    ("Live 2/7", "Na żywo 2/7"),
    ("Lock", "Zablokuj"),
    ("Looks", "Wygląd"),
    ("Looks 3/7", "Wygląd 3/7"),
    ("Loupe", "Lupa"),
    ("Macro", "Makro"),
    ("Map", "Mapa"),
    ("Mark in", "Początek"),
    ("Mark out", "Koniec"),
    ("Marker", "Znacznik"),
    ("Marquee", "Zaznaczenie"),
    ("Meet camera", "Meet kamera"),
    ("Meet mic", "Meet mikrofon"),
    ("Meeting", "Spotkanie"),
    ("Memory", "Pamięć"),
    ("Merge down", "Scal w dół"),
    ("Mic", "Mikrofon"),
    ("Mic level", "Poziom mikrofonu"),
    ("Mixture", "Mieszanka"),
    ("Mount", "Wierzchowiec"),
    ("Move", "Przesuń"),
    ("Music", "Muzyka"),
    ("Mute", "Wycisz"),
    ("Mute all", "Wycisz wszystko"),
    ("My games", "Moje gry"),
    ("New", "Nowy"),
    ("New Meet", "Nowe Meet"),
    ("New layer", "Nowa warstwa"),
    ("New tab", "Nowa karta"),
    ("Next", "Następny"),
    ("Next edit", "Nast. cięcie"),
    ("Next profile", "Następny profil"),
    ("Next set", "Następny zestaw"),
    ("Next →", "Dalej →"),
    ("Night focus", "Nocne skupienie"),
    ("Notepad", "Notatnik"),
    ("Notes", "Notatki"),
    ("Notices", "Powiadomienia"),
    ("Now", "Teraz"),
    ("Now playing", "Teraz gra"),
    ("OBS live", "OBS na żywo"),
    ("OBS record", "OBS nagrywanie"),
    ("OBS streaming", "Streaming w OBS"),
    ("Outline", "Kontur"),
    ("Paintbrush", "Pędzel"),
    ("Paste", "Wklej"),
    ("Paste back", "Wklej z tyłu"),
    ("Paste front", "Wklej z przodu"),
    ("Paste in place", "Wklej w miejscu"),
    ("Pen", "Pióro"),
    ("Phone", "Telefon"),
    ("Photos", "Zdjęcia"),
    ("Pick", "Wybierz"),
    ("Picture", "Obraz"),
    ("Picture 6/7", "Obraz 6/7"),
    ("Pistol", "Pistolet"),
    ("Play", "Odtwórz"),
    ("Players", "Gracze"),
    ("Playing", "Teraz gra"),
    ("Potion", "Mikstura"),
    ("Prev", "Poprzedni"),
    ("Prev edit", "Poprz. cięcie"),
    ("Primary", "Główna"),
    ("Private", "Prywatne"),
    ("Prop", "Rekwizyt"),
    ("Pulse", "Puls"),
    ("Push to talk", "Naciśnij, by mówić"),
    ("Quick select", "Szybkie zaznaczanie"),
    ("Quickload", "Szybki odczyt"),
    ("Quicksave", "Szybki zapis"),
    ("Rainbow", "Tęcza"),
    ("Razor", "Brzytwa"),
    ("Recall", "Powrót"),
    ("Record", "Nagrywaj"),
    ("Rectangle", "Prostokąt"),
    ("Redo", "Ponów"),
    ("Reject", "Odrzuć"),
    ("Reload", "Przeładuj"),
    ("Remove", "Usuń"),
    ("Reopen tab", "Przywróć kartę"),
    ("Repair", "Napraw"),
    ("Repeat", "Powtarzaj"),
    ("Replace", "Zastąp"),
    ("Replay", "Powtórka"),
    ("Reverse", "Wstecz"),
    ("Ripple delete", "Usuń i zsuń"),
    ("Roof", "Dach"),
    ("Save", "Zapisz"),
    ("Save to", "Zapisz do"),
    ("Scene 1", "Scena 1"),
    ("Scene 2", "Scena 2"),
    ("Scene 3", "Scena 3"),
    ("Scene 4", "Scena 4"),
    ("Scene 5", "Scena 5"),
    ("Scores", "Wyniki"),
    ("Screen", "Ekran"),
    ("Screenshot", "Zrzut ekranu"),
    ("Scroll", "Przewijanie"),
    ("Scroll ↔", "Przewijanie ↔"),
    ("Search", "Szukaj"),
    ("Seek", "Przewiń"),
    ("Select", "Zaznacz"),
    ("Selection", "Zaznaczenie"),
    ("Sessions", "Sesje"),
    ("Shimmer", "Migotanie"),
    ("Shop", "Sklep"),
    ("Shot", "Strzał"),
    ("Shove", "Pchnij"),
    ("Shuffle", "Losowo"),
    ("Sideways", "Na boki"),
    ("Sim panel", "Panel symulatora"),
    ("Skill 1", "Umiejętność 1"),
    ("Skill 2", "Umiejętność 2"),
    ("Skill 3", "Umiejętność 3"),
    ("Skill 4", "Umiejętność 4"),
    ("Skills", "Umiejętności"),
    ("Slider", "Suwak"),
    ("Sneak", "Skradanie"),
    ("Snip", "Wycinek"),
    ("Sound off", "Dźwięk wył."),
    ("Sound on", "Dźwięk wł."),
    ("Spellbook", "Księga czarów"),
    ("Split", "Podziel"),
    ("Split all", "Podziel wszystko"),
    ("Split mode", "Tryb podziału"),
    ("Spotify studio", "Studio Spotify"),
    ("Stairs", "Schody"),
    ("Starting", "Start"),
    ("Status board", "Tablica statusu"),
    ("Stream live", "Stream na żywo"),
    ("Swap colors", "Zamień kolory"),
    ("Swap hands", "Zamień ręce"),
    ("System mic", "Mikrofon systemu"),
    ("Tap, hold, 2×", "Naciśnij, przytrzymaj, 2×"),
    ("Task view", "Widok zadań"),
    ("Team chat", "Czat drużyny"),
    ("Teams camera", "Teams kamera"),
    ("Teams hand", "Teams ręka"),
    ("Teams leave", "Teams wyjdź"),
    ("Teams mute", "Teams wycisz"),
    ("Text", "Tekst"),
    ("Throttle", "Przepustnica"),
    ("Throw", "Rzut"),
    ("Time", "Czas"),
    ("To tab 1", "Do karty 1"),
    ("Tracks", "Utwory"),
    ("Trap", "Pułapka"),
    ("Trim", "Przytnij"),
    ("Trinket", "Talizman"),
    ("Turn-based", "Turowe"),
    ("Type", "Pisz"),
    ("Type text", "Wpisz tekst"),
    ("Ultimate", "Ulti"),
    ("Undo", "Cofnij"),
    ("Unflag", "Odznacz"),
    ("Ungroup", "Rozgrupuj"),
    ("Use", "Użyj"),
    ("Volume", "Głośność"),
    ("Wall", "Ściana"),
    ("Weather", "Pogoda"),
    ("Web", "Internet"),
    ("Website", "Strona"),
    ("Wheel", "Kółko"),
    ("World map", "Mapa świata"),
    ("Your colors", "Twoje kolory"),
    ("Zoom camera", "Zoom kamera"),
    ("Zoom hand", "Zoom ręka"),
    ("Zoom leave", "Zoom wyjdź"),
    ("Zoom mute", "Zoom wycisz"),
];

fn polish_word(en: &str) -> Option<&'static str> {
    PL.iter().find(|(e, _)| *e == en).map(|(_, p)| *p)
}

fn word(text: &mut String) {
    if let Some(p) = polish_word(text) {
        *text = p.to_string();
    }
}

/// A binding's labels (its look, its "on" look, the names of its modes) in Polish.
fn binding_pl(b: &mut Binding) {
    if let Some(l) = b.look.label.as_mut() {
        word(l);
    }
    if let Some(l) = b.look_on.as_mut().and_then(|on| on.label.as_mut()) {
        word(l);
    }
    if let Some(modes) = b.params.get_mut("modes").and_then(Value::as_array_mut) {
        for m in modes {
            if let Some(Value::String(l)) = m.get_mut("label") {
                word(l);
            }
        }
    }
}

fn profile_pl(p: &mut Profile) {
    word(&mut p.name);
    for page in &mut p.pages {
        word(&mut page.name);
    }
    for b in p.controls.values_mut() {
        binding_pl(b);
    }
}

/// A profile being made now, in the window's language.
pub fn profile(mut p: Profile) -> Profile {
    if polish() {
        profile_pl(&mut p);
    }
    p
}

/// A suggested binding, in the window's language.
pub fn binding(mut b: Binding) -> Binding {
    if polish() {
        binding_pl(&mut b);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::profile::{Look, Page};
    use crate::engine::starter::{self, AppKind};

    /// Every name, page name and label HubDeck can make (test profiles aside).
    fn everything() -> Vec<String> {
        let mut profiles: Vec<Profile> = crate::engine::presets::catalog().into_iter().filter(|p| !p.test).map(|p| p.make()).collect();
        for kind in [AppKind::Browser, AppKind::Editor, AppKind::Chat, AppKind::Media, AppKind::Game, AppKind::Other] {
            profiles.push(starter::starter(kind, "app.exe", "App", "", None));
        }
        let mut out = Vec::new();
        let label = |b: &Binding, out: &mut Vec<String>| {
            out.extend(b.look.label.clone());
            out.extend(b.look_on.as_ref().and_then(|on| on.label.clone()));
            if let Some(modes) = b.params.get("modes").and_then(Value::as_array) {
                out.extend(modes.iter().filter_map(|m| m["label"].as_str().map(String::from)));
            }
        };
        for p in &profiles {
            out.push(p.name.clone());
            out.extend(p.pages.iter().map(|g| g.name.clone()));
            // a game's key is named after the game (My games lists this PC's library)
            for b in p.controls.values().filter(|b| b.action != "core.game") {
                label(b, &mut out);
            }
        }
        for kind in ["keys", "dial", "knobs", "faders", "rollers"] {
            for b in crate::engine::suggest::for_control(&Profile::new("x"), &format!("{kind}.0")) {
                label(&b, &mut out);
            }
        }
        out.retain(|s| !s.trim().is_empty());
        out.sort();
        out.dedup();
        out
    }

    /// Words that stay as they are in Polish: brands, game and app names, key names, numbers.
    const SAME: &[&str] = &[
        "0",
        "1",
        "1 min",
        "2",
        "3",
        "4",
        "42",
        "5",
        "6",
        "7",
        "8",
        "9",
        "Auto ripple",
        "B1",
        "B2",
        "B3",
        "B4",
        "B5",
        "B6",
        "B7",
        "B8",
        "B9",
        "B10",
        "B11",
        "B12",
        "B13",
        "B14",
        "B15",
        "B16 / B17",
        "B18 / B19",
        "BRB",
        "Baldur's Gate 3",
        "Bitcoin",
        "CPU",
        "CapCut",
        "Chrome",
        "Claude Code",
        "Counter-Strike 2",
        "DaVinci Resolve",
        "Debug",
        "Diablo IV",
        "Discord",
        "E",
        "Emoji",
        "F",
        "F13",
        "F14",
        "F15",
        "F16",
        "F17",
        "Firefox",
        "Fortnite",
        "GPU",
        "Gradient",
        "Illustrator",
        "Lasso",
        "League of Legends",
        "Lightroom",
        "Lo-fi",
        "Menu",
        "Minecraft",
        "Photoshop",
        "Ping",
        "Portal",
        "Premiere Pro",
        "Q",
        "R",
        "Radio",
        "Shotcut",
        "Shuttle",
        "Slot 1",
        "Slot 2",
        "Slot 3",
        "Slot 4",
        "Slot 5",
        "Spike",
        "Spotify",
        "Spray",
        "Steam",
        "Stop",
        "Stream",
        "Summoner D",
        "Summoner F",
        "System",
        "Tab",
        "Teams",
        "VEGAS Pro",
        "Valorant",
        "W",
        "Zoom",
    ];

    /// The session keys of the Claude Code preset (Agent 1...) are numbered the same in both.
    fn same(w: &str) -> bool {
        SAME.contains(&w) || w.strip_prefix("Agent ").is_some_and(|n| n.parse::<u32>().is_ok())
    }

    #[test]
    fn every_word_hubdeck_makes_has_its_polish() {
        let missing: Vec<String> = everything().into_iter().filter(|w| polish_word(w).is_none() && !same(w)).collect();
        assert!(missing.is_empty(), "add these to PL (or to SAME when they stay as they are): {missing:?}");
    }

    #[test]
    fn the_table_has_each_word_once_and_no_empty_polish() {
        for (i, (en, pl)) in PL.iter().enumerate() {
            assert!(!pl.trim().is_empty(), "{en}");
            assert!(PL[..i].iter().all(|(e, _)| e != en), "{en} twice");
            // a Polish word that is also an English key would be translated again
            assert!(polish_word(pl).is_none(), "{pl} is also an English word of the table");
        }
    }

    fn labelled(action: &str, label: &str, params: Value) -> Binding {
        Binding { action: action.into(), params, look: Look { label: Some(label.into()), ..Look::default() }, accel: false, look_on: None, swap_turns: false }
    }

    #[test]
    fn a_profile_made_in_polish_gets_polish_names_pages_and_labels() {
        let mut p = Profile::new("Everyday desk");
        p.pages = vec![Page { id: "looks".into(), name: "Looks".into() }];
        let mut mic = labelled("core.mic", "Mic", serde_json::json!({}));
        mic.look_on = Some(Look { label: Some("Mute".into()), ..Look::default() });
        p.controls.insert("keys.0".into(), mic);
        p.controls.insert("keys.1".into(), labelled("core.text", "My own words", serde_json::json!({})));
        p.controls.insert("dial".into(), labelled("core.modes", "Volume", serde_json::json!({"modes": [{"label": "Volume"}, {"label": "Own mode"}]})));
        profile_pl(&mut p);
        assert_eq!(p.name, "Codzienne biurko");
        assert_eq!(p.pages[0].name, "Wygląd");
        assert_eq!(p.controls["keys.0"].look.label.as_deref(), Some("Mikrofon"));
        assert_eq!(p.controls["keys.0"].look_on.as_ref().and_then(|l| l.label.as_deref()), Some("Wycisz"));
        assert_eq!(p.controls["keys.1"].look.label.as_deref(), Some("My own words"), "a word not in the table stays");
        assert_eq!(p.controls["dial"].params["modes"][0]["label"], "Głośność");
        assert_eq!(p.controls["dial"].params["modes"][1]["label"], "Own mode");
        // made twice, nothing changes the second time (Polish is not a key of the table)
        let mut again = p.clone();
        profile_pl(&mut again);
        assert_eq!(again, p);
    }
}
