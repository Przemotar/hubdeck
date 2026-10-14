//! What is playing now: title, artist, cover art and play state of the app the system shows in its
//! media overlay (Spotify, a browser, a video player). Windows: the system media transport controls
//! (no API key, no app plugin). macOS: Spotify and Apple Music through AppleScript (the system's
//! own "now playing" is private there); the first time macOS asks the user to let HubDeck control
//! them. Elsewhere: not wired up yet, so nothing is reported.

use std::time::Duration;

use crossbeam_channel::Sender;

use super::Msg;

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    /// Cover art as `(mime type, bytes)`; None when the app gives none.
    pub cover: Option<(String, Vec<u8>)>,
    /// Where in the song: (seconds played, seconds long), when the player says.
    pub position: Option<(f32, f32)>,
}

const POLL: Duration = Duration::from_millis(1000);

/// Report the current track to the engine whenever it changes. Runs on its own thread forever.
pub fn watch(tx: Sender<Msg>) {
    platform::init();
    let mut last: Option<(String, String, bool)> = None;
    let mut sent_nothing = false;
    loop {
        match platform::current(last.as_ref().map(|(t, a, _)| (t.as_str(), a.as_str()))) {
            Some(track) => {
                // where in the song, every poll (the dial's ring follows it)
                if tx.send(Msg::Progress(track.position.map(|(at, of)| (at, of, track.playing)))).is_err() {
                    return;
                }
                let key = (track.title.clone(), track.artist.clone(), track.playing);
                if last.as_ref() != Some(&key) {
                    last = Some(key);
                    sent_nothing = false;
                    if tx.send(Msg::NowPlaying(Some(track))).is_err() {
                        return;
                    }
                }
            }
            None if !sent_nothing => {
                last = None;
                sent_nothing = true;
                if tx.send(Msg::NowPlaying(None)).is_err() {
                    return;
                }
            }
            None => {}
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(windows)]
mod platform {
    use windows::Media::Control::{GlobalSystemMediaTransportControlsSessionManager, GlobalSystemMediaTransportControlsSessionPlaybackStatus};
    use windows::Storage::Streams::DataReader;
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

    use super::Track;

    pub fn init() {
        // SAFETY: COM/WinRT initialisation for this thread only.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    thread_local! {
        /// Made once: asking Windows for the manager every second costs more than the poll itself.
        static MANAGER: std::cell::RefCell<Option<GlobalSystemMediaTransportControlsSessionManager>> = const { std::cell::RefCell::new(None) };
    }

    /// `same`: the title and artist reported last time; the cover is read again only for a new track.
    pub fn current(same: Option<(&str, &str)>) -> Option<Track> {
        let manager = MANAGER.with(|m| {
            let mut m = m.borrow_mut();
            if m.is_none() {
                *m = GlobalSystemMediaTransportControlsSessionManager::RequestAsync().ok().and_then(|op| op.join().ok());
            }
            m.clone()
        })?;
        let session = manager.GetCurrentSession().ok()?;
        let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
        let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
        let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
        if title.is_empty() && artist.is_empty() {
            return None;
        }
        let playing = session.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).map(|s| s == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing).unwrap_or(false);
        let cover = if same == Some((title.as_str(), artist.as_str())) { None } else { thumbnail(&props) };
        let position = session.GetTimelineProperties().ok().and_then(|t| {
            let at = t.Position().ok()?.Duration as f32 / 10_000_000.0;
            let end = t.EndTime().ok()?.Duration as f32 / 10_000_000.0;
            (end > 0.0).then_some((at, end))
        });
        Some(Track { title, artist, playing, cover, position })
    }

    pub fn seek(seconds: f64) -> Result<(), String> {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync().map_err(|e| e.to_string())?.join().map_err(|e| e.to_string())?;
        let session = manager.GetCurrentSession().map_err(|_| "nothing is playing".to_string())?;
        let timeline = session.GetTimelineProperties().map_err(|e| e.to_string())?;
        let now = timeline.Position().map_err(|e| e.to_string())?.Duration;
        let target = (now + (seconds * 10_000_000.0) as i64).max(0);
        session.TryChangePlaybackPositionAsync(target).map_err(|e| e.to_string())?.join().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Windows: the media keys go to the session the dial shows (the system's current one).
    pub fn command(_command: super::Command) -> Option<Result<(), String>> {
        None
    }

    pub fn seek_to(seconds: f64) -> Result<(), String> {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync().map_err(|e| e.to_string())?.join().map_err(|e| e.to_string())?;
        let session = manager.GetCurrentSession().map_err(|_| "nothing is playing".to_string())?;
        session.TryChangePlaybackPositionAsync((seconds * 10_000_000.0) as i64).map_err(|e| e.to_string())?.join().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn thumbnail(props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties) -> Option<(String, Vec<u8>)> {
        let stream = props.Thumbnail().ok()?.OpenReadAsync().ok()?.join().ok()?;
        let mime = stream.ContentType().map(|s| s.to_string()).unwrap_or_else(|_| "image/jpeg".into());
        let size = stream.Size().ok()? as u32;
        if size == 0 || size > 8 * 1024 * 1024 {
            return None;
        }
        let reader = DataReader::CreateDataReader(&stream).ok()?;
        reader.LoadAsync(size).ok()?.join().ok()?;
        let mut bytes = vec![0u8; size as usize];
        reader.ReadBytes(&mut bytes).ok()?;
        Some((if mime.starts_with("image/") { mime } else { "image/jpeg".into() }, bytes))
    }
}

/// Jump `seconds` forward (or back, negative) in what plays now. macOS: Spotify or Apple Music.
pub fn seek(seconds: f64) -> Result<(), String> {
    platform::seek(seconds)
}

/// Go to `seconds` from the start of what plays now (the dial scrubbing a song).
pub fn seek_to(seconds: f64) -> Result<(), String> {
    platform::seek_to(seconds.max(0.0))
}

/// Play/pause, next or previous track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    PlayPause,
    Next,
    Previous,
}

/// Send `command` to the player whose song the dial shows. macOS: the system's media keys go to
/// the app that played last (a YouTube tab) while the dial shows Spotify or Music (owner,
/// 2026-10-14: "klikam żeby ją wznowić, a wznawia się to co na youtube"), so the player is told
/// directly. None: no such player here (the caller presses the media key).
pub fn command(command: Command) -> Option<Result<(), String>> {
    platform::command(command)
}

#[cfg(target_os = "macos")]
mod platform {
    use super::Track;

    pub fn init() {}

    /// The players asked, in order; a player that is not running is not started by the question.
    const PLAYERS: [&str; 2] = ["Spotify", "Music"];

    fn osascript(script: &str) -> Option<String> {
        let out = std::process::Command::new("/usr/bin/osascript").args(["-e", script]).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn running(app: &str) -> bool {
        // a quick look at the process list first: AppleScript is slower and the poll is every second
        std::process::Command::new("/usr/bin/pgrep").args(["-xq", app]).status().is_ok_and(|s| s.success())
    }

    pub fn current(same: Option<(&str, &str)>) -> Option<Track> {
        for app in PLAYERS.into_iter().filter(|a| running(a)) {
            // tab-separated: state, title, artist, artwork address (Spotify only), position, length
            let art = if app == "Spotify" { "artwork url of current track" } else { "\"\"" };
            let script = format!(
                r#"if application "{app}" is running then tell application "{app}" to if player state is not stopped then return (player state as string) & tab & (name of current track) & tab & (artist of current track) & tab & ({art}) & tab & (player position as string) & tab & (duration of current track as string)"#
            );
            let Some(line) = osascript(&script).filter(|l| !l.is_empty()) else { continue };
            let mut parts = line.split('\t');
            let (state, title, artist, art) = (parts.next()?, parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            // numbers may come with a decimal comma; Spotify gives the length in milliseconds
            let num = |s: Option<&str>| s.and_then(|v| v.trim().replace(',', ".").parse::<f32>().ok());
            let (at, of) = (num(parts.next()), num(parts.next()).map(|d| if app == "Spotify" { d / 1000.0 } else { d }));
            let position = at.zip(of).filter(|(_, of)| *of > 0.0);
            if title.is_empty() && artist.is_empty() {
                continue;
            }
            let cover = if same == Some((title, artist)) || !art.starts_with("https://") { None } else { download(art) };
            return Some(Track { title: title.to_string(), artist: artist.to_string(), playing: state == "playing", cover, position });
        }
        None
    }

    fn download(url: &str) -> Option<(String, Vec<u8>)> {
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(5))).build().into();
        let mut res = agent.get(url).call().ok()?;
        let bytes = res.body_mut().with_config().limit(8 * 1024 * 1024).read_to_vec().ok()?;
        Some(("image/jpeg".to_string(), bytes))
    }

    /// The player whose song shows: the first running one with a song (as `current` picks it).
    fn shown() -> Option<&'static str> {
        PLAYERS
            .into_iter()
            .filter(|a| running(a))
            .find(|app| osascript(&format!(r#"if application "{app}" is running then tell application "{app}" to if player state is not stopped then return "yes""#)).is_some_and(|l| l == "yes"))
    }

    pub fn command(command: super::Command) -> Option<Result<(), String>> {
        let app = shown()?;
        let verb = match command {
            super::Command::PlayPause => "playpause",
            super::Command::Next => "next track",
            super::Command::Previous => "previous track",
        };
        Some(
            osascript(&format!(r#"tell application "{app}" to {verb}"#))
                .map(|_| ())
                .ok_or_else(|| format!("{app} did not answer (System Settings > Privacy & Security > Automation: allow HubDeck to control {app})")),
        )
    }

    pub fn seek(seconds: f64) -> Result<(), String> {
        let app = shown().ok_or("neither Spotify nor Music plays a song")?;
        osascript(&format!(r#"tell application "{app}" to set player position to (player position + ({seconds}))"#))
            .map(|_| ())
            .ok_or_else(|| format!("{app} did not seek (System Settings > Privacy & Security > Automation: allow HubDeck to control {app})"))
    }

    pub fn seek_to(seconds: f64) -> Result<(), String> {
        let app = shown().ok_or("neither Spotify nor Music plays a song")?;
        osascript(&format!(r#"tell application "{app}" to set player position to {seconds:.1}"#))
            .map(|_| ())
            .ok_or_else(|| format!("{app} did not seek (System Settings > Privacy & Security > Automation: allow HubDeck to control {app})"))
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::Track;

    pub fn init() {}

    pub fn current(_same: Option<(&str, &str)>) -> Option<Track> {
        None
    }

    pub fn seek(_seconds: f64) -> Result<(), String> {
        Err("seeking is not wired up on this system yet".into())
    }

    pub fn command(_command: super::Command) -> Option<Result<(), String>> {
        None
    }

    pub fn seek_to(_seconds: f64) -> Result<(), String> {
        Err("seeking is not wired up on this system yet".into())
    }
}
