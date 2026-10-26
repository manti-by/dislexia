#!/usr/bin/env python3
"""Normalize an SVG icon source into a glyph any rasterizer can draw.

The source art is authored for a dark canvas: `media/dislexia.svg` is a white
stroke on a transparent, non-square (265x278) viewBox whose strokes run to the
frame edges. Both facts break the icon pipeline:

  - `qlmanage` (the rasterizer that ships with macOS) paints an opaque *white*
    background, so white ink rasterizes to a blank white square;
  - the iconset and the tray PNGs are square, and a glyph drawn edge-to-edge
    leaves no margin.

This script rewrites the source into an equivalent square SVG:

  - white-ish `fill`/`stroke` values become black ink (the tray template and
    the black-on-transparent app icon are both ink → alpha);
  - the artwork is scaled into a centred square canvas with `--padding` of
    margin on every side;
  - `--stroke-weight` scales every `stroke-width`, which optically bolds a
    hairline glyph for the small tray sizes where it would otherwise fade;
  - root presentation attributes (`fill`, `stroke-linecap`, …) are preserved on
    a wrapper group, so a `fill="none"` source stays an outline instead of
    turning into a solid blob.

Only the stdlib is used so this runs with the system Python 3 that ships with
the Xcode command line tools.

Usage: svg-glyph.py <input.svg> <output.svg> [--padding FRACTION]
                    [--stroke-weight FACTOR] [--canvas N]
"""

import re
import sys

DEFAULT_PADDING = 0.09
DEFAULT_CANVAS = 1024

# Ink that reads as "white" on a light canvas; all of it becomes black.
WHITE_INK = re.compile(
    r'\b(fill|stroke)="(white|#fff|#ffffff|rgb\(255,\s*255,\s*255\))"', re.IGNORECASE
)
STROKE_WIDTH = re.compile(r'\bstroke-width="([^"]*)"', re.IGNORECASE)
ROOT_TAG = re.compile(r"<svg\b([^>]*)>", re.IGNORECASE | re.DOTALL)
ATTRIBUTE = re.compile(r'([\w:-]+)="([^"]*)"')
COMMENT = re.compile(r"<!--.*?-->", re.DOTALL)

# Attributes that are re-emitted on the wrapper group; the rest of the root tag
# is layout (width/height/viewBox) or namespace, which the wrapper replaces.
PRESENTATION = {
    "fill",
    "fill-rule",
    "fill-opacity",
    "stroke",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-opacity",
    "shape-rendering",
    "opacity",
}


def parse_length(value):
    match = re.match(r"\s*(-?[\d.]+)", value or "")
    if not match:
        raise SystemExit(f"unparseable length: {value!r}")
    return float(match.group(1))


def geometry(attributes):
    """Canvas size and viewBox of the source, as (width, height, viewBox)."""
    view_box = attributes.get("viewBox") or attributes.get("viewbox")
    if view_box:
        parts = [float(part) for part in re.split(r"[,\s]+", view_box.strip()) if part]
        if len(parts) == 4:
            _, _, width, height = parts
        else:
            raise SystemExit(f"unexpected viewBox: {view_box!r}")
        origin = f"{parts[0]:g} {parts[1]:g}"
    else:
        if "width" not in attributes or "height" not in attributes:
            raise SystemExit("source SVG needs a viewBox or width/height")
        width = parse_length(attributes["width"])
        height = parse_length(attributes["height"])
        origin = "0 0"

    if width <= 0 or height <= 0:
        raise SystemExit(f"non-positive canvas: {width}x{height}")
    return width, height, origin


def normalize(source, padding, canvas, stroke_weight=1.0):
    without_comments = COMMENT.sub("", source)
    root = ROOT_TAG.search(without_comments)
    if not root:
        raise SystemExit("source is not an SVG document")

    attributes = dict(ATTRIBUTE.findall(root.group(1)))
    width, height, origin = geometry(attributes)
    opening_end = root.end()
    closing_start = without_comments.rfind("</svg>")
    if closing_start < opening_end:
        raise SystemExit("source SVG is not closed")

    body = without_comments[opening_end:closing_start]
    body = WHITE_INK.sub(lambda m: f'{m.group(1)}="#000000"', body).strip()
    if stroke_weight != 1.0:
        body = STROKE_WIDTH.sub(
            lambda m: f'stroke-width="{float(m.group(1)) * stroke_weight:g}"', body
        )

    square = max(width, height)
    scale = (1 - 2 * padding) * square / max(width, height)
    offset_x = (square - width * scale) / 2
    offset_y = (square - height * scale) / 2

    # Keep the source's own presentation attributes, but pin the ink colour:
    # an outset glyph needs black ink, whatever the art was authored in.
    inherited = {
        name: value
        for name, value in attributes.items()
        if name.lower() in PRESENTATION
    }
    inherited.setdefault("fill", "#000000")
    if inherited["fill"].lower() in ("white", "#fff", "#ffffff"):
        inherited["fill"] = "#000000"
    if inherited.get("stroke", "").lower() in ("white", "#fff", "#ffffff"):
        inherited["stroke"] = "#000000"

    wrapper_attributes = " ".join(
        f'{name}="{value}"' for name, value in inherited.items()
    )
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{canvas}" '
        f'height="{canvas}" viewBox="0 0 {square:g} {square:g}">\n'
        f'  <g transform="translate({offset_x:.4f} {offset_y:.4f}) '
        f'scale({scale:.6f})" {wrapper_attributes}>\n'
        f"    {body}\n"
        f"  </g>\n"
        f"</svg>\n"
    )


def main(argv):
    if len(argv) < 3:
        raise SystemExit(
            "usage: svg-glyph.py <input.svg> <output.svg> "
            "[--padding FRACTION] [--canvas N]"
        )

    padding = DEFAULT_PADDING
    canvas = DEFAULT_CANVAS
    stroke_weight = 1.0
    rest = argv[3:]
    while rest:
        flag = rest.pop(0)
        if flag == "--padding" and rest:
            padding = float(rest.pop(0))
        elif flag == "--stroke-weight" and rest:
            stroke_weight = float(rest.pop(0))
        elif flag == "--canvas" and rest:
            canvas = int(rest.pop(0))
        else:
            raise SystemExit(f"unknown argument: {flag}")
    if not 0 <= padding < 0.5:
        raise SystemExit(f"padding must be in [0, 0.5): {padding}")
    if stroke_weight <= 0:
        raise SystemExit(f"stroke weight must be positive: {stroke_weight}")

    source = open(argv[1], encoding="utf-8").read()
    open(argv[2], "w", encoding="utf-8").write(
        normalize(source, padding, canvas, stroke_weight)
    )


if __name__ == "__main__":
    main(sys.argv)
