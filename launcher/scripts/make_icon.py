#!/usr/bin/env python3
# Regenerate the placeholder icon: python3 scripts/make_icon.py src-tauri/icons/app-icon.png
# then: npx tauri icon src-tauri/icons/app-icon.png
# An original placeholder in the game's colours (no game art, no logo): a
# blood-red sun and a white katana across it, on an ink-black rounded square.
# Standard library only.
import math, struct, sys, zlib

N = 1024

def rr(px, py, r=200, m=40):
    """Inside the rounded square."""
    x0, y0, x1, y1 = m, m, N - m, N - m
    cx = min(max(px, x0 + r), x1 - r)
    cy = min(max(py, y0 + r), y1 - r)
    return (px - cx) ** 2 + (py - cy) ** 2 <= r * r and x0 <= px <= x1 and y0 <= py <= y1

SUN = (585, 440, 262)                      # centre and radius
A, B = (205, 845), (850, 200)              # the katana: from the pommel to the tip
L = math.hypot(B[0] - A[0], B[1] - A[1])
UX, UY = (B[0] - A[0]) / L, (B[1] - A[1]) / L

def along(px, py):
    """Distance along the sword from the pommel, and across it."""
    dx, dy = px - A[0], py - A[1]
    return dx * UX + dy * UY, -dx * UY + dy * UX

def layer(px, py):
    """The colour at a point, or None for the background."""
    t, n = along(px, py)
    # the hilt (tsuka): dark wrap with red diamonds, then the guard (tsuba)
    if 0 <= t <= 205 and abs(n) <= 25:
        return (200, 16, 46) if (int((t + abs(n)) / 22) % 2 == 0 and abs(n) <= 14) else (24, 20, 21)
    if 205 < t <= 228 and abs(n) <= 52:
        return (214, 170, 70)
    # the blade: a slight curve, a sharp tip, a bright edge
    if 228 < t <= L:
        k = (t - 228) / (L - 228)
        bow = 18 * math.sin(math.pi * k)          # the sori
        w = 21 * (1 - k ** 6)                      # tapers to the point
        m = n + bow
        if -w <= m <= w:
            return (250, 246, 238) if m < w * 0.25 else (196, 190, 182)
    d = math.hypot(px - SUN[0], py - SUN[1])
    if d <= SUN[2]:
        return (200, 16, 46) if d < SUN[2] - 14 else (150, 10, 32)
    return None

S = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)]
rows = []
for y in range(N):
    row = bytearray([0])
    for x in range(N):
        inside = 0
        acc = [0, 0, 0]
        for sx, sy in S:
            px, py = x + sx, y + sy
            if not rr(px, py):
                continue
            inside += 1
            t = py / N
            bg = (int(18 - 10 * t), int(15 - 9 * t), int(16 - 9 * t))   # ink black, darker below
            c = layer(px, py) or bg
            for i in range(3):
                acc[i] += c[i]
        if inside == 0:
            row += b"\0\0\0\0"
            continue
        row += bytes([acc[i] // inside for i in range(3)] + [int(255 * inside / 4)])
    rows.append(bytes(row))

def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(b"".join(rows), 9)) + chunk(b"IEND", b""))
open(sys.argv[1], "wb").write(png)
