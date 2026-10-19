//! Sensors from LibreHardwareMonitor, when the user runs it: its web server's `data.json`
//! (Options > Remote Web Server > Run, port 8085), or, up to its 0.9.4, WMI
//! (`root\LibreHardwareMonitor`, class `Sensor`; 0.9.5 dropped WMI with .NET 10, its issue #2143;
//! a user with an AMD CPU and GPU saw only "-" on 2026-10-19). That gives the CPU temperature and AMD / Intel GPU
//! load and temperature without HubDeck shipping a kernel driver (LibreHardwareMonitor brings its
//! own, installed by the user's choice). Read only while a control shows one of these values.
#![cfg_attr(not(windows), allow(dead_code))]

/// One WMI `Sensor` row: identifier (`/amdcpu/0/temperature/2`), name (`Core (Tctl/Tdie)`), type
/// (`Temperature`, `Load`...) and value.
#[derive(Debug, Clone, PartialEq)]
pub struct Sensor {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub value: f32,
}

/// What HubDeck uses from LibreHardwareMonitor.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Reading {
    pub cpu_temp: Option<f32>,
    pub gpu_load: Option<f32>,
    pub gpu_temp: Option<f32>,
}

fn is_cpu(id: &str) -> bool {
    id.contains("cpu/")
}

fn is_gpu(id: &str) -> bool {
    id.starts_with("/gpu")
}

/// The best sensor of a list: the first whose name contains one of `prefer` (in that order), else
/// the highest value.
fn best(list: &[&Sensor], prefer: &[&str]) -> Option<f32> {
    for p in prefer {
        if let Some(s) = list.iter().find(|s| s.name.to_lowercase().contains(p)) {
            return Some(s.value);
        }
    }
    list.iter().map(|s| s.value).fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v))))
}

/// Pick the CPU package temperature and the GPU core load and temperature.
pub fn pick(sensors: &[Sensor]) -> Reading {
    let of = |cpu: bool, kind: &str| -> Vec<&Sensor> { sensors.iter().filter(|s| s.kind == kind && if cpu { is_cpu(&s.id) } else { is_gpu(&s.id) } && s.value.is_finite()).collect() };
    Reading {
        cpu_temp: best(&of(true, "Temperature"), &["package", "tctl", "tdie", "core average", "core max"]),
        gpu_load: best(&of(false, "Load"), &["gpu core", "d3d 3d", "core"]),
        gpu_temp: best(&of(false, "Temperature"), &["gpu core", "core", "edge"]),
    }
}

/// LibreHardwareMonitor's web server on this computer (its default port).
pub const WEB: &str = "http://127.0.0.1:8085/data.json";

/// The temperature and load sensors in a `data.json` tree: nodes with `SensorId`, `Type` and the
/// number `RawValue` (else the shown `Value`, "61,0 °C", read up to its unit).
pub fn from_json(v: &serde_json::Value) -> Vec<Sensor> {
    fn walk(n: &serde_json::Value, out: &mut Vec<Sensor>) {
        let (id, kind) = (n["SensorId"].as_str(), n["Type"].as_str());
        if let (Some(id), Some(kind)) = (id, kind)
            && (kind == "Temperature" || kind == "Load")
        {
            let shown = || n["Value"].as_str().and_then(|s| s.split_whitespace().next()).and_then(|s| s.replace(',', ".").parse::<f32>().ok());
            if let Some(value) = n["RawValue"].as_f64().map(|v| v as f32).or_else(shown) {
                out.push(Sensor { id: id.to_string(), name: n["Text"].as_str().unwrap_or("").to_string(), kind: kind.to_string(), value });
            }
        }
        for c in n["Children"].as_array().into_iter().flatten() {
            walk(c, out);
        }
    }
    let mut out = Vec::new();
    walk(v, &mut out);
    out
}

/// The sensors from the web server now; Err when it does not answer (not running, or its web
/// server is off).
pub fn from_web() -> Result<Vec<Sensor>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_millis(600))).build().into();
    let v: serde_json::Value = agent.get(WEB).call().map_err(|e| e.to_string())?.body_mut().read_json().map_err(|e| e.to_string())?;
    Ok(from_json(&v))
}

#[cfg(windows)]
pub use win::Lhm;

#[cfg(windows)]
mod win {
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoSetProxyBlanket, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
    };
    use windows::Win32::System::Variant::{VARIANT, VT_BSTR, VT_R4, VT_R8, VariantClear};
    use windows::Win32::System::Wmi::{IWbemClassObject, IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_INFINITE, WbemLocator};
    use windows::core::{BSTR, PCWSTR, w};

    use super::Sensor;

    /// A connection to LibreHardwareMonitor's WMI namespace.
    pub struct Lhm {
        services: IWbemServices,
    }

    fn text(v: &VARIANT) -> Option<String> {
        // SAFETY: the union is read according to its type tag.
        unsafe {
            let inner = &v.Anonymous.Anonymous;
            (inner.vt == VT_BSTR).then(|| inner.Anonymous.bstrVal.to_string())
        }
    }

    fn number(v: &VARIANT) -> Option<f32> {
        // SAFETY: as above.
        unsafe {
            let inner = &v.Anonymous.Anonymous;
            match inner.vt {
                VT_R4 => Some(inner.Anonymous.fltVal),
                VT_R8 => Some(inner.Anonymous.dblVal as f32),
                _ => None,
            }
        }
    }

    fn get(obj: &IWbemClassObject, name: PCWSTR) -> VARIANT {
        let mut v = VARIANT::default();
        // SAFETY: a valid object and an out VARIANT that we clear after use.
        let _ = unsafe { obj.Get(name, 0, &mut v, None, None) };
        v
    }

    impl Lhm {
        /// Connect on this thread (COM is set up for it); None when LibreHardwareMonitor does not
        /// run (its namespace exists only then).
        pub fn connect() -> Option<Lhm> {
            // SAFETY: COM calls with valid arguments; failures end in None.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).ok()?;
                let services = locator.ConnectServer(&BSTR::from("root\\LibreHardwareMonitor"), &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None).ok()?;
                CoSetProxyBlanket(&services, 10, 0, PCWSTR::null(), RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE).ok()?;
                Some(Lhm { services })
            }
        }

        /// The temperature and load sensors now; Err when LibreHardwareMonitor went away.
        pub fn sensors(&self) -> Result<Vec<Sensor>, String> {
            let query = BSTR::from("SELECT Identifier, Name, SensorType, Value FROM Sensor WHERE SensorType = 'Temperature' OR SensorType = 'Load'");
            // SAFETY: as above; every VARIANT is cleared.
            unsafe {
                let rows = self.services.ExecQuery(&BSTR::from("WQL"), &query, WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY, None).map_err(|e| e.to_string())?;
                let mut out = Vec::new();
                loop {
                    let mut objs = [None];
                    let mut got = 0u32;
                    let _ = rows.Next(WBEM_INFINITE, &mut objs, &mut got);
                    let Some(obj) = objs[0].take().filter(|_| got == 1) else { break };
                    let mut id = get(&obj, w!("Identifier"));
                    let mut name = get(&obj, w!("Name"));
                    let mut kind = get(&obj, w!("SensorType"));
                    let mut value = get(&obj, w!("Value"));
                    if let (Some(i), Some(n), Some(k), Some(v)) = (text(&id), text(&name), text(&kind), number(&value)) {
                        out.push(Sensor { id: i, name: n, kind: k, value: v });
                    }
                    for v in [&mut id, &mut name, &mut kind, &mut value] {
                        let _ = VariantClear(v);
                    }
                }
                Ok(out)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With LibreHardwareMonitor running: `cargo test -p hubdeck lhm_live -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn lhm_live() {
        match Lhm::connect() {
            Some(l) => {
                let sensors = l.sensors().unwrap();
                println!("{} sensors; picked {:?}", sensors.len(), pick(&sensors));
            }
            None => println!("LibreHardwareMonitor is not running"),
        }
    }

    #[test]
    fn reads_the_web_servers_tree() {
        // the shape of LibreHardwareMonitor 0.9.6's data.json (HttpServer.cs GenerateJsonForNode)
        let v = serde_json::json!({"id": 0, "Text": "Sensor", "Children": [{"id": 1, "Text": "PC", "Children": [
            {"id": 2, "Text": "AMD Ryzen 7 7800X3D", "HardwareId": "/amdcpu/0", "Children": [
                {"id": 3, "Text": "Temperatures", "Children": [
                    {"id": 4, "Text": "Core (Tctl/Tdie)", "SensorId": "/amdcpu/0/temperature/2", "Type": "Temperature", "Value": "71,3 °C", "RawValue": 71.25, "Children": []},
                    {"id": 5, "Text": "CCD1 (Tdie)", "SensorId": "/amdcpu/0/temperature/3", "Type": "Temperature", "Value": "66,0 °C", "Children": []}
                ]},
                {"id": 6, "Text": "Clocks", "Children": [{"id": 7, "Text": "Core #1", "SensorId": "/amdcpu/0/clock/1", "Type": "Clock", "RawValue": 4800.0, "Children": []}]}
            ]},
            {"id": 8, "Text": "AMD Radeon RX 7800 XT", "HardwareId": "/gpu-amd/0", "Children": [
                {"id": 9, "Text": "GPU Core", "SensorId": "/gpu-amd/0/load/0", "Type": "Load", "Value": "35,0 %", "RawValue": 35.0, "Children": []},
                {"id": 10, "Text": "GPU Core", "SensorId": "/gpu-amd/0/temperature/0", "Type": "Temperature", "RawValue": null, "Value": "52,0 °C", "Children": []}
            ]}
        ]}]});
        let sensors = from_json(&v);
        assert_eq!(sensors.len(), 4, "temperatures and loads only");
        assert_eq!(sensors[1].value, 66.0, "the shown value, decimal comma, without RawValue");
        assert_eq!(pick(&sensors), Reading { cpu_temp: Some(71.25), gpu_load: Some(35.0), gpu_temp: Some(52.0) });
    }

    fn s(id: &str, name: &str, kind: &str, value: f32) -> Sensor {
        Sensor { id: id.into(), name: name.into(), kind: kind.into(), value }
    }

    #[test]
    fn picks_the_package_temperature_and_the_gpu_core() {
        let sensors = [
            s("/intelcpu/0/temperature/0", "CPU Core #1", "Temperature", 55.0),
            s("/intelcpu/0/temperature/8", "CPU Package", "Temperature", 61.0),
            s("/intelcpu/0/load/0", "CPU Total", "Load", 12.0),
            s("/gpu-amd/0/load/0", "GPU Core", "Load", 33.0),
            s("/gpu-amd/0/load/1", "GPU Memory Controller", "Load", 70.0),
            s("/gpu-amd/0/temperature/0", "GPU Core", "Temperature", 48.0),
            s("/nvme/0/temperature/0", "Temperature", "Temperature", 40.0),
        ];
        assert_eq!(pick(&sensors), Reading { cpu_temp: Some(61.0), gpu_load: Some(33.0), gpu_temp: Some(48.0) });
        let amd = [s("/amdcpu/0/temperature/2", "Core (Tctl/Tdie)", "Temperature", 70.5), s("/amdcpu/0/temperature/3", "CCD1 (Tdie)", "Temperature", 66.0)];
        assert_eq!(pick(&amd).cpu_temp, Some(70.5));
        let unnamed = [s("/lpc/cpu/temperature/0", "Core A", "Temperature", 50.0), s("/lpc/cpu/temperature/1", "Core B", "Temperature", 58.0)];
        assert_eq!(pick(&unnamed).cpu_temp, Some(58.0), "else the hottest");
        assert_eq!(pick(&[]), Reading::default());
    }
}
