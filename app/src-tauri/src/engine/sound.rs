//! "Play sound": a small soundboard. One audio thread, started on the first sound, plays files
//! (WAV, MP3, FLAC, OGG) through rodio on the default speakers or a chosen device. A press plays,
//! a press on a playing sound stops it (or starts it over), and "stop all" silences everything.
//! The output streams close after a quiet half minute, so an idle HubDeck holds no audio device.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};

use super::Msg;

/// How a press treats a sound that is still playing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    /// Press plays; a press while it plays stops it.
    PlayStop,
    /// Every press starts it from the beginning.
    Restart,
    /// Overlapping: every press adds one more copy.
    Overlap,
}

impl Mode {
    pub fn from_param(s: &str) -> Self {
        match s {
            "restart" => Mode::Restart,
            "overlap" => Mode::Overlap,
            _ => Mode::PlayStop,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    Play {
        file: String,
        /// 0.0..=1.0 (up to 2.0 louder than the file).
        volume: f32,
        /// Output device name; empty = the default speakers.
        device: String,
        mode: Mode,
    },
    StopAll,
}

static WORKER: OnceLock<Sender<Cmd>> = OnceLock::new();
static ENGINE: OnceLock<Sender<Msg>> = OnceLock::new();

/// Where errors go (the engine shows them on the window); the thread itself starts on first use.
pub fn init(engine: Sender<Msg>) {
    let _ = ENGINE.set(engine);
}

pub fn send(cmd: Cmd) {
    let tx = WORKER.get_or_init(|| {
        let (tx, rx) = unbounded();
        std::thread::Builder::new().name("sound".into()).spawn(move || run(rx)).expect("spawn sound thread");
        tx
    });
    let _ = tx.send(cmd);
}

/// Checks a sound file before it reaches the audio thread: there, and a kind rodio can read.
pub fn check_file(file: &str) -> Result<(), String> {
    let path = std::path::Path::new(file.trim());
    if file.trim().is_empty() {
        return Err("choose the sound file to play".into());
    }
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();
    if !["wav", "mp3", "flac", "ogg", "oga"].contains(&ext.as_str()) {
        return Err(format!("{ext:?} files cannot be played; use WAV, MP3, FLAC or OGG"));
    }
    if !path.is_file() {
        return Err(format!("no file at {}", path.display()));
    }
    Ok(())
}

fn error(message: String) {
    if let Some(tx) = ENGINE.get() {
        let _ = tx.send(Msg::IntegrationError { source: "Sound".into(), message });
    }
}

/// Close the audio devices after this long without a sound.
const IDLE: Duration = Duration::from_secs(30);

fn run(rx: Receiver<Cmd>) {
    // per device name ("" = default): the stream must stay alive while its sinks play
    let mut outputs: HashMap<String, (rodio::OutputStream, rodio::OutputStreamHandle)> = HashMap::new();
    let mut playing: Vec<(String, rodio::Sink)> = Vec::new();
    let mut last_sound = Instant::now();
    loop {
        // silent and no device open: sleep until the next sound
        let next = if playing.is_empty() && outputs.is_empty() { rx.recv().map_err(|_| crossbeam_channel::RecvTimeoutError::Disconnected) } else { rx.recv_timeout(Duration::from_millis(500)) };
        match next {
            Ok(Cmd::StopAll) => {
                for (_, s) in playing.drain(..) {
                    s.stop();
                }
            }
            Ok(Cmd::Play { file, volume, device, mode }) => {
                last_sound = Instant::now();
                let key = format!("{device}|{file}");
                let already = playing.iter().any(|(k, s)| *k == key && !s.empty());
                if already && mode != Mode::Overlap {
                    playing.retain(|(k, s)| {
                        if *k == key {
                            s.stop();
                        }
                        *k != key
                    });
                    if mode == Mode::PlayStop {
                        continue;
                    }
                }
                match play(&mut outputs, &file, volume, &device) {
                    Ok(sink) => playing.push((key, sink)),
                    Err(e) => error(e),
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
        }
        playing.retain(|(_, s)| !s.empty());
        if !playing.is_empty() {
            last_sound = Instant::now();
        } else if !outputs.is_empty() && last_sound.elapsed() > IDLE {
            outputs.clear();
        }
    }
}

fn play(outputs: &mut HashMap<String, (rodio::OutputStream, rodio::OutputStreamHandle)>, file: &str, volume: f32, device: &str) -> Result<rodio::Sink, String> {
    if !outputs.contains_key(device) {
        let stream = if device.is_empty() {
            rodio::OutputStream::try_default().map_err(|e| format!("no speakers to play on: {e}"))?
        } else {
            use rodio::cpal::traits::{DeviceTrait, HostTrait};
            let found = rodio::cpal::default_host().output_devices().map_err(|e| e.to_string())?.find(|d| d.name().ok().as_deref() == Some(device));
            let dev = found.ok_or_else(|| format!("no output device named {device:?}"))?;
            rodio::OutputStream::try_from_device(&dev).map_err(|e| format!("cannot play on {device}: {e}"))?
        };
        outputs.insert(device.to_string(), stream);
    }
    let (_, handle) = outputs.get(device).expect("just inserted");
    let reader = std::io::BufReader::new(std::fs::File::open(file.trim()).map_err(|e| format!("{file}: {e}"))?);
    let source = rodio::Decoder::new(reader).map_err(|e| format!("cannot read {file}: {e}"))?;
    let sink = rodio::Sink::try_new(handle).map_err(|e| e.to_string())?;
    sink.set_volume(volume.clamp(0.0, 2.0));
    sink.append(source);
    Ok(sink)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_checked_before_playing() {
        assert!(check_file("").unwrap_err().contains("choose"));
        assert!(check_file("C:/x/song.exe").unwrap_err().contains("cannot be played"));
        assert!(check_file("C:/surely/not/here.wav").unwrap_err().contains("no file"));
        assert_eq!(Mode::from_param("restart"), Mode::Restart);
        assert_eq!(Mode::from_param(""), Mode::PlayStop);
    }

    #[test]
    fn a_generated_wav_decodes() {
        // 0.1 s of silence as a WAV, written by hand: rodio must read what we would play
        let samples = 4410u32;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + samples * 2).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes()); // PCM
        wav.extend(1u16.to_le_bytes()); // mono
        wav.extend(44100u32.to_le_bytes());
        wav.extend((44100u32 * 2).to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((samples * 2).to_le_bytes());
        wav.extend(vec![0u8; samples as usize * 2]);
        let decoder = rodio::Decoder::new(std::io::Cursor::new(wav)).unwrap();
        use rodio::Source;
        assert_eq!(decoder.sample_rate(), 44100);
    }
}
