#!/usr/bin/env python3
"""Pixelate an image into N-pixel blocks (default 23).

The same idea as pixelify.fasu.dev, with no Python image dependency: decoding
and encoding go through ImageMagick, while the core (alpha-aware downsample,
nearest-neighbour upscale and optional palette quantization) lives here so the
grid stays under our control.

    tools/pixelify.py logo.svg -o logo-pixel.png --block 23
    tools/pixelify.py logo.png  -o logo-pixel.png --block 23 --colors 8
    tools/pixelify.py logo.svg -o logo-pixel.png --block 23 --nearest
"""
from __future__ import annotations

import argparse
import struct
import subprocess
import sys
import zlib


def load(path: str) -> tuple[int, int, bytearray]:
    """Decode any format ImageMagick supports into raw RGBA."""
    raw = subprocess.run(
        ["convert", path, "-background", "none", "-alpha", "on", "rgba:-"],
        capture_output=True, check=True,
    ).stdout
    w, h = (int(v) for v in subprocess.run(
        ["identify", "-format", "%w %h", path], capture_output=True, text=True, check=True,
    ).stdout.split())
    return w, h, bytearray(raw)


def save(path: str, w: int, h: int, pixels: bytearray) -> None:
    rows = b"".join(b"\x00" + bytes(pixels[y * w * 4:(y + 1) * w * 4]) for y in range(h))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(rows, 9))
           + chunk(b"IEND", b""))
    with open(path, "wb") as fh:
        fh.write(png)


def downsample(w: int, h: int, px: bytearray, block: int, nearest: bool) -> tuple[int, int, bytearray]:
    sw = max(1, round(w / block))
    sh = max(1, round(h / block))
    out = bytearray(sw * sh * 4)
    for sy in range(sh):
        y0, y1 = sy * h // sh, (sy + 1) * h // sh
        for sx in range(sw):
            x0, x1 = sx * w // sw, (sx + 1) * w // sw
            if nearest:
                cx, cy = (x0 + x1) // 2, (y0 + y1) // 2
                i = (cy * w + cx) * 4
                out[(sy * sw + sx) * 4:(sy * sw + sx) * 4 + 4] = px[i:i + 4]
                continue
            r = g = b = a = n = 0
            for y in range(y0, max(y1, y0 + 1)):
                row = y * w * 4
                for x in range(x0, max(x1, x0 + 1)):
                    i = row + x * 4
                    alpha = px[i + 3]
                    if alpha == 0:
                        continue
                    r += px[i] * alpha
                    g += px[i + 1] * alpha
                    b += px[i + 2] * alpha
                    a += alpha
                    n += 1
            if n == 0 or a == 0:
                continue
            j = (sy * sw + sx) * 4
            out[j] = r // a
            out[j + 1] = g // a
            out[j + 2] = b // a
            out[j + 3] = a // n
    return sw, sh, out


def quantize(sw: int, sh: int, px: bytearray, colors: int) -> bytearray:
    """Simple median cut: group the most frequent opaque colors and map each
    block to its group center. Shrinks the palette without dithering (dithering
    on small blocks turns into noise)."""
    hist: dict[tuple[int, int, int], int] = {}
    for i in range(0, len(px), 4):
        if px[i + 3] < 8:
            continue
        key = (px[i], px[i + 1], px[i + 2])
        hist[key] = hist.get(key, 0) + 1
    if not hist:
        return px
    palette = list(hist)
    palette = _median_cut(palette, colors)
    out = bytearray(px)
    for i in range(0, len(out), 4):
        if out[i + 3] < 8:
            continue
        src = (out[i], out[i + 1], out[i + 2])
        nearest = min(palette, key=lambda c: (c[0] - src[0]) ** 2 + (c[1] - src[1]) ** 2 + (c[2] - src[2]) ** 2)
        out[i], out[i + 1], out[i + 2] = nearest
    return out


def _median_cut(palette: list[tuple[int, int, int]], colors: int) -> list[tuple[int, int, int]]:
    boxes = [palette]
    while len(boxes) < colors:
        boxes.sort(key=len, reverse=True)
        box = boxes.pop(0)
        if len(box) < 2:
            boxes.append(box)
            break
        channel = max(range(3), key=lambda c: max(p[c] for p in box) - min(p[c] for p in box))
        box.sort(key=lambda p: p[channel])
        mid = len(box) // 2
        boxes.extend([box[:mid], box[mid:]])
    return [tuple(sum(p[c] for p in box) // len(box) for c in range(3)) for box in boxes]


def upscale(sw: int, sh: int, px: bytearray, block: int) -> tuple[int, int, bytearray]:
    w, h = sw * block, sh * block
    out = bytearray(w * h * 4)
    for sy in range(sh):
        for sx in range(sw):
            i = (sy * sw + sx) * 4
            pixel = px[i:i + 4]
            for y in range(sy * block, (sy + 1) * block):
                row = y * w * 4
                for x in range(sx * block, (sx + 1) * block):
                    j = row + x * 4
                    out[j:j + 4] = pixel
    return w, h, out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("input")
    ap.add_argument("-o", "--output", required=True)
    ap.add_argument("--block", type=int, default=23, help="block side in px (default 23)")
    ap.add_argument("--colors", type=int, default=0, help="0 = no quantization")
    ap.add_argument("--nearest", action="store_true", help="sample the center pixel instead of averaging")
    ap.add_argument("--tint", default="", help="recolor opaque pixels (hex), e.g. #ebe7e4")
    ap.add_argument("--grid", type=int, default=0, help="force N blocks per side (rasterize the input at that size before downsampling)")
    args = ap.parse_args()

    source = args.input
    if args.grid > 0:
        target = args.grid * args.block
        raster = f"/tmp/pixelify-src-{args.grid}-{args.block}.png"
        resize_filter = ["-filter", "point"] if args.nearest else []
        subprocess.run(["convert", "-background", "none", "-density", "600",
                        args.input, *resize_filter, "-resize", f"{target}x{target}",
                        "-alpha", "on", "-define", "png:color-type=6", raster], check=True)
        source = raster

    w, h, px = load(source)
    sw, sh, small = downsample(w, h, px, args.block, args.nearest)
    if args.tint:
        r, g, b = (int(args.tint.lstrip("#")[i:i+2], 16) for i in (0, 2, 4))
        for i in range(0, len(small), 4):
            if small[i + 3] >= 8:
                small[i], small[i + 1], small[i + 2] = r, g, b
                small[i + 3] = 255
    if args.colors > 1:
        small = quantize(sw, sh, small, args.colors)
    ow, oh, big = upscale(sw, sh, small, args.block)
    save(args.output, ow, oh, big)
    print(f"{args.input}: {w}x{h} -> grid {sw}x{sh} -> {ow}x{oh} (block {args.block})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
