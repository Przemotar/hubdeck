//! Check that the key panel and dial backlight can be set: dims both to 20 %, reads the value back
//! (software mode, so the read is safe), then sets 100 % again.
//!
//!     cargo run -p mh-device --example backlight
//!
//! Quit HubDeck first.

use std::time::Duration;

use mh_device::Hub;
use mh_protocol::report;
use mh_protocol::{ModuleKind, Query};

fn main() {
    let hub = Hub::start().expect("hub");
    std::thread::sleep(Duration::from_millis(500));
    for kind in [ModuleKind::Keys, ModuleKind::Dial] {
        let Some(m) = hub.first(kind) else { continue };
        let before = m.query(Query::Backlight).map(|a| a.get(2).copied());
        m.send(&report::backlight(kind, 20).expect("has a backlight")).expect("send");
        std::thread::sleep(Duration::from_millis(300));
        let dimmed = m.query(Query::Backlight).map(|a| a.get(2).copied());
        std::thread::sleep(Duration::from_secs(2));
        m.send(&report::backlight(kind, 100).expect("has a backlight")).expect("send");
        std::thread::sleep(Duration::from_millis(300));
        let after = m.query(Query::Backlight).map(|a| a.get(2).copied());
        println!("{}: before {before:?}, set 20 -> read {dimmed:?}, set 100 -> read {after:?}", kind.name());
    }
    drop(hub);
}
