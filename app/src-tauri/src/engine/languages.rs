//! Languages people add themselves (Discussions #3, 2026-10-18: "users could load their own language
//! files externally, without having to modify and rebuild the application"): a JSON file per
//! language in `languages/` in HubDeck's data folder, picked in Settings > General like English and
//! Polish. A file holds the window's words (`strings`) and, when it has them, the actions', presets'
//! and games' names; anything it lacks shows in English. "Make a language file" writes every English
//! text into `languages/template.json` to start from.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::Engine;

/// Bigger files are not read (all of HubDeck's English is about 400 KB).
const MAX_FILE: u64 = 4 * 1024 * 1024;
/// HubDeck's own languages: a file cannot take their place.
const BUILT_IN: [&str; 2] = ["en", "pl"];
const TEMPLATE: &str = "template";

#[derive(Debug, Serialize, PartialEq)]
pub struct LanguageFile {
    /// The file's name without `.json` (`ja`), the window's code for the language.
    pub code: String,
    /// The language's own name from the file (`日本語`), else the code.
    pub name: String,
}

fn folder(data: &Path) -> PathBuf {
    data.join("languages")
}

/// A language code from a file name: letters, digits, `-` and `_`, not one of HubDeck's own.
fn code_of(path: &Path) -> Option<String> {
    if path.extension()?.to_str()? != "json" {
        return None;
    }
    let code = path.file_stem()?.to_str()?.to_lowercase();
    let ok = !code.is_empty() && code.len() <= 32 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    (ok && !BUILT_IN.contains(&code.as_str()) && code != TEMPLATE).then_some(code)
}

fn read(path: &Path) -> Result<Value, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_FILE {
        return Err(format!("{} is larger than 4 MB", path.display()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{} is not readable JSON: {e}", path.display()))?;
    if !value.get("strings").is_some_and(Value::is_object) {
        return Err(format!("{} has no \"strings\"", path.display()));
    }
    Ok(value)
}

/// The language files in `data`'s `languages` folder, by name.
pub fn list(data: &Path) -> Vec<LanguageFile> {
    let mut out: Vec<LanguageFile> = std::fs::read_dir(folder(data))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let code = code_of(&path)?;
            let file = read(&path).map_err(|e| log::warn!("languages: {e}")).ok()?;
            let name = file.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&code).to_string();
            Some(LanguageFile { code, name })
        })
        .collect();
    out.sort_by_key(|l| l.name.to_lowercase());
    out
}

/// One language file, whole.
pub fn load(data: &Path, code: &str) -> Result<Value, String> {
    let path = folder(data).join(format!("{code}.json"));
    if code_of(&path).as_deref() != Some(code) {
        return Err(format!("{code} is not a language file"));
    }
    read(&path)
}

#[tauri::command]
pub fn languages_list(engine: tauri::State<'_, Engine>) -> Vec<LanguageFile> {
    list(engine.data_dir())
}

#[tauri::command]
pub fn language_file(engine: tauri::State<'_, Engine>, code: String) -> Result<Value, String> {
    load(engine.data_dir(), &code)
}

/// Write `template` (every English text, made by the window) as `languages/template.json` and show
/// the folder. Returns the file's path.
#[tauri::command]
pub fn language_template(app: tauri::AppHandle, engine: tauri::State<'_, Engine>, template: Value) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt as _;
    let dir = folder(engine.data_dir());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{TEMPLATE}.json"));
    let text = serde_json::to_string_pretty(&template).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    if !crate::automation::blocks_opening(&dir.to_string_lossy()) {
        app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())?;
    }
    Ok(path.to_string_lossy().into_owned())
}

/// Show the `languages` folder (made when missing).
#[tauri::command]
pub fn languages_open_folder(app: tauri::AppHandle, engine: tauri::State<'_, Engine>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let dir = folder(engine.data_dir());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if crate::automation::blocks_opening(&dir.to_string_lossy()) {
        return Ok(());
    }
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_and_loads_language_files() {
        let data = std::env::temp_dir().join(format!("hubdeck-languages-{}", std::process::id()));
        let dir = folder(&data);
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ja.json"), "\u{feff}{\"name\": \"日本語\", \"strings\": {\"topbar.settings\": \"設定\"}}").unwrap();
        std::fs::write(dir.join("de.json"), r#"{"strings": {}}"#).unwrap();
        std::fs::write(dir.join("pl.json"), r#"{"name": "Mine", "strings": {}}"#).unwrap();
        std::fs::write(dir.join("template.json"), r#"{"strings": {}}"#).unwrap();
        std::fs::write(dir.join("broken.json"), "{").unwrap();
        std::fs::write(dir.join("nostrings.json"), r#"{"name": "X"}"#).unwrap();
        std::fs::write(dir.join("bad name!.json"), r#"{"strings": {}}"#).unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let langs = list(&data);
        assert_eq!(
            langs,
            vec![LanguageFile { code: "de".into(), name: "de".into() }, LanguageFile { code: "ja".into(), name: "日本語".into() }],
            "HubDeck's own, the template, broken and odd files left out"
        );
        assert_eq!(load(&data, "ja").unwrap()["strings"]["topbar.settings"], "設定", "a byte order mark is fine");
        assert!(load(&data, "pl").is_err() && load(&data, "../ja").is_err() && load(&data, "missing").is_err());
        let _ = std::fs::remove_dir_all(&data);
    }
}
