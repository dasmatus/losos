"""Drive Collabora Online the way its editor page does, without a browser.

The editor page opens one WebSocket per person to
/cool/<document address>/ws, sends `coolclient` and `load`, and then sends
what the person does (`textinput`, `save`, `saveas`) as text messages. This
script does the same through nginx, so the VM test exercises the real path:
nginx, coolwsd, a LibreOffice kit, and the WOPI host behind them.

    office-client.py edit   --session S.json --text T [--save] [--saveas NAME]
    office-client.py coedit --session A.json --session B.json --text T --text U

A session file is the JSON `POST /office/api/open` answers.
"""

import argparse
import json
import os
import sys
import time
import urllib.parse

import websocket

# nginx on the box; a bare coolwsd for a run outside the VM.
BASE = os.environ.get("OFFICE_WS", "ws://localhost")


def log(*args):
    print(*args, file=sys.stderr, flush=True)


class Editor:
    def __init__(self, session, name):
        self.name = name
        query = urllib.parse.parse_qs(urllib.parse.urlsplit(session["url"]).query)
        self.wopi_src = query["WOPISrc"][0]
        params = urllib.parse.urlencode(
            {
                "access_token": session["accessToken"],
                "access_token_ttl": session["accessTokenTtl"],
            }
        )
        self.doc = f"{self.wopi_src}?{params}"
        quoted = urllib.parse.quote(self.doc, safe="")
        src = urllib.parse.quote(self.wopi_src, safe="")
        uri = f"{BASE}/cool/{quoted}/ws?WOPISrc={src}&compat=/ws"
        self.ws = websocket.create_connection(
            uri, timeout=600, header=["Origin: https://box.local"]
        )
        self.seen = []

    def send(self, text):
        log(f"[{self.name}] > {text[:120]}")
        self.ws.send(text)

    def recv(self, timeout):
        self.ws.settimeout(timeout)
        try:
            frame = self.ws.recv()
        except websocket.WebSocketTimeoutException:
            return None
        if isinstance(frame, bytes):
            # Tiles and other binary frames start with a text line.
            frame = frame.split(b"\n", 1)[0].decode("utf-8", "replace")
        self.seen.append(frame)
        return frame

    def wait_for(self, predicate, what, timeout=900):
        deadline = time.time() + timeout
        while time.time() < deadline:
            msg = self.recv(min(30, max(1, deadline - time.time())))
            if msg is None:
                continue
            if msg.startswith("error:"):
                log(f"[{self.name}] < {msg}")
            if predicate(msg):
                log(f"[{self.name}] < {msg[:200]}")
                return msg
        kinds = sorted({m.split(":", 1)[0] for m in self.seen})
        raise SystemExit(f"[{self.name}] gave up waiting for {what}; saw {kinds}; last: {self.seen[-5:]}")

    def load(self):
        now = int(time.time() * 1000)
        self.send(f"coolclient 0.1 {now} 0")
        self.send(
            "load url="
            + urllib.parse.quote(self.doc, safe="")
            + " lang=en-US accessibilityState=false deviceFormFactor=desktop"
        )
        status = self.wait_for(lambda m: m.startswith("status:"), "the document to load")
        self.status = status[len("status:") :]
        self.send("clientzoom tilepixelwidth=256 tilepixelheight=256 tiletwipwidth=3840 tiletwipheight=3840")
        self.send("clientvisiblearea x=0 y=0 width=20000 height=20000")
        return status

    def type(self, text):
        self.send("textinput id=0 text=" + urllib.parse.quote(text, safe=""))
        # Enter, so the next person's text starts a paragraph of its own.
        self.send("key type=input char=13 key=1280")
        self.send("key type=up char=0 key=1280")

    def save(self):
        self.send("save dontTerminateEdit=1 dontSaveIfUnmodified=0")
        return self.wait_for(
            lambda m: m.startswith("unocommandresult:") and ".uno:Save" in m,
            "the save to finish",
        )

    def saveas(self, name):
        self.send(f"saveas url=wopi:{urllib.parse.quote(name, safe='')} format=pdf options=")
        return self.wait_for(lambda m: m.startswith("saveas:"), "save-as to finish")

    def close(self):
        try:
            self.send("closedocument")
        finally:
            self.ws.close()


def moved(msg, view):
    """Whether `msg` says the cursor of view `view` moved."""
    if not msg.startswith("invalidateviewcursor:"):
        return False
    try:
        return int(json.loads(msg.split(":", 1)[1])["viewId"]) == view
    except (ValueError, KeyError):
        return False


def views(msg):
    return json.loads(msg[len("viewinfo: ") :])


def main():
    p = argparse.ArgumentParser()
    p.add_argument("mode", choices=["edit", "coedit"])
    p.add_argument("--session", action="append", required=True)
    p.add_argument("--text", action="append", default=[])
    p.add_argument("--save", action="store_true")
    p.add_argument("--saveas")
    a = p.parse_args()
    sessions = [json.load(open(path)) for path in a.session]

    if a.mode == "edit":
        e = Editor(sessions[0], "one")
        status = e.load()
        result = {"status": status}
        for t in a.text:
            e.type(t)
        if a.save:
            result["save"] = e.save()
        if a.saveas:
            result["saveas"] = e.saveas(a.saveas)
        e.close()
        print(json.dumps(result))
        return

    # Two people in one document: both see each other, both type, one save
    # carries both.
    first = Editor(sessions[0], "first")
    first.load()
    second = Editor(sessions[1], "second")
    second.load()
    together = first.wait_for(
        lambda m: m.startswith("viewinfo:") and len(views(m)) == 2,
        "the second person to join",
    )
    people = sorted(v.get("username", "") for v in views(together))
    first_view = json.loads(first.status)["viewid"]
    second_view = json.loads(second.status)["viewid"]
    first.type(a.text[0])
    # Each sees the other's cursor move as the other types.
    second.wait_for(lambda m: moved(m, first_view), "the first person's edit")
    second.type(a.text[1])
    first.wait_for(lambda m: moved(m, second_view), "the second person's edit")
    saved = first.save()
    first.close()
    second.close()
    print(json.dumps({"people": people, "save": saved}))


if __name__ == "__main__":
    main()
