//! How long one LED color takes to reach the base, through the interrupt pipe (`write`, what
//! HubDeck uses) and through the control pipe (`send_output_report`, HID SET_REPORT). With a deck
//! attached: `cargo test -p mh-device --test output_timing -- --ignored --nocapture`. It only sets
//! the edge's color (dim green and violet), which the running app sets again at its next change.
use std::time::{Duration, Instant};

use mh_protocol::ModuleKind;
use mh_protocol::report::{self, LedTarget, Report};

#[test]
#[ignore]
fn edge_color_by_both_pipes() {
    let api = hidapi::HidApi::new().expect("hidapi");
    let Some(info) = api.device_list().find(|d| d.vendor_id() == mh_protocol::VENDOR_ID && d.product_id() == ModuleKind::Base.product_id()) else {
        println!("no base attached");
        return;
    };
    let device = info.open_device(&api).expect("open the base");
    let colors = [[0, 60, 0], [40, 0, 60]];
    let bytes = |i: usize| match report::led_color(ModuleKind::Base, LedTarget::All, colors[i % 2], false) {
        Some(Report::Output(mut b)) => {
            b.resize(64, 0);
            b
        }
        _ => panic!("the base takes colors as output reports"),
    };
    for (name, by_control) in [("interrupt pipe (write)", false), ("control pipe (send_output_report)", true)] {
        let mut times = Vec::new();
        for i in 0..20 {
            let b = bytes(i);
            let start = Instant::now();
            let r = if by_control { device.send_output_report(&b).map(|_| b.len()) } else { device.write(&b) };
            times.push(start.elapsed());
            if let Err(e) = r {
                println!("{name}: failed: {e}");
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        let ms: Vec<f32> = times.iter().map(|d| d.as_secs_f32() * 1000.0).collect();
        let avg = ms.iter().sum::<f32>() / ms.len().max(1) as f32;
        let max = ms.iter().fold(0.0f32, |m, v| m.max(*v));
        println!("{name}: {} sends, {avg:.1} ms each on average, {max:.1} ms at most", ms.len());
    }
}
