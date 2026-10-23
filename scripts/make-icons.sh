#!/usr/bin/env bash
# Generates every icon asset from the single source SVG:
#   media/icon-typewriter.svg
#     ├─ rasterize 1024px → icons.iconset/ → iconutil → assets/icon.icns
#     └─ rasterize 16/32px (black glyph, transparent bg) → assets/tray-*.png
# Nothing under assets/ is hand-edited. See plan/02-project-setup.md §8.
set -euo pipefail
cd "$(dirname "$0")/.."

SVG=media/icon-typewriter.svg
OUT=assets
WORK=icons.iconset
MASTER=.master-1024.png

mkdir -p "$OUT" "$WORK"

# Rasterize the SVG to a 1024px master PNG. rsvg-convert keeps the
# transparent background and exact black fill; qlmanage is the fallback that
# ships with macOS.
rm -f "$WORK"/icon_*.png "$WORK"/master-1024.png "$WORK/icon-typewriter.svg.png" "$MASTER"
if command -v rsvg-convert >/dev/null 2>&1; then
    rsvg-convert -w 1024 -h 1024 "$SVG" -o "$MASTER"
else
    qlmanage -t -s 1024 -o "$WORK" "$SVG" >/dev/null 2>&1
    mv "$WORK/icon-typewriter.svg.png" "$MASTER"
fi

# App icon: the full iconset (macOS rounds the canvas at display time).
icon_size() { # size 2x
    local size=$1 two=$2
    if [ "$two" = yes ]; then
        sips -z $((size * 2)) $((size * 2)) "$MASTER" --out "$WORK/icon_${size}x${size}@2x.png" >/dev/null
    else
        sips -z "$size" "$size" "$MASTER" --out "$WORK/icon_${size}x${size}.png" >/dev/null
    fi
}
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
# with set_template(true) so macOS adapts them to light/dark menu bars.
sips -z 16 16 "$MASTER" --out "$OUT/tray-16@1x.png" >/dev/null
sips -z 32 32 "$MASTER" --out "$OUT/tray-32@2x.png" >/dev/null

rm -f "$MASTER"
rm -f "$OUT/icon.icns"
iconutil -c icns "$WORK" -o "$OUT/icon.icns"
echo "Generated $OUT/icon.icns + tray PNGs from $SVG"
