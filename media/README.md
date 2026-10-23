# media/ — icon source assets

| File | Purpose |
|---|---|
| `icon-typewriter.svg` | **The** Dislexia icon: minimal monochrome typewriter ("writing machine"). Single glyph, pure black on transparent, 24-unit viewBox. Source for everything icon-related in the app. |
| `icon-preview-512.png` | Quick preview render of the SVG (for humans); not used by the app or build. |

## Origin & license

- Glyph: **"typewriter"** from [Material Design Icons](https://pictogrammers.com/library/mdi/icon/typewriter/) by Pictogrammers, license **Apache-2.0** (use and modification allowed; attribution not required but kept here on purpose).
- The checked-in SVG is a re-export of the original icon: path data is unchanged; `width`/`height` were fixed to `512` and `fill` pinned to `#000000` so rasterizers that mishandle `1em`/`currentColor` still produce correct output.

## How this file is used (see `plan/02-project-setup.md` §7 and `plan/07-theme.md` §5)

All icon assets consumed by the app are **generated** from this one SVG — never hand-edit the PNGs:

| Generated file | Made by |
|---|---|
| `assets/icon.icns` (app bundle icon) | `scripts/make-icons.sh`: rasterize SVG → 1024px PNG → `iconutil`-style iconset → `.icns` |
| `assets/tray-16@1x.png`, `assets/tray-32@2x.png` | same script: rasterize at 16px / 32px, black glyph on transparency, loaded as **template** images so macOS tints them for light/dark menu bars |

If the app icon ever changes, replace `icon-typewriter.svg` here and re-run the script — do not edit derived files under `assets/`.
