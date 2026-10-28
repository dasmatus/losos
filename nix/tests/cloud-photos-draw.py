# Landscapes for nix/tests/cloud-photos.nix, drawn here rather than
# downloaded, each with the EXIF block a phone writes: the camera, the lens,
# the exposure, when and where it was taken. The places and dates are
# trips; the pictures are shapes.
#
#   python3 cloud-photos-draw.py OUT
import math
import random
import sys
from fractions import Fraction
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter
from PIL.TiffImagePlugin import IFDRational

TRIPS = [
    # name, taken, lat, lon, altitude, palette
    ("old-town.jpg", "2025:05:17 18:42:10", 48.14366, 17.10795, 140, "dusk"),
    ("danube.jpg", "2025:05:18 09:15:44", 48.13950, 17.11690, 136, "day"),
    ("vienna.jpg", "2025:08:02 14:03:27", 48.20849, 16.37208, 171, "day"),
    ("tatras.jpg", "2026:02:14 11:27:03", 49.17940, 20.08810, 1980, "snow"),
]

PALETTES = {
    "day": ((96, 156, 220), (198, 226, 246), (74, 120, 82), (48, 86, 60)),
    "dusk": ((48, 52, 112), (240, 150, 96), (60, 54, 72), (34, 30, 44)),
    "snow": ((70, 130, 200), (210, 230, 250), (236, 240, 246), (150, 164, 186)),
}


def rational(x, limit=10000):
    f = Fraction(x).limit_denominator(limit)
    return IFDRational(f.numerator, f.denominator)


def dms(value):
    value = abs(value)
    d = int(value)
    m = int((value - d) * 60)
    s = (value - d - m / 60) * 3600
    return (rational(d), rational(m), rational(round(s, 2), 100))


def draw(palette, seed, w=960, h=640):
    top, bottom, near, far = PALETTES[palette]
    rnd = random.Random(seed)
    img = Image.new("RGB", (w, h))
    d = ImageDraw.Draw(img)
    for y in range(h):
        t = y / h
        d.line([(0, y), (w, y)], fill=tuple(int(a + (b - a) * t) for a, b in zip(top, bottom)))
    sun = (int(w * rnd.uniform(0.2, 0.8)), int(h * rnd.uniform(0.15, 0.35)))
    d.ellipse([sun[0] - 60, sun[1] - 60, sun[0] + 60, sun[1] + 60], fill=(255, 236, 190))
    for layer, colour in ((0.55, far), (0.72, near)):
        points = [(0, h)]
        x = 0
        while x <= w:
            points.append((x, int(h * layer + math.sin(x / rnd.uniform(90, 200)) * 40 + rnd.uniform(-30, 30))))
            x += 40
        points.append((w, h))
        d.polygon(points, fill=colour)
    return img.filter(ImageFilter.GaussianBlur(1.2))


def exif_for(taken, lat, lon, alt):
    img = Image.new("RGB", (1, 1))
    exif = img.getexif()
    exif[0x010F] = "Google"
    exif[0x0110] = "Pixel 8"
    exif[0x0131] = "HDR+ 1.0"
    exif[0x0132] = taken
    sub = exif.get_ifd(0x8769)
    sub[0x9003] = taken
    sub[0x9004] = taken
    sub[0x829A] = rational(1 / 640)
    sub[0x829D] = rational(1.7)
    sub[0x8827] = 50
    sub[0x920A] = rational(6.9)
    sub[0xA434] = "Pixel 8 back camera 6.9mm f/1.68"
    gps = exif.get_ifd(0x8825)
    gps[1] = "N" if lat >= 0 else "S"
    gps[2] = dms(lat)
    gps[3] = "E" if lon >= 0 else "W"
    gps[4] = dms(lon)
    gps[5] = b"\x00"
    gps[6] = rational(alt)
    return exif


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for i, (name, taken, lat, lon, alt, palette) in enumerate(TRIPS):
        draw(palette, i).save(out / name, quality=88, exif=exif_for(taken, lat, lon, alt))


main()
