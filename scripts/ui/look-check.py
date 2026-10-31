"""One trial scene, checked the way the owner looks at it: 1:1 in both turns, from 60 cm, with numbers.

A branch of a preset test (owner, 2026-10-29: branches A/B-tested by fresh agents, judged blind)
writes one WGSL file, `x-<name>.wgsl`, drawn in world-square coordinates (`world()` in the
shader prelude, `.claude/skills/living-deck-preset/API.md`). This renders it with the engine (`fx_film`, sample
icons like the owner's, a press at 1 s), in landscape (turn 0) and upright (turn 90), and writes:

- `<out>/land.png`, `<out>/port.png`: the deck as it shows a still at `--at` seconds (`deck-view.py`);
- `<out>/land-60cm.png`, `<out>/port-60cm.png`: the same shrunk to roughly what the eye resolves at
  60 cm and back up, a preview;
- `<out>/press-*.png`: the press at 1 s as a strip (landscape);
- `<out>/numbers.txt`: the engine's cost a frame, the light, detail in the gaps, the big shapes;
- with `--bare`, `<out>/bare-land.png` and `<out>/bare-port.png`: the scene alone, no icons, no grille.

    python -I scripts/ui/look-check.py <x-name.wgsl> --out <dir> [--at 3.0] [--seconds 4]

Needs the release test binary (`powershell -File scripts/cargo-low.ps1 test --release -p hubdeck --no-run`).
"""
import argparse
import glob
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

from PIL import Image, ImageFilter, ImageStat

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
deck_view = __import__("deck-view").deck_view  # noqa: E402  (the 1:1 view, same folder)

KW, KH, DX, DY, W, H = 79, 75, 100, 99, 480, 272


def binary() -> pathlib.Path:
    # Windows: the .exe; Linux and macOS: the executable without an extension (a cloud session, 2026-10-29)
    found = sorted([f for f in glob.glob(str(ROOT / "target" / "release" / "deps" / "hubdeck_lib-*"))
                    if f.endswith(".exe") or (os.name != "nt" and "." not in os.path.basename(f) and os.access(f, os.X_OK))],
                   key=os.path.getmtime)
    if not found:
        sys.exit("no release test binary: powershell -File scripts/cargo-low.ps1 test --release -p hubdeck --no-run")
    return pathlib.Path(found[-1])


PROFILE = None  # a profile file whose main page stands on the keys (--profile), else the engine's sample


def film(shader: pathlib.Path, turn: int, seconds: float, sheet: pathlib.Path, bare: bool = False) -> str:
    kind = shader.stem
    env = dict(os.environ, HUBDECK_SHADER_DIR=str(shader.parent), HUBDECK_FX=kind, HUBDECK_SHEET=str(sheet),
               HUBDECK_SECONDS=str(seconds), HUBDECK_PRESSES="7@1.0", HUBDECK_TURN=str(turn))
    if bare:
        env["HUBDECK_BARE"] = "1"
    if PROFILE:
        env["HUBDECK_PROFILE"] = str(PROFILE)
    run = subprocess.run([str(binary()), "fx_film", "--ignored", "--nocapture", "--exact", "engine::canvas::tests::fx_film"],
                         env=env, capture_output=True, text=True, timeout=600)
    text = run.stdout + run.stderr
    if run.returncode != 0 or not any(sheet.glob("*.png")):
        sys.exit(f"fx_film failed for turn {turn}:\n{text[-3000:]}")
    return text


def windows(turn: int):
    """The 15 windows in the panel's own pixels (the frames fx_film writes are the panel)."""
    return [(c * DX, r * DY, c * DX + KW, r * DY + KH) for r in range(3) for c in range(5)]


def numbers(frame: Image.Image) -> dict:
    g = frame.convert("L")
    edges = g.filter(ImageFilter.FIND_EDGES)
    inside = Image.new("L", g.size, 0)
    for x0, y0, x1, y1 in windows(0):
        inside.paste(255, (x0, y0, x1, y1))
    gap = Image.eval(inside, lambda v: 255 - v)
    e_in = ImageStat.Stat(edges, inside).mean[0]
    e_gap = ImageStat.Stat(edges, gap).mean[0]
    light = ImageStat.Stat(g, inside).mean[0] / 255
    # big shapes: the scene blurred to the size of a quarter of a key still varies across the deck
    big = g.resize((W // 20, H // 20), Image.BOX)
    return {"light": light, "gap_detail": e_gap / max(e_in, 1e-6), "big_shapes": ImageStat.Stat(big).stddev[0]}


def at_60cm(img: Image.Image) -> Image.Image:
    # about 1/6 of the 1:1 view's size is what reads of a 2-3x zoom from arm's length (a rough model:
    # a key window ~2 cm at 60 cm), then back up so it can be looked at
    small = img.resize((max(1, img.width // 6), max(1, img.height // 6)), Image.LANCZOS)
    return small.resize(img.size, Image.NEAREST)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("shader")
    ap.add_argument("--out", required=True)
    ap.add_argument("--at", type=float, default=3.0)
    ap.add_argument("--seconds", type=float, default=4.0)
    ap.add_argument("--profile", help="a profile .json whose main page stands on the keys (the owner's real page)")
    ap.add_argument("--bare", action="store_true", help="also the bare scene: no icons, no grille (bare-land.png, bare-port.png)")
    a = ap.parse_args()
    global PROFILE
    PROFILE = a.profile
    shader = pathlib.Path(a.shader).resolve()
    if not re.fullmatch(r"x-[a-z0-9-]+", shader.stem):
        sys.exit("the file must be named x-<name>.wgsl (a trial layer)")
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    report = []
    with tempfile.TemporaryDirectory() as tmp:
        for turn, name in ((0, "land"), (90, "port")):
            sheet = pathlib.Path(tmp) / name
            text = film(shader, turn, a.seconds, sheet)
            cost = re.search(r"CPU ([0-9.]+) ms a frame", text)
            frames = sorted(sheet.glob("*.png"))
            still = Image.open(frames[min(len(frames) - 1, int(a.at * 24))])
            view = deck_view(still, 2, turn)
            view.save(out / f"{name}.png")
            at_60cm(view).save(out / f"{name}-60cm.png")
            n = numbers(still)
            report.append(f"{name}: cost {cost.group(1) if cost else '?'} ms a frame (canvas and panel, all layers); "
                          f"light in the windows {n['light']:.0%}; detail in the gaps {n['gap_detail']:.2f} of the windows'; "
                          f"big shapes {n['big_shapes']:.1f}")
            if turn == 0:
                for i, t in enumerate((1.0, 1.25, 1.5, 2.0)):
                    deck_view(Image.open(frames[min(len(frames) - 1, int(t * 24))]), 2, 0).save(out / f"press-{i}.png")
            # a GPU layer that does not compile is left out without a word on the deck (KNOWLEDGE, session 33)
            bad = [l for l in text.splitlines() if "does not compile" in l or "GPU layer" in l]
            if bad:
                report.append(f"{name}: the engine printed: " + " | ".join(bad)[:600])
        if a.bare:
            # the scene alone, the whole panel turned upright, at 2x (owner, 2026-10-29: "gołe tła? Bez ikon i ramek?")
            for turn, name in ((0, "land"), (90, "port")):
                sheet = pathlib.Path(tmp) / f"bare-{name}"
                film(shader, turn, a.seconds, sheet, bare=True)
                frames = sorted(sheet.glob("*.png"))
                still = Image.open(frames[min(len(frames) - 1, int(a.at * 24))]).convert("RGB")
                still = still.rotate(turn, expand=True) if turn else still
                still.resize((still.width * 2, still.height * 2), Image.LANCZOS).save(out / f"bare-{name}.png")
                # detail of the scene alone: edge strength at full size (fine) and at a quarter (mid)
                g = still.convert("L")
                fine = ImageStat.Stat(g.filter(ImageFilter.FIND_EDGES)).mean[0]
                mid = ImageStat.Stat(g.resize((g.width // 4, g.height // 4), Image.BOX).filter(ImageFilter.FIND_EDGES)).mean[0]
                report.append(f"bare {name}: detail fine {fine:.1f}, mid {mid:.1f}")
    (out / "numbers.txt").write_text("\n".join(report) + "\n", encoding="utf-8")
    print("\n".join(report))
    print(f"-> {out}")


if __name__ == "__main__":
    main()
