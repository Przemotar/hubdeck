//! Running an action for one input. Runs on the engine thread.

use enigo::{Axis, Direction, Enigo, Key, Keyboard, Mouse, Settings};

use super::actions::{param_f64, param_str};
use super::profile::Binding;
use super::{audiodev, brightness, homeassistant as ha, hotkey, midi, obs, spotify, streamlabs as sl, twitch, volume};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    /// true = down, false = up. Actions fire on down.
    Press(bool),
    /// Detents: +1 clockwise / up, -1 counter-clockwise / down.
    Turn(i8),
    /// Absolute 0..=100 (faders).
    Slide(u8),
}

/// What the dial should show for a moment after an action changed a value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeatherCmd {
    Hours(i32),
    View,
    /// Open the forecast for the place in the browser.
    Forecast,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Feedback {
    /// 0..=1 drawn as a ring with a number; None shows only the caption (e.g. a dial mode name).
    pub value: Option<f32>,
    pub caption: String,
}

/// Side effects the engine applies after an action.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub feedback: Option<Feedback>,
    pub switch_profile: Option<String>,
    /// Show this key page ("" = the main page).
    pub page: Option<String>,
    /// Go this many profiles forward (+) or back (-) in name order.
    pub profile_step: Option<i32>,
    /// Change the backlight of the key panel and the dial.
    pub deck_brightness: Option<DeckBrightness>,
    /// A lighting effect for a module kind (or `all`) until the profile changes.
    pub lighting: Option<Lighting>,
    /// Start, pause, reset or set the control's countdown timer.
    pub timer: Option<super::timer::TimerCmd>,
    /// The control's clock: its stopwatch, view or hours.
    pub clock: Option<super::timer::ClockCmd>,
    /// A system monitor shows the next (+) or previous (-) metric.
    pub monitor_step: Option<i32>,
    /// The control's pomodoro.
    pub pomodoro: Option<super::timer::PomodoroCmd>,
    /// What a gesture did to the dial pet.
    pub pet: Option<super::pet::PetCmd>,
    /// A key puts Mochi on the dial, or takes it off.
    pub pet_dial: bool,
    /// Show a screensaver now for a moment: its effect, "" for the next one.
    pub saver: Option<String>,
    /// The weather scene: hours ahead by detents (`Hours`), or the next view (`View`).
    pub weather: Option<WeatherCmd>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lighting {
    /// `knobs`, `faders`, `base` or `all`.
    pub target: String,
    /// None = back to the profile's own lights.
    pub effect: Option<super::leds::Override>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeckBrightness {
    Set(u8),
    By(i32),
}

/// Lowest backlight an action can set (0 would look like a broken deck).
pub const MIN_BRIGHTNESS: u8 = 5;

impl DeckBrightness {
    /// The new level from `current`, kept within 5..=100.
    pub fn apply(self, current: u8) -> u8 {
        let v = match self {
            DeckBrightness::Set(v) => v as i32,
            DeckBrightness::By(d) => current as i32 + d,
        };
        v.clamp(MIN_BRIGHTNESS as i32, 100) as u8
    }
}

/// The profile `step` places away from `current` in `ordered` (wrapping); None for no profiles.
pub fn step_profile(ordered: &[String], current: Option<&str>, step: i32) -> Option<String> {
    if ordered.is_empty() {
        return None;
    }
    let n = ordered.len() as i32;
    let i = current.and_then(|c| ordered.iter().position(|id| id == c)).map(|i| i as i32);
    let next = match i {
        Some(i) => (i + step).rem_euclid(n),
        None if step < 0 => n - 1,
        None => 0,
    };
    Some(ordered[next as usize].clone())
}

/// The actions a test copy (`automation`) runs: they change only HubDeck itself.
/// `out.page` for Next page / Previous page: resolved against the active profile's pages.
pub const NEXT_PAGE: &str = ":next";
pub const PREVIOUS_PAGE: &str = ":previous";

const TEST_COPY_RUNS: [&str; 11] =
    ["core.page", "core.nextPage", "core.previousPage", "core.profile", "core.clock", "core.timer", "core.lighting", "core.deckBrightness", "core.dialGallery", "core.saver", "core.pet"];

/// The app, to send keys, clicks and text from its main thread: on macOS 26 enigo's keyboard
/// layout lookup (TSMGetInputSourceProperty) stops the app when called from any other thread
/// (the owner's deck test, session 9: a shortcut key crashed HubDeck).
pub static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

thread_local! {
    /// One keyboard and mouse per thread that sends (the main thread on macOS).
    static ENIGO: std::cell::RefCell<Option<Enigo>> = const { std::cell::RefCell::new(None) };
}

/// Run `f` with the keyboard and mouse: on the main thread on macOS, here elsewhere.
/// `{date}` and `{time}` in a typed text become the date and time of the press (one of the keys
/// other deck owners use most, PAIN-POINTS RD8).
fn with_now(text: &str, now: chrono::DateTime<chrono::Local>) -> String {
    text.replace("{date}", &now.format("%Y-%m-%d").to_string()).replace("{time}", &now.format("%H:%M").to_string())
}

fn keyboard<T: Send + 'static>(f: impl FnOnce(&mut Enigo) -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let run = move || {
        ENIGO.with(|cell| {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                // macOS asks for Accessibility permission the first time
                *slot = Some(Enigo::new(&Settings::default()).map_err(|e| format!("keyboard/mouse control unavailable: {e}"))?);
            }
            f(slot.as_mut().expect("just set"))
        })
    };
    #[cfg(target_os = "macos")]
    if let Some(app) = APP.get() {
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(run());
        })
        .map_err(|e| e.to_string())?;
        return rx.recv_timeout(std::time::Duration::from_secs(5)).map_err(|_| "the keyboard did not answer in time".to_string())?;
    }
    run()
}

#[derive(Default)]
pub struct Executor {
    /// Executable in front, set by the engine before each run (volume of "the app in front").
    pub foreground: String,
    /// Apps whose volume other controls of the active profile set (volume of "everything else").
    pub mapped: Vec<String>,
    /// Last volume we set per target; fast turns build on it instead of reading the system back.
    volumes: std::collections::HashMap<String, (f32, std::time::Instant)>,
}

/// Within this time after our own change, our remembered value is the truth.
const VOLUME_MEMORY: std::time::Duration = std::time::Duration::from_millis(1500);

impl Executor {
    pub fn run(&mut self, binding: &Binding, input: InputEvent) -> Result<Outcome, String> {
        // a test copy runs next to the person working at this computer: only what stays inside
        // HubDeck (no media keys, volume, shortcuts, programs, smart home...)
        // The film copy (`automation::real_deck`) drives the real deck for the owner's hands, so its
        // actions run (2026-10-15: his dial turn on the film changed nothing)
        if crate::automation::get().is_some() && !crate::automation::real_deck() && !TEST_COPY_RUNS.contains(&binding.action.as_str()) {
            log::info!("test copy: {} not run (it would act on this computer)", binding.action);
            return Ok(Outcome::default());
        }
        let p = &binding.params;
        // a key standing in for a turn (presets fitted to a deck without knobs): one detent per press
        let input = match (input, param_f64(p, "turnOnPress", 0.0) as i8) {
            (InputEvent::Press(true), d) if d != 0 => InputEvent::Turn(d),
            (InputEvent::Press(false), d) if d != 0 => return Ok(Outcome::default()),
            (i, _) => i,
        };
        let mut out = Outcome::default();
        let midi_args = || (param_str(p, "port").to_string(), param_f64(p, "channel", 1.0) as u8, param_f64(p, "cc", 1.0) as u8);
        match (binding.action.as_str(), input) {
            // a note sounds while the key is down
            ("midi.note", InputEvent::Press(down)) => {
                midi::note(param_str(p, "port"), param_f64(p, "channel", 1.0) as u8, param_f64(p, "note", 60.0) as u8, down, param_f64(p, "velocity", 100.0) as u8)?
            }
            // push-to-talk / push-to-mute: the press and the release set the mute
            ("core.mic", InputEvent::Press(down)) if super::actions::push_mute(p, down).is_some() => {
                let muted = super::actions::push_mute(p, down).unwrap_or(false);
                volume::set_mute(&volume::Target::Mic, muted)?;
                out.feedback = Some(Feedback { value: None, caption: if muted { "Mic off".into() } else { "Mic on".into() } });
            }
            ("obs.mute", InputEvent::Press(down)) if super::actions::push_mute(p, down).is_some() => {
                obs::request(obs::Request::SetMute { input: param_str(p, "input").trim().to_string(), muted: super::actions::push_mute(p, down).unwrap_or(false) })
            }
            // joystick buttons and hats are held with the key, axes take every input
            (a, input) if a.starts_with("vjoy.") => out.feedback = super::vjoy::run(a, p, input)?,
            (_, InputEvent::Press(false)) => {}

            ("midi.cc", InputEvent::Slide(v)) => {
                let (port, ch, cc) = midi_args();
                let value = (v as f32 * 1.27).round() as u8;
                midi::cc(&port, ch, cc, value)?;
                out.feedback = Some(Feedback { value: Some(v as f32 / 100.0), caption: format!("CC {cc}") });
            }
            ("midi.cc", InputEvent::Turn(d)) => {
                let (port, ch, cc) = midi_args();
                let value = midi::cc_turn(&port, ch, cc, d as i32 * param_f64(p, "step", 2.0) as i32)?;
                out.feedback = Some(Feedback { value: Some(value as f32 / 127.0), caption: format!("CC {cc}") });
            }
            ("midi.cc", InputEvent::Press(true)) => {
                let (port, ch, cc) = midi_args();
                midi::cc(&port, ch, cc, param_f64(p, "value", 127.0) as u8)?;
            }

            ("core.launch", InputEvent::Press(true)) => {
                let target = param_str(p, "target").trim();
                if target.is_empty() {
                    return Err("nothing to open: set a target".into());
                }
                // a key may name an installed app instead of its program ("Visual Studio Code",
                // as the assistant wrote them, session 6): open that app
                if let Err(e) = crate::system::shell_open(target) {
                    match super::installed::target_named(target).or_else(|| super::apps::seen_named(target)) {
                        Some(t) if t != target => crate::system::shell_open(&t)?,
                        _ => return Err(e),
                    }
                }
            }

            (super::actions::HTTP, InputEvent::Press(true)) => {
                let (code, text) = http_request(param_str(p, "method"), param_str(p, "url"), param_str(p, "body"), param_str(p, "headers"))?;
                let short: String = text.trim().chars().take(40).collect();
                out.feedback = Some(Feedback { value: None, caption: if short.is_empty() { format!("{code}") } else { format!("{code} {short}") } });
            }

            ("core.game", InputEvent::Press(true)) => {
                let target = param_str(p, "game").trim();
                if target.is_empty() {
                    return Err("pick a game first".into());
                }
                crate::system::shell_open(target)?;
            }

            ("core.hotkey", InputEvent::Press(true)) if param_str(p, "mode") == "game" => {
                let hold = std::time::Duration::from_millis(param_f64(p, "hold", 30.0).clamp(1.0, 500.0) as u64);
                let keys = param_str(p, "keys").to_string();
                keyboard(move |e| hotkey::send_game(e, &keys, hold))?;
            }
            ("core.hotkey", InputEvent::Press(true)) => {
                let keys = param_str(p, "keys").to_string();
                keyboard(move |e| send_if_set(e, &keys))?;
            }
            ("core.hotkey", InputEvent::Turn(d)) => {
                let combo = if d > 0 { param_str(p, "clockwise") } else { param_str(p, "counterClockwise") };
                let combo = combo.to_string();
                let times = d.unsigned_abs();
                keyboard(move |e| (0..times).try_for_each(|_| send_if_set(e, &combo)))?;
            }

            ("core.media", InputEvent::Press(true)) => {
                let key = match param_str(p, "command") {
                    "next" => Key::MediaNextTrack,
                    "previous" => Key::MediaPrevTrack,
                    #[cfg(not(target_os = "macos"))]
                    "stop" => Key::MediaStop,
                    _ => Key::MediaPlayPause,
                };
                keyboard(move |e| e.key(key, Direction::Click).map_err(|e| e.to_string()))?;
            }
            ("core.media", InputEvent::Turn(d)) => {
                let key = if d > 0 { Key::MediaNextTrack } else { Key::MediaPrevTrack };
                keyboard(move |e| e.key(key, Direction::Click).map_err(|e| e.to_string()))?;
            }

            ("core.volume", input) => {
                let target = match param_str(p, "target") {
                    "app" => {
                        let app = param_str(p, "app").trim();
                        if app.is_empty() {
                            return Err("choose the app whose volume to control".into());
                        }
                        volume::Target::App(app.to_string())
                    }
                    "mic" => volume::Target::Mic,
                    "front" if !self.foreground.is_empty() => volume::Target::App(self.foreground.clone()),
                    "others" => volume::Target::Others(self.mapped.clone()),
                    "front" => return Err("no app in front yet".into()),
                    _ => volume::Target::System,
                };
                // the control's own name first ("Music", "Zoom"), so the dial says what it changes
                let caption = binding.look.label.as_deref().map(|l| l.replace('\n', " ").trim().to_string()).filter(|l| !l.is_empty()).unwrap_or_else(|| match &target {
                    volume::Target::System => "Volume".to_string(),
                    volume::Target::Mic => "Microphone".to_string(),
                    volume::Target::App(app) => app.trim_end_matches(".exe").to_string(),
                    volume::Target::Others(_) => "Other apps".to_string(),
                });
                // system volume with the Windows popup: a volume key shows it (see `volume::with_popup`)
                let popup = target == volume::Target::System && param_str(p, "popup") != "hide" && volume::POPUP_AVAILABLE;
                // an app with no sound open now: the dial says so, no error banner (a tour's call app
                // or the app in front often is silent)
                macro_rules! or_silent {
                    ($e:expr) => {
                        match $e {
                            Ok(v) => v,
                            Err(e) if matches!(target, volume::Target::App(_)) && e.ends_with(volume::SILENT) => {
                                out.feedback = Some(Feedback { value: None, caption: format!("{caption}: no sound now") });
                                return Ok(out);
                            }
                            Err(e) => return Err(e),
                        }
                    };
                }
                let key = format!("{target:?}");
                let last = match self.volumes.get(&key) {
                    Some((v, at)) if at.elapsed() < VOLUME_MEMORY => Some(*v),
                    _ => None,
                };
                let value = match input {
                    InputEvent::Turn(_) | InputEvent::Slide(_) => {
                        let (v, up) = match input {
                            InputEvent::Turn(d) => {
                                let base = match last {
                                    Some(v) => v,
                                    None => or_silent!(volume::get(&target)),
                                };
                                ((base + param_f64(p, "step", 2.0) as f32 / 100.0 * d as f32).clamp(0.0, 1.0), d > 0)
                            }
                            InputEvent::Slide(s) => {
                                let v = s as f32 / 100.0;
                                (v, last.is_none_or(|l| v >= l))
                            }
                            InputEvent::Press(_) => unreachable!("matched above"),
                        };
                        if popup {
                            volume::with_popup(v, up);
                        } else {
                            or_silent!(volume::set(&target, v));
                        }
                        self.volumes.insert(key, (v, std::time::Instant::now()));
                        v
                    }
                    InputEvent::Press(true) => {
                        let muted = if popup {
                            let muted = !volume::is_muted(&target)?;
                            keyboard(move |e| e.key(Key::VolumeMute, Direction::Click).map_err(|e| e.to_string()))?;
                            muted
                        } else {
                            match volume::toggle_mute(&target) {
                                Ok(m) => m,
                                // an app that is not open: a press opens it (owner, 2026-10-14: the
                                // Spotify knob should start Spotify when it is closed)
                                Err(e) if e.ends_with(volume::SILENT) && open_closed_app(&target)? => {
                                    out.feedback = Some(Feedback { value: None, caption: format!("{caption}: opening") });
                                    return Ok(out);
                                }
                                other => or_silent!(other),
                            }
                        };
                        out.feedback = Some(Feedback { value: Some(if muted { 0.0 } else { or_silent!(volume::get(&target)) }), caption: if muted { format!("{caption}: muted") } else { caption } });
                        return Ok(out);
                    }
                    InputEvent::Press(false) => return Ok(out),
                };
                out.feedback = Some(Feedback { value: Some(value), caption });
            }

            ("core.scroll", InputEvent::Turn(d)) => {
                let amount = param_f64(p, "amount", 3.0).round() as i32;
                let axis = if param_str(p, "axis") == "horizontal" { Axis::Horizontal } else { Axis::Vertical };
                // a positive length scrolls down/right; turning up or clockwise should scroll up/left
                let by = -(d as i32) * amount;
                let sent = keyboard(move |e| e.scroll(by, axis).map_err(|e| e.to_string()));
                log::info!("scroll {by} {axis:?}: {sent:?}");
                sent?;
            }

            ("core.text", InputEvent::Press(true)) => {
                let text = param_str(p, "text");
                if !text.is_empty() {
                    let text = with_now(text, chrono::Local::now());
                    keyboard(move |e| e.text(&text).map_err(|e| e.to_string()))?;
                }
            }

            ("core.profile", InputEvent::Press(true)) => match param_str(p, "which") {
                "next" => out.profile_step = Some(1),
                "previous" => out.profile_step = Some(-1),
                _ => {
                    let id = param_str(p, "profile");
                    if !id.is_empty() {
                        out.switch_profile = Some(id.to_string());
                    }
                }
            },
            ("core.profile", InputEvent::Turn(d)) => out.profile_step = Some(d.signum() as i32),

            ("core.timer", InputEvent::Press(true)) => {
                out.timer = Some(match param_str(p, "press") {
                    "reset" => super::timer::TimerCmd::Reset,
                    "addMinute" => super::timer::TimerCmd::AddMinute,
                    _ => super::timer::TimerCmd::StartPause,
                })
            }
            ("core.clock", InputEvent::Press(true)) => {
                out.clock = match param_str(p, "press") {
                    "stopwatch" => Some(super::timer::ClockCmd::Stopwatch),
                    "stopwatchReset" => Some(super::timer::ClockCmd::StopwatchReset),
                    "hours" => Some(super::timer::ClockCmd::Hours),
                    _ => None,
                }
            }
            ("core.clock", InputEvent::Turn(d)) => out.clock = Some(super::timer::ClockCmd::View(d as i32)),
            ("core.pomodoro", InputEvent::Press(true)) => {
                out.pomodoro = Some(match param_str(p, "press") {
                    "skip" => super::timer::PomodoroCmd::Skip,
                    "reset" => super::timer::PomodoroCmd::Reset,
                    _ => super::timer::PomodoroCmd::StartPause,
                })
            }
            ("core.pet", InputEvent::Turn(d)) => out.pet = Some(super::pet::PetCmd::Play(d as i32)),
            ("core.pet", InputEvent::Press(true)) if param_str(p, "press") == "dial" => out.pet_dial = true,
            (super::actions::SAVER, InputEvent::Press(true)) => out.saver = Some(param_str(p, "effect").to_string()),
            ("core.pet", InputEvent::Press(true)) => {
                out.pet = Some(match param_str(p, "press") {
                    "feed" => super::pet::PetCmd::Feed,
                    "sleep" => super::pet::PetCmd::Sleep,
                    _ => super::pet::PetCmd::Pet,
                })
            }
            ("core.pomodoro", InputEvent::Turn(d)) => out.pomodoro = Some(super::timer::PomodoroCmd::Adjust(d as i32)),
            ("core.timer", InputEvent::Turn(d)) => out.timer = Some(super::timer::TimerCmd::Adjust(d as i32)),

            ("core.lighting", InputEvent::Press(true)) => {
                let effect = param_str(p, "effect");
                out.lighting = Some(Lighting {
                    target: param_str(p, "target").to_string(),
                    effect: (effect != "profile").then(|| super::leds::Override {
                        mode: super::leds::effect_mode(Some(effect)),
                        rgb: super::leds::parse_hex(param_str(p, "color")).unwrap_or([255, 255, 255]),
                        speed: param_f64(p, "speed", 50.0).clamp(0.0, 100.0) as u8,
                    }),
                });
            }
            ("core.deckBrightness", InputEvent::Press(true)) => {
                let step = param_f64(p, "step", 10.0) as i32;
                out.deck_brightness = Some(DeckBrightness::By(if param_str(p, "press") == "darker" { -step } else { step }));
            }
            ("core.deckBrightness", InputEvent::Turn(d)) => out.deck_brightness = Some(DeckBrightness::By(d as i32 * param_f64(p, "step", 10.0) as i32)),
            ("core.deckBrightness", InputEvent::Slide(v)) => out.deck_brightness = Some(DeckBrightness::Set(v)),

            // the player the dial shows; else the media key
            ("core.nowPlaying", InputEvent::Press(true)) => {
                let player = param_str(p, "player");
                if !player.trim().is_empty() {
                    super::media::prefer(player);
                }
                let (command, key) = match param_str(p, "press") {
                    "next" => (super::media::Command::Next, Key::MediaNextTrack),
                    "previous" => (super::media::Command::Previous, Key::MediaPrevTrack),
                    _ => (super::media::Command::PlayPause, Key::MediaPlayPause),
                };
                match super::media::command(command) {
                    Some(done) => done?,
                    None => keyboard(move |e| e.key(key, Direction::Click).map_err(|e| e.to_string()))?,
                }
            }
            ("core.nowPlaying", InputEvent::Turn(d)) => {
                if param_str(p, "turn") == "seek" {
                    // a detent is five seconds
                    super::media::seek(f64::from(d) * 5.0)?;
                } else if param_str(p, "turn") == "tracks" {
                    match super::media::command(if d > 0 { super::media::Command::Next } else { super::media::Command::Previous }) {
                        Some(done) => done?,
                        None => {
                            let key = if d > 0 { Key::MediaNextTrack } else { Key::MediaPrevTrack };
                            keyboard(move |e| e.key(key, Direction::Click).map_err(|e| e.to_string()))?;
                        }
                    }
                } else {
                    let volume = Binding { action: "core.volume".into(), params: serde_json::json!({"target": "system", "step": 2}), ..binding.clone() };
                    return self.run(&volume, input);
                }
            }

            ("core.audioDevice", InputEvent::Press(true) | InputEvent::Turn(_)) => {
                let input_dev = param_str(p, "kind") == "input";
                let name = match (input, param_str(p, "device").trim()) {
                    (InputEvent::Turn(d), _) => audiodev::cycle(input_dev, d.signum() as i32)?,
                    (_, "") => audiodev::cycle(input_dev, 1)?,
                    (_, device) => audiodev::set_default(input_dev, device)?,
                };
                out.feedback = Some(Feedback { value: None, caption: name });
            }

            (super::actions::DAY_MODE, InputEvent::Press(true)) => {
                // the devices first (a profile switch draws the deck and may take a moment)
                for (key, input_dev) in [("output", false), ("input", true)] {
                    let device = param_str(p, key).trim();
                    if !device.is_empty() {
                        audiodev::set_default(input_dev, device)?;
                    }
                }
                let profile = param_str(p, "profile").trim();
                if !profile.is_empty() {
                    out.switch_profile = Some(profile.to_string());
                }
                if let Some(rgb) = super::leds::parse_hex(param_str(p, "color").trim()) {
                    out.lighting = Some(Lighting { target: "base".into(), effect: Some(super::leds::Override { mode: mh_protocol::report::LedMode::Static, rgb, speed: 50 }) });
                }
                out.feedback = Some(Feedback { value: None, caption: binding.look.label.clone().unwrap_or_else(|| "Mode".into()) });
            }

            ("core.run", InputEvent::Press(true)) => run_command(param_str(p, "command"), param_str(p, "args"), param_str(p, "folder"), param_str(p, "window") != "normal")?,

            (super::states::ACTION, InputEvent::Press(true)) => {
                if let Some(caption) = super::states::press(p)? {
                    out.feedback = Some(Feedback { value: None, caption });
                }
            }

            // the weather scene on the dial: turning goes through the hours, a press through the days
            (super::live::ACTION, InputEvent::Turn(d)) if param_str(p, "dial") == "scene" => out.weather = Some(WeatherCmd::Hours(d as i32)),
            (super::live::ACTION, InputEvent::Press(true)) if param_str(p, "press") == "view" => out.weather = Some(WeatherCmd::View),
            (super::live::ACTION, InputEvent::Press(true)) if param_str(p, "press") == "forecast" => out.weather = Some(WeatherCmd::Forecast),
            (super::live::ACTION, InputEvent::Press(true)) => {
                if let Some(caption) = super::live::press(p)? {
                    out.feedback = Some(Feedback { value: None, caption });
                }
            }

            ("core.monitor", InputEvent::Turn(d)) => out.monitor_step = Some(d as i32),
            ("core.monitor", InputEvent::Press(true)) if param_str(p, "press") == "next" => out.monitor_step = Some(1),
            ("core.monitor", InputEvent::Press(true)) => {
                let tool = if cfg!(target_os = "macos") { "/System/Applications/Utilities/Activity Monitor.app" } else { "taskmgr.exe" };
                crate::system::shell_open(tool)?;
            }

            ("core.brightness", InputEvent::Turn(_) | InputEvent::Slide(_)) => {
                let which = brightness::Which::from_param(param_str(p, "monitor"));
                let value = match input {
                    InputEvent::Turn(d) => brightness::get(which)? + param_f64(p, "step", 5.0) as f32 * d as f32,
                    InputEvent::Slide(v) => v as f32,
                    InputEvent::Press(_) => unreachable!("matched above"),
                };
                let value = value.clamp(0.0, 100.0);
                brightness::set(which, value);
                out.feedback = Some(Feedback { value: Some(value / 100.0), caption: "Brightness".into() });
            }

            ("obs.scene", InputEvent::Press(true)) => obs::request(obs::Request::Scene(param_str(p, "scene").trim().to_string())),
            ("obs.record", InputEvent::Press(true)) => obs::request(obs::Request::ToggleRecord),
            ("obs.stream", InputEvent::Press(true)) => obs::request(obs::Request::ToggleStream),
            ("obs.replay", InputEvent::Press(true)) => obs::request(obs::Request::SaveReplay),
            ("obs.mute", InputEvent::Press(true)) => obs::request(obs::Request::ToggleMute(param_str(p, "input").trim().to_string())),
            ("obs.volume", InputEvent::Turn(d)) => {
                obs::request(obs::Request::AdjustVolume { input: param_str(p, "input").trim().to_string(), db: param_f64(p, "step", 1.0) as f32 * d as f32 });
            }
            ("obs.volume", InputEvent::Slide(v)) => {
                obs::request(obs::Request::SetVolume { input: param_str(p, "input").trim().to_string(), level: v as f32 / 100.0 });
                out.feedback = Some(Feedback { value: Some(v as f32 / 100.0), caption: param_str(p, "input").to_string() });
            }
            ("twitch.chat", InputEvent::Press(true)) => twitch::request(twitch::Request::Chat(required(p, "message", "the message")?))?,
            ("twitch.clearChat", InputEvent::Press(true)) => twitch::request(twitch::Request::ClearChat)?,
            ("twitch.chatMode", InputEvent::Press(true)) => {
                twitch::request(twitch::Request::ChatMode { mode: param_str(p, "mode").to_string(), duration: param_f64(p, "duration", 10.0).max(0.0) as u32 })?
            }
            ("twitch.shield", InputEvent::Press(true)) => twitch::request(twitch::Request::Shield)?,
            ("twitch.ad", InputEvent::Press(true)) => twitch::request(twitch::Request::Ad(param_f64(p, "length", 30.0) as u32))?,
            ("twitch.title", InputEvent::Press(true)) => twitch::request(twitch::Request::TitleGame { title: param_str(p, "title").to_string(), game: param_str(p, "game").to_string() })?,
            ("twitch.marker", InputEvent::Press(true)) => twitch::request(twitch::Request::Marker(param_str(p, "description").to_string()))?,
            ("twitch.clip", InputEvent::Press(true)) => twitch::request(twitch::Request::Clip)?,
            ("twitch.openClip", InputEvent::Press(true)) => twitch::request(twitch::Request::OpenLastClip)?,
            ("streamlabs.scene", InputEvent::Press(true)) => sl::request(sl::Request::Scene(required(p, "scene", "the scene")?)),
            ("streamlabs.record", InputEvent::Press(true)) => sl::request(sl::Request::ToggleRecord),
            ("streamlabs.stream", InputEvent::Press(true)) => sl::request(sl::Request::ToggleStream),
            ("streamlabs.replay", InputEvent::Press(true)) => sl::request(sl::Request::SaveReplay),
            ("streamlabs.replayBuffer", InputEvent::Press(true)) => sl::request(sl::Request::ToggleReplayBuffer),
            ("streamlabs.studioMode", InputEvent::Press(true)) => sl::request(sl::Request::ToggleStudioMode),
            ("streamlabs.transition", InputEvent::Press(true)) => sl::request(sl::Request::Transition),
            ("streamlabs.source", InputEvent::Press(true)) => sl::request(sl::Request::ToggleSource { scene: param_str(p, "scene").trim().to_string(), source: required(p, "source", "the source")? }),
            ("streamlabs.mute", InputEvent::Press(true)) => sl::request(sl::Request::ToggleMute(required(p, "input", "the audio source")?)),
            ("streamlabs.volume", InputEvent::Turn(d)) => {
                sl::request(sl::Request::AdjustVolume { source: required(p, "input", "the audio source")?, by: param_f64(p, "step", 2.0) as f32 / 100.0 * d as f32 })
            }
            ("streamlabs.volume", InputEvent::Slide(v)) => sl::request(sl::Request::SetVolume { source: required(p, "input", "the audio source")?, level: v as f32 / 100.0 }),
            ("obs.recordPause", InputEvent::Press(true)) => obs::request(obs::Request::ToggleRecordPause),
            ("obs.virtualCam", InputEvent::Press(true)) => obs::request(obs::Request::ToggleVirtualCam),
            ("obs.replayBuffer", InputEvent::Press(true)) => obs::request(obs::Request::ToggleReplayBuffer),
            ("obs.studioMode", InputEvent::Press(true)) => obs::request(obs::Request::ToggleStudioMode),
            ("obs.transition", InputEvent::Press(true)) => obs::request(obs::Request::Transition(param_str(p, "transition").trim().to_string())),
            ("obs.sceneCollection", InputEvent::Press(true)) => obs::request(obs::Request::SceneCollection(required(p, "collection", "the scene collection")?)),
            ("obs.profile", InputEvent::Press(true)) => obs::request(obs::Request::Profile(required(p, "profile", "the OBS profile")?)),
            ("obs.filter", InputEvent::Press(true)) => obs::request(obs::Request::ToggleFilter { source: required(p, "source", "the source")?, filter: required(p, "filter", "the filter")? }),
            ("obs.screenshot", InputEvent::Press(true)) => {
                obs::request(obs::Request::Screenshot { source: param_str(p, "source").trim().to_string(), folder: param_str(p, "folder").trim().to_string() })
            }
            ("obs.source", InputEvent::Press(true)) => {
                obs::request(obs::Request::ToggleSource { scene: param_str(p, "scene").trim().to_string(), source: param_str(p, "source").trim().to_string() });
            }

            ("spotify.like", InputEvent::Press(true)) => spotify::request(spotify::Request::ToggleLike),
            ("spotify.remove", InputEvent::Press(true)) => spotify::request(spotify::Request::RemoveFromPlaylist),
            ("spotify.add", InputEvent::Press(true)) => spotify::request(spotify::Request::AddToPlaylist(param_str(p, "playlist").to_string())),
            ("ha.toggle", InputEvent::Press(true)) => ha::request(ha::Request::Toggle(entity(p)?)),
            ("ha.activate", InputEvent::Press(true)) => ha::request(ha::Request::Activate(entity(p)?)),
            ("ha.brightness", InputEvent::Press(true)) => ha::request(ha::Request::Toggle(entity(p)?)),
            ("ha.brightness", InputEvent::Turn(d)) => ha::request(ha::Request::BrightnessStep { entity: entity(p)?, step: param_f64(p, "step", 5.0) as f32 * d as f32 }),
            ("ha.brightness", InputEvent::Slide(v)) => {
                ha::request(ha::Request::Brightness { entity: entity(p)?, percent: v as f32 });
                out.feedback = Some(Feedback { value: Some(v as f32 / 100.0), caption: "Light".into() });
            }
            ("ha.service", InputEvent::Press(true)) => {
                let data: serde_json::Value = match param_str(p, "data").trim() {
                    "" => serde_json::json!({}),
                    text => serde_json::from_str(text).map_err(|e| format!("the data is not valid JSON: {e}"))?,
                };
                ha::request(ha::Request::Service { service: param_str(p, "service").to_string(), data });
            }

            ("core.seek", InputEvent::Press(true)) => {
                let (fwd, back) = super::seek::keys(param_str(p, "player"));
                let combo = (if param_str(p, "press") == "back" { back } else { fwd }).to_string();
                keyboard(move |e| send_if_set(e, &combo))?;
            }
            ("core.seek", InputEvent::Turn(d)) => {
                let (fwd, back) = super::seek::keys(param_str(p, "player"));
                let combo = (if d > 0 { fwd } else { back }).to_string();
                let times = d.unsigned_abs();
                keyboard(move |e| (0..times).try_for_each(|_| send_if_set(e, &combo)))?;
            }
            ("core.sound", InputEvent::Press(true)) => match param_str(p, "mode") {
                "stopAll" => super::sound::send(super::sound::Cmd::StopAll),
                mode => {
                    let file = param_str(p, "file").trim().to_string();
                    super::sound::check_file(&file)?;
                    super::sound::send(super::sound::Cmd::Play {
                        file,
                        volume: param_f64(p, "volume", 80.0) as f32 / 100.0,
                        device: param_str(p, "device").trim().to_string(),
                        mode: super::sound::Mode::from_param(mode),
                    });
                }
            },

            ("core.mic", InputEvent::Press(true)) => {
                let muted = volume::toggle_mute(&volume::Target::Mic)?;
                out.feedback = Some(Feedback { value: None, caption: if muted { "Mic off".into() } else { "Mic on".into() } });
            }

            ("core.page", InputEvent::Press(true)) => out.page = Some(param_str(p, "page").to_string()),
            ("core.nextPage", InputEvent::Press(true)) => out.page = Some(NEXT_PAGE.to_string()),
            ("core.previousPage", InputEvent::Press(true)) => out.page = Some(PREVIOUS_PAGE.to_string()),

            // an input this action does not use (e.g. turning a knob bound to "Open app")
            _ => {}
        }
        // keys sent to an app that runs as administrator never arrive: say so (they were sent anyway)
        if matches!(binding.action.as_str(), "core.hotkey" | "core.text")
            && matches!(input, InputEvent::Press(true) | InputEvent::Turn(_))
            && let Some(reason) = super::elevated::blocked_reason()
        {
            return Err(reason);
        }
        Ok(out)
    }
}

/// A text param that must be set: trimmed, or an error naming what is missing.
fn required(p: &serde_json::Value, key: &str, what: &str) -> Result<String, String> {
    let v = param_str(p, key).trim();
    if v.is_empty() { Err(format!("set {what} first")) } else { Ok(v.to_string()) }
}

/// Split arguments on spaces, keeping "quoted parts" together.
pub fn split_args(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

/// Start a program or script; scripts go through their interpreter. Never waits for it.
fn run_command(command: &str, args: &str, folder: &str, hidden: bool) -> Result<(), String> {
    let command = command.trim().trim_matches('"');
    if command.is_empty() {
        return Err("set the program or script to run".into());
    }
    let lower = command.to_lowercase();
    let mut cmd = if lower.ends_with(".ps1") {
        let mut c = std::process::Command::new("powershell.exe");
        c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", command]);
        c
    } else if lower.ends_with(".bat") || lower.ends_with(".cmd") {
        let mut c = std::process::Command::new("cmd.exe");
        c.args(["/C", command]);
        c
    } else if lower.ends_with(".sh") {
        let mut c = std::process::Command::new("sh");
        c.arg(command);
        c
    } else {
        std::process::Command::new(command)
    };
    cmd.args(split_args(args));
    if !folder.trim().is_empty() {
        cmd.current_dir(folder.trim());
    }
    #[cfg(windows)]
    if hidden {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))]
    let _ = hidden;
    cmd.spawn().map(|_| ()).map_err(|e| format!("cannot run {command}: {e}"))
}

/// The `entity` param of a Home Assistant action, e.g. `light.desk`.
fn entity(p: &serde_json::Value) -> Result<String, String> {
    let e = param_str(p, "entity").trim();
    if e.contains('.') { Ok(e.to_string()) } else { Err("set the entity, e.g. light.desk".into()) }
}

/// One HTTP request of a key (`core.http`): (status, the answer's text). A short timeout, since a
/// key press waits for it; statuses of 400 and up are errors the key shows.
pub fn http_request(method: &str, url: &str, body: &str, headers: &str) -> Result<(u16, String), String> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("set an address starting with http:// or https://".into());
    }
    let agent: ureq::Agent = crate::engine::calls::builder().timeout_global(Some(std::time::Duration::from_secs(5))).http_status_as_error(false).build().into();
    let headers: Vec<(String, String)> = headers.lines().filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).filter(|(k, _)| !k.is_empty()).collect();
    let json = body.trim_start().starts_with(['{', '[']);
    macro_rules! with_headers {
        ($req:expr) => {{
            let mut r = $req;
            for (k, v) in &headers {
                r = r.header(k.as_str(), v.as_str());
            }
            r
        }};
    }
    let sent = match method.to_uppercase().as_str() {
        "GET" | "" => with_headers!(agent.get(url)).call(),
        "DELETE" => with_headers!(agent.delete(url)).call(),
        m @ ("POST" | "PUT" | "PATCH") => {
            let r = match m {
                "POST" => agent.post(url),
                "PUT" => agent.put(url),
                _ => agent.patch(url),
            };
            let r = with_headers!(r);
            let r = if json { r.header("Content-Type", "application/json") } else { r };
            r.send(body)
        }
        other => return Err(format!("unknown method {other}")),
    };
    let mut answer = sent.map_err(|e| format!("{url}: {e}"))?;
    let code = answer.status().as_u16();
    let text = answer.body_mut().read_to_string().unwrap_or_default();
    if code >= 400 {
        return Err(format!("{url} answered {code} {}", text.trim().chars().take(80).collect::<String>()));
    }
    Ok((code, text))
}

fn send_if_set(enigo: &mut Enigo, combo: &str) -> Result<(), String> {
    if combo.trim().is_empty() {
        return Ok(());
    }
    // Windows ignores a sent Win+L (it locks only from the real keyboard): lock it directly
    #[cfg(windows)]
    if is_lock_combo(combo) {
        // SAFETY: a plain call without arguments; it only starts locking the session
        return unsafe { windows::Win32::System::Shutdown::LockWorkStation() }.map_err(|e| format!("cannot lock: {e}"));
    }
    // macOS takes its screenshot shortcuts only from the real keyboard: run its own tool instead
    #[cfg(target_os = "macos")]
    if let Some(shot) = mac_screenshot(combo) {
        return shot;
    }
    hotkey::send(enigo, combo)
}

/// Cmd+Shift+3 / 4 / 5 on a Mac: the whole screen or a part of it to the desktop, or the Screenshot
/// app (owner, 2026-10-14: the tour's Screenshot key did nothing on his Mac).
#[cfg(target_os = "macos")]
fn mac_screenshot(combo: &str) -> Option<Result<(), String>> {
    let mut parts: Vec<String> = combo.split('+').map(|p| p.trim().to_lowercase()).collect();
    let key = parts.pop()?;
    parts.sort();
    if parts != ["cmd", "shift"] {
        return None;
    }
    let file = || {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/Desktop/Screenshot {}.png", chrono::Local::now().format("%Y-%m-%d at %H.%M.%S"))
    };
    let spawn = |cmd: &str, args: Vec<String>| std::process::Command::new(cmd).args(args).spawn().map(|_| ()).map_err(|e| e.to_string());
    match key.as_str() {
        "3" => Some(spawn("/usr/sbin/screencapture", vec![file()])),
        "4" => Some(spawn("/usr/sbin/screencapture", vec!["-i".into(), file()])),
        "5" => Some(spawn("/usr/bin/open", vec!["-a".into(), "Screenshot".into()])),
        _ => None,
    }
}

/// `Win+L` in any spelling (`win + l`, `Meta+L`).
#[cfg_attr(not(windows), allow(dead_code))]
fn is_lock_combo(combo: &str) -> bool {
    let parts: Vec<String> = combo.split('+').map(|p| p.trim().to_lowercase()).collect();
    parts.len() == 2 && ["win", "meta", "super", "cmd"].contains(&parts[0].as_str()) && parts[1] == "l"
}

/// A press on the volume of an app with no window open: open it, when HubDeck knows where it is
/// (Spotify by its own link). False when it is open already (it is only silent) or unknown.
fn open_closed_app(target: &volume::Target) -> Result<bool, String> {
    let volume::Target::App(app) = target else { return Ok(false) };
    let stem = app.trim().trim_end_matches(".exe").to_lowercase();
    let open = super::apps::list().iter().any(|a| a.running && a.exe.to_lowercase().trim_end_matches(".exe") == stem);
    if open {
        return Ok(false);
    }
    let Some(what) = (if stem == "spotify" { Some("spotify:".to_string()) } else { super::installed::target_named(&stem) }) else { return Ok(false) };
    crate::system::shell_open(&what)?;
    log::info!("{app} was closed: opened {what}");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_and_time_are_filled_in_typed_text() {
        use chrono::TimeZone;
        let now = chrono::Local.with_ymd_and_hms(2026, 10, 16, 9, 5, 0).unwrap();
        assert_eq!(with_now("{date} {time}", now), "2026-10-16 09:05");
        assert_eq!(with_now("no marks", now), "no marks");
    }
    use serde_json::json;

    #[test]
    fn win_l_is_recognised_as_locking() {
        assert!(is_lock_combo("Win+L") && is_lock_combo(" win + l ") && is_lock_combo("Meta+L"));
        assert!(!is_lock_combo("Ctrl+L") && !is_lock_combo("Win+Shift+L") && !is_lock_combo("L"));
    }

    fn binding(action: &str, params: serde_json::Value) -> Binding {
        Binding { action: action.into(), params, look: Default::default(), accel: false, look_on: None, swap_turns: false }
    }

    #[test]
    fn inputs_an_action_does_not_use_are_ignored() {
        let mut x = Executor::default();
        // turning a knob bound to "open" and releasing any key do nothing (and touch no OS API)
        assert_eq!(x.run(&binding("core.launch", json!({"target": "x"})), InputEvent::Turn(1)).unwrap(), Outcome::default());
        assert_eq!(x.run(&binding("core.hotkey", json!({})), InputEvent::Press(false)).unwrap(), Outcome::default());
        assert_eq!(x.run(&binding("unknown.action", json!({})), InputEvent::Press(true)).unwrap(), Outcome::default());
    }

    #[test]
    fn profile_switch_is_reported_not_performed() {
        let mut x = Executor::default();
        let out = x.run(&binding("core.profile", json!({"profile": "abc"})), InputEvent::Press(true)).unwrap();
        assert_eq!(out.switch_profile.as_deref(), Some("abc"));
    }

    #[test]
    fn profiles_step_in_order_and_wrap() {
        let ids: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!(step_profile(&ids, Some("a"), 1).as_deref(), Some("b"));
        assert_eq!(step_profile(&ids, Some("c"), 1).as_deref(), Some("a"));
        assert_eq!(step_profile(&ids, Some("a"), -1).as_deref(), Some("c"));
        assert_eq!(step_profile(&ids, None, -1).as_deref(), Some("c"));
        assert_eq!(step_profile(&[], None, 1), None);
        let mut x = Executor::default();
        assert_eq!(x.run(&binding("core.profile", json!({"which": "previous"})), InputEvent::Press(true)).unwrap().profile_step, Some(-1));
        assert_eq!(x.run(&binding("core.profile", json!({"which": "named"})), InputEvent::Turn(-3)).unwrap().profile_step, Some(-1));
    }

    #[test]
    fn deck_brightness_stays_in_range() {
        assert_eq!(DeckBrightness::By(10).apply(95), 100);
        assert_eq!(DeckBrightness::By(-30).apply(20), MIN_BRIGHTNESS);
        assert_eq!(DeckBrightness::Set(0).apply(50), MIN_BRIGHTNESS);
        let mut x = Executor::default();
        let out = x.run(&binding("core.deckBrightness", json!({"press": "darker", "step": 15})), InputEvent::Press(true)).unwrap();
        assert_eq!(out.deck_brightness, Some(DeckBrightness::By(-15)));
        assert_eq!(x.run(&binding("core.deckBrightness", json!({})), InputEvent::Turn(2)).unwrap().deck_brightness, Some(DeckBrightness::By(20)));
    }

    #[test]
    fn arguments_split_with_quotes() {
        assert_eq!(split_args(r#"-a "two words" b"#), vec!["-a", "two words", "b"]);
        assert_eq!(split_args(r#""" x"#), vec!["", "x"]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn launch_without_target_explains() {
        let mut x = Executor::default();
        assert!(x.run(&binding("core.launch", json!({})), InputEvent::Press(true)).unwrap_err().contains("target"));
    }
}
