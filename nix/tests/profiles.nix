# nixos-test-vms config for profiles (backend/src/profiles.rs).
#
# Each person on LosOS Login has a picture, a banner, a colour, pronouns and
# a line about themselves, kept once by lososd and read by every page that
# shows a person. A laptop on the LAN does what a browser does, through
# nginx and LosOS Login with a session cookie, and checks what only a booted
# box can show:
#
#   * a phone photo, with its position in the Exif block and stored on its
#     side, arrives as a JPEG of 512 by 512, upright, with no Exif left;
#   * a banner with transparency stays a PNG of 1500 by 500;
#   * everyone signed in reads everyone's profile, nobody signed out does,
#     and the address a person gave LosOS Login is never in it;
#   * only the person changes their own; an administrator, and only one,
#     takes a picture down;
#   * a refusal says why, and leaves the old picture;
#   * profiles outlive a restart of lososd, sit in its 0700 state directory,
#     and go when the person does.
#
# The box is nix/tests/profile-box.nix: Authelia, lososd, nginx's front
# vhost in its container shape, LosOS cloud as a stub.
{ pkgs }:

let
  # Makes the pictures the laptop sends and reads back the ones it gets.
  python = pkgs.python3.withPackages (ps: [ ps.pillow ]);
  pictures = pkgs.writeText "pictures.py" ''
    import io, json, sys
    from PIL import Image

    def phone_photo(path):
        # Left half red, right half blue, 1200 x 800, stored on its side:
        # orientation 6 says "turn a quarter clockwise to view". The Exif
        # block carries a GPS position, the thing that must not survive.
        img = Image.new("RGB", (1200, 800), (30, 30, 200))
        img.paste((200, 30, 30), (0, 0, 600, 800))
        exif = Image.Exif()
        exif[0x0112] = 6
        exif[0x010F] = "LosOS test camera"
        gps = exif.get_ifd(0x8825)
        gps[1] = "N"
        gps[2] = (48.0, 8.0, 55.0)
        gps[3] = "E"
        gps[4] = (17.0, 6.0, 28.0)
        img.save(path, "JPEG", exif=exif.tobytes(), quality=90)

    def banner(path):
        img = Image.new("RGBA", (3000, 600), (0, 120, 130, 255))
        # In the middle: the box crops 3000 x 600 to the middle 1800 x 600.
        img.paste((0, 0, 0, 0), (1400, 200, 1600, 400))
        img.save(path, "PNG")

    def look(path):
        img = Image.open(path)
        info = {"format": img.format, "size": list(img.size), "mode": img.mode,
                "exif": len(img.getexif()) > 0 or "exif" in img.info}
        if img.format == "JPEG":
            rgb = img.convert("RGB")
            w, h = img.size
            info["top"] = rgb.getpixel((w // 2, h // 10))
            info["bottom"] = rgb.getpixel((w // 2, h - h // 10))
        raw = open(path, "rb").read()
        info["gpsBytes"] = b"GPS" in raw or b"LosOS test camera" in raw
        print(json.dumps(info))

    {"phone": phone_photo, "banner": banner, "look": look}[sys.argv[1]](sys.argv[2])
  '';
in

pkgs.testers.nixosTest {
  name = "losos-profiles";

  nodes = {
    box = import ./profile-box.nix { inherit pkgs; };

    laptop = _: {
      environment.systemPackages = [
        pkgs.curl
        python
      ];
      virtualisation.vlans = [ 1 ];
    };
  };

  testScript = ''
    import json
    import re

    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA_PASSWORD = "Eva-Long-Password-3"
    STORE = "/var/lib/losos/profiles"

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def api(path, cookie=None, method="GET", data=None, upload=None, out=None):
        args = f"-X {method} 'https://{BOX}{path}'"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        if upload is not None:
            args += f" -H 'Content-Type: application/octet-stream' --data-binary @{upload}"
        if out is not None:
            code = curl(f"-o {out} -w '%{{http_code}}' {args}")
            return int(code), None
        body, code = curl(f"-w '\\n%{{http_code}}' {args}").rsplit("\n", 1)
        return int(code), body

    def look(path):
        return json.loads(laptop.succeed(f"python3 ${pictures} look {path}"))

    with subtest("the owner claims the box and adds Eva, who may change her own account"):
        code, body = api("/api/setup/claim", method="POST",
                         data={"name": "matus", "password": PASSWORD})
        assert code == 200, (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        laptop.wait_until_succeeds(f"curl -sf http://{BOX}/auth/api/health", timeout=60)
        matus = sign_in("matus", PASSWORD)
        code, body = api("/api/people", matus, method="POST", data={
            "name": "eva", "displayName": "Eva Novak", "email": "eva@mattbox.local",
            "password": EVA_PASSWORD, "groups": ["losos-account"],
        })
        assert code == 200, (code, body)
        laptop.wait_until_succeeds(
            f"curl -sk -o /dev/null -w '%{{http_code}}' https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' "
            f"-d '{json.dumps({'username': 'eva', 'password': EVA_PASSWORD})}' | grep -q 200",
            timeout=60,
        )
        eva = sign_in("eva", EVA_PASSWORD)

    with subtest("a fresh profile is a colour and the name, and never the address"):
        code, body = api("/api/profiles", eva)
        assert code == 200, (code, body)
        people = {p["name"]: p for p in json.loads(body)["people"]}
        assert set(people) == {"matus", "eva"}, people
        e = people["eva"]
        assert e["displayName"] == "Eva Novak" and e["picture"] is None and e["banner"] is None, e
        assert e["accent"] in ["teal", "blue", "violet", "rose", "amber", "green"], e
        assert e["accentChosen"] is False, e
        assert "eva@mattbox.local" not in body and "email" not in e, body
        # Signed out, nothing.
        assert api("/api/profiles")[0] == 401
        assert api("/api/profiles/eva/picture?v=1")[0] == 401

    with subtest("a phone photo arrives upright, square and without its position"):
        laptop.succeed("python3 ${pictures} phone /tmp/phone.jpg")
        sent = look("/tmp/phone.jpg")
        assert sent["exif"] and sent["gpsBytes"], sent
        code, body = api("/api/account/picture", eva, method="PUT", upload="/tmp/phone.jpg")
        assert code == 200, (code, body)
        profile = json.loads(body)
        url = profile["picture"]
        assert re.fullmatch(r"/api/profiles/eva/picture\?v=\d+", url), profile
        code, _ = api(url, matus, out="/tmp/got.jpg")
        assert code == 200, code
        got = look("/tmp/got.jpg")
        assert got["format"] == "JPEG" and got["size"] == [512, 512], got
        assert not got["exif"] and not got["gpsBytes"], got
        # Orientation 6: the red left half is now on top, the blue at the bottom.
        assert got["top"][0] > 150 and got["top"][2] < 100, got
        assert got["bottom"][2] > 150 and got["bottom"][0] < 100, got
        head = curl(f"-D - -o /dev/null -H 'Cookie: {matus}' 'https://{BOX}{url}'")
        assert re.search(r"(?im)^cache-control: private, max-age=31536000, immutable", head), head
        assert re.search(r"(?im)^content-type: image/jpeg", head), head
        # The store keeps only what was made again, owner-only, one file
        # per version.
        version = url.rsplit("=", 1)[1]
        stored = box.succeed(f"stat -c '%a %U' {STORE}/{version}.picture").strip()
        assert stored == "600 root", stored
        box.fail(f"grep -q 'LosOS test camera' {STORE}/{version}.picture")
        # A version it never gave out is not found, so no cache keeps one.
        assert api(f"/api/profiles/eva/picture?v={int(version) + 100}", matus)[0] == 404
        assert api("/api/profiles/eva/picture", matus)[0] == 404
        assert box.succeed("stat -c '%a' /var/lib/losos").strip() == "700"

    with subtest("a banner with transparent pixels stays a PNG of its shape"):
        laptop.succeed("python3 ${pictures} banner /tmp/banner.png")
        code, body = api("/api/account/banner", eva, method="PUT", upload="/tmp/banner.png")
        assert code == 200, (code, body)
        banner_url = json.loads(body)["banner"]
        assert api(banner_url, matus, out="/tmp/banner-got.png")[0] == 200
        got = look("/tmp/banner-got.png")
        assert got["format"] == "PNG" and got["size"] == [1500, 500], got

    with subtest("words and colour, one field at a time, and a refusal in words"):
        code, body = api("/api/account/profile", eva, method="POST",
                         data={"pronouns": "she/her", "accent": "rose"})
        assert code == 200, (code, body)
        code, body = api("/api/account/profile", eva, method="POST",
                         data={"about": "Keeps the family photos in order."})
        p = json.loads(body)
        assert (p["pronouns"], p["accent"], p["accentChosen"]) == ("she/her", "rose", True), p
        assert p["about"] == "Keeps the family photos in order.", p
        code, body = api("/api/account/profile", eva, method="POST", data={"about": "x" * 161})
        assert code == 400 and "160 characters" in json.loads(body)["error"], (code, body)
        code, body = api("/api/account/profile", eva, method="POST", data={"accent": "mauve"})
        assert code == 400, (code, body)
        laptop.succeed("printf 'not a picture' > /tmp/words.txt")
        code, body = api("/api/account/picture", eva, method="PUT", upload="/tmp/words.txt")
        assert code == 400 and "PNG, JPEG, GIF or WebP" in json.loads(body)["error"], (code, body)
        # The refusal left the picture in place.
        assert json.loads(api("/api/profiles/eva", matus)[1])["picture"] == url
        # The top bar's picture comes with /api/me.
        me = json.loads(api("/api/me", eva)[1])
        assert me["picture"] == url and me["accent"] == "rose", me
        # And the Users pane's rows carry it.
        rows = {r["name"]: r for r in json.loads(api("/api/people", matus)[1])["people"]}
        assert rows["eva"]["picture"] == url and rows["eva"]["banner"] == banner_url, rows

    with subtest("only the person changes their profile, and only an administrator takes a picture down"):
        code, body = api("/api/people/profile/clear", eva, method="POST",
                         data={"name": "matus", "kind": "picture"})
        assert code == 403, (code, body)
        code, body = api("/api/people/profile/clear", matus, method="POST",
                         data={"name": "eva", "kind": "picture"})
        assert code == 200, (code, body)
        p = json.loads(api("/api/profiles/eva", eva)[1])
        assert p["picture"] is None and p["banner"] == banner_url and p["pronouns"] == "she/her", p
        assert api(url, matus)[0] == 404
        box.fail(f"ls {STORE} | grep -q '\\.picture$'")
        # A new picture never takes the old URL, and replacing it again
        # leaves one file and a 404 on the one before.
        code, body = api("/api/account/picture", eva, method="PUT", upload="/tmp/phone.jpg")
        assert code == 200 and json.loads(body)["picture"] != url, body
        replaced = json.loads(body)["picture"]
        code, body = api("/api/account/picture", eva, method="PUT", upload="/tmp/phone.jpg")
        assert code == 200, (code, body)
        url = json.loads(body)["picture"]
        assert url != replaced and api(replaced, matus)[0] == 404
        assert api(url, matus)[0] == 200
        assert box.succeed(f"ls {STORE} | grep '\\.picture$'").split() == [f"{url.rsplit('=', 1)[1]}.picture"]

    with subtest("profiles outlive a restart of lososd"):
        box.systemctl("restart lososd.service")
        box.wait_for_unit("lososd.service")
        box.wait_for_open_port(8082)
        p = json.loads(api("/api/profiles/eva", matus)[1])
        assert p["picture"] == url and p["about"] == "Keeps the family photos in order.", p
        assert api(url, matus)[0] == 200

    with subtest("a person taken off the box takes their profile along"):
        code, body = api("/api/people/remove", matus, method="POST", data={"name": "eva"})
        assert code == 200, (code, body)
        # Matus never set a picture, so the store keeps none at all.
        assert box.succeed(f"ls {STORE}").split() == ["profiles.json"], box.succeed(f"ls {STORE}")
        assert "eva" not in box.succeed(f"cat {STORE}/profiles.json")
        assert api(url, matus)[0] == 404
        names = [p["name"] for p in json.loads(api("/api/profiles", matus)[1])["people"]]
        assert names == ["matus"], names
        # The name taken again starts after every version the old Eva had.
        code, body = api("/api/people", matus, method="POST", data={
            "name": "eva", "displayName": "Eva Again", "email": "eva@mattbox.local",
            "password": EVA_PASSWORD, "groups": ["losos-account"],
        })
        assert code == 200, (code, body)
        laptop.wait_until_succeeds(
            f"curl -sk -o /dev/null -w '%{{http_code}}' https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' "
            f"-d '{json.dumps({'username': 'eva', 'password': EVA_PASSWORD})}' | grep -q 200",
            timeout=60,
        )
        eva = sign_in("eva", EVA_PASSWORD)
        code, body = api("/api/account/picture", eva, method="PUT", upload="/tmp/phone.jpg")
        assert code == 200, (code, body)
        again = json.loads(body)["picture"]
        assert int(again.rsplit("=", 1)[1]) > int(url.rsplit("=", 1)[1]), (again, url)
  '';
}
