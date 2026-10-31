"""Get or set a secret for HubDeck's own scripts in the system's credential store: the Windows
Credential Manager (a generic credential named `hubdeck/<name>`) or the macOS Keychain (a generic
password with the service `hubdeck/<name>`). No packages beyond the standard library.

    python -I scripts/secret.py set discord-bot              # asks for the value, nothing is echoed
    python -I scripts/secret.py set discord-bot --clipboard  # takes it from the clipboard, then empties it
    python -I scripts/secret.py check discord-bot            # says whether it is there, never prints it

The value is never taken as an argument and never printed. Scripts read it with `get(name)`.
In the Windows console the hidden prompt does not paste with Ctrl+V (it reads the key as the control
character 0x16): paste with a right click there, or copy the value and use `--clipboard`.
"""

import getpass
import subprocess
import sys

TARGET = "hubdeck/{}"


class SecretError(Exception):
    pass


def _windows():
    import ctypes
    from ctypes import wintypes

    class CREDENTIAL(ctypes.Structure):
        _fields_ = [
            ("Flags", wintypes.DWORD),
            ("Type", wintypes.DWORD),
            ("TargetName", wintypes.LPWSTR),
            ("Comment", wintypes.LPWSTR),
            ("LastWritten", wintypes.FILETIME),
            ("CredentialBlobSize", wintypes.DWORD),
            ("CredentialBlob", ctypes.POINTER(ctypes.c_ubyte)),
            ("Persist", wintypes.DWORD),
            ("AttributeCount", wintypes.DWORD),
            ("Attributes", ctypes.c_void_p),
            ("TargetAlias", wintypes.LPWSTR),
            ("UserName", wintypes.LPWSTR),
        ]

    advapi = ctypes.WinDLL("advapi32", use_last_error=True)
    advapi.CredReadW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.POINTER(ctypes.POINTER(CREDENTIAL))]
    advapi.CredReadW.restype = wintypes.BOOL
    advapi.CredWriteW.argtypes = [ctypes.POINTER(CREDENTIAL), wintypes.DWORD]
    advapi.CredWriteW.restype = wintypes.BOOL
    advapi.CredFree.argtypes = [ctypes.c_void_p]
    advapi.CredFree.restype = None
    return ctypes, CREDENTIAL, advapi


CRED_TYPE_GENERIC, CRED_PERSIST_LOCAL_MACHINE, ERROR_NOT_FOUND = 1, 2, 1168


def _win_get(name: str) -> str | None:
    ctypes, CREDENTIAL, advapi = _windows()
    cred = ctypes.POINTER(CREDENTIAL)()
    if not advapi.CredReadW(TARGET.format(name), CRED_TYPE_GENERIC, 0, ctypes.byref(cred)):
        err = ctypes.get_last_error()
        if err == ERROR_NOT_FOUND:
            return None
        raise SecretError(f"CredReadW failed: error {err}")
    try:
        c = cred.contents
        return ctypes.string_at(c.CredentialBlob, c.CredentialBlobSize).decode("utf-8")
    finally:
        advapi.CredFree(cred)


def _win_set(name: str, value: str) -> None:
    ctypes, CREDENTIAL, advapi = _windows()
    blob = value.encode("utf-8")
    buf = (ctypes.c_ubyte * len(blob)).from_buffer_copy(blob)
    cred = CREDENTIAL()
    cred.Type = CRED_TYPE_GENERIC
    cred.TargetName = TARGET.format(name)
    cred.CredentialBlobSize = len(blob)
    cred.CredentialBlob = ctypes.cast(buf, ctypes.POINTER(ctypes.c_ubyte))
    cred.Persist = CRED_PERSIST_LOCAL_MACHINE
    cred.UserName = "hubdeck"
    if not advapi.CredWriteW(ctypes.byref(cred), 0):
        raise SecretError(f"CredWriteW failed: error {ctypes.get_last_error()}")


def _mac_get(name: str) -> str | None:
    r = subprocess.run(
        ["security", "find-generic-password", "-a", "hubdeck", "-s", TARGET.format(name), "-w"],
        capture_output=True, text=True,
    )
    return r.stdout.rstrip("\n") if r.returncode == 0 else None


def _mac_set(name: str, value: str) -> None:
    # through `security -i` on stdin, so the value never shows in the process list
    quoted = value.replace("\\", "\\\\").replace('"', '\\"')
    cmd = f'add-generic-password -U -a hubdeck -s "{TARGET.format(name)}" -w "{quoted}"\n'
    r = subprocess.run(["security", "-i"], input=cmd, capture_output=True, text=True)
    if r.returncode != 0 or "error" in r.stderr.lower():
        raise SecretError(f"security add-generic-password failed: {r.stderr.strip()}")


def get(name: str) -> str | None:
    """The secret, or None when it is not set."""
    if sys.platform == "win32":
        return _win_get(name)
    if sys.platform == "darwin":
        return _mac_get(name)
    raise SecretError("no credential store for this system (Windows and macOS only)")


def _clipboard() -> str:
    """The clipboard's text; the clipboard is emptied after reading, so the secret does not stay there."""
    if sys.platform == "win32":
        r = subprocess.run(["powershell", "-NoProfile", "-Command", "Get-Clipboard -Raw"], capture_output=True, text=True, encoding="utf-8")
        if r.returncode != 0:
            raise SecretError(f"Get-Clipboard failed: {r.stderr.strip()}")
        import ctypes
        user = ctypes.WinDLL("user32")
        if user.OpenClipboard(None):
            user.EmptyClipboard()
            user.CloseClipboard()
        return r.stdout
    if sys.platform == "darwin":
        r = subprocess.run(["pbpaste"], capture_output=True, text=True)
        subprocess.run(["pbcopy"], input="", text=True)
        return r.stdout
    raise SecretError("no clipboard for this system (Windows and macOS only)")


def put(name: str, value: str) -> None:
    if not value:
        raise SecretError("an empty value is not stored")
    if any(ord(c) < 32 or ord(c) == 127 for c in value):
        raise SecretError("the value has control characters, not stored (Ctrl+V in the Windows console types 0x16: "
                          "paste with a right click, or copy the value and add --clipboard)")
    if sys.platform == "win32":
        _win_set(name, value)
    elif sys.platform == "darwin":
        _mac_set(name, value)
    else:
        raise SecretError("no credential store for this system (Windows and macOS only)")


def need(name: str) -> str:
    """The secret, or exit with how to set it."""
    try:
        value = get(name)
    except SecretError as e:
        sys.exit(f"secret {name}: {e}")
    if not value:
        sys.exit(f"secret {name} is not set: python -I scripts/secret.py set {name}")
    return value


def main() -> None:
    args = sys.argv[1:]
    from_clipboard = "--clipboard" in args
    if from_clipboard:
        args.remove("--clipboard")
    if len(args) != 2 or args[0] not in ("set", "check") or (from_clipboard and args[0] != "set"):
        sys.exit("usage: secret.py set <name> [--clipboard] | check <name>")
    action, name = args
    try:
        if action == "set":
            value = _clipboard() if from_clipboard else getpass.getpass(f"{name} (not shown): ")
            value = value.strip()
            put(name, value)
            # the length only, so a wrong paste shows without showing the value
            print(f"{name} stored as {TARGET.format(name)} ({len(value)} characters)")
        else:
            print(f"{name}: {'set' if get(name) else 'not set'}")
    except SecretError as e:
        sys.exit(str(e))


if __name__ == "__main__":
    main()
