#!/usr/bin/env python3
"""Generate the LosOS cloud and LosOS Git marks from fish.png.

fish.png is the source: 16x16 pixel art, stored at 10x (160x160). This script
reads it back to its 16x16 grid and writes each mark as a self-contained SVG
whose fish is one square per pixel, so it stays crisp at any size, from a 16px
favicon to the login page. The PNGs and the .ico each app also wants are
rasterized from these SVGs at build time (admin-ui/themes/default.nix), so
only the SVGs are committed.

Standard library only. Run it after changing fish.png or a shape below:

    python3 admin-ui/themes/brand/marks.py
"""

import os
import struct
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
SCALE = 10  # fish.png is drawn at 10 screen pixels per art pixel


def read_png(path):
    """Decode an 8-bit RGBA, non-interlaced PNG to rows of (r, g, b, a)."""
    data = open(path, "rb").read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG"
    pos, idat = 8, b""
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind, body = data[pos + 4 : pos + 8], data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", body)
            assert (depth, colour, interlace) == (8, 6, 0), "need 8-bit RGBA, not interlaced"
        elif kind == b"IDAT":
            idat += body
    raw, bpp = zlib.decompress(idat), 4
    stride, rows, prev, i = width * bpp, [], bytearray(width * bpp), 0
    for _ in range(height):
        kind, line = raw[i], bytearray(raw[i + 1 : i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if kind == 1:
                line[x] = (line[x] + a) & 255
            elif kind == 2:
                line[x] = (line[x] + b) & 255
            elif kind == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append([tuple(line[x : x + 4]) for x in range(0, stride, 4)])
        prev = line
    return rows


def fish_grid():
    rows = read_png(os.path.join(HERE, "fish.png"))
    size = len(rows) // SCALE
    return [[rows[y * SCALE][x * SCALE] for x in range(size)] for y in range(size)]


def fish_paths(grid):
    """One <path> per colour, each horizontal run of a colour one rectangle."""
    runs = {}
    for y, row in enumerate(grid):
        x = 0
        while x < len(row):
            r, g, b, a = row[x]
            if a == 0:
                x += 1
                continue
            end = x
            while end < len(row) and row[end] == row[x]:
                end += 1
            runs.setdefault("#%02x%02x%02x" % (r, g, b), []).append(f"M{x} {y}h{end - x}v1h-{end - x}z")
            x = end
    return "\n".join(
        f'    <path fill="{colour}" d="{"".join(d)}"/>' for colour, d in sorted(runs.items())
    )


def fish(grid, x, y, size):
    """The fish as a nested 16-unit viewport placed at (x, y), size units wide."""
    n = len(grid)
    return (
        f'  <svg x="{x}" y="{y}" width="{size}" height="{size}" viewBox="0 0 {n} {n}" '
        f'shape-rendering="crispEdges">\n{fish_paths(grid)}\n  </svg>'
    )


# The cloud is ☁️ drawn the way the emoji fonts draw it: a soft white cloud
# that fades to a cool grey underneath, with an outline so it still reads on a
# white page. Three puffs and a flat base, with the fish
# centred in the body of the cloud. Drawn twice from the same shapes,
# stroked and then filled, so the outline traces the union and not each puff.
CLOUD = """\
    <circle cx="32" cy="33" r="18"/>
    <circle cx="14" cy="44" r="11"/>
    <circle cx="50" cy="43" r="12"/>
    <rect x="14" y="40" width="36" height="15"/>"""


def cloud_mark(grid):
    return f"""\
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <title>LosOS cloud</title>
  <defs>
    <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#ffffff"/>
      <stop offset="1" stop-color="#d5e1ea"/>
    </linearGradient>
  </defs>
  <g fill="#8fa5b5" stroke="#8fa5b5" stroke-width="3" stroke-linejoin="round">
{CLOUD}
  </g>
  <g fill="url(#sky)">
{CLOUD}
  </g>
{fish(grid, 15, 20, 32)}
</svg>
"""


# LosOS Git: the same fish, centred on the master branch the way the cloud
# mark centres it in the cloud. The badge is the LosOS teal, a step lighter
# than the light palette's --accent so it still holds on the dark palette's
# ground (an image cannot follow the page's mode). Master runs down the
# middle, commit to commit, with the fish on it; one feature branch forks off
# to the right so the shape still says "git" at favicon size.
def git_mark(grid):
    return f"""\
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <title>LosOS Git</title>
  <rect x="4" y="4" width="56" height="56" rx="12" fill="#167f8e"/>
  <g fill="none" stroke="#ffffff" stroke-width="3.5" stroke-linecap="round">
    <path d="M32 12v40"/>
    <path d="M32 45c0-8 17-8 17-17v-12"/>
  </g>
  <g fill="#167f8e" stroke="#ffffff" stroke-width="3">
    <circle cx="32" cy="12" r="3.5"/>
    <circle cx="32" cy="52" r="3.5"/>
    <circle cx="49" cy="15" r="3.5"/>
  </g>
{fish(grid, 18, 17, 28)}
</svg>
"""


# The admin pages' own logo and favicon: the cooked salmon alone, no badge.
# It is written into the SPA's source tree rather than referenced from here,
# because losos-admin-ui's source root is admin-ui/app and nothing outside
# it reaches the build.
def bare_mark(grid):
    n = len(grid)
    return f"""\
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {n} {n}" width="{n * 4}" height="{n * 4}" shape-rendering="crispEdges">
  <title>LosOS</title>
{fish_paths(grid)}
</svg>
"""


def main():
    grid = fish_grid()
    app_assets = os.path.join(HERE, "..", "..", "app", "src", "assets")
    for path, svg in (
        (os.path.join(HERE, "losos-cloud.svg"), cloud_mark(grid)),
        (os.path.join(HERE, "losos-git.svg"), git_mark(grid)),
        (os.path.join(app_assets, "losos.svg"), bare_mark(grid)),
    ):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w") as out:
            out.write(svg)
        print("wrote", os.path.relpath(path, HERE))


if __name__ == "__main__":
    main()
