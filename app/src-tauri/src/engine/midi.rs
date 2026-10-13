//! MIDI output for any DAW or synth: faders and knobs send control changes, keys play notes.
//! Windows has no built-in virtual MIDI port, so the user runs one (e.g. loopMIDI) and picks it;
//! macOS can use the IAC driver. Connections stay open per port name.

use std::collections::HashMap;
use std::sync::Mutex;

use midir::{MidiOutput, MidiOutputConnection};

static OPEN: Mutex<Option<HashMap<String, MidiOutputConnection>>> = Mutex::new(None);
/// (port, channel, controller) -> value.
type CcValues = HashMap<(String, u8, u8), u8>;
/// Last value sent per (port, channel, cc), so knobs turn from where they are.
static CC: Mutex<Option<CcValues>> = Mutex::new(None);

pub fn ports() -> Vec<String> {
    let Ok(out) = MidiOutput::new("HubDeck") else { return Vec::new() };
    out.ports().iter().filter_map(|p| out.port_name(p).ok()).collect()
}

fn send(port: &str, bytes: &[u8]) -> Result<(), String> {
    let mut open = OPEN.lock().unwrap_or_else(|e| e.into_inner());
    let open = open.get_or_insert_with(HashMap::new);
    if let Some(conn) = open.get_mut(port) {
        if conn.send(bytes).is_ok() {
            return Ok(());
        }
        open.remove(port); // the port went away; reconnect below
    }
    let out = MidiOutput::new("HubDeck").map_err(|e| e.to_string())?;
    let found = out.ports().into_iter().find(|p| out.port_name(p).is_ok_and(|n| n == port));
    let Some(found) = found else {
        let available = ports();
        return Err(if available.is_empty() {
            "no MIDI output found. On Windows install a virtual port such as loopMIDI, then pick it here".into()
        } else {
            format!("MIDI port {port:?} not found; available: {}", available.join(", "))
        });
    };
    let mut conn = out.connect(&found, "HubDeck").map_err(|e| e.to_string())?;
    conn.send(bytes).map_err(|e| e.to_string())?;
    open.insert(port.to_string(), conn);
    Ok(())
}

/// Control change, value 0..=127 on channel 1..=16.
pub fn cc(port: &str, channel: u8, controller: u8, value: u8) -> Result<(), String> {
    let (ch, controller, value) = (channel.clamp(1, 16) - 1, controller.min(127), value.min(127));
    send(port, &[0xB0 | ch, controller, value])?;
    CC.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert((port.to_string(), ch, controller), value);
    Ok(())
}

/// Turn a CC by `delta` steps from the last value sent (starting at 64).
pub fn cc_turn(port: &str, channel: u8, controller: u8, delta: i32) -> Result<u8, String> {
    let ch = channel.clamp(1, 16) - 1;
    let now = CC.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&(port.to_string(), ch, controller.min(127))).copied()).unwrap_or(64);
    let next = (now as i32 + delta).clamp(0, 127) as u8;
    cc(port, channel, controller, next)?;
    Ok(next)
}

pub fn note(port: &str, channel: u8, note: u8, on: bool, velocity: u8) -> Result<(), String> {
    let ch = channel.clamp(1, 16) - 1;
    let status = if on { 0x90 } else { 0x80 } | ch;
    send(port, &[status, note.min(127), if on { velocity.clamp(1, 127) } else { 0 }])
}
