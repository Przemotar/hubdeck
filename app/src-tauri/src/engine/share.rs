//! Sharing profiles: one-file packages and the online gallery.
//!
//! A package (`.hubdeck`) is a zip with `profile.json` and `images/<name>` for every picture the
//! profile uses (keys on all pages and gestures, the look while on, the full-panel picture). On
//! import the pictures are stored like any imported picture (named by their content, so two
//! different pictures never clash) and the profile is added under a fresh id: an import never
//! overwrites anything. Format and the gallery's `index.json`: `docs/SHARING.md`.

use std::collections::{BTreeSet, HashMap};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;

use super::Engine;
use super::commands::Imported;
use super::presets::Moved;
use super::profile::{Look, PROFILE_VERSION, PanelContent, Profile};

/// File extension of a package.
pub const EXT: &str = "hubdeck";
const PROFILE_ENTRY: &str = "profile.json";
const IMAGES_DIR: &str = "images/";
/// Limits against broken or hostile files: one picture, the whole unpacked package, entries.
const MAX_ENTRY: u64 = 15 * 1024 * 1024;
const MAX_TOTAL: u64 = 100 * 1024 * 1024;
const MAX_ENTRIES: usize = 1000;
/// The gallery: an index of shared profiles in a public GitHub repository.
pub const GALLERY_INDEX: &str = "https://raw.githubusercontent.com/Przemotar/hubdeck-profiles/main/index.json";
/// Packages are downloaded only from the gallery's repository.
const GALLERY_PREFIXES: [&str; 2] = ["https://raw.githubusercontent.com/Przemotar/hubdeck-profiles/", "https://github.com/Przemotar/hubdeck-profiles/"];
const TIMEOUT: Duration = Duration::from_secs(5);

/// Every picture name a profile refers to.
pub fn image_names(profile: &Profile) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut add = |look: &Look| {
        if let Some(image) = &look.image {
            names.insert(image.clone());
        }
    };
    for binding in profile.controls.values() {
        add(&binding.look);
        if let Some(on) = &binding.look_on {
            add(on);
        }
    }
    if let Some(PanelContent::Picture { image }) = &profile.panel {
        names.insert(image.clone());
    }
    names
}

/// Picture names HubDeck makes: letters, digits and one dot; never a path.
fn safe_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') && !name.starts_with('.') && name.contains('.')
}

/// The package bytes: `profile.json` and the pictures `read` finds (a missing picture is left out).
pub fn pack(profile: &Profile, read: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> Result<Vec<u8>, String> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(PROFILE_ENTRY, opts).map_err(|e| e.to_string())?;
        zip.write_all(&serde_json::to_vec_pretty(profile).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        for name in image_names(profile) {
            if !safe_name(&name) {
                continue;
            }
            let Some(bytes) = read(&name) else {
                log::warn!("package: picture {name} is missing, left out");
                continue;
            };
            // pictures are compressed already
            let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file(format!("{IMAGES_DIR}{name}"), stored).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        zip.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

/// A zip with a `profile.json` at its root (a `.mhz` file has none).
pub fn is_package(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") && zip::ZipArchive::new(Cursor::new(bytes)).is_ok_and(|a| a.index_for_name(PROFILE_ENTRY).is_some())
}

/// Read a package: the profile, its pictures handed to `store(bytes, extension)`, which returns the
/// name to use instead; a picture that cannot be stored is dropped from the looks.
pub fn unpack(bytes: &[u8], store: &mut dyn FnMut(&[u8], &str) -> Option<String>) -> Result<Profile, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("not a HubDeck profile package: {e}"))?;
    if zip.len() > MAX_ENTRIES {
        return Err("the package has too many files".into());
    }
    let mut profile_json = None;
    let mut renamed: HashMap<String, Option<String>> = HashMap::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let picture = name.strip_prefix(IMAGES_DIR).filter(|n| safe_name(n)).map(str::to_string);
        if name != PROFILE_ENTRY && picture.is_none() {
            continue;
        }
        if entry.size() > MAX_ENTRY {
            return Err(format!("{name} in the package is too big"));
        }
        let mut data = Vec::new();
        entry.take(MAX_ENTRY + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
        total += data.len() as u64;
        if data.len() as u64 > MAX_ENTRY || total > MAX_TOTAL {
            return Err("the package is too big".into());
        }
        match picture {
            None => profile_json = Some(data),
            Some(old) => {
                let ext = old.rsplit('.').next().unwrap_or("png").to_lowercase();
                renamed.insert(old, store(&data, &ext));
            }
        }
    }
    let json = profile_json.ok_or("the package holds no profile.json")?;
    let mut profile: Profile = serde_json::from_slice(&json).map_err(|e| format!("the profile in the package is not readable: {e}"))?;
    let fix = |look: &mut Look| {
        if let Some(old) = look.image.take() {
            look.image = renamed.get(&old).cloned().flatten();
        }
    };
    for binding in profile.controls.values_mut() {
        fix(&mut binding.look);
        if let Some(on) = binding.look_on.as_mut() {
            fix(on);
        }
    }
    if let Some(PanelContent::Picture { image }) = &profile.panel {
        match renamed.get(image).cloned().flatten() {
            Some(new) => profile.panel = Some(PanelContent::Picture { image: new }),
            None => profile.panel = None,
        }
    }
    Ok(profile)
}

/// Add an imported profile under a fresh id (never overwrites) and tell the window.
/// `adapt` fits it to the user's modules first.
fn add(engine: &Engine, mut profile: Profile, adapt: &dyn Fn(Profile) -> (Profile, Vec<Moved>)) -> Result<Imported, String> {
    if profile.version > PROFILE_VERSION {
        return Err(format!("this profile was made by a newer version (format {})", profile.version));
    }
    profile.id = uuid::Uuid::new_v4().to_string();
    profile.version = PROFILE_VERSION;
    let (profile, moved) = adapt(profile);
    engine.lock().store.save_profile(profile.clone()).map_err(|e| e.to_string())?;
    engine.changed();
    Ok(Imported { profile, moved })
}

/// Import package bytes: pictures into the images folder, the profile as a new one.
pub fn import_package(engine: &Engine, bytes: &[u8], adapt: &dyn Fn(Profile) -> (Profile, Vec<Moved>)) -> Result<Imported, String> {
    let profile = unpack(bytes, &mut |data, ext| super::images::import_bytes(data, ext).map_err(|e| log::warn!("package picture: {e}")).ok())?;
    add(engine, profile, adapt)
}

/// Write a profile as a package file.
pub fn export_package(engine: &Engine, id: &str, path: &Path) -> Result<(), String> {
    let mut profile = engine.lock().store.profiles.get(id).cloned().ok_or_else(|| format!("no profile {id}"))?;
    // game covers from a launcher's cache stay on this PC: a game icon goes out instead
    for b in profile.controls.values_mut() {
        if b.look.image.as_deref().is_some_and(super::images::is_local_only) {
            b.look.image = None;
            b.look.icon = Some("gamepad-2".into());
        }
    }
    let images = engine.data_dir().join("images");
    let bytes = pack(&profile, &mut |name| std::fs::read(images.join(name)).ok())?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// One profile in the gallery's `index.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GalleryEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    /// Module kinds the profile uses: `keys`, `dial`, `knobs`, `faders`, `rollers`.
    #[serde(default)]
    pub kinds: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Picture of the deck with the profile, a URL.
    #[serde(default)]
    pub preview: Option<String>,
    /// The `.hubdeck` package, a URL in the gallery repository.
    pub package: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GalleryIndex {
    #[serde(default)]
    profiles: Vec<GalleryEntry>,
}

/// Parse the gallery's `index.json`.
pub fn parse_index(bytes: &[u8]) -> Result<Vec<GalleryEntry>, String> {
    let index: GalleryIndex = serde_json::from_slice(bytes).map_err(|e| format!("the gallery index is not readable: {e}"))?;
    Ok(index.profiles)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).build().into()
}

fn download(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut response = agent().get(url).call().map_err(|e| e.to_string())?;
    response.body_mut().with_config().limit(limit).read_to_vec().map_err(|e| e.to_string())
}

/// The profiles in the gallery; an error when it cannot be reached (offline, not online yet).
#[tauri::command]
pub async fn gallery_list() -> Result<Vec<GalleryEntry>, String> {
    tauri::async_runtime::spawn_blocking(|| parse_index(&download(GALLERY_INDEX, 2 * 1024 * 1024)?)).await.map_err(|e| e.to_string())?
}

/// Download a gallery package and import it as a new profile.
#[tauri::command]
pub async fn gallery_add(app: tauri::AppHandle, engine: State<'_, Engine>, package: String) -> Result<Imported, String> {
    if !GALLERY_PREFIXES.iter().any(|p| package.starts_with(p)) {
        return Err("packages come only from the HubDeck profile gallery".into());
    }
    let bytes = tauri::async_runtime::spawn_blocking(move || download(&package, MAX_TOTAL)).await.map_err(|e| e.to_string())??;
    if !is_package(&bytes) {
        return Err("the download is not a HubDeck profile package".into());
    }
    super::backup::create(engine.data_dir(), "before import").map_err(|e| e.to_string())?;
    import_package(&engine, &bytes, &|p| super::api::adapt_to_user(&app, p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::profile::Binding;

    fn look(image: Option<&str>) -> Look {
        Look { label: Some("x".into()), image: image.map(str::to_string), ..Default::default() }
    }

    fn profile() -> Profile {
        let mut p: Profile = serde_json::from_value(serde_json::json!({ "id": "p1", "name": "Shared" })).unwrap();
        let key = |img: Option<&str>, on: Option<&str>| Binding {
            action: "core.launch".into(),
            params: serde_json::json!({}),
            look: look(img),
            accel: false,
            look_on: on.map(|i| look(Some(i))),
            swap_turns: false,
        };
        p.controls.insert("keys.0".into(), key(Some("aaaa.png"), Some("bbbb.gif")));
        p.controls.insert("pg1/keys.3:hold".into(), key(Some("cccc.jpg"), None));
        p.controls.insert("keys.1".into(), key(None, None));
        p.panel = Some(PanelContent::Picture { image: "dddd.webp".into() });
        p
    }

    #[test]
    fn finds_every_picture() {
        let names: Vec<_> = image_names(&profile()).into_iter().collect();
        assert_eq!(names, ["aaaa.png", "bbbb.gif", "cccc.jpg", "dddd.webp"]);
    }

    #[test]
    fn round_trip_keeps_profile_and_pictures() {
        let p = profile();
        let files: HashMap<&str, Vec<u8>> = [("aaaa.png", vec![1, 2, 3]), ("bbbb.gif", vec![4]), ("cccc.jpg", vec![5, 6]), ("dddd.webp", vec![7])].into_iter().collect();
        let bytes = pack(&p, &mut |n| files.get(n).cloned()).unwrap();
        assert!(is_package(&bytes));
        let mut stored: Vec<(Vec<u8>, String)> = Vec::new();
        let back = unpack(&bytes, &mut |data, ext| {
            stored.push((data.to_vec(), ext.to_string()));
            Some(format!("new{}.{ext}", stored.len()))
        })
        .unwrap();
        assert_eq!(stored.len(), 4);
        // each picture came back with its bytes, under the name the store gave it
        for binding in back.controls.values() {
            for l in [Some(&binding.look), binding.look_on.as_ref()].into_iter().flatten() {
                if let Some(name) = &l.image {
                    let i: usize = name[3..name.find('.').unwrap()].parse().unwrap();
                    let (data, ext) = &stored[i - 1];
                    assert!(name.ends_with(ext));
                    assert!(files.values().any(|v| v == data));
                }
            }
        }
        assert_eq!(back.name, "Shared");
        assert_eq!(back.controls.len(), 3);
        assert!(back.controls["keys.1"].look.image.is_none());
        assert!(matches!(&back.panel, Some(PanelContent::Picture { image }) if image.starts_with("new")));
    }

    #[test]
    fn missing_or_unstorable_pictures_are_dropped() {
        let p = profile();
        let bytes = pack(&p, &mut |n| (n == "aaaa.png").then(|| vec![9])).unwrap();
        let back = unpack(&bytes, &mut |_, _| None).unwrap();
        assert!(back.controls["keys.0"].look.image.is_none());
        assert!(back.panel.is_none());
    }

    #[test]
    fn rejects_zips_that_are_not_packages() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            z.start_file("profile.mhz.json", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"{}").unwrap();
            z.start_file("../evil.png", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        let bytes = buf.into_inner();
        assert!(!is_package(&bytes));
        assert!(unpack(&bytes, &mut |_, _| Some("x.png".into())).is_err());
        assert!(!is_package(b"{\"id\":\"x\"}"));
    }

    #[test]
    fn parses_the_gallery_index() {
        let json = br#"{ "version": 1, "profiles": [
            { "id": "obs-basic", "name": "OBS basics", "author": "Przemotar", "description": "Scenes and mute",
              "kinds": ["keys", "dial"], "tags": ["streaming"], "preview": "https://raw.githubusercontent.com/Przemotar/hubdeck-profiles/main/obs-basic/preview.png",
              "package": "https://raw.githubusercontent.com/Przemotar/hubdeck-profiles/main/obs-basic/obs-basic.hubdeck", "version": "1.0" },
            { "id": "min", "name": "Minimal", "package": "https://raw.githubusercontent.com/Przemotar/hubdeck-profiles/main/min.hubdeck" } ] }"#;
        let list = parse_index(json).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].kinds, ["keys", "dial"]);
        assert!(list[1].preview.is_none() && list[1].kinds.is_empty());
    }
}
