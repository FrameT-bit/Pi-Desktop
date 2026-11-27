# UI glyph sources

`globe`, `play`, `circle` and `help` use Pixelarticons, the same family as the existing
set (see `LICENSE-pixelarticons`). `brain` and `file-plus` start as Phosphor fill
SVGs (MIT, see `LICENSE`). Sources are vendored; normal generation is offline.

The other glyphs retain the project's existing pixel masks. The generator uses
their ink/dark/1x alpha channel, without modifying their silhouettes.

The two Phosphor SVGs pass through
`tools/pixelify.py --grid 24 --block 1 --nearest --tint '#ffffff'` before tinting:
the finer pixel grid retains the brain lobes and file's plus at the 24px slot.
Both use hard pixels, not antialiased vector edges. The 48px scale is
nearest-neighbour. Pixelarticons sources keep their native 24px pixel grid.

Run `bash tools/gen-icons.sh` to generate all 12 variants per icon. Colors come
from `resources/tokens-site.json`; PNGs are packaged by `build.rs`.
