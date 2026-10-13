//! List connected modules with firmware and screen info. Keys and dial are switched to
//! software mode to answer, then handed back to hardware mode when the hub closes.
//!
//!     cargo run -p mh-device --example probe

use mh_device::Hub;
use mh_protocol::{Query, report};

fn main() -> mh_device::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let hub = Hub::start()?;
    let mut modules = hub.modules();
    modules.sort_by_key(|m| m.id.clone());
    if modules.is_empty() {
        println!("no modules found");
    }
    for m in &modules {
        let firmware = m.firmware()?.unwrap_or_else(|| "?".into());
        let mut line = format!("{:8} {:22} fw {firmware:12} out {:3} B", m.kind().name(), m.id.serial, m.output_len);
        if let Ok(info) = m.query(Query::DeviceInfo)
            && let Some((w, h)) = report::parse_screen_size(&info)
        {
            line += &format!("  screen {w}x{h}");
        }
        if let Ok(v) = m.query(Query::FaderValues)
            && let Some(values) = report::parse_fader_values(&v)
        {
            line += &format!("  faders {values:?}");
        }
        println!("{line}");
    }
    hub.close();
    Ok(())
}
