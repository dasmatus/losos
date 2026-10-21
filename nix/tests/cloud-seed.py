"""Fill a LosOS Cloud account with a believable set of files.

Folders, documents and pictures, uploaded over WebDAV the way the desktop
client sends them, with modification times spread over the past months so
Recent and Photos have something to sort. The pictures are drawn here
(skies, hills, water), so nothing in a screenshot is a stock photo.

    cloud-seed.py BASE LOGIN PASSWORD [CAFILE]

BASE is the server's origin (https://mattbox.local), PASSWORD an app
password or the person's LosOS Login password, CAFILE the box's own
certificate when the system does not trust it yet.
"""

import base64
import io
import random
import ssl
import sys
import time
import urllib.parse
import urllib.request

from PIL import Image, ImageDraw, ImageFilter

DAY = 86_400


def picture(seed: int, width: int = 4032, height: int = 3024) -> bytes:
    """A landscape: a sky gradient, a sun, layered hills, sometimes a lake."""
    rnd = random.Random(seed)
    skies = [
        ((255, 183, 120), (90, 120, 200)),
        ((250, 220, 170), (120, 170, 230)),
        ((255, 140, 110), (60, 60, 140)),
        ((200, 225, 245), (70, 140, 210)),
        ((240, 200, 210), (110, 110, 180)),
    ]
    low, high = skies[seed % len(skies)]
    img = Image.new("RGB", (width, height))
    draw = ImageDraw.Draw(img)
    for y in range(height):
        f = y / height
        draw.line(
            [(0, y), (width, y)],
            fill=tuple(int(high[i] + (low[i] - high[i]) * f) for i in range(3)),
        )
    sx, sy, sr = rnd.randint(200, width - 200), rnd.randint(150, height // 2), rnd.randint(50, 110)
    draw.ellipse([sx - sr, sy - sr, sx + sr, sy + sr], fill=(255, 240, 200))
    base = rnd.choice([(40, 90, 60), (70, 80, 110), (90, 70, 60), (50, 70, 90)])
    layers = rnd.randint(3, 5)
    for layer in range(layers):
        shade = tuple(min(255, int(c * (0.6 + 0.5 * (layers - layer) / layers))) for c in base)
        top = int(height * (0.45 + 0.1 * layer))
        points = [(0, height)]
        x = 0
        y = top + rnd.randint(-60, 60)
        while x <= width:
            points.append((x, y))
            x += rnd.randint(60, 160)
            y = max(int(height * 0.3), min(height, y + rnd.randint(-70, 70)))
        points.append((width, height))
        draw.polygon(points, fill=shade)
    if rnd.random() < 0.5:
        lake = int(height * 0.8)
        for y in range(lake, height):
            f = (y - lake) / (height - lake)
            draw.line([(0, y), (width, y)], fill=(int(60 + 40 * f), int(110 + 30 * f), int(160 + 20 * f)))
    img = img.filter(ImageFilter.GaussianBlur(1.2))
    # A camera's grain, so the file weighs what a phone's picture does.
    grain = Image.effect_noise((width, height), 24).convert("RGB")
    img = Image.blend(img, grain, 0.1)
    out = io.BytesIO()
    img.save(out, "JPEG", quality=92)
    return out.getvalue()


def document(title: str, lines: list[str], size: int = 0) -> bytes:
    """A text document, filled out to about `size` bytes with numbered
    lines, so its size reads like the real kind of file."""
    body = "\n".join([title, "=" * len(title), "", *lines, ""])
    n = 1
    while len(body) < size:
        body += f"{n:05d}  {lines[n % len(lines)]}\n"
        n += 1
    return body.encode()


def tree(now: float) -> list[tuple[str, bytes | None, float]]:
    """(path, content or None for a folder, mtime)."""
    files: list[tuple[str, bytes | None, float]] = []
    for folder in ["Documents", "Documents/Receipts", "Holiday 2026", "Photos", "Projects", "Projects/Garden shed", "Music"]:
        files.append((folder, None, now))
    files += [
        ("Documents/budget.ods", document("Budget 2026", ["Rent 650", "Food 320", "Travel 120"], 21_400), now - 2 * 3600),
        ("Documents/Offer for the kitchen.pdf", document("Kitchen offer", ["Cabinets, worktop, fitting: 4 180 EUR"], 212_000), now - 1 * DAY),
        ("Documents/notes.md", document("Notes", ["- Call the plumber on Monday", "- Renew the car insurance"], 1_900), now - 3 * DAY),
        ("Documents/old notes.txt", document("Old notes", ["- Pick up the bike from the repair shop"], 3_100), now - 90 * DAY),
        ("Documents/budget draft.ods", document("Budget 2026 (draft)", ["Rent 650", "Food 300"], 18_900), now - 30 * DAY),
        ("Documents/CV Matus.docx", document("Curriculum vitae", ["Matus", "Systems engineering"], 41_800), now - 40 * DAY),
        ("Documents/Receipts/hardware store.pdf", document("Receipt", ["Screws, hinges, varnish: 38.90 EUR"], 64_500), now - 12 * DAY),
        ("Documents/Receipts/laptop.pdf", document("Receipt", ["Laptop repair: 89 EUR"], 88_200), now - 70 * DAY),
        ("Projects/Garden shed/plan.txt", document("Garden shed", ["2.4 x 1.8 m, larch cladding", "Roof: EPDM"], 4_600), now - 5 * DAY),
        ("Projects/Garden shed/materials.csv", b"item,count\nposts,6\nboards,48\nscrews,400\n", now - 5 * DAY),
        ("Projects/Fix the backup script.md", document("Fix the backup script", ["Keep 14 days, not 7."], 2_300), now - 6 * 3600),
        ("Music/playlist.m3u", b"#EXTM3U\n", now - 90 * DAY),
    ]
    for i in range(6):
        files.append((f"Holiday 2026/day {i + 1}.jpg", picture(i), now - (60 - i) * DAY))
    for i in range(8):
        files.append((f"Photos/IMG_{2041 + i}.jpg", picture(10 + i), now - (8 + 15 * i) * DAY))
    # A second copy of one picture, the kind of file a person deletes.
    files.append(("Photos/IMG_2043 (2).jpg", picture(12), now - 38 * DAY))
    return files


def main() -> None:
    base, login, password = sys.argv[1:4]
    context = ssl.create_default_context(cafile=sys.argv[4]) if len(sys.argv) > 4 else None
    auth = "Basic " + base64.b64encode(f"{login}:{password}".encode()).decode()
    root = f"{base.rstrip('/')}/remote.php/dav/files/{urllib.parse.quote(login)}/"

    def send(method: str, path: str, body: bytes | None = None, headers: dict[str, str] | None = None) -> int:
        url = root + urllib.parse.quote(path)
        req = urllib.request.Request(url, data=body, method=method, headers={"Authorization": auth, **(headers or {})})
        try:
            with urllib.request.urlopen(req, context=context) as r:
                return r.status
        except urllib.error.HTTPError as e:
            if method == "MKCOL" and e.code == 405:
                return 405
            raise

    now = time.time()
    for path, content, mtime in tree(now):
        if content is None:
            send("MKCOL", path)
        else:
            send("PUT", path, content, {"X-OC-Mtime": str(int(mtime)), "Content-Type": "application/octet-stream"})
        print(path, flush=True)


if __name__ == "__main__":
    main()
