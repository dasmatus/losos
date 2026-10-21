"""Pictures of LosOS Cloud on a real box, from a real browser.

Chromium (Playwright) signs in through LosOS Login as the owner and walks
the web app, one full-window PNG per state, with a manifest that says where
each picture was taken and where the things on it are. nix/tests/cloud.nix
runs it in its laptop VM against the box's own nginx; the same pictures
serve the pull request and the launch film. Run it again against any box:

    cloud-shots.py BASE NAME PASSWORD OUT [--before] [--commit SHA] [--test NAME]

BASE is the box (https://mattbox.local), OUT a folder for NN-slug.png and
manifest.json. The browser trusts the box's certificate by its public key
(/etc/losos-box.pem). --before takes the same addresses on a box without
LosOS Cloud, for the "before" column.

The window is 1280 x 900 at twice the pixels, dark, en-US, reduced motion,
Europe/Bratislava. Each picture waits for the page to settle: no request
in flight, fonts and pictures loaded, no spinner and no toast on screen.
"""

import base64
import hashlib
import json
import os
import subprocess
import sys
import time
import urllib.parse

from playwright.sync_api import sync_playwright

CA = "/etc/losos-box.pem"
VIEW = {"width": 1280, "height": 900}


def spki_pin(pem_file: str) -> str:
    """The certificate's public key hash, which Chromium pins to."""
    der = subprocess.run(
        ["openssl", "x509", "-in", pem_file, "-pubkey", "-noout"], check=True, capture_output=True
    ).stdout
    raw = subprocess.run(
        ["openssl", "pkey", "-pubin", "-outform", "der"], input=der, check=True, capture_output=True
    ).stdout
    return base64.b64encode(hashlib.sha256(raw).digest()).decode()


class Shots:
    def __init__(self, page, base, out, meta):
        self.page = page
        self.base = base
        self.out = out
        self.host = urllib.parse.urlparse(base).hostname
        self.steps = []
        self.meta = meta
        os.makedirs(out, exist_ok=True)

    def settle(self):
        p = self.page
        p.wait_for_load_state("networkidle")
        p.evaluate("document.fonts.ready.then(() => true)")
        # Every picture on screen has arrived, no spinner turns, no toast is
        # still sliding.
        p.wait_for_function(
            """() => {
              const seen = (el) => { const r = el.getBoundingClientRect();
                return r.bottom > 0 && r.top < innerHeight && r.width > 0; };
              const imgs = [...document.images].filter(seen);
              if (imgs.some((i) => !i.complete || i.naturalWidth === 0)) return false;
              if ([...document.querySelectorAll('[data-slot=spinner]')].some(seen)) return false;
              if (document.querySelector('[data-sonner-toast]')) return false;
              return true;
            }""",
            timeout=120_000,
        )
        # The caret would blink into one picture and not the next. A CSSOM
        # write, which the box's style-src 'self' allows.
        p.evaluate("document.documentElement.style.caretColor = 'transparent'")
        time.sleep(0.3)

    def box(self, locator):
        b = locator.bounding_box()
        return {k: round(b[k]) for k in ("x", "y", "width", "height")} if b else None

    def take(self, slug, title, action=None, targets=None):
        self.settle()
        n = len(self.steps) + 1
        name = f"{n:02d}-{slug}.png"
        self.page.screenshot(path=os.path.join(self.out, name))
        found = {}
        for key, loc in (targets or {}).items():
            b = self.box(loc)
            if b:
                found[key] = {"x": b["x"], "y": b["y"], "w": b["width"], "h": b["height"]}
        path = urllib.parse.urlparse(self.page.url)
        self.steps.append(
            {
                "file": name,
                "url": f"{self.host}{path.path}{'?' + path.query if path.query else ''}",
                "title": title,
                "action": action or {"kind": "none"},
                "targets": found,
            }
        )
        print(name, title, flush=True)

    def save(self):
        manifest = {
            "feature": "cloud",
            "viewport": VIEW,
            "dpr": 2,
            **self.meta,
            "steps": self.steps,
        }
        with open(os.path.join(self.out, "manifest.json"), "w") as f:
            json.dump(manifest, f, indent=2)


def sign_in(page, base, name, password):
    """LosOS Login, as its page does it; the cookie is the browser's."""
    r = page.request.post(
        f"{base}/auth/api/firstfactor",
        data={"username": name, "password": password, "keepMeLoggedIn": False, "targetURL": f"{base}/cloud/"},
    )
    assert r.ok, r.text()


def after(s: Shots, base: str):
    p = s.page
    p.goto(f"{base}/cloud/files")
    p.wait_for_selector("[data-files]")
    s.take(
        "files",
        "the owner's files",
        {"kind": "click", "target": "documents"},
        {
            "documents": p.get_by_role("link", name="Documents", exact=True),
            "upload": p.get_by_role("button", name="Upload", exact=True),
            "places": p.get_by_role("navigation", name="Places"),
        },
    )
    p.get_by_role("link", name="Documents", exact=True).click()
    p.wait_for_selector('[data-entry="budget.ods"]')
    s.take(
        "documents",
        "the Documents folder",
        {"kind": "click", "target": "budget"},
        {
            "budget": p.get_by_role("checkbox", name="Select budget.ods"),
            "breadcrumb": p.get_by_role("navigation", name="Folder path"),
        },
    )
    p.get_by_role("checkbox", name="Select budget.ods").check()
    p.wait_for_selector("[data-selection]")
    s.take(
        "selected",
        "one file picked, and what can be done with it",
        {"kind": "click", "target": "photos"},
        {
            "bar": p.locator("[data-selection]"),
            "versions": p.get_by_role("button", name="Versions"),
            "photos": p.get_by_role("link", name="Photos", exact=True),
        },
    )
    p.get_by_role("link", name="Photos", exact=True).click()
    p.wait_for_selector("[data-photos] img")
    first = p.locator("[data-photos] button").first
    s.take("photos", "every picture, by month", {"kind": "click", "target": "photo"}, {"photo": first})
    first.click()
    p.wait_for_selector("dialog[open] img")
    s.take(
        "photo",
        "one picture, large",
        {"kind": "click", "target": "next"},
        {"next": p.get_by_role("button", name="Next photo")},
    )
    p.keyboard.press("Escape")
    p.get_by_role("link", name="Recent", exact=True).click()
    p.wait_for_selector("[data-files]")
    s.take("recent", "what changed lately", {"kind": "click", "target": "search"}, {"search": p.get_by_role("searchbox")})
    field = p.get_by_role("searchbox")
    field.click()
    typed = ""
    for ch in "budget":
        typed += ch
        field.press(ch)
        s.take(f"search-{typed}", f"typing “{typed}”", {"kind": "type", "target": "search"}, {"search": field})
    field.press("Enter")
    p.wait_for_selector('[data-entry="budget.ods"]')
    s.take("search-results", "what the search found", {"kind": "click", "target": "devices"}, {"devices": p.get_by_role("link", name="Devices", exact=True)})
    p.get_by_role("link", name="Devices", exact=True).click()
    p.wait_for_selector("[data-devices]")
    s.take("devices", "the computers and phones that sync", {"kind": "none"}, {"list": p.locator("[data-devices]")})
    p.get_by_role("link", name="Trash", exact=True).click()
    p.wait_for_selector("[data-trash]")
    s.take("trash", "deleted files, ready to come back", {"kind": "none"}, {"list": p.locator("[data-trash]")})


def connect(s: Shots, base: str, login_url: str):
    """The page the desktop client opens to sign in, before and after Allow."""
    p = s.page
    p.goto(login_url)
    p.wait_for_selector("[data-connect]")
    allow = p.get_by_role("button", name="Allow")
    s.take("connect", "the desktop client asks to sync", {"kind": "click", "target": "allow"}, {"allow": allow})
    allow.click()
    p.get_by_text("Connected").wait_for()
    s.take("connected", "the desktop client got its password", {"kind": "none"}, {})


def before(s: Shots, base: str):
    p = s.page
    for path, slug, title in [
        ("/cloud/files", "files", "the files address, before LosOS Cloud"),
        ("/cloud/photos", "photos", "the photos address, before LosOS Cloud"),
        ("/cloud/devices", "devices", "the devices address, before LosOS Cloud"),
    ]:
        p.goto(f"{base}{path}")
        s.take(slug, title)


def main():
    args = sys.argv[1:]
    flags = {a for a in args if a.startswith("--")}
    pos = [a for a in args if not a.startswith("--")]
    opts = {}
    for key in ("--commit", "--test", "--connect"):
        if key in args:
            i = args.index(key)
            opts[key] = args[i + 1]
            pos.remove(args[i + 1])
    base, name, password, out = pos[:4]
    meta = {"commit": opts.get("--commit", ""), "test": opts.get("--test", "")}
    with sync_playwright() as pw:
        browser = pw.chromium.launch(
            args=[f"--ignore-certificate-errors-spki-list={spki_pin(CA)}"],
        )
        context = browser.new_context(
            viewport=VIEW,
            device_scale_factor=2,
            color_scheme="dark",
            locale="en-US",
            timezone_id="Europe/Bratislava",
            reduced_motion="reduce",
        )
        page = context.new_page()
        problems = []
        page.on("console", lambda m: problems.append(m.text) if m.type == "error" else None)
        page.on("pageerror", lambda e: problems.append(str(e)))
        sign_in(page, base, name, password)
        s = Shots(page, base, out, meta)
        if "--before" in flags:
            before(s, base)
        else:
            after(s, base)
            if "--connect" in opts:
                connect(s, base, opts["--connect"])
        s.save()
        browser.close()
        if problems and "--before" not in flags:
            sys.exit("the page reported errors:\n" + "\n".join(problems))


if __name__ == "__main__":
    main()
