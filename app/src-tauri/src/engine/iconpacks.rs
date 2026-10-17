//! Icon packs the user installs: pictures that show up as a category in the icon picker. Two
//! formats are read: HubDeck's own (a folder or zip with `manifest.json`) and Elgato Stream Deck
//! icon packs (`.streamDeckIconPack`: a zip with `<id>.sdIconPack/manifest.json`, `icons.json` and
//! `icons/`). A pack is installed into `<data>/iconpacks/<id>/` as `pack.json` plus its pictures.
//! Packs are never bundled with HubDeck; their licence is the pack author's. Format:
//! `docs/ICON-PACKS.md`.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::Engine;

const EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "svg"];
const MAX_FILE: u64 = 5 * 1024 * 1024;
const MAX_TOTAL: u64 = 200 * 1024 * 1024;
const MAX_ICONS: usize = 3000;

/// An installed pack as the picker lists it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub id: String,
    pub name: String,
    pub author: String,
    pub count: usize,
}

/// One icon of a pack, with a data URI to show it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIcon {
    pub file: String,
    pub name: String,
    pub tags: Vec<String>,
    pub uri: String,
}

/// `pack.json` of an installed pack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Installed {
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    license: String,
    /// `hubdeck` or `streamdeck`.
    source: String,
    icons: Vec<IconEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct IconEntry {
    /// Path inside the installed pack folder, e.g. `icons/0007.png`.
    file: String,
    name: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// HubDeck's own `manifest.json`.
#[derive(Debug, Deserialize)]
struct Manifest {
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    license: String,
    /// Optional: without it every picture in the pack is an icon, named after its file.
    #[serde(default)]
    icons: Option<Vec<ManifestIcon>>,
}

#[derive(Debug, Deserialize)]
struct ManifestIcon {
    file: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

/// Stream Deck's `manifest.json` (fields as Elgato documents them) and `icons.json` entries.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SdManifest {
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    version: String,
}

#[derive(Debug, Deserialize)]
struct SdIcon {
    path: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

fn ext_of(name: &str) -> Option<String> {
    let ext = name.rsplit('.').next()?.to_lowercase();
    (name.contains('.') && EXTENSIONS.contains(&ext.as_str())).then_some(ext)
}

fn stem(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base).replace(['_', '-'], " ")
}

/// A folder name for a pack: lowercase letters, digits and dashes.
fn slug(name: &str) -> String {
    let s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "pack".into() } else { s.chars().take(40).collect() }
}

/// Every file of a folder or a zip, by its path with forward slashes (limits applied).
fn read_files(path: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    let mut add = |name: String, data: Vec<u8>| -> Result<(), String> {
        total += data.len() as u64;
        if total > MAX_TOTAL || files.len() > MAX_ICONS * 2 {
            return Err("the icon pack is too big".to_string());
        }
        files.insert(name, data);
        Ok(())
    };
    if path.is_dir() {
        let mut stack = vec![path.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if entry.metadata().map(|m| m.len()).unwrap_or(u64::MAX) <= MAX_FILE {
                    let rel = p.strip_prefix(path).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
                    add(rel, std::fs::read(&p).map_err(|e| e.to_string())?)?;
                }
            }
        }
    } else {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("not an icon pack (a folder or a zip): {e}"))?;
        for i in 0..zip.len() {
            let entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().unwrap_or_default().replace('\\', "/");
            if entry.is_dir() || entry.size() > MAX_FILE || name.split('/').any(|p| p == "..") {
                continue;
            }
            let mut data = Vec::new();
            entry.take(MAX_FILE + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
            add(name, data)?;
        }
    }
    Ok(files)
}

/// Turn the files of a pack (either format) into its description and the pictures to copy.
/// A pack read from its files: its description and each picture (path in the installed pack, bytes).
type Parsed<'a> = (Installed, Vec<(String, &'a Vec<u8>)>);

fn parse(files: &BTreeMap<String, Vec<u8>>) -> Result<Parsed<'_>, String> {
    // the pack's root: the shallowest folder with a manifest.json
    let manifest_path = files
        .keys()
        .filter(|k| k.ends_with("manifest.json") && (k.as_str() == "manifest.json" || k.ends_with("/manifest.json")))
        .min_by_key(|k| k.matches('/').count())
        .ok_or("no manifest.json in the icon pack")?;
    let root = manifest_path.trim_end_matches("manifest.json").to_string();
    let manifest = &files[manifest_path];
    let mut pictures = Vec::new();
    let mut icons = Vec::new();
    let mut take = |path: &str, name: Option<String>, tags: Vec<String>| {
        let full = format!("{root}{}", path.trim_start_matches("./"));
        let (Some(ext), Some(data)) = (ext_of(&full), files.get(&full)) else { return };
        if icons.len() >= MAX_ICONS {
            return;
        }
        let file = format!("icons/{:04}.{ext}", icons.len());
        icons.push(IconEntry { file: file.clone(), name: name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| stem(path)), tags });
        pictures.push((file, data));
    };
    let installed = if let Some(list) = files.get(&format!("{root}icons.json")) {
        // Stream Deck: icons.json lists the pictures in icons/
        let sd: SdManifest = serde_json::from_slice(super::sdplugins::without_bom(manifest)).map_err(|e| format!("the Stream Deck manifest is not readable: {e}"))?;
        let list: Vec<SdIcon> = serde_json::from_slice(super::sdplugins::without_bom(list)).map_err(|e| format!("icons.json is not readable: {e}"))?;
        for icon in list {
            take(&format!("icons/{}", icon.path.trim_start_matches("icons/")), icon.name, icon.tags);
        }
        Installed { name: sd.name, author: sd.author, version: sd.version, license: String::new(), source: "streamdeck".into(), icons }
    } else {
        let m: Manifest = serde_json::from_slice(super::sdplugins::without_bom(manifest)).map_err(|e| format!("manifest.json is not readable: {e}"))?;
        match m.icons {
            Some(list) => {
                for icon in list {
                    take(&icon.file, icon.name, icon.tags);
                }
            }
            None => {
                let all: Vec<String> = files.keys().filter(|k| k.starts_with(&root) && ext_of(k).is_some()).map(|k| k[root.len()..].to_string()).collect();
                for path in all {
                    take(&path, None, Vec::new());
                }
            }
        }
        Installed { name: m.name, author: m.author, version: m.version, license: m.license, source: "hubdeck".into(), icons }
    };
    if installed.icons.is_empty() {
        return Err("the icon pack has no pictures HubDeck can use (PNG, JPG, GIF, WebP or SVG)".into());
    }
    if installed.name.trim().is_empty() {
        return Err("the icon pack has no name".into());
    }
    Ok((installed, pictures))
}

fn packs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("iconpacks")
}

/// Install a pack from a folder or zip into `<data>/iconpacks/<id>/`; never replaces one.
pub fn install(data_dir: &Path, source: &Path) -> Result<PackInfo, String> {
    let files = read_files(source)?;
    let (pack, pictures) = parse(&files)?;
    let base = packs_dir(data_dir);
    let mut id = slug(&pack.name);
    let mut n = 2;
    while base.join(&id).exists() {
        id = format!("{}-{n}", slug(&pack.name));
        n += 1;
    }
    let dir = base.join(&id);
    std::fs::create_dir_all(dir.join("icons")).map_err(|e| e.to_string())?;
    for (file, data) in pictures {
        std::fs::write(dir.join(&file), data).map_err(|e| e.to_string())?;
    }
    std::fs::write(dir.join("pack.json"), serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(PackInfo { id, name: pack.name, author: pack.author, count: pack.icons.len() })
}

/// A pack id as stored: our own slugs only, so it can never point outside the packs folder.
fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn load(data_dir: &Path, id: &str) -> Result<Installed, String> {
    if !safe_id(id) {
        return Err("no such icon pack".into());
    }
    let bytes = std::fs::read(packs_dir(data_dir).join(id).join("pack.json")).map_err(|_| "no such icon pack".to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

pub fn list(data_dir: &Path) -> Vec<PackInfo> {
    let Ok(entries) = std::fs::read_dir(packs_dir(data_dir)) else { return Vec::new() };
    let mut packs: Vec<PackInfo> = entries
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            let pack = load(data_dir, &id).ok()?;
            Some(PackInfo { id, name: pack.name, author: pack.author, count: pack.icons.len() })
        })
        .collect();
    packs.sort_by_key(|p| p.name.to_lowercase());
    packs
}

fn mime(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/jpeg",
    }
}

pub fn icons(data_dir: &Path, id: &str) -> Result<Vec<PackIcon>, String> {
    let pack = load(data_dir, id)?;
    let dir = packs_dir(data_dir).join(id);
    Ok(pack
        .icons
        .into_iter()
        .filter_map(|i| {
            let ext = ext_of(&i.file)?;
            let bytes = std::fs::read(dir.join(&i.file)).ok()?;
            let uri = format!("data:{};base64,{}", mime(&ext), base64::engine::general_purpose::STANDARD.encode(bytes));
            Some(PackIcon { file: i.file, name: i.name, tags: i.tags, uri })
        })
        .collect())
}

/// The bytes and extension of one icon (only files the pack lists).
pub fn icon_bytes(data_dir: &Path, id: &str, file: &str) -> Result<(Vec<u8>, String), String> {
    let pack = load(data_dir, id)?;
    let entry = pack.icons.iter().find(|i| i.file == file).ok_or("no such icon in the pack")?;
    let ext = ext_of(&entry.file).ok_or("not a picture")?;
    let bytes = std::fs::read(packs_dir(data_dir).join(id).join(&entry.file)).map_err(|e| e.to_string())?;
    Ok((bytes, ext))
}

pub fn remove(data_dir: &Path, id: &str) -> Result<(), String> {
    load(data_dir, id)?;
    std::fs::remove_dir_all(packs_dir(data_dir).join(id)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn iconpacks_list(engine: State<'_, Engine>) -> Vec<PackInfo> {
    list(engine.data_dir())
}

#[tauri::command]
pub async fn iconpack_icons(engine: State<'_, Engine>, id: String) -> Result<Vec<PackIcon>, String> {
    let dir = engine.data_dir().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || icons(&dir, &id)).await.map_err(|e| e.to_string())?
}

/// Install a pack from a folder, a zip or a `.streamDeckIconPack`.
#[tauri::command]
pub async fn iconpack_import(engine: State<'_, Engine>, path: String) -> Result<PackInfo, String> {
    let dir = engine.data_dir().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || install(&dir, Path::new(&path))).await.map_err(|e| e.to_string())?
}

/// Copy one pack icon into the pictures; returns the picture name for a look.
#[tauri::command]
pub fn iconpack_use(engine: State<'_, Engine>, pack: String, file: String) -> Result<String, String> {
    let (bytes, ext) = icon_bytes(engine.data_dir(), &pack, &file)?;
    super::images::import_bytes(&bytes, &ext)
}

#[tauri::command]
pub fn iconpack_remove(engine: State<'_, Engine>, id: String) -> Result<(), String> {
    remove(engine.data_dir(), &id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hubdeck-iconpacks-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            for (name, data) in files {
                z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(data).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn installs_a_stream_deck_pack() {
        let root = temp("sd");
        let data = root.join("data");
        let pack = root.join("Cats.streamDeckIconPack");
        let bytes = zip_of(&[
            ("com.example.cats.sdIconPack/manifest.json", br#"{"Name":"Cats","Author":"Someone","Version":"1.0","Icon":"icon.png"}"#),
            ("com.example.cats.sdIconPack/icons.json", br#"[{"path":"tabby.png","name":"Tabby","tags":["cat","orange"]},{"path":"missing.png","name":"Gone"},{"path":"notes.txt"}]"#),
            ("com.example.cats.sdIconPack/icons/tabby.png", b"\x89PNG fake"),
            ("com.example.cats.sdIconPack/icon.png", b"\x89PNG cover"),
        ]);
        std::fs::write(&pack, bytes).unwrap();
        let info = install(&data, &pack).unwrap();
        assert_eq!((info.id.as_str(), info.name.as_str(), info.count), ("cats", "Cats", 1));
        let listed = icons(&data, "cats").unwrap();
        assert_eq!(listed[0].name, "Tabby");
        assert_eq!(listed[0].tags, ["cat", "orange"]);
        assert!(listed[0].uri.starts_with("data:image/png;base64,"));
        assert_eq!(icon_bytes(&data, "cats", &listed[0].file).unwrap().0, b"\x89PNG fake");
        // a second install of the same pack gets its own folder
        assert_eq!(install(&data, &pack).unwrap().id, "cats-2");
        assert_eq!(list(&data).len(), 2);
        remove(&data, "cats-2").unwrap();
        assert_eq!(list(&data).len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn installs_our_own_folder_format_without_an_icon_list() {
        let root = temp("own");
        let src = root.join("My Pack");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("manifest.json"), br#"{"name":"My Pack","author":"Me","license":"CC0"}"#).unwrap();
        std::fs::write(src.join("fire_ball.svg"), b"<svg/>").unwrap();
        std::fs::write(src.join("sub/ice.png"), b"png").unwrap();
        std::fs::write(src.join("readme.md"), b"text").unwrap();
        let data = root.join("data");
        let info = install(&data, &src).unwrap();
        assert_eq!((info.id.as_str(), info.count), ("my-pack", 2));
        let names: Vec<String> = icons(&data, "my-pack").unwrap().into_iter().map(|i| i.name).collect();
        assert_eq!(names, ["fire ball", "ice"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_bad_packs_and_paths() {
        let root = temp("bad");
        let empty = root.join("empty.zip");
        std::fs::write(&empty, zip_of(&[("manifest.json", br#"{"name":"Empty"}"#), ("../evil.png", b"x")])).unwrap();
        assert!(install(&root.join("data"), &empty).is_err());
        let no_manifest = root.join("none.zip");
        std::fs::write(&no_manifest, zip_of(&[("a.png", b"x")])).unwrap();
        assert!(install(&root.join("data"), &no_manifest).is_err());
        assert!(load(&root.join("data"), "../x").is_err());
        assert!(icon_bytes(&root.join("data"), "nope", "icons/0000.png").is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
