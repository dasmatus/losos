"""Photograph the Office editor in a real browser, one step per call.

Chromium runs on the client VM with the DevTools protocol on 127.0.0.1:9222,
its windows 1280 x 900 at device scale 2, in the dark colour scheme, with
reduced motion (the flags are in nix/tests/office.nix). Each call connects,
does one thing in one tab and leaves; the tabs stay open in between, so the
test script can check the box between two pictures.

    office-shots.py open   TAB --name NAME --cookie VALUE --url URL
    office-shots.py listed TAB                    until the document list is up
    office-shots.py loaded TAB                    until the editor has the document
    office-shots.py click  TAB SELECTOR           once it is there
    office-shots.py key    TAB KEY [KEY...]       ctrl+End, ArrowDown, Enter
    office-shots.py type   TAB TEXT --shot NN-slug --title T   one PNG a letter
    office-shots.py write  TAB TEXT               the same, no pictures
    office-shots.py saved  TAB                    ctrl+s, until the editor says so
    office-shots.py views  TAB N                  until N people are in it
    office-shots.py signin TAB                    the sign-in dialog's button
    office-shots.py close  TAB                    closes the tab
    office-shots.py where  TAB URL-PREFIX         until the tab is there, prints it
    office-shots.py shot   TAB NN-slug --title T [--action KIND[:TARGET]]
                           [--target NAME | --target NAME=SELECTOR]...

Each person gets a browser context of their own, so two tabs can be two
people. Who a tab is is LosOS Login's session cookie, which the test script
gets by signing that person in at /auth/ and hands to `open`.

Pictures go to $SHOTS as NN-slug.png, each with a step in $SHOTS/manifest.json
(see the film's capture spec): the address shown, a title, what happens next,
and the boxes the film may point at, in CSS pixels of the whole window.
"""

import argparse
import base64
import json
import os
import sys
import time
import urllib.request

import websocket

SHOTS = os.environ.get("SHOTS", "/tmp/shots")
STATE = "/tmp/office-shots-tabs.json"
HOST = os.environ.get("SHOTS_HOST", "mattbox.local")

# What a picture may point at, as (in the editor frame?, selector).
TARGETS = {
    # The editor's own bar, the one right above its <main>; the shell's
    # top bar is a <header> too.
    "back": (False, "header:has(+ main) button"),
    "name": (False, "header:has(+ main) h1"),
    "document": (False, "main iframe"),
    "save": (True, '[id^="save"].unotoolbutton'),
    "page": (True, "#document-container"),
}

# Hides the text caret (the film draws its own pointer) and stops the
# editor's cursor blinking, so two pictures of the same moment are the same.
STILL = """
* { caret-color: transparent !important; }
.blinking-cursor { animation: none !important; }
"""


def log(*args):
    print(*args, file=sys.stderr, flush=True)


class Tab:
    def __init__(self, target):
        info = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json/version"))
        self.ws = websocket.create_connection(
            info["webSocketDebuggerUrl"], timeout=900, suppress_origin=True
        )
        self.next = 0
        self.session = None
        if target:
            self.session = self.call(
                "Target.attachToTarget", {"targetId": target, "flatten": True}
            )["sessionId"]

    def call(self, method, params=None, session=True):
        self.next += 1
        msg = {"id": self.next, "method": method, "params": params or {}}
        if session and self.session:
            msg["sessionId"] = self.session
        self.ws.send(json.dumps(msg))
        while True:
            reply = json.loads(self.ws.recv())
            if reply.get("id") == self.next:
                if "error" in reply:
                    raise SystemExit(f"{method}: {reply['error']}")
                return reply.get("result", {})

    def js(self, expr):
        result = self.call(
            "Runtime.evaluate",
            {"expression": expr, "returnByValue": True, "awaitPromise": True},
        )
        if "exceptionDetails" in result:
            raise SystemExit(f"{expr[:80]}: {result['exceptionDetails']}")
        return result["result"].get("value")

    def until(self, expr, what, timeout=900):
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.js(expr):
                return
            time.sleep(1)
        raise SystemExit(f"gave up waiting for {what}")

    def key(self, combo):
        *mods, name = combo.split("+")
        bits = {"alt": 1, "ctrl": 2, "meta": 4, "shift": 8}
        modifiers = sum(bits[m.lower()] for m in mods)
        codes = {
            "Tab": 9, "Enter": 13, "End": 35, "Home": 36,
            "ArrowLeft": 37, "ArrowUp": 38, "ArrowRight": 39, "ArrowDown": 40, "s": 83,
        }
        event = {
            "modifiers": modifiers,
            "key": name,
            "code": name if len(name) > 1 else f"Key{name.upper()}",
            "windowsVirtualKeyCode": codes.get(name, ord(name.upper()[0])),
        }
        if name == "Enter":
            event["text"] = "\r"
        self.call("Input.dispatchKeyEvent", {"type": "keyDown", **event})
        self.call("Input.dispatchKeyEvent", {"type": "keyUp", **event})


def frame(expr):
    """`expr` evaluated in the editor frame, which is on the page's origin."""
    return (
        "(() => { const w = document.querySelector('main iframe')?.contentWindow;"
        f" if (!w || !w.document) return null; return (function() {{ return {expr}; }}).call(w); }})()"
    )


def tabs():
    try:
        with open(STATE) as f:
            return json.load(f)
    except FileNotFoundError:
        return {}


def attach(name):
    return Tab(tabs()[name]["target"])


def keep_still(tab):
    # A constructed sheet: the admin page's style-src 'self' refuses a
    # <style> element made at runtime, and allows this.
    css = json.dumps(STILL)
    tab.js(
        f"""(() => {{
          for (const d of [document, document.querySelector('main iframe')?.contentDocument]) {{
            if (!d || d.losos_still) continue;
            const s = new d.defaultView.CSSStyleSheet(); s.replaceSync({css});
            d.adoptedStyleSheets = [...d.adoptedStyleSheets, s];
            d.losos_still = true;
          }}
        }})()"""
    )


def box(tab, name):
    name, _, css = name.partition("=")
    in_frame, selector = (False, css) if css else TARGETS[name]
    sel = json.dumps(selector)
    return tab.js(
        f"""(() => {{
          let x = 0, y = 0, d = document;
          if ({json.dumps(in_frame)}) {{
            const f = document.querySelector('main iframe');
            if (!f) return null;
            const r = f.getBoundingClientRect(); x = r.x; y = r.y; d = f.contentDocument;
          }}
          const e = d.querySelector({sel});
          if (!e) return null;
          const r = e.getBoundingClientRect();
          if (r.width === 0 || r.height === 0) return null;
          return {{ x: Math.round(r.x + x), y: Math.round(r.y + y),
                    w: Math.round(r.width), h: Math.round(r.height) }};
        }})()"""
    )


def settle(tab, seconds=4):
    tab.until("document.fonts.ready.then(() => true)", "the fonts")
    tab.js(frame("this.document.fonts.ready.then(() => true)"))
    keep_still(tab)
    time.sleep(seconds)


def shoot(tab, name, file, title, action="none", targets=()):
    os.makedirs(SHOTS, exist_ok=True)
    size = tab.js("[innerWidth, innerHeight, devicePixelRatio]")
    assert size == [1280, 900, 2], f"the window is {size}, not 1280 x 900 at 2x"
    png = tab.call("Page.captureScreenshot", {"format": "png"})["data"]
    path = os.path.join(SHOTS, f"{file}.png")
    with open(path, "wb") as f:
        f.write(base64.b64decode(png))
    kind, _, target = action.partition(":")
    # --action click:trip goes with --target trip=SELECTOR; the step names it
    # by the short name.
    target = next((t for t in targets if t.partition("=")[0] == target), target)
    step = {
        "file": f"{file}.png",
        "url": HOST + tab.js("location.pathname + location.hash"),
        "title": title,
        "person": tabs()[name]["person"],
        "action": {"kind": kind, **({"target": target.partition("=")[0]} if target else {})},
        "targets": {},
    }
    for t in {*targets, *([target] if target else [])}:
        found = box(tab, t)
        if found:
            step["targets"][t.partition("=")[0]] = found
    manifest_path = os.path.join(SHOTS, "manifest.json")
    try:
        with open(manifest_path) as f:
            manifest = json.load(f)
    except FileNotFoundError:
        manifest = {
            "feature": "office",
            "viewport": {"width": 1280, "height": 900},
            "dpr": 2,
            "commit": os.environ.get("SHOTS_COMMIT", ""),
            "test": "losos-office",
            "steps": [],
        }
    manifest["steps"] = [s for s in manifest["steps"] if s["file"] != step["file"]]
    manifest["steps"].append(step)
    manifest["steps"].sort(key=lambda s: s["file"])
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=2)
    log(f"shot {path}")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("command", choices=["open", "listed", "loaded", "click", "key", "type", "write", "saved", "views", "signin", "close", "where", "shot"])
    p.add_argument("tab")
    p.add_argument("args", nargs="*")
    p.add_argument("--name")
    p.add_argument("--cookie")
    p.add_argument("--url")
    p.add_argument("--shot")
    p.add_argument("--title", default="")
    p.add_argument("--action", default="none")
    p.add_argument("--target", action="append", default=[])
    a = p.parse_args()

    if a.command == "open":
        browser = Tab(None)
        context = browser.call(
            "Target.createBrowserContext", {"disposeOnDetach": False}
        )["browserContextId"]
        cookies = [
            {
                "name": "losos_session",
                "value": a.cookie,
                "domain": HOST,
                "path": "/",
                "secure": True,
                "httpOnly": True,
                "sameSite": "Lax",
            }
        ]
        browser.call(
            "Storage.setCookies", {"cookies": cookies, "browserContextId": context}
        )
        target = browser.call(
            "Target.createTarget",
            {
                "url": "about:blank",
                "browserContextId": context,
                "newWindow": True,
                "width": 1280,
                "height": 900,
            },
        )["targetId"]
        # The window's height counts a strip the page does not get; grow the
        # window by it, so the page itself is 1280 x 900.
        tab = Tab(target)
        window = browser.call("Browser.getWindowForTarget", {"targetId": target})
        inner = tab.js("innerHeight")
        browser.call(
            "Browser.setWindowBounds",
            {
                "windowId": window["windowId"],
                "bounds": {"width": 1280, "height": 900 + window["bounds"]["height"] - inner},
            },
        )
        tab.until("innerWidth === 1280 && innerHeight === 900", "the window size", 30)
        tab.call("Page.enable")
        tab.call("Page.navigate", {"url": a.url})
        state = tabs()
        state[a.tab] = {"target": target, "person": a.name}
        with open(STATE, "w") as f:
            json.dump(state, f)
        return

    if a.command == "close":
        state = tabs()
        Tab(None).call("Target.closeTarget", {"targetId": state.pop(a.tab)["target"]})
        with open(STATE, "w") as f:
            json.dump(state, f)
        return

    tab = attach(a.tab)
    tab.call("Page.enable")

    if a.command == "listed":
        tab.until("document.querySelector('main [aria-busy=\"false\"] table') !== null", "the list")
        settle(tab)
    elif a.command == "click":
        (selector,) = a.args
        tab.until(f"document.querySelector({json.dumps(selector)}) !== null", selector, 300)
        found = tab.js(
            f"(() => {{ const r = document.querySelector({json.dumps(selector)})?.getBoundingClientRect();"
            " return r ? [r.x + r.width / 2, r.y + r.height / 2] : null; })()"
        )
        if not found:
            raise SystemExit(f"nothing to click at {selector}")
        x, y = found
        for kind in ["mousePressed", "mouseReleased"]:
            tab.call(
                "Input.dispatchMouseEvent",
                {"type": kind, "x": x, "y": y, "button": "left", "clickCount": 1},
            )
    elif a.command == "loaded":
        # The page drops aria-busy when the editor says Document_Loaded.
        tab.until("document.querySelector('main[aria-busy=\"false\"] iframe') !== null", "the document")
        tab.until(frame("this.document.querySelector('#document-container') !== null"), "the editor")
        settle(tab, 10)
    elif a.command == "key":
        for combo in a.args:
            tab.key(combo)
            time.sleep(1)
        settle(tab)
    elif a.command == "type":
        (text,) = a.args
        for i, ch in enumerate(text):
            tab.call("Input.insertText", {"text": ch})
            settle(tab, 2)
            shoot(
                tab,
                a.tab,
                f"{a.shot}-{i + 1:02d}",
                a.title,
                "type" if i + 1 < len(text) else a.action,
                ["page"],
            )
    elif a.command == "write":
        (text,) = a.args
        for ch in text:
            tab.call("Input.insertText", {"text": ch})
            time.sleep(0.3)
        settle(tab)
    elif a.command == "saved":
        # Saved when the editor says the document is unmodified again (an
        # idle save may already have done it).
        tab.key("ctrl+s")
        tab.until(
            frame(
                "this.app && this.app.map && this.app.map.stateChangeHandler"
                " && String(this.app.map.stateChangeHandler.getItemValue('.uno:ModifiedStatus')) === 'false'"
            ),
            "the save",
        )
        # and, for the picture, its status bar no longer says "Saving...".
        # In a shared document that line can stay up long after the file is
        # written; the test checks the file on the box itself.
        try:
            tab.until(frame("!/Saving/.test(this.document.body.innerText)"), "the status bar", 120)
        except SystemExit as e:
            log(f"{e}; the save itself is done")
        settle(tab)
    elif a.command == "views":
        (count,) = a.args
        # The people in the document, as the editor's own list counts them.
        tab.until(
            frame(f"this.app && this.app.map && Object.keys(this.app.map._viewInfo || {{}}).length >= {int(count)}"),
            f"{count} people in the document",
        )
        settle(tab, 6)
    elif a.command == "signin":
        button = "dialog[open] button[type=submit]"
        tab.until(f"document.querySelector({json.dumps(button)})?.disabled === false", "the sign-in dialog")
        tab.js(f"document.querySelector({json.dumps(button)}).click()")
    elif a.command == "where":
        (prefix,) = a.args
        tab.until(f"location.href.startsWith({json.dumps(prefix)})", prefix, 300)
        print(tab.js("location.href"))
    elif a.command == "shot":
        settle(tab)
        shoot(tab, a.tab, a.args[0], a.title, a.action, a.target)


if __name__ == "__main__":
    main()
