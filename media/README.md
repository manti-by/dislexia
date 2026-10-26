# media/ — icon source assets

| File | Purpose |
|---|---|
| `dislexia.svg` | **The** Dislexia icon: a single hand-drawn spiral stroke, white ink on a transparent 265×278 canvas. Source for everything icon-related in the app. |

Nothing else lives here — the previous typewriter glyph and its PNG preview are gone.

## Origin & license

- Artwork authored for this project (single path, `stroke-width="10"`, no fill). Project license: BSD-3-Clause (see the repository root).

## How this file is used (see `plan 02 §8` and `plan 07 §5`)

All icon assets consumed by the app are **generated** from this one SVG — never hand-edit the PNGs:

| Generated file | Made by |
|---|---|
| `assets/icon.icns` (app bundle icon) | `scripts/make-icons.sh`: `svg-glyph.py` → rasterize → `png-template.py` → iconset → `.icns` |
| `assets/tray-16@1x.png`, `assets/tray-32@2x.png` | same script: the glyph is rasterized a second time with a tighter margin and a heavier stroke (optical sizing), then `sips`-downsized; loaded as **template** images so macOS tints them for light/dark menu bars |

Two normalization steps sit between the art and the rasterizer, because the
source is drawn for a dark canvas:

- `scripts/svg-glyph.py` turns the white ink black, squares the canvas and adds
  a margin. The app icon gets a macOS-sized margin (9%); the tray glyph a tight
  one (6%) plus a 1.25× stroke, because a hairline mark disappears at 16px.
- `scripts/png-template.py` converts the rasterized ink to alpha. `qlmanage`
  draws on opaque white; without this the icon is a white square, in the Dock
  as well as in the menu bar.

If the app icon ever changes, replace `dislexia.svg` here and re-run
`make icons` — do not edit derived files under `assets/`.
