//! Re-enumerate one module (macOS), as pulling it off the base and back does, and time its return.
//! Opens nothing, so it works while the app runs (the app sees the module leave and come back).
//!
//!   cargo run -p mh-device --example reenumerate -- keys      # or dial, knobs, rollers, faders
use std::time::{Duration, Instant};

use mh_protocol::ModuleKind;

fn find(api: &mut hidapi::HidApi, kind: ModuleKind) -> Option<std::ffi::CString> {
    api.refresh_devices().ok()?;
    api.device_list().find(|d| d.vendor_id() == mh_protocol::VENDOR_ID && ModuleKind::from_product_id(d.product_id()) == Some(kind)).map(|d| d.path().to_owned())
}

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| "keys".into());
    let kind = [ModuleKind::Keys, ModuleKind::Dial, ModuleKind::Knobs, ModuleKind::Rollers, ModuleKind::Faders]
        .into_iter()
        .find(|k| k.name().eq_ignore_ascii_case(&name))
        .expect("kind: keys, dial, knobs, rollers or faders");
    let mut api = hidapi::HidApi::new().expect("hidapi");
    let Some(path) = find(&mut api, kind) else {
        eprintln!("no {} module attached", kind.name());
        std::process::exit(1);
    };
    println!("{} at {path:?}: re-enumerating", kind.name());
    let start = Instant::now();
    if let Err(e) = mh_device::recover::reenumerate(&path) {
        eprintln!("failed: {e}");
        std::process::exit(1);
    }
    println!("reset sent in {:?}", start.elapsed());
    let mut gone = false;
    while start.elapsed() < Duration::from_secs(20) {
        match find(&mut api, kind) {
            None => gone = true,
            Some(p) if p != path => {
                println!("back as {p:?} after {:?} (seen gone: {gone})", start.elapsed());
                return;
            }
            Some(_) => {}
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    eprintln!("not back within 20 s");
    std::process::exit(1);
}
