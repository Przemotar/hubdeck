//! `hubdeck cli doctor`: checks the system integrations the deck's controls use, on the machine
//! it runs on. Every check runs the same code a key, dial or fader runs, or says why it cannot
//! run here. Volume and mute are put back; nothing else is changed. Home Assistant, Hue and other
//! smart-home integrations are not part of it.

use std::fmt::Write as _;

use super::mcp::Api;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Fail,
    Skip,
}

impl Status {
    fn mark(self) -> &'static str {
        match self {
            Status::Ok => "ok  ",
            Status::Fail => "FAIL",
            Status::Skip => "skip",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    fn add(&mut self, name: &'static str, status: Status, detail: impl Into<String>) {
        self.checks.push(Check { name, status, detail: detail.into() });
    }

    pub fn ok(&mut self, name: &'static str, detail: impl Into<String>) {
        self.add(name, Status::Ok, detail);
    }

    pub fn fail(&mut self, name: &'static str, detail: impl Into<String>) {
        self.add(name, Status::Fail, detail);
    }

    pub fn skip(&mut self, name: &'static str, detail: impl Into<String>) {
        self.add(name, Status::Skip, detail);
    }

    pub fn failures(&self) -> usize {
        self.checks.iter().filter(|c| c.status == Status::Fail).count()
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "HubDeck {} doctor, {} ({}, {})", env!("CARGO_PKG_VERSION"), sysinfo::System::long_os_version().unwrap_or_default(), std::env::consts::OS, std::env::consts::ARCH);
        for check in &self.checks {
            let _ = writeln!(out, "  {}  {:<22} {}", check.status.mark(), check.name, check.detail);
        }
        let skipped = self.checks.iter().filter(|c| c.status == Status::Skip).count();
        let _ = writeln!(out, "\n{} checks: {} failed, {} skipped", self.checks.len(), self.failures(), skipped);
        out
    }
}

/// Run every check. `api` reaches the running app for the deck's modules; the rest talks to the
/// system directly, so a broken app still gets a useful answer.
pub fn run(api: &dyn Api) -> Report {
    let mut report = Report::default();
    deck(&mut report, api);
    system(&mut report);
    report
}

fn deck(report: &mut Report, api: &dyn Api) {
    match api.call("GET", "/api/modules", None) {
        Ok(m) => {
            let attached: Vec<String> = m["attached"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
            if attached.is_empty() {
                report.fail("deck modules", "none attached: check the USB cable and the base");
            } else {
                report.ok("deck modules", attached.join(", "));
            }
        }
        Err(e) => report.fail("deck modules", format!("HubDeck is not answering (start it first): {e}")),
    }
}

#[cfg(target_os = "linux")]
fn system(report: &mut Report) {
    use super::volume::{self, Target};

    let percent = |level: f32| format!("{}%", (level * 100.0).round() as i32);

    // the setter the volume keys and the dial use, without changing what comes out of the speakers
    match volume::get(&Target::System) {
        Ok(level) => match volume::set(&Target::System, level).and_then(|()| volume::get(&Target::System)) {
            Ok(back) if (back - level).abs() <= 0.02 => report.ok("system volume", format!("{} (set and read back)", percent(back))),
            Ok(back) => report.fail("system volume", format!("set {} but read back {}", percent(level), percent(back))),
            Err(e) => report.fail("system volume", e),
        },
        Err(e) => report.fail("system volume", e),
    }

    // toggle twice, so the state is put back even if the first answer was surprising
    match volume::is_muted(&Target::System) {
        Ok(true) => report.ok("mute", "muted (left as it was)"),
        Ok(false) => {
            let on = volume::toggle_mute(&Target::System);
            // always toggle back after a successful toggle: a test must not leave the system muted
            let back = if on.is_ok() { volume::toggle_mute(&Target::System) } else { Err("not toggled".into()) };
            match (on, back) {
                (Ok(true), Ok(false)) => report.ok("mute", "off -> on -> off"),
                (Ok(true), Ok(true)) => report.fail("mute", "turning mute off again did not take"),
                (Ok(false), _) => report.fail("mute", "turning mute on did not take (state put back)"),
                (Ok(_), Err(e)) => report.fail("mute", format!("could not unmute again: {e}")),
                (Err(e), _) => report.fail("mute", e),
            }
        }
        Err(e) => report.fail("mute", e),
    }

    match (volume::get(&Target::Mic), volume::is_muted(&Target::Mic)) {
        (Ok(level), Ok(muted)) => report.ok("microphone", format!("{} ({})", percent(level), if muted { "muted" } else { "on" })),
        (Err(e), _) | (_, Err(e)) => report.fail("microphone", e),
    }

    match super::media::current_now() {
        Some(track) => report.ok("media (MPRIS)", format!("{} - {} ({})", track.title, track.artist, if track.playing { "playing" } else { "paused" })),
        None => report.skip("media (MPRIS)", "no player has a track now: start Spotify or a browser tab and run this again"),
    }

    match super::idle::system_idle() {
        Some(d) => report.ok("idle time", format!("{} s since the last input", d.as_secs())),
        None => report.skip("idle time", "logind has no idle hint on this desktop (KDE keeps its own)"),
    }

    match super::idle::session_locked() {
        Some(true) => report.ok("session lock", "locked"),
        Some(false) => report.ok("session lock", "unlocked"),
        None => report.fail("session lock", "logind did not report LockedHint"),
    }

    let apps = super::installed::list(false).apps;
    if apps.is_empty() {
        report.fail("installed apps", "no .desktop entries found");
    } else {
        report.ok("installed apps", format!("{} entries", apps.len()));
    }

    let icon = apps
        .iter()
        .map(|a| a.logo.as_str())
        .find(|logo| !logo.is_empty())
        .map(str::to_string)
        .or_else(|| ["utilities-terminal", "firefox", "applications-system"].into_iter().find(|n| super::appicon::path(n).is_some()).map(str::to_string));
    match icon {
        Some(name) => match super::appicon::png(&name) {
            Ok(png) => report.ok("app icons", format!("{name}: {} bytes of PNG", png.len())),
            Err(e) => report.fail("app icons", e),
        },
        None => report.fail("app icons", "no icon found in the icon themes"),
    }

    match super::uinput::probe() {
        Ok(()) => report.ok("virtual keyboard", "/dev/uinput opens and a device is created"),
        Err(e) => report.fail("virtual keyboard", e),
    }

    match super::uinput::self_check() {
        Ok(n) => report.ok("key mappings", format!("{n} hotkey and text mappings")),
        Err(e) => report.fail("key mappings", e),
    }

    launch(report);

    match active_win_pos_rs::get_active_window() {
        Ok(win) => {
            let title = if win.title.is_empty() { win.app_name } else { win.title };
            report.ok("app in front", title);
        }
        Err(e) => report.skip("app in front", format!("not readable: {e:?}")),
    }

    match super::brightness::get(super::brightness::Which::All) {
        Ok(level) => report.ok("monitor brightness", format!("{}%", level.round())),
        Err(e) => report.skip("monitor brightness", e),
    }
}

/// A program started from a path, as `core.run` and an app key do, is a program that actually ran.
#[cfg(target_os = "linux")]
fn launch(report: &mut Report) {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = std::env::temp_dir().join(format!("hubdeck-doctor-{}", std::process::id()));
    let script = dir.join("open-test.sh");
    let marker = dir.join("opened");
    let _ = std::fs::remove_dir_all(&dir);
    let prepared = std::fs::create_dir_all(&dir).is_ok()
        && std::fs::write(&script, format!("#!/bin/sh\n: > '{}'\n", marker.display())).is_ok()
        && std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).is_ok();
    if !prepared {
        report.fail("launch", "cannot write a test script in the temp folder");
        return;
    }
    match crate::system::shell_open(&script.to_string_lossy()) {
        Ok(()) => {
            let mut seen = false;
            for _ in 0..40 {
                if marker.exists() {
                    seen = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if seen {
                report.ok("launch", "a program started from a path");
            } else {
                report.fail("launch", "the program did not start");
            }
        }
        Err(e) => report.fail("launch", e),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(not(target_os = "linux"))]
fn system(report: &mut Report) {
    report.skip("system checks", "the doctor's system checks are written for Linux");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_lists_every_check_and_counts_failures() {
        let mut report = Report::default();
        report.ok("volume", "50%");
        report.skip("media", "nothing playing");
        report.fail("keyboard", "no /dev/uinput");
        let text = report.text();
        assert!(text.contains("ok    volume"), "{text}");
        assert!(text.contains("skip  media"), "{text}");
        assert!(text.contains("FAIL  keyboard"), "{text}");
        assert!(text.contains("3 checks: 1 failed, 1 skipped"), "{text}");
        assert_eq!(report.failures(), 1);
    }
}
