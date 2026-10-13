//! Debug helper: open the first module of a kind, optionally enter software mode, dump feature reports.
//!
//!     cargo run -p mh-device --example raw_query -- keys 38 3a 3b

use hidapi::HidApi;
use mh_protocol::{ModuleKind, VENDOR_ID, report};

fn main() {
    let mut args = std::env::args().skip(1);
    let kind = ModuleKind::from_name(&args.next().unwrap_or("keys".into())).expect("module kind");
    let ids: Vec<u8> = args.map(|a| u8::from_str_radix(&a, 16).expect("hex report id")).collect();
    let api = HidApi::new().expect("hidapi");
    let info = api.device_list().find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == kind.product_id()).expect("module not connected");
    println!("path {:?} serial(descriptor) {:?}", info.path(), info.serial_number());
    let dev = info.open_device(&api).expect("open");
    if let Some(on) = report::software_mode(kind, true) {
        let mut buf = on.bytes().to_vec();
        buf.resize(report::FEATURE_LEN, 0);
        println!("software mode -> {:?}", dev.send_feature_report(&buf));
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    for id in ids {
        for size in [0x20usize, 0x21, 0x40] {
            let mut buf = vec![0u8; size];
            buf[0] = id;
            match dev.get_feature_report(&mut buf) {
                Ok(n) => println!("0x{id:02x} size {size:#x}: n={n} {:02x?}", &buf[..n.min(24)]),
                Err(e) => println!("0x{id:02x} size {size:#x}: ERR {e}"),
            }
        }
    }
    if let Some(off) = report::software_mode(kind, false) {
        let mut buf = off.bytes().to_vec();
        buf.resize(report::FEATURE_LEN, 0);
        let _ = dev.send_feature_report(&buf);
    }
}
