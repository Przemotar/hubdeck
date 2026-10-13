//! Is a call on? (owner, 2026-10-12: "wykrywanie rozmowy"). A call app that records the default
//! microphone right now means yes: Zoom, Teams, Discord, Slack, Webex, Skype, or a browser (Google
//! Meet and the web versions). Looked at every few seconds; the engine shows the call's profile and
//! the edge says whether the microphone is on.

use std::time::Duration;

/// How often the microphone's users are looked at.
pub const EVERY: Duration = Duration::from_secs(2);

/// Programs used for calls (lowercase, without `.exe`).
const CALL_APPS: [&str; 15] = ["zoom", "ms-teams", "teams", "discord", "slack", "webexmta", "ciscocollabhost", "skype", "chrome", "msedge", "firefox", "brave", "opera", "whatsapp", "telegram"];

/// The call app among the programs recording the microphone, if any.
pub fn call_app(mic_users: &[String]) -> Option<String> {
    mic_users
        .iter()
        .find(|exe| {
            let lower = exe.to_lowercase();
            CALL_APPS.contains(&lower.trim_end_matches(".exe"))
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cargo test -p hubdeck mic_users_now -- --ignored --nocapture`: who records the microphone now
    #[test]
    #[ignore]
    fn mic_users_now() {
        super::super::volume::init_thread();
        let users = super::super::volume::mic_users();
        eprintln!("recording the microphone: {users:?}; call app: {:?}", call_app(&users));
    }

    #[test]
    fn a_call_app_on_the_microphone_is_a_call() {
        assert_eq!(call_app(&["obs64.exe".into(), "Zoom.exe".into()]).as_deref(), Some("Zoom.exe"));
        assert_eq!(call_app(&["obs64.exe".into()]), None, "recording is not a call");
        assert_eq!(call_app(&[]), None);
    }
}
