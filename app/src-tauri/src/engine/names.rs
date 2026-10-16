//! Names the services know, for the window's lists (owner, 2026-10-15: "Tam gdzie mamy mieć coś z
//! apek to powinna być jakaś lista ... a nie miejsce na nazwę"; his 4a, 2026-10-16): Streamlabs
//! scenes and sources, Home Assistant entities, Spotify playlists, Twitch categories. Each worker
//! writes what it read; the window asks with `names_list`. A kind that is absent means the service
//! never answered (not connected): the field stays typed.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Name {
    /// What the param keeps (an entity id, a playlist link, a scene name).
    pub value: String,
    /// What the list shows.
    pub label: String,
}

impl Name {
    pub fn same(s: &str) -> Self {
        Name { value: s.to_string(), label: s.to_string() }
    }
}

static KNOWN: Mutex<Option<HashMap<String, Vec<Name>>>> = Mutex::new(None);

/// Keep `names` as the list of `kind` (e.g. `streamlabs.scene`, `ha.light`).
pub fn set(kind: &str, mut names: Vec<Name>) {
    names.sort_by_key(|n| n.label.to_lowercase());
    names.dedup_by(|a, b| a.value == b.value);
    if let Ok(mut k) = KNOWN.lock() {
        k.get_or_insert_with(HashMap::new).insert(kind.to_string(), names);
    }
}

/// Forget every list of a service (it disconnected or its settings changed).
pub fn forget(prefix: &str) {
    if let Ok(mut k) = KNOWN.lock()
        && let Some(map) = k.as_mut()
    {
        map.retain(|kind, _| !kind.starts_with(prefix));
    }
}

pub fn get(kind: &str) -> Option<Vec<Name>> {
    KNOWN.lock().ok()?.as_ref()?.get(kind).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_sorted_without_repeats_and_forgotten_by_service() {
        set("test.a", vec![Name::same("b"), Name::same("A"), Name::same("b")]);
        assert_eq!(get("test.a").unwrap().iter().map(|n| n.value.as_str()).collect::<Vec<_>>(), ["A", "b"]);
        assert!(get("test.none").is_none());
        forget("test.");
        assert!(get("test.a").is_none());
    }
}
