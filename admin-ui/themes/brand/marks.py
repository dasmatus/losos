#!/usr/bin/env python3
"""Generate the LosOS cloud and LosOS Git marks from salmon.png.

salmon.png is the source: a live coho salmon, cut out of a NOAA Fisheries
photo on a transparent ground, 512 px square (CREDITS.md has the source).
This script embeds it in each mark as a data: URI, so every SVG is
self-contained and still draws where it is served on its own (Nextcloud's
logo.svg, Forgejo's home page). The PNGs and the .ico each app also wants
are rasterized from these SVGs at build time (admin-ui/themes/default.nix),
so only the SVGs and the cut-outs are committed.

plate.png, the earlier logo, is a plate of salmon. The admin pages and the
handbook show it on Halloween only, so this script copies it next to the
salmon for them. It never goes into a mark, because an app's logo cannot
change by date.

Standard library only. Run it after changing a cut-out or a shape below:

    python3 admin-ui/themes/brand/marks.py
"""

import base64
import os
import shutil

HERE = os.path.dirname(os.path.abspath(__file__))
SALMON = os.path.join(HERE, "salmon.png")


def salmon_uri():
    with open(SALMON, "rb") as f:
        return "data:image/png;base64," + base64.b64encode(f.read()).decode("ascii")


def salmon(uri, x, y, size):
    """The salmon, placed at (x, y) in a box size units square (the cut-out
    is square with the fish centred in it, so the box is the frame)."""
    return f'  <image x="{x}" y="{y}" width="{size}" height="{size}" href="{uri}"/>'


# The cloud is ☁️ drawn the way the emoji fonts draw it: a soft white cloud
# that fades to a cool grey underneath, with an outline so it still reads on a
# white page. Three puffs and a flat base, with the salmon centred in the body
# of the cloud. Drawn twice from the same shapes, stroked and then filled, so
# the outline traces the union and not each puff. The gradient is in the
# drawing's own units, so the puffs and the base share one sky rather than
# each shading top to bottom on its own.
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
    <linearGradient id="sky" gradientUnits="userSpaceOnUse" x1="0" y1="15" x2="0" y2="55">
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
{salmon(uri, 11, 15, 42)}
</svg>
"""


# LosOS Git: the same salmon, centred on the master branch the way the cloud
# mark centres it in the cloud. The badge is the LosOS teal, a step lighter
# than the light palette's --accent so it still holds on the dark palette's
# ground (an image cannot follow the page's mode). Master runs down the
# middle, commit to commit, with the salmon on it; one feature branch forks off
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
{salmon(uri, 11, 11, 42)}
</svg>
"""


def main():
    uri = salmon_uri()
    for path, svg in (
        (os.path.join(HERE, "losos-cloud.svg"), cloud_mark(uri)),
        (os.path.join(HERE, "losos-git.svg"), git_mark(uri)),
    ):
        with open(path, "w") as out:
            out.write(svg)
        print("wrote", os.path.relpath(path, HERE))
    # The admin pages' own logo and favicon, and the handbook's: the salmon
    # alone, and the plate for Halloween, each a 128 px copy of its cut-out
    # (shown at 32 CSS px, so sharp on a 4x display). Copied rather than
    # referenced, because losos-admin-ui's source root is admin-ui/app and
    # nothing outside it reaches the build, and the handbook is built from
    # handbook/ alone.
    app = os.path.join(HERE, "..", "..", "app", "src", "assets")
    site = os.path.join(HERE, "..", "..", "..", "handbook", "static", "img")
    copies = (
        ("salmon-128.png", os.path.join(app, "losos.png")),
        ("plate-128.png", os.path.join(app, "losos-halloween.png")),
        ("salmon-128.png", os.path.join(site, "losos.png")),
        ("plate-128.png", os.path.join(site, "losos-halloween.png")),
        # The public site's landing page shows the logo large, at 240 CSS
        # px, so it gets the 512 px cut-outs themselves.
        ("salmon.png", os.path.join(site, "salmon.png")),
        ("plate.png", os.path.join(site, "plate.png")),
    )
    for name, path in copies:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        shutil.copyfile(os.path.join(HERE, name), path)
        print("wrote", os.path.relpath(path, HERE))


if __name__ == "__main__":
    main()
