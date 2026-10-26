#!/usr/bin/env bash
# Generates every icon asset from the single source SVG:
#   media/dislexia.svg
#     ├─ svg-glyph.py → black-ink square glyph
#     ├─ rasterize 1024px → png-template.py (ink → alpha) → master PNG
#     ├─ app master → icons.iconset/ → iconutil → assets/icon.icns
#     └─ tray master → sips 16/32px → assets/tray-*.png
# Nothing under assets/ is hand-edited. See plan 02 §8.
set -euo pipefail
cd "$(dirname "$0")/.."

SVG=media/dislexia.svg
OUT=assets
WORK=icons.iconset
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$OUT" "$WORK"

# Rasterizes the source into a transparent 1024px master with black ink.
#   $1 name (also the master's file stem)
#   $2 padding around the glyph, as a fraction of the canvas
#   $3 stroke weight multiplier — optical sizing for the small tray sizes
rasterize() {
    local name=$1 padding=$2 weight=$3
    python3 scripts/svg-glyph.py "$SVG" "$TMP/$name.svg" \
        --padding "$padding" --stroke-weight "$weight"

    # rsvg-convert keeps the transparent background; qlmanage is the fallback
    # that ships with macOS and draws on opaque white.
    if command -v rsvg-convert >/dev/null 2>&1; then
        rsvg-convert -w 1024 -h 1024 "$TMP/$name.svg" -o "$TMP/$name-raw.png"
    else
        qlmanage -t -s 1024 -o "$TMP" "$TMP/$name.svg" >/dev/null 2>&1
        mv "$TMP/$name.svg.png" "$TMP/$name-raw.png"
    fi

    # Ink → alpha: black ink on transparency. This is also what keeps the app
    # icon from being a white square in the Dock when the rasterizer drew on an
    # opaque background.
    python3 scripts/png-template.py "$TMP/$name-raw.png" "$TMP/$name.png"
}

# The app icon keeps a macOS-sized margin; the tray glyph is drawn larger and
# a touch heavier, because a hairline mark vanishes at 16px in the menu bar.
rasterize app 0.09 1.0
rasterize tray 0.06 1.25

# App icon: the full iconset (macOS rounds the canvas at display time).
icon_size() { # size 2x
    local size=$1 two=$2
    if [ "$two" = yes ]; then
        sips -z $((size * 2)) $((size * 2)) "$TMP/app.png" --out "$WORK/icon_${size}x${size}@2x.png" >/dev/null
    else
        sips -z "$size" "$size" "$TMP/app.png" --out "$WORK/icon_${size}x${size}.png" >/dev/null
    fi
}
rm -f "$WORK"/icon_*.png
icon_size 16 no
icon_size 16 yes
icon_size 32 no
icon_size 32 yes
icon_size 128 no
icon_size 128 yes
icon_size 256 no
icon_size 256 yes
icon_size 512 no
icon_size 512 yes

# Tray icons: template images (opaque black glyph on transparency), loaded
# with set_template(true) so macOS adapts them to light/dark menu bars. The
# master is already a template, so no second ink pass is needed.
sips -z 16 16 "$TMP/tray.png" --out "$OUT/tray-16@1x.png" >/dev/null
sips -z 32 32 "$TMP/tray.png" --out "$OUT/tray-32@2x.png" >/dev/null

rm -f "$OUT/icon.icns"
iconutil -c icns "$WORK" -o "$OUT/icon.icns"
echo "Generated $OUT/icon.icns + tray PNGs from $SVG"
