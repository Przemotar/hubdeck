//! "Seek in a player": the forward and back shortcuts of common players, so a key or a knob jumps
//! in a video or song without the user looking them up. The players' own default shortcuts.

/// (id, name, forward, back).
pub const PLAYERS: [(&str, &str, &str, &str); 12] = [
    ("vlc", "VLC (10 s)", "Alt+Right", "Alt+Left"),
    ("vlcShort", "VLC (3 s)", "Shift+Right", "Shift+Left"),
    ("mediaPlayer", "Windows Media Player (new)", "Ctrl+Right", "Ctrl+Left"),
    ("wmpLegacy", "Windows Media Player Legacy", "Ctrl+Shift+F", "Ctrl+Shift+B"),
    ("mpc", "MPC-HC / MPC-BE", "Right", "Left"),
    ("potplayer", "PotPlayer", "Right", "Left"),
    ("kmplayer", "KMPlayer", "Right", "Left"),
    ("gom", "GOM Player", "Right", "Left"),
    ("youtube", "YouTube in a browser (10 s)", "L", "J"),
    ("spotify", "Spotify", "Shift+Right", "Shift+Left"),
    ("netflix", "Netflix in a browser", "Right", "Left"),
    ("arrows", "Arrow keys (most players)", "Right", "Left"),
];

/// Forward and back shortcuts of a player (arrow keys for an unknown one).
pub fn keys(player: &str) -> (&'static str, &'static str) {
    PLAYERS.iter().find(|(id, ..)| *id == player).map(|(_, _, f, b)| (*f, *b)).unwrap_or(("Right", "Left"))
}

/// The official app's player ids (`vlc-media-player`, `windows-media-player-legacy`...) -> ours.
pub fn from_official(id: &str) -> &'static str {
    let id = id.to_lowercase();
    if id.contains("vlc") {
        "vlc"
    } else if id.contains("legacy") {
        "wmpLegacy"
    } else if id.contains("windows") {
        "mediaPlayer"
    } else if id.contains("pot") {
        "potplayer"
    } else if id.contains("km") {
        "kmplayer"
    } else if id.contains("gom") {
        "gom"
    } else if id.contains("classic") || id.contains("mpc") {
        "mpc"
    } else {
        "arrows"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_player_shortcut_parses() {
        let mut ids = std::collections::HashSet::new();
        for (id, _, f, b) in PLAYERS {
            assert!(ids.insert(id), "unique {id}");
            assert!(super::super::hotkey::parse(f).is_some() && super::super::hotkey::parse(b).is_some(), "{id}");
        }
        assert_eq!(keys("youtube"), ("L", "J"));
        assert_eq!(keys("nope"), ("Right", "Left"));
        assert_eq!(from_official("windows-media-player-legacy"), "wmpLegacy");
        assert_eq!(from_official("vlc-media-player"), "vlc");
        assert_eq!(from_official("divx"), "arrows");
    }
}
