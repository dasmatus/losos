#!/usr/bin/env python3
"""Generate the LosOS cloud and LosOS Git marks from plate.png.

plate.png is the source: the plate of salmon, cut out of the photo on a
transparent ground, 512 px square. This script embeds it in each mark as a
data: URI, so every SVG is self-contained and still draws where it is served
on its own (Nextcloud's logo.svg, Forgejo's home page). The PNGs and the
.ico each app also wants are rasterized from these SVGs at build time
(admin-ui/themes/default.nix), so only the SVGs and plate.png are committed.

Standard library only. Run it after changing plate.png or a shape below:

    python3 admin-ui/themes/brand/marks.py
"""

import base64
import os
import shutil

HERE = os.path.dirname(os.path.abspath(__file__))
PLATE = os.path.join(HERE, "plate.png")


def plate_uri():
    with open(PLATE, "rb") as f:
        return "data:image/png;base64," + base64.b64encode(f.read()).decode("ascii")


def plate(uri, x, y, size):
    """The plate, placed at (x, y) in a box size units square (the photo is
    square with the plate centred in it, so the box is the frame)."""
    return f'  <image x="{x}" y="{y}" width="{size}" height="{size}" href="{uri}"/>'


# The cloud is ☁️ drawn the way the emoji fonts draw it: a soft white cloud
# that fades to a cool grey underneath, with an outline so it still reads on a
# white page. Three puffs and a flat base, with the plate centred in the body
# of the cloud. Drawn twice from the same shapes, stroked and then filled, so
# the outline traces the union and not each puff.
CLOUD = """\
    <circle cx="32" cy="33" r="18"/>
    <circle cx="14" cy="44" r="11"/>
    <circle cx="50" cy="43" r="12"/>
    <rect x="14" y="40" width="36" height="15"/>"""


def cloud_mark(uri):
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
{plate(uri, 13, 19, 38)}
</svg>
"""


# LosOS Git: the same plate, centred on the master branch the way the cloud
# mark centres it in the cloud. The badge is the LosOS teal, a step lighter
# than the light palette's --accent so it still holds on the dark palette's
# ground (an image cannot follow the page's mode). Master runs down the
# middle, commit to commit, with the plate on it; one feature branch forks off
# to the right so the shape still says "git" at favicon size.
def git_mark(uri):
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
{plate(uri, 14, 14, 36)}
</svg>
"""


def main():
    uri = plate_uri()
    for path, svg in (
        (os.path.join(HERE, "losos-cloud.svg"), cloud_mark(uri)),
        (os.path.join(HERE, "losos-git.svg"), git_mark(uri)),
    ):
        with open(path, "w") as out:
            out.write(svg)
        print("wrote", os.path.relpath(path, HERE))
    # The admin pages' own logo and favicon, and the handbook's: the plate
    # alone, a 128 px copy of the same cut-out (shown at 32 CSS px, so sharp
    # on a 4x display). Copied rather than referenced, because losos-admin-ui's
    # source root is admin-ui/app and nothing outside it reaches the build,
    # and the handbook is built from handbook/ alone.
    small = os.path.join(HERE, "plate-128.png")
    for path in (
        os.path.join(HERE, "..", "..", "app", "src", "assets", "losos.png"),
        os.path.join(HERE, "..", "..", "..", "handbook", "static", "img", "losos.png"),
    ):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        shutil.copyfile(small, path)
        print("wrote", os.path.relpath(path, HERE))
    # The public site's landing page shows the plate large, at 240 CSS px,
    # so it gets the 512 px cut-out itself.
    hero = os.path.join(HERE, "..", "..", "..", "handbook", "static", "img", "plate.png")
    shutil.copyfile(os.path.join(HERE, "plate.png"), hero)
    print("wrote", os.path.relpath(hero, HERE))


if __name__ == "__main__":
    main()
