#!/usr/bin/env python3
"""Turn a rasterized glyph into a macOS template image.

`qlmanage` (the rasterizer that ships with macOS) draws SVGs on an opaque
white canvas, so a tray PNG generated from it is a solid white square once
macOS treats it as a template. This script rewrites the image as an opaque
black glyph whose alpha is the ink coverage:

  - opaque source (white background + dark glyph): alpha = 255 - luminance
  - source that already has transparency: alpha is kept, RGB becomes black

Only the stdlib is used so this runs with the system Python 3 that ships
with the Xcode command line tools.

Usage: png-template.py <input.png> <output.png>
"""

import struct
import sys
import zlib

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def read_png(path):
    data = open(path, "rb").read()
    if data[:8] != PNG_SIGNATURE:
        raise SystemExit(f"{path}: not a PNG")

    pos = 8
    idat = []
    width = height = bit_depth = color_type = interlace = None
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        chunk_type = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if chunk_type == b"IHDR":
            width, height, bit_depth, color_type, _, _, interlace = struct.unpack(
                ">IIBBBBB", chunk
            )
        elif chunk_type == b"IDAT":
            idat.append(chunk)
        elif chunk_type == b"IEND":
            break

    if bit_depth != 8 or color_type not in (2, 6) or interlace != 0:
        raise SystemExit(
            f"{path}: expected an 8-bit, non-interlaced RGB/RGBA PNG "
            f"(got {bit_depth}-bit, color type {color_type}, interlace {interlace})"
        )

    channels = 4 if color_type == 6 else 3
    stride = width * channels
    raw = zlib.decompress(b"".join(idat))
    rows = []
    previous = bytearray(stride)
    offset = 0
    for _ in range(height):
        filter_type = raw[offset]
        offset += 1
        row = bytearray(raw[offset : offset + stride])
        offset += stride
        if filter_type == 1:  # Sub
            for index in range(channels, stride):
                row[index] = (row[index] + row[index - channels]) & 0xFF
        elif filter_type == 2:  # Up
            for index in range(stride):
                row[index] = (row[index] + previous[index]) & 0xFF
        elif filter_type == 3:  # Average
            for index in range(stride):
                left = row[index - channels] if index >= channels else 0
                row[index] = (row[index] + ((left + previous[index]) >> 1)) & 0xFF
        elif filter_type == 4:  # Paeth
            for index in range(stride):
                left = row[index - channels] if index >= channels else 0
                up = previous[index]
                upper_left = previous[index - channels] if index >= channels else 0
                estimate = left + up - upper_left
                distance_left = abs(estimate - left)
                distance_up = abs(estimate - up)
                distance_upper_left = abs(estimate - upper_left)
                if distance_left <= distance_up and distance_left <= distance_upper_left:
                    predictor = left
                elif distance_up <= distance_upper_left:
                    predictor = up
                else:
                    predictor = upper_left
                row[index] = (row[index] + predictor) & 0xFF
        elif filter_type != 0:
            raise SystemExit(f"{path}: unsupported PNG filter {filter_type}")
        rows.append(row)
        previous = row

    return width, height, channels, rows


def to_template(width, height, channels, rows):
    has_alpha = channels == 4 and any(
        rows[y][x * 4 + 3] != 255 for y in range(height) for x in range(width)
    )
    out = bytearray()
    for y in range(height):
        out.append(0)  # filter: none
        row = rows[y]
        for x in range(width):
            if channels == 4:
                red, green, blue, alpha = row[x * 4 : x * 4 + 4]
            else:
                red, green, blue = row[x * 3 : x * 3 + 3]
                alpha = 255
            if not has_alpha:
                luminance = (2126 * red + 7152 * green + 722 * blue) // 10000
                alpha = 255 - luminance
            out.extend((0, 0, 0, alpha))
    return bytes(out)


def write_png(path, width, height, pixels):
    def chunk(chunk_type, payload):
        checksum = zlib.crc32(chunk_type + payload) & 0xFFFFFFFF
        return (
            struct.pack(">I", len(payload))
            + chunk_type
            + payload
            + struct.pack(">I", checksum)
        )

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    png = (
        PNG_SIGNATURE
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(pixels, 9))
        + chunk(b"IEND", b"")
    )
    open(path, "wb").write(png)


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: png-template.py <input.png> <output.png>")
    width, height, channels, rows = read_png(sys.argv[1])
    write_png(
        sys.argv[2], width, height, to_template(width, height, channels, rows)
    )


if __name__ == "__main__":
    main()
