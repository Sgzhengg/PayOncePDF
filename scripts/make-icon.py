#!/usr/bin/env python3
"""Generate the PayOnce PDF source icon (1024x1024 RGBA PNG) with the standard library only.

Run `node_modules/.bin/tauri icon src-tauri/icons/source.png` afterwards to produce
icon.ico / icon.icns / 32x32.png / 128x128.png / 128x128@2x.png for both platforms.

Design: teal rounded square, a white "1" (buy once) and a white baseline.
"""
from __future__ import annotations

import math
import struct
import zlib
from pathlib import Path

SIZE = 1024
TEAL = (15, 118, 110, 255)       # #0f766e, matches the landing page
TEAL_DARK = (12, 96, 90, 255)
WHITE = (255, 255, 255, 255)
OUT = Path(__file__).resolve().parent.parent / "src-tauri" / "icons" / "source.png"


def rounded_alpha(x: float, y: float, radius: float, inset: float) -> float:
    """Coverage of a rounded rect with a 1px antialiased edge."""
    left, top, right, bottom = inset, inset, SIZE - inset, SIZE - inset
    dx = max(left + radius - x, 0.0, x - (right - radius))
    dy = max(top + radius - y, 0.0, y - (bottom - radius))
    if dx or dy:
        d = math.hypot(dx, dy) - radius
    else:
        d = max(left - x, x - right, top - y, y - bottom)
    return max(0.0, min(1.0, 0.5 - d))


def in_rect(x: float, y: float, x0: float, y0: float, x1: float, y1: float) -> bool:
    return x0 <= x <= x1 and y0 <= y <= y1


def in_digit_one(x: float, y: float) -> bool:
    # Vertical stem.
    if in_rect(x, y, 468, 250, 556, 730):
        return True
    # Angled flag of the "1": a parallelogram above the stem.
    if 250 <= y <= 430:
        t = (y - 250) / 180.0
        x0 = 468 - 130 * (1.0 - t)
        if in_rect(x, y, x0, 250, x0 + 88, 430):
            return True
    # Baseline.
    if in_rect(x, y, 380, 730, 644, 790):
        return True
    return False


def pixel(x: int, y: int) -> tuple[int, int, int, int]:
    a = rounded_alpha(x + 0.5, y + 0.5, 205.0, 36.0)
    if a <= 0.0:
        return (0, 0, 0, 0)
    # Subtle vertical shading, still flat enough for small sizes.
    t = y / SIZE
    base = tuple(int(TEAL[i] * (1 - t) + TEAL_DARK[i] * t) for i in range(3))
    if in_digit_one(x + 0.5, y + 0.5):
        base = WHITE[:3]
    return (base[0], base[1], base[2], int(a * 255))


def main() -> None:
    raw = bytearray()
    for y in range(SIZE):
        raw.append(0)  # filter type 0
        for x in range(SIZE):
            raw.extend(pixel(x, y))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(png)
    print(f"wrote {OUT} ({len(png)} bytes)")


if __name__ == "__main__":
    main()
