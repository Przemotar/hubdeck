//! Commands that draw on the screens and light the LEDs. HID writes block, so each one runs on
//! a blocking thread, never on the UI thread or the async runtime.

use mh_protocol::ModuleKind;
use mh_protocol::report::{self, LedMode, LedTarget};
use serde::Deserialize;
use tauri::State;

use crate::deck::Deck;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Screen {
    /// One key of the 15-key module (`index` 0..15).
    Key,
    /// The whole 480x272 panel behind the keys.
    Panel,
    /// The 240x240 dial screen.
    Dial,
}

async fn blocking<F>(f: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// Show a JPEG on a key, the whole key panel, or the dial.
#[tauri::command]
pub async fn draw_jpeg(deck: State<'_, Deck>, module: String, screen: Screen, index: u8, jpeg: Vec<u8>) -> Result<(), String> {
    let m = deck.module(&module).ok_or_else(|| format!("{module} is not connected"))?;
    blocking(move || {
        let reports = match (screen, m.kind()) {
            (Screen::Key, ModuleKind::Keys) => report::key_image(index, &jpeg, m.output_len),
            (Screen::Panel, ModuleKind::Keys) => report::panel_image(&jpeg, m.output_len),
            (Screen::Dial, ModuleKind::Dial) => report::dial_image(&jpeg, m.output_len),
            (s, k) => return Err(format!("{s:?} is not a screen of {}", k.name())),
        };
        m.send_all(&reports).map_err(|e| e.to_string())
    })
    .await
}

/// Fill the key panel or the dial with one color (black clears it).
#[tauri::command]
pub async fn fill_screen(deck: State<'_, Deck>, module: String, rgb: [u8; 3]) -> Result<(), String> {
    let m = deck.module(&module).ok_or_else(|| format!("{module} is not connected"))?;
    blocking(move || {
        let r = report::fill_screen(m.kind(), rgb).ok_or("module has no screen")?;
        m.send(&r).map_err(|e| e.to_string())
    })
    .await
}

/// Static LED color per control (knobs, faders) or for the base edge; `None` restores the
/// module's default rainbow effect.
#[tauri::command]
pub async fn set_leds(deck: State<'_, Deck>, module: String, colors: Option<Vec<[u8; 3]>>) -> Result<(), String> {
    let m = deck.module(&module).ok_or_else(|| format!("{module} is not connected"))?;
    blocking(move || {
        let kind = m.kind();
        let send = |r: Option<mh_protocol::Report>| match r {
            Some(r) => m.send(&r).map_err(|e| e.to_string()),
            None => Err(format!("{} has no LEDs", kind.name())),
        };
        match colors {
            None => send(report::led_mode(kind, LedTarget::All, LedMode::Rainbow, false)),
            Some(colors) => {
                send(report::led_mode(kind, LedTarget::All, LedMode::Static, false))?;
                for (i, rgb) in colors.iter().enumerate() {
                    let target = if kind == ModuleKind::Base { LedTarget::All } else { LedTarget::Index(i as u8) };
                    send(report::led_color(kind, target, *rgb, false))?;
                }
                Ok(())
            }
        }
    })
    .await
}
