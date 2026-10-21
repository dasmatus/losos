# What the Nextcloud apps do to get a password of their own from LosOS
# Cloud, for nix/tests/cloud.nix: the desktop client's Login Flow v2 and the
# phone apps' Login Flow v1, with this script doing the browser's part. The
# browser is signed in to nothing; it meets LosOS Login on the way, as a
# person would, signs in there the way its page does, and presses Allow on
# LosOS Cloud's connect page the way that page does.
#
#   cloud-flows flow2 https://mattbox.local NAME PASSWORD
#   cloud-flows flow1 https://mattbox.local NAME PASSWORD
#
# Prints one JSON object: what the app would store (server, login name,
# app password) and the address of every page the browser passed through.
import json
import sys
import urllib.parse

import requests

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
        self.s.headers["Accept"] = "text/html,application/xhtml+xml,*/*;q=0.8"
        self.seen = []

    def go(self, method, url, **kw):
        r = self.s.request(method, url, allow_redirects=True, **kw)
        self.seen += [h.url for h in r.history] + [r.url]
        r.raise_for_status()
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

    def api(self, method, path, **kw):
        # What the page's script sends: same origin, JSON.
        r = self.s.request(
            method,
            f"{self.base}/cloud/api/{path}",
            headers={"Origin": self.base, "Accept": "application/json"},
            **kw,
        )
        self.seen.append(r.url)
        r.raise_for_status()
        return r.json()


def flow2(b):
    init = requests.post(
        f"{b.base}/index.php/login/v2", headers={"User-Agent": DESKTOP}, verify=CA
    )
    init.raise_for_status()
    init = init.json()
    page = b.go("GET", init["login"])
    path = urllib.parse.urlparse(page.url).path
    assert path.startswith("/cloud/connect/"), page.url
    assert '<div id="cloud">' in page.text, page.text[:400]
    token = path.rsplit("/", 1)[1]
    asked = b.api("GET", f"connect/{token}")
    granted = b.api("POST", f"connect/{token}")
    poll = requests.post(
        init["poll"]["endpoint"], data={"token": init["poll"]["token"]}, verify=CA
    )
    poll.raise_for_status()
    again = requests.post(
        init["poll"]["endpoint"], data={"token": init["poll"]["token"]}, verify=CA
    )
    return {
        "init": init,
        "asked": asked,
        "granted": granted,
        "secondPoll": again.status_code,
        **poll.json(),
    }


def flow1(b):
    page = b.go(
        "GET",
        f"{b.base}/index.php/login/flow",
        headers={"OCS-APIREQUEST": "true", "User-Agent": PHONE},
    )
    assert urllib.parse.urlparse(page.url).path == "/cloud/connect/v1", page.url
    got = b.api("POST", "connect-v1", json={"client": "Nextcloud-android/3.30.0"})
    return {"server": got["server"], "loginName": got["login"], "appPassword": got["appPassword"]}


def main():
    kind, base, name, password = sys.argv[1:5]
    b = Browser(base, name, password)
    out = {"flow2": flow2, "flow1": flow1}[kind](b)
    out["seen"] = b.seen
    print(json.dumps(out))


main()
