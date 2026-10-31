//! Windows: `.streamDeckPlugin` and `.streamDeckIconPack` files list HubDeck under Open with, for
//! this user only. HubDeck becomes a file's app only when nothing else opens it; where Elgato's Stream Deck app opens
//! them it stays so (the installer's own file associations would take them over, so they are
//! not used; the uninstaller takes these keys out, `windows/hooks.nsh`).

use windows::Win32::System::Registry::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegGetValueW, RegSetKeyValueW};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use windows::core::{HSTRING, PCWSTR};

/// The kinds of files: extension, HubDeck's ProgId for it, what Explorer calls it.
const KINDS: [(&str, &str, &str); 2] = [(".streamDeckPlugin", "HubDeck.streamDeckPlugin", "Stream Deck plugin"), (".streamDeckIconPack", "HubDeck.streamDeckIconPack", "Stream Deck icon pack")];

fn set(key: &str, name: Option<&str>, value: &str) -> bool {
    let data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let name = name.map(HSTRING::from);
    // SAFETY: the strings live through the call; the data is a NUL-terminated UTF-16 string
    unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(key), name.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr())), REG_SZ.0, Some(data.as_ptr().cast()), (data.len() * 2) as u32).is_ok()
    }
}

/// The ProgId that opens files with extension `ext` now (this user's or the machine's), if any.
fn current_app(ext: &str) -> Option<String> {
    let mut buf = [0u16; 256];
    let mut len = (buf.len() * 2) as u32;
    // SAFETY: the buffer and its length in bytes are passed together
    unsafe { RegGetValueW(HKEY_CLASSES_ROOT, &HSTRING::from(ext), PCWSTR::null(), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut len)) }.ok().ok()?;
    let s = String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]);
    (!s.is_empty()).then_some(s)
}

/// Register the installed HubDeck (not a dev build) at each start: the path follows updates.
pub fn register() {
    if cfg!(debug_assertions) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.to_string_lossy();
    for (ext, prog_id, what) in KINDS {
        let class = format!("Software\\Classes\\{prog_id}");
        let mut ok = set(&class, None, what)
            && set(&format!("{class}\\DefaultIcon"), None, &format!("\"{exe}\",0"))
            && set(&format!("{class}\\shell\\open\\command"), None, &format!("\"{exe}\" \"%1\""))
            && set(&format!("Software\\Classes\\{ext}\\OpenWithProgids"), Some(prog_id), "");
        let before = current_app(ext);
        if ok && before.is_none() {
            ok = set(&format!("Software\\Classes\\{ext}"), None, prog_id);
        }
        if !ok {
            log::warn!("could not list HubDeck under Open with for {ext} files");
            continue;
        }
        log::info!("{ext} files: HubDeck under Open with; opened by {}", before.as_deref().unwrap_or(prog_id));
    }
    // SAFETY: no item pointers with SHCNE_ASSOCCHANGED
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}
