#!/usr/bin/env python3
"""Generate UI icons from Pixelarticons masks and pixelified Phosphor glyphs."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import urllib.request
import sys

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "resources/icons/ui"
SOURCE = ROOT / "resources/icons/src"
NEW = ("brain", "globe", "file-plus", "play", "circle", "help")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--fetch-sources", action="store_true", help="Vendor missing Pixelarticons/Phosphor glyph SVGs (MIT)")
    args = parser.parse_args()
    SOURCE.mkdir(exist_ok=True)
    if args.fetch_sources:
        for name in NEW:
            target = SOURCE / f"{name}.svg"
            if not target.exists():
                url = (f"https://raw.githubusercontent.com/phosphor-icons/core/main/assets/fill/{name}-fill.svg"
                       if name in ("brain", "file-plus") else
                       f"https://raw.githubusercontent.com/halfmage/pixelarticons/master/svg/{'circle-question' if name == 'help' else name}.svg")
                data = urllib.request.urlopen(url, timeout=30).read()
                if b"<svg" not in data:
                    raise ValueError(f"Not an SVG: {url}")
                target.write_bytes(data)
    for name in NEW:
        if not (SOURCE / f"{name}.svg").is_file():
            raise FileNotFoundError(f"{name}.svg missing: run with --fetch-sources once")
    tokens = json.loads((ROOT / "resources/tokens-site.json").read_text())
    names = sorted({p.name.removesuffix("-ink-dark-1x.png") for p in OUT.glob("*-ink-dark-1x.png")} | set(NEW))
    # Stage masks before replacing outputs; generation must never read its own new tint.
    with tempfile.TemporaryDirectory(prefix="pi-icons-") as directory:
        for name in names:
            source = SOURCE / f"{name}.svg" if name in NEW else OUT / f"{name}-ink-dark-1x.png"
            mask = Path(directory) / f"{name}.png"
            if name in ("brain", "file-plus"):
                subprocess.run([sys.executable, str(ROOT / "tools/pixelify.py"), str(source),
                                "-o", str(mask), "--grid", "24", "--block", "1", "--nearest", "--tint", "#ffffff"], check=True)
            else:
                subprocess.run(["magick", "-background", "none", str(source), "-resize", "24x24!", str(mask)], check=True)
            for theme in ("dark", "light"):
                for tone, token in (("ink", "text"), ("accent", "accent"), ("muted", "muted")):
                    for scale in (1, 2):
                        target = OUT / f"{name}-{tone}-{theme}-{scale}x.png"
                        subprocess.run(["magick", str(mask), "-channel", "RGB", "-fill", tokens[theme][token],
                                        "-colorize", "100", "+channel", "-filter", "point",
                                        "-resize", f"{24 * scale}x{24 * scale}!", "-strip", str(target)], check=True)
    print(f"{len(names)} icons × 12 variants: {OUT}")


if __name__ == "__main__":
    main()
