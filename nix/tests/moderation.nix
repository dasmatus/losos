# Roles and moderation on a booted box: LosOS Login, lososd, Stalwart for
# LosOS Mail and the front vhost, with people in every role.
#
# The laptop is everyone on the LAN, each signed in with their own cookie:
#
#   * a role goes out as groups and comes back as a role, in /api/people and
#     /api/me; a role of the box's own moves everyone who holds it when its
#     parts change, and leaves them their own account when it goes;
#   * a guest reads and reports but changes nothing on the box, and a viewer
#     cannot read the moderation queue;
#   * Tom sends Eva spam, Eva reports it, and Mia, a moderator, holds it:
#     it leaves Eva's inbox, lososd refuses to open it, and letting it go
#     puts it back;
#   * a report about a child keeps no note, cannot be dismissed, and hiding
#     it takes Tom's picture down unopened;
#   * a muted Tom cannot send mail or change his profile, and a one-minute
#     mute ends by itself; a suspended Gus is out of LosOS Login at once and
#     back in when lifted;
#   * a moderator cannot act on an administrator, the owner or themselves,
#     and an administrator cannot act on the owner;
#   * every action is a line in moderation.log, which only root reads.
#
# `screens = true` is the same run with the real admin pages and Chromium
# on the laptop (nix/tests/moderation-screens.mjs): the PR's shots and the
# film's in dark, and the handbook's pictures in light in English, Slovak
# and German, under `shots/` in the run's output directory. With `before =
# true` (pass the older lososd and admin pages as `lososCtl` and `adminUi`)
# the box has no roles or reports, and only the Users pane is taken, for a
# pull request's Before column.
{
  pkgs,
  screens ? false,
  before ? false,
  adminUi ? null,
  lososCtl ? (import ../packages.nix { inherit pkgs; }).losos-ctl,
}:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  ui =
    if adminUi != null then
      adminUi
    else if screens then
      lososPkgs.losos-admin-ui
    else
      pkgs.runCommand "stub-admin-ui" { } ''
        mkdir -p $out
        echo "admin ui" > $out/index.html
      '';

  # Chromium and playwright-core from nixpkgs' playwright-driver, as
  # nix/tests/profile-screens.nix has them.
  python = pkgs.python3.withPackages (ps: [ ps.pillow ]);
  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  scripts = pkgs.runCommand "moderation-screens-scripts" { } ''
    mkdir -p $out/node_modules
    ln -s ${pkgs.playwright-driver} $out/node_modules/playwright-core
    cp ${./screens.mjs} $out/screens.mjs
    cp ${./moderation-screens.mjs} $out/moderation-screens.mjs
  '';

  # Tom's picture and banner: a plain figure and a plain sky, in the
  # palette. Nobody's photo, and nothing anyone would mind seeing.
  pictures = pkgs.runCommand "moderation-pictures" { nativeBuildInputs = [ python ]; } ''
    mkdir -p $out
    python3 - $out <<'EOF'
    import sys
    from PIL import Image, ImageDraw
    out = sys.argv[1]
    img = Image.new("RGB", (512, 512), (176, 106, 40))
    d = ImageDraw.Draw(img)
    d.ellipse([150, 90, 362, 302], fill=(248, 236, 220))
    d.ellipse([80, 330, 432, 700], fill=(120, 70, 30))
    img.save(f"{out}/picture.png")
    img = Image.new("RGB", (1500, 500))
    d = ImageDraw.Draw(img)
    for y in range(500):
        d.line([(0, y), (1500, y)], fill=(200 - y // 5, 120 - y // 8, 60))
    img.save(f"{out}/banner.png")
    EOF
  '';
in

pkgs.testers.nixosTest {
  name =
    if before then
      "losos-moderation-before"
    else if screens then
      "losos-moderation-screens"
    else
      "losos-moderation";

  nodes = {
    box =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/containers.nix
          ../../modules/tls.nix
          ../../modules/daemon.nix
          ../../modules/identity.nix
          ../../modules/mail.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososCtl;
          admin.ui = ui;
          git.enable = false;
          sharingMyStorage = true;
          # In a mesh as far as Mail's gate goes; nothing joins anything.
          cluster.enable = true;
          shared.fscrypt.enable = false;
        };
        networking.hostName = "mattbox";
        # The shared user as modules/configuration.nix makes it, and the
        # data domain modules/impermanence.nix binds in.
        users.groups.shared.gid = 1001;
        users.users.shared = {
          uid = 1001;
          isNormalUser = true;
          group = "shared";
          homeMode = "700";
        };
        systemd.tmpfiles.rules = [ "d /home/shared/data 0700 shared shared -" ];
        systemd.services.lososd.path = lib.mkForce [ pkgs.coreutils ];
        services.authelia.instances.losos.settings.server.endpoints.rate_limits.session_elevation_start.enable =
          false;
        environment.systemPackages = [ pkgs.jq ];
        virtualisation = {
          memorySize = 2048;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    laptop =
      { lib, ... }:
      {
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
        ]
        ++ lib.optionals screens [
          pkgs.bun
          python
        ];
        networking.hosts."192.168.1.1" = [ "mattbox.local" ];
        environment.variables = lib.mkIf screens { PLAYWRIGHT_BROWSERS_PATH = "${browsers}"; };
        fonts = lib.mkIf screens {
          packages = [ pkgs.inter ];
          fontconfig.defaultFonts.sansSerif = [ "Inter" ];
        };
        virtualisation = {
          memorySize = if screens then 3072 else 1024;
          cores = 2;
          vlans = [ 1 ];
        };
      };
  };

  testScript = ''
    import json
    import re
    import time

    BEFORE = ${if before then "True" else "False"}
    SCREENS = ${if screens then "True" else "False"}
    BOX = "mattbox.local"
    DOMAIN = "mattbox.home.arpa"
    PASSWORD = "Correct-Horse-Battery-9"
    PASSWORDS = {
        "jan": "Jan-Long-Password-1",
        "mia": "Mia-Long-Password-2",
        "eva": "Eva-Long-Password-3",
        "tom": "Tom-Long-Password-4",
        "gus": "Gus-Long-Password-5",
        "oma": "Oma-Long-Password-6",
    }

    # One machine at a time: under TCG two at once can miss the driver's
    # wait for a shell.
    box.start()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("losos-mail.service")
    box.wait_for_open_port(443)
    laptop.start()
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def api(path, cookie=None, method="GET", data=None, raw=None, extra=""):
        args = f"-X {method} 'https://{BOX}{path}' {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            body = json.dumps(data).replace("'", "'\\'''")
            args += f" -H 'Content-Type: application/json' -d '{body}'"
        if raw is not None:
            args += f" -H 'Content-Type: image/png' --data-binary @{raw}"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def ok(path, cookie, **kw):
        code, body = api(path, cookie, **kw)
        assert code == 200, (path, code, body)
        return json.loads(body) if body else {}

    def refused(path, cookie, codes, **kw):
        code, body = api(path, cookie, **kw)
        assert code in codes, (path, code, body)
        return json.loads(body) if body.startswith("{") else {}

    def first_factor(name, password):
        body = json.dumps({"username": name, "password": password})
        return curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )

    def sign_in(name, password):
        head = first_factor(name, password)
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def folder(home, role):
        return next(m["id"] for m in home["mailboxes"] if m["role"] == role)

    def inbox_subjects(cookie, inbox):
        return [m["subject"] for m in ok(f"/api/mail/messages?mailbox={inbox}", cookie)["messages"]]

    def wait_for(cookie, mailbox, subject, timeout=120):
        deadline = time.time() + timeout
        while time.time() < deadline:
            found = [m for m in ok(f"/api/mail/messages?mailbox={mailbox}", cookie)["messages"] if m["subject"] == subject]
            if found:
                return found[0]
            time.sleep(2)
        raise AssertionError(f"{subject!r} never arrived")

    def role_of(cookie, name):
        return next(p for p in ok("/api/people", cookie)["people"] if p["name"] == name)

    def log_actions():
        lines = box.succeed("cat /var/lib/losos/moderation.log").splitlines()
        return [json.loads(l)["action"] for l in lines]

    # What Apply would have written: sharing on, in a mesh, so Mail is on.
    box.succeed(
        "mkdir -p /etc/nixos/modules && cat > /etc/nixos/modules/overrides.nix <<'EOF'\n"
        "{ ... }:\n{\n  losos.sharingMyStorage = true;\n  losos.cluster.enable = true;\n}\nEOF"
    )

    with subtest("the owner claims the box and adds everyone"):
        laptop.wait_until_succeeds(f"curl -sfk https://{BOX}/api/setup/claim | jq -e .ready", timeout=300)
        code, body = api("/api/setup/claim", method="POST", data={"name": "matus", "password": PASSWORD})
        assert code == 200, (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        laptop.wait_until_succeeds(f"curl -skf https://{BOX}/auth/api/health", timeout=120)
        matus = sign_in("matus", PASSWORD)
        ok("/api/account", matus, method="POST", data={"displayName": "Matus", "email": f"matus@{DOMAIN}"})
        groups = {
            "jan": ["losos-admins"],
            "mia": ["losos-account"] if BEFORE else ["losos-moderation", "losos-account"],
            "eva": ["losos-account", "losos-backup", "losos-storage"],
            "tom": ["losos-account"],
            "gus": ["losos-account"] if BEFORE else ["losos-guests"],
            "oma": ["losos-viewers", "losos-account"],
        }
        names = {"jan": "Jan", "mia": "Mia", "eva": "Eva Novak", "tom": "Tom", "gus": "Gus", "oma": "Oma"}
        for name, g in groups.items():
            ok("/api/people", matus, method="POST", data={
                "name": name, "displayName": names[name], "password": PASSWORDS[name], "groups": g,
            })
        laptop.wait_until_succeeds(
            f"curl -sk https://{BOX}/auth/api/firstfactor -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"username": "oma", "password": PASSWORDS["oma"]})}' | grep -q OK",
            timeout=60,
        )
        cookies = {name: sign_in(name, PASSWORDS[name]) for name in groups}
        jan, mia, eva, tom, gus, oma = (cookies[n] for n in ["jan", "mia", "eva", "tom", "gus", "oma"])
        ok("/api/account/picture", tom, method="PUT", raw="${pictures}/picture.png")
        ok("/api/account/banner", tom, method="PUT", raw="${pictures}/banner.png")

    if not BEFORE:
        with subtest("a role goes out as groups and comes back as a role"):
            roles = {p["name"]: p["role"] for p in ok("/api/people", matus)["people"]}
            assert roles == {
                "matus": "owner", "jan": "admin", "mia": "moderator",
                "eva": "member", "tom": "member", "gus": "guest", "oma": "member",
            }, roles
            assert ok("/api/me", mia)["role"] == "moderator"
            assert ok("/api/me", gus)["role"] == "guest"
            # A guest is their own account and nothing more, whatever was asked.
            assert sorted(role_of(matus, "gus")["groups"]) == ["losos-account", "losos-guests"]
            # The badge reaches every app through the profile.
            assert ok("/api/profiles/mia", eva)["role"] == "moderator"

        with subtest("a role of the box's own moves everyone who holds it"):
            made = ok("/api/roles", matus, method="POST", data={"name": "Backup helpers", "sections": ["backup", "account"]})
            rid = made["id"]
            assert rid == "backup-helpers", made
            ok("/api/people/access", matus, method="POST", data={"name": "eva", "groups": [f"losos-role-{rid}"]})
            g = sorted(role_of(matus, "eva")["groups"])
            assert g == ["losos-account", "losos-backup", f"losos-role-{rid}"], g
            assert role_of(matus, "eva")["roleName"] == "Backup helpers"
            ok("/api/roles", matus, method="POST", data={"id": rid, "name": "Backup helpers", "sections": ["backup", "storage", "account"]})
            assert "storage" in ok("/api/me", eva)["sections"]
            refused("/api/roles", matus, [409], method="POST", data={"name": "backup helpers", "sections": []})
            refused("/api/roles", mia, [403], method="POST", data={"name": "Mods", "sections": []})

        with subtest("a guest reads and reports but changes nothing"):
            refused("/api/apply", gus, [403], method="POST", data={"hostName": "evil"})
            refused("/api/people", gus, [403])
            ok("/api/me", gus)
            # Viewers read every section, but never the moderators' queue.
            refused("/api/moderation", oma, [403])
            refused("/api/moderation", tom, [403])
            assert ok("/api/moderation", mia)["reports"] == []

        with subtest("Tom sends Eva spam; Eva reports it and Mia holds it"):
            ehome = ok("/api/mail", eva)
            ok("/api/mail", tom)
            inbox = folder(ehome, "inbox")
            ok("/api/mail/send", tom, method="POST", data={
                "to": [f"eva@{DOMAIN}"], "subject": "Cheap watches", "text": "Twelve for the price of one.",
            })
            spam = wait_for(eva, inbox, "Cheap watches")
            rep = ok("/api/reports", eva, method="POST", data={
                "app": "mail", "ref": spam["id"], "reason": "spam", "about": "tom", "note": "Twelve of these today.",
            })
            # The same report twice is one report.
            again = ok("/api/reports", eva, method="POST", data={
                "app": "mail", "ref": spam["id"], "reason": "spam", "about": "tom",
            })
            assert again["id"] == rep["id"], (rep, again)
            assert ok("/api/me", mia)["reports"] == 1
            queue = ok("/api/moderation", mia)
            assert queue["hides"] == ["mail", "profile"], queue["hides"]
            [r] = queue["reports"]
            assert r["note"] == "Twelve of these today." and r["state"] == "open", r
            # Nothing in the queue carries the message itself.
            assert "Twelve for the price" not in json.dumps(queue)
            ok(f"/api/moderation/reports/{r['id']}", mia, method="POST", data={"action": "hide"})
            assert "Cheap watches" not in inbox_subjects(eva, inbox)
            for path in [f"/api/mail/messages/{spam['id']}", f"/api/mail/messages/{spam['id']}/html"]:
                body = refused(path, eva, [403])
                assert body.get("held") is True, body
            # The quarantine is not a folder anyone sees.
            assert all(m["name"] != "Quarantine" for m in ok("/api/mail", eva)["mailboxes"])
            ok(f"/api/moderation/reports/{r['id']}", mia, method="POST", data={"action": "release"})
            assert "Cheap watches" in inbox_subjects(eva, inbox)
            ok(f"/api/mail/messages/{spam['id']}", eva)
            refused(f"/api/moderation/reports/{r['id']}", mia, [409], method="POST", data={"action": "hide"})

        with subtest("a report about a child keeps no note and is hidden unopened"):
            assert ok("/api/profiles/tom", eva)["picture"] is not None
            rep = ok("/api/reports", gus, method="POST", data={
                "app": "profile", "ref": "picture", "reason": "child", "about": "tom", "note": "a description",
            })
            [r] = [r for r in ok("/api/moderation", mia)["reports"] if r["id"] == rep["id"]]
            assert r["note"] == "", r
            assert "a description" not in box.succeed("cat /var/lib/losos/moderation.json /var/lib/losos/moderation.log")
            refused(f"/api/moderation/reports/{r['id']}", mia, [409], method="POST", data={"action": "dismiss"})
            refused(f"/api/moderation/reports/{r['id']}", mia, [409], method="POST", data={"action": "resolve"})
            ok(f"/api/moderation/reports/{r['id']}", mia, method="POST", data={"action": "hide"})
            assert ok("/api/profiles/tom", eva)["picture"] is None
            assert ok("/api/profiles/tom", eva)["banner"] is not None

        with subtest("a muted Tom cannot send or change his profile, and a short mute ends"):
            ok("/api/moderation/mute", mia, method="POST", data={"name": "tom", "minutes": 1, "reason": "Spam"})
            body = refused("/api/mail/send", tom, [403], method="POST", data={"to": [f"eva@{DOMAIN}"], "subject": "x", "text": "x"})
            assert body.get("muted", {}).get("by") == "mia", body
            refused("/api/account/profile", tom, [403], method="POST", data={"about": "hello"})
            # Reading goes on.
            ok("/api/mail", tom)
            ok("/api/profiles", tom)
            box.wait_until_succeeds("grep -q '\"mute ended\"' /var/lib/losos/moderation.log", timeout=180)
            ok("/api/mail/send", tom, method="POST", data={"to": [f"eva@{DOMAIN}"], "subject": "Sorry", "text": "Won't happen again."})

        with subtest("a suspended Gus is out at once and back when lifted"):
            ok("/api/moderation/suspend", mia, method="POST", data={"name": "gus", "minutes": None, "reason": "Spam in reports"})
            laptop.wait_until_succeeds(
                f"curl -sk -o /dev/null -w '%{{http_code}}' -H 'Cookie: {gus}' https://{BOX}/api/me | grep -qE '401|403'",
                timeout=60,
            )
            laptop.fail(
                f"curl -sk https://{BOX}/auth/api/firstfactor -H 'Content-Type: application/json'"
                f" -d '{json.dumps({"username": "gus", "password": PASSWORDS["gus"]})}' | grep -q '\"OK\"'"
            )
            assert role_of(matus, "gus")["suspended"]["by"] == "mia"
            ok("/api/moderation/unsuspend", mia, method="POST", data={"name": "gus"})
            laptop.wait_until_succeeds(
                f"curl -sk https://{BOX}/auth/api/firstfactor -H 'Content-Type: application/json'"
                f" -d '{json.dumps({"username": "gus", "password": PASSWORDS["gus"]})}' | grep -q OK",
                timeout=60,
            )
            gus = sign_in("gus", PASSWORDS["gus"])
            ok("/api/me", gus)

        with subtest("nobody holds someone of a higher rank, or themselves"):
            for target in ["jan", "matus", "mia"]:
                refused("/api/moderation/mute", mia, [403, 409], method="POST", data={"name": target, "minutes": 60})
            refused("/api/moderation/suspend", jan, [403, 409], method="POST", data={"name": "matus", "minutes": 60})
            refused("/api/moderation/mute", jan, [403, 409], method="POST", data={"name": "jan", "minutes": 60})
            refused("/api/moderation/mute", tom, [403], method="POST", data={"name": "gus", "minutes": 60})
            # An administrator may hold a moderator.
            ok("/api/moderation/mute", jan, method="POST", data={"name": "mia", "minutes": 60})
            ok("/api/moderation/unmute", jan, method="POST", data={"name": "mia"})

        with subtest("removing a role leaves its people their own account"):
            ok("/api/roles/remove", matus, method="POST", data={"id": rid})
            assert role_of(matus, "eva")["groups"] == ["losos-account"]
            ok("/api/people/access", matus, method="POST", data={"name": "eva", "groups": ["losos-account", "losos-backup", "losos-storage"]})

        with subtest("every action is a line in the log only root reads"):
            actions = log_actions()
            for want in ["role saved", "role", "report", "hide", "release", "mute", "mute ended", "suspend", "unsuspend", "unmute", "role removed"]:
                assert want in actions, (want, actions)
            assert box.succeed("stat -c %a /var/lib/losos/moderation.log").strip() == "600"
            lines = ok("/api/moderation", jan)["log"]
            assert lines[0]["action"] == "role removed", lines[0]

    if SCREENS:
        if not BEFORE:
            # Fresh reports for the pictures: spam in mail, Tom's banner
            # reported for a child, and an insult in chat.
            ok("/api/mail/send", tom, method="POST", data={
                "to": [f"eva@{DOMAIN}"], "subject": "Lottery win", "text": "Claim your prize today.",
            })
            spam = wait_for(eva, folder(ok("/api/mail", eva), "inbox"), "Lottery win")
            ok("/api/reports", tom, method="POST", data={
                "app": "chat", "ref": "!family:box.local/$insult-1", "reason": "abuse", "about": "eva", "note": "Called me names in the family room.",
            })
            ok("/api/reports", eva, method="POST", data={
                "app": "mail", "ref": spam["id"], "reason": "spam", "about": "tom",
                "note": "The same offer again, this time to everyone.",
            })
            ok("/api/reports", gus, method="POST", data={"app": "profile", "ref": "banner", "reason": "child", "about": "tom"})
            made = ok("/api/roles", matus, method="POST", data={"name": "Backup helpers", "sections": ["backup", "storage", "account"]})
            ok("/api/people/access", matus, method="POST", data={"name": "eva", "groups": [f"losos-role-{made['id']}"]})
            ok("/api/roles", matus, method="POST", data={"name": "Photo keepers", "sections": ["apps", "account"]})
        laptop.succeed("rm -rf /tmp/shots")
        if not BEFORE:
            for locale in ["en-US", "sk-SK", "de-DE"]:
                laptop.succeed(f"cd ${scripts} && bun moderation-screens.mjs handbook {locale} /tmp/shots/handbook >&2", timeout=3600)
        mode = "before" if BEFORE else "after"
        laptop.succeed(f"cd ${scripts} && bun moderation-screens.mjs {mode} dark /tmp/shots/{mode} >&2", timeout=3600)
        steps = json.loads(laptop.succeed(f"cat /tmp/shots/{mode}/manifest.json"))["steps"]
        assert len(steps) >= 2, steps
        for step in steps:
            size = laptop.succeed(
                f"python3 -c \"from PIL import Image; print(*Image.open('/tmp/shots/{mode}/{step['file']}').size)\""
            ).split()
            assert size == ["2560", "1800"], (step, size)
        laptop.succeed("tar -C /tmp -cf /tmp/shots.tar shots")
        laptop.copy_from_machine("/tmp/shots.tar", "")
  '';
}
