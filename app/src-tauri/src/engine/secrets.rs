//! Passwords and tokens for integrations (OBS, Home Assistant) live in the system's credential
//! store (Windows Credential Manager, macOS Keychain), never in the settings file.

const SERVICE: &str = "HubDeck";

pub fn get(name: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, name).ok()?.get_password().ok().filter(|s| !s.is_empty())
}

/// Store a secret; an empty value deletes it.
pub fn set(name: &str, value: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())?;
    if value.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    } else {
        entry.set_password(value).map_err(|e| e.to_string())
    }
}
