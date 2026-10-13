//! How long one hot-plug scan (HID enumeration) takes: the hub runs one every second.
//!
//!     cargo run --release -p mh-device --example enumtime

use std::time::Instant;

#[allow(deprecated)] // measuring enumeration alone needs an api without the first one
fn main() {
    let mut api = hidapi::HidApi::new_without_enumerate().expect("hidapi");
    let rounds = 10;
    let t = Instant::now();
    for _ in 0..rounds {
        api.reset_devices().expect("reset");
        api.add_devices(mh_protocol::VENDOR_ID, 0).expect("enumerate");
    }
    let per = t.elapsed() / rounds;
    println!("one scan (vendor filter) {per:?}, {} devices", api.device_list().count());
}
