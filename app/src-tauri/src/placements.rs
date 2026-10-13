//! Where each module sits on the base, as arranged by the user: `"<kind>@<slot>"` -> cell and
//! rotation. Slots are physical connectors, so one table serves every base of the same model.
//!
//! Stored as JSON in the app data dir; written to a temp file and renamed, so a crash never
//! leaves a half-written file, and a missing or corrupt file falls back to an empty table.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Placement {
    /// Top-left cell of the rotated footprint on the 4x6 grid (base standing portrait).
    pub col: u8,
    pub row: u8,
    /// Clockwise degrees: 0, 90, 180 or 270.
    pub rotation: u16,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    version: u32,
    placements: BTreeMap<String, Placement>,
}

pub struct Placements {
    path: PathBuf,
    table: Mutex<BTreeMap<String, Placement>>,
}

impl Placements {
    pub fn load(path: PathBuf) -> Self {
        let table = std::fs::read(&path).ok().and_then(|bytes| serde_json::from_slice::<File>(&bytes).ok()).map(|f| f.placements).unwrap_or_default();
        Self { path, table: Mutex::new(table) }
    }

    pub fn all(&self) -> BTreeMap<String, Placement> {
        self.table.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set(&self, key: String, placement: Option<Placement>) -> std::io::Result<()> {
        let snapshot = {
            let mut table = self.table.lock().unwrap_or_else(|e| e.into_inner());
            match placement {
                Some(p) => table.insert(key, p),
                None => table.remove(&key),
            };
            table.clone()
        };
        let json = serde_json::to_vec_pretty(&File { version: 1, placements: snapshot })?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_corrupt_file() {
        let dir = std::env::temp_dir().join(format!("mh-placements-{}", std::process::id()));
        let path = dir.join("placements.json");
        let p = Placements::load(path.clone());
        assert!(p.all().is_empty());
        p.set("knobs@1-4-3".into(), Some(Placement { col: 2, row: 3, rotation: 90 })).unwrap();
        assert_eq!(Placements::load(path.clone()).all()["knobs@1-4-3"].rotation, 90);
        p.set("knobs@1-4-3".into(), None).unwrap();
        assert!(Placements::load(path.clone()).all().is_empty());
        std::fs::write(&path, b"{ not json").unwrap();
        assert!(Placements::load(path).all().is_empty(), "corrupt file falls back to empty");
        let _ = std::fs::remove_dir_all(dir);
    }
}
