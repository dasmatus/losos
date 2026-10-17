# What the LosOS cloud apps do to get an app password, for
# nix/tests/cloud-webdav.nix: the desktop client's Login Flow v2 and the
# phone apps' Login Flow v1, with this script doing the browser's part. The
# browser is not signed in to anything; it meets LosOS Login on the way, as
# a person would, and signs in there the way the portal page does.
#
#   cloud-client flow2 https://mattbox.local NAME PASSWORD
#   cloud-client flow1 https://mattbox.local NAME PASSWORD
#
# The laptop trusts the box's certificate (CA below), as an owner's computer
# does after the first-run page. Prints one JSON object: what the app would store (server, login name, app
# password) and the address of every page the browser passed through.
import base64
import json
import re
import sys
import urllib.parse

import requests

# The box's own certificate, which the laptop has installed, as the
# handbook's first-run page tells an owner to.
CA = "/etc/losos-box.pem"

DESKTOP = "Mozilla/5.0 (Linux) mirall/3.16.0 (Nextcloud, nixos-test)"
PHONE = "Mozilla/5.0 (Android) Nextcloud-android/3.30.0"


class Browser:
    def __init__(self, base, name, password):
        self.base = base
        self.name = name
        self.password = password
        self.s = requests.Session()
        self.s.verify = CA
        # A browser asks for pages. Nextcloud answers a signed-out request
        # for a page with its sign-in page, anything else with a bare 401.
        self.s.headers["Accept"] = "text/html,application/xhtml+xml,*/*;q=0.8"
        self.seen = []

    def go(self, method, url, **kw):
        r = self.s.request(method, url, allow_redirects=True, **kw)
        self.seen += [h.url for h in r.history] + [r.url]
        r.raise_for_status()
        # LosOS Login's portal: sign in as its page does, then go where it
        # says.
        if urllib.parse.urlparse(r.url).path.startswith("/auth/"):
            rd = urllib.parse.parse_qs(urllib.parse.urlparse(r.url).query)["rd"][0]
            a = self.s.post(
                f"{self.base}/auth/api/firstfactor",
                json={
                    "username": self.name,
                    "password": self.password,
                    "keepMeLoggedIn": False,
                    "targetURL": rd,
                },
            )
            a.raise_for_status()
            where = (a.json().get("data") or {}).get("redirect") or rd
            return self.go("GET", where)
        return r


def state(page, key):
    m = re.search(rf'id="initial-state-core-{key}" value="([^"]+)"', page.text)
    if not m:
        sys.exit(f"no {key} on {page.url}: {page.text[:600]}")
    return json.loads(base64.b64decode(m.group(1)))


def requesttoken(page):
    m = re.search(r'data-requesttoken="([^"]+)"', page.text)
    if not m:
        sys.exit(f"no request token on {page.url}")
    return m.group(1)


def flow2(b):
    # The desktop client asks for a flow and opens `login` in the browser.
    init = requests.post(
        f"{b.base}/nextcloud/index.php/login/v2",
        headers={"User-Agent": DESKTOP},
        verify=CA,
    )
    init.raise_for_status()
    init = init.json()
    picker = b.go("GET", init["login"])
    auth = state(picker, "loginFlowAuth")
    grant = b.go("GET", auth["loginRedirectUrl"])
    form = state(grant, "loginFlowGrant")
    done = b.go(
        "POST",
        form["actionUrl"],
        data={"stateToken": form["stateToken"], "requesttoken": requesttoken(grant)},
    )
    assert state(done, "loginFlowState") == "done", done.url
    # Meanwhile the client polls.
    poll = requests.post(
        init["poll"]["endpoint"],
        data={"token": init["poll"]["token"]},
        verify=CA,
    )
    poll.raise_for_status()
    return {"init": init, "grantUser": form["userId"], **poll.json()}


def flow1(b):
    # The phone app opens /login/flow in its web view with this header.
    picker = b.go(
        "GET",
        f"{b.base}/nextcloud/index.php/login/flow",
        headers={"OCS-APIREQUEST": "true", "User-Agent": PHONE},
    )
    auth = state(picker, "loginFlowAuth")
    grant = b.go("GET", urllib.parse.urljoin(picker.url, auth["loginRedirectUrl"]))
    form = state(grant, "loginFlowGrant")
    r = b.s.post(
        form["actionUrl"],
        data={"stateToken": form["stateToken"], "requesttoken": requesttoken(grant)},
        allow_redirects=False,
    )
    where = r.headers.get("Location", "")
    if not where.startswith("nc://login/"):
        sys.exit(f"no nc:// redirect ({r.status_code}): {where} {r.text[:400]}")
    fields = dict(
        part.split(":", 1) for part in where[len("nc://login/"):].split("&")
    )
    return {
        "server": fields["server"],
        "loginName": urllib.parse.unquote(fields["user"]),
        "appPassword": urllib.parse.unquote(fields["password"]),
        "grantUser": form["userId"],
    }


def main():
    kind, base, name, password = sys.argv[1:5]
    b = Browser(base, name, password)
    out = {"flow2": flow2, "flow1": flow1}[kind](b)
    out["seen"] = b.seen
    print(json.dumps(out))


main()
