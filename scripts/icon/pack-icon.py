"""Packs the app icon after `node scripts/icon/build-icon.mjs` and
`pnpm tauri icon app/src-tauri/icons/build/hubdeck-1024.png` (run in app/; then remove icons/android and
icons/ios): rewrites icon.ico and the Windows PNGs from the Windows version (the same drawing with a
smaller margin, so it is not smaller than its neighbours on the taskbar), copies the site's icon.

    python -I scripts/icon/pack-icon.py
"""
import pathlib
import shutil

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parents[2]
ICONS = ROOT / "app" / "src-tauri" / "icons"
BUILD = ICONS / "build"


def main() -> None:
    win = {s: Image.open(BUILD / f"hubdeck-win-{s}.png").convert("RGBA") for s in (16, 20, 24, 32, 48, 64, 128, 256)}
    sizes = sorted(win)
    win[256].save(ICONS / "icon.ico", format="ICO", sizes=[(s, s) for s in sizes], append_images=[win[s] for s in sizes[:-1]])
    win[32].save(ICONS / "32x32.png")
    win[64].save(ICONS / "64x64.png")
    win[128].save(ICONS / "128x128.png")
    win[256].save(ICONS / "128x128@2x.png")
    for name in ("Square30x30Logo", "Square44x44Logo", "Square71x71Logo", "Square89x89Logo", "Square107x107Logo", "Square142x142Logo", "Square150x150Logo", "Square284x284Logo", "Square310x310Logo", "StoreLogo"):
        p = ICONS / f"{name}.png"
        size = Image.open(p).size
        win[256].resize(size, Image.LANCZOS).save(p)
    shutil.copy(BUILD / "hubdeck-256.png", ROOT / "site" / "images" / "icon.png")
    print("icon.ico sizes:", sorted(Image.open(ICONS / "icon.ico").info.get("sizes", [])))


if __name__ == "__main__":
    main()
