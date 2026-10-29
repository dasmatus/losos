# nixos-test-vms config for LosOS Chat (modules/chat.nix, backend/src/chat.rs,
# admin-ui/app/src/chat/).
#
# The box runs LosOS Login, lososd, nginx and Synapse as they ship; a laptop
# on the LAN carries the browser's traffic. What only a booted box can show:
#
#   * chat is there only while the box shares its storage in a mesh: with
#     either off there is no homeserver, no /_matrix/ route, no /api/chat,
#     no Chat in the sidebar, and /chat/ goes home;
#   * nobody has a Matrix password: an account and a device come only from
#     lososd for the person LosOS Login signed in, and every request to the
#     homeserver needs that sign-in and a token of that same person;
#   * signing out of LosOS Login ends chat at the next request, and removing
#     a person signs their devices out of the homeserver;
#   * Synapse runs as `shared`, keeps everything in the shared data domain,
#     and only root, nginx and `shared` may open its port;
#   * messages, uploads and the thread list work through nginx;
#   * a moderator's hide redacts a reported message as its sender, and a
#     muted person reads on but sends nothing until unmuted;
#   * the chat page itself: two people in two browsers, an end-to-end
#     encrypted conversation, a reply, a reaction, a thread, a picture and an
#     edit, and a recovery key that lets a second device read the past;
#   * pictures from others wait blurred and unfetched; an administrator
#     marks a person a child or an adult, which holds the nudity warning on
#     or allows "No one"; the warning (on a stand-in model that flags red)
#     holds a picture back and asks before one is sent; card and IBAN
#     numbers are black bars, asked about before sending; Report opens.
#
# The browser is Chromium under Playwright in the test driver
# (nix/tests/chat-browser.mjs), talking to the box through a SOCKS proxy on
# the laptop, so the address bar says
# https://mattbox.local/ and LosOS Login's cookie is the box's own. Every
# screenshot lands in the test's output: `nix build
# .#checks.x86_64-linux.losos-chat` and look in result/shots/. The launch
# film's states are numbered there with a manifest.json; the pull request's
# pictures are under result/shots/pr/.
#
# LosOS Git is off; LosOS Cloud is the real service, for the files chat
# sends from it and keeps in it.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # The nudity warning's stand-in model (nudity-stand-in.py): red pictures
  # are flagged, green ones are not, and nothing real is ever looked at.
  nudityStandIn = pkgs.runCommand "nudity-stand-in.onnx" {
    nativeBuildInputs = [ (pkgs.python3.withPackages (p: [ p.onnx ])) ];
  } "python3 ${./nudity-stand-in.py} $out";
  squares = pkgs.runCommand "chat-test-squares" { nativeBuildInputs = [ pkgs.imagemagick ]; } ''
    mkdir $out
    magick -size 480x320 xc:'#d01010' $out/red.png
    magick -size 480x320 xc:'#10b030' $out/green.png
  '';

  # The admin UI as it ships, without LosOS Diagrams' WebAssembly, which
  # takes longer to build than the test to run: stand-ins take the place of
  # the three packages, so the bundle builds without Bevy, and only
  # /diagrams, which no step here opens, would fail. The nudity model is
  # the stand-in above.
  ui = lososPkgs.losos-admin-ui.overrideAttrs (old: {
    pname = "losos-admin-ui-without-lab";
    env = old.env // {
      LOSOS_NUDITY_MODEL = "${nudityStandIn}";
    };
    preBuild = ''
      for f in core-pkg/losos_lab_core virt-pkg/losos_lab_virt \
               render-pkg/webgpu/losos_lab_render render-pkg/webgl2/losos_lab_render; do
        mkdir -p "src/lab/$(dirname "$f")"
        echo 'export default function init() { throw new Error("not in this build"); } export class Lab {}' > "src/lab/$f.js"
        : > "src/lab/''${f}_bg.wasm"
      done
      cat > vite.chat-test.config.ts <<'EOF'
      import base from "./vite.config";
      const root = import.meta.dirname;
      const config = base as Record<string, any>;
      config.build.rolldownOptions.input = {
        main: `''${root}/index.html`,
        chat: `''${root}/chat/index.html`,
        manage: `''${root}/manage/index.html`,
      };
      export default config;
      EOF
    '';
    buildPhase = ''
      runHook preBuild
      bun --bun vite build --config vite.chat-test.config.ts
      runHook postBuild
    '';
  });

  # The playwright the admin UI pins (to nixpkgs' playwright-driver, which
  # nix/tests/admin-ui.nix asserts), from its own node_modules.
  playwright = "${lososPkgs.losos-admin-ui.nodeModules}/node_modules/playwright";
  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  fonts = pkgs.makeFontsConf {
    fontDirectories = [
      pkgs.dejavu_fonts
      pkgs.noto-fonts-color-emoji
    ];
  };
  picture = ../../admin-ui/themes/brand/salmon.png;
in

pkgs.testers.nixosTest {
  name = "losos-chat";
  # The handbook's pictures in three languages take most of an hour on
  # their own under TCG.
  globalTimeout = 3 * 3600;

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
          ../../modules/chat.nix
          ../../modules/cloud.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          # The pins and the look are plugins; without them the page sees 404s.
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = ui;
          git.enable = false;
          # The shared user's side of the box: chat's gate.
          sharingMyStorage = true;
          cluster.enable = true;
          # LosOS Cloud, for sending files from it and keeping files in it.
          cloud.enable = true;
        };
        # The same box, sharing switched off; the test switches to it and
        # back (switch-to-configuration, as an Apply would).
        specialisation.unshared.configuration.losos.sharingMyStorage = lib.mkForce false;

        # The name a real box has; the claim wants it in Host.
        networking.hostName = "mattbox";
        # configuration.nix's two data users, without the rest of it.
        users.users.shared = {
          isNormalUser = true;
          uid = 1001;
          group = "shared";
          homeMode = "700";
        };
        users.groups.shared.gid = 1001;

        systemd.services.lososd.path = lib.mkForce [ pkgs.coreutils ];
        environment.systemPackages = [
          pkgs.jq
          pkgs.curl
          pkgs.sqlite
        ];
        virtualisation = {
          memorySize = 2048;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    # A computer on the LAN. The browser runs in the test driver and reaches
    # the box through this machine's SOCKS proxy, which resolves names here.
    laptop =
      { nodes, ... }:
      {
        networking.extraHosts = "${nodes.box.networking.primaryIPAddress} mattbox.local";
        systemd.services.socks = {
          wantedBy = [ "multi-user.target" ];
          after = [ "network-online.target" ];
          wants = [ "network-online.target" ];
          serviceConfig = {
            ExecStart = "${pkgs.microsocks}/bin/microsocks -i 0.0.0.0 -p 1080";
            DynamicUser = true;
          };
        };
        networking.firewall.allowedTCPPorts = [ 1080 ];
        virtualisation.forwardPorts = [
          {
            from = "host";
            host.port = 1080;
            guest.port = 1080;
          }
        ];
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
        ];
        virtualisation.vlans = [ 1 ];
      };
  };

  testScript = ''
    import json
    import os
    import re
    import subprocess
    import time
    import urllib.parse

    HOST = "mattbox.local"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA_PASSWORD = "Eva-Long-Password-3"
    PEOPLE = "/var/lib/losos-identity/users.yml"
    OUT = os.environ.get("out", os.getcwd())
    SHOTS = f"{OUT}/shots"
    os.makedirs(SHOTS, exist_ok=True)

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("losos-chat.service")
    box.wait_for_open_port(443)
    box.wait_for_open_port(8008)
    # lososd loads its plugins before it listens; the unit is active
    # earlier than that.
    box.wait_for_open_port(8082)
    laptop.wait_for_unit("socks.service")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def call(path, cookie=None, method="GET", data=None, token=None, extra=""):
        args = f"-X {method} 'https://{HOST}{path}' {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if token:
            args += f" -H 'Authorization: Bearer {token}'"
        if data is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{HOST}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def session(cookie):
        code, body = call("/api/chat/session", cookie, method="POST", data={})
        assert code == 200, (code, body)
        return json.loads(body)

    def whoami(cookie, token):
        return call("/_matrix/client/v3/account/whoami", cookie, token=token)

    def switch(to):
        """Switch the running box, as an Apply's rebuild would."""
        path = "/run/booted-system" if to is None else f"/run/booted-system/specialisation/{to}"
        box.succeed(f"{path}/bin/switch-to-configuration test >&2")
        box.wait_for_unit("lososd.service")
        box.wait_for_unit("nginx.service")
        box.wait_for_open_port(8082)

    def saved(shared):
        """Write the box's saved settings to match the switch, as an Apply
        would. The mesh banner and /api/settings read these, not the
        evaluated system, so without them the chat page would sit under a
        "not joined a mesh" banner."""
        value = "true" if shared else "false"
        content = "\n".join([
            "{ ... }:",
            "{",
            f"  losos.sharingMyStorage = {value};",
            f"  losos.cluster.enable = {value};",
            "}",
        ])
        box.succeed(f"mkdir -p /etc/nixos/modules && cat > /etc/nixos/modules/overrides.nix <<'EOF'\n{content}\nEOF")

    with subtest("the claim makes Matus, and an administrator adds Eva, who may open her own account"):
        code, body = call(
            "/api/setup/claim",
            method="POST",
            data={"name": "matus", "password": PASSWORD},
        )
        assert code == 200, (code, body)
        spare = json.loads(body)["token"]
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)
        box.wait_until_succeeds(f"grep -q \"domain: '{HOST}'\" /run/losos-identity/session.yml")
        matus = sign_in("matus", PASSWORD)
        code, body = call("/api/account", matus, method="POST",
                          data={"displayName": "Matus", "email": "matus@mattbox.local"})
        assert code == 200, (code, body)
        code, body = call(
            "/api/people",
            matus,
            method="POST",
            data={"name": "eva", "displayName": "Eva", "email": "eva@mattbox.local",
                  "password": EVA_PASSWORD, "groups": ["losos-account"]},
        )
        assert code == 200, (code, body)
        box.wait_until_succeeds(f"grep -q eva {PEOPLE}")
        time.sleep(5)
        eva = sign_in("eva", EVA_PASSWORD)

    with subtest("Synapse runs as shared, inside the shared data domain"):
        pid = box.succeed("systemctl show -P MainPID losos-chat.service").strip()
        assert box.succeed(f"ps -o user= -p {pid}").split() == ["shared"]
        assert box.succeed("stat -c '%U %a' /home/shared/data/chat").strip() == "shared 700"
        assert box.succeed("stat -c '%U %a' /home/shared/data/chat/synapse").strip() == "shared 700"
        box.wait_until_succeeds("test -s /home/shared/data/chat/synapse/homeserver.db")
        assert "shared" in box.succeed("stat -c '%U' /home/shared/data/chat/synapse/homeserver.db")
        # Sticky and delayed events, which calls need.
        versions = json.loads(box.succeed("curl -sf http://127.0.0.1:8008/_matrix/client/versions"))
        assert versions["unstable_features"].get("org.matrix.msc4354") is True, versions
        assert versions["unstable_features"].get("org.matrix.msc4140") is True, versions
        # Only root, nginx and shared may open its port on loopback.
        box.succeed("curl -sf http://127.0.0.1:8008/_matrix/client/versions")
        box.fail("runuser -u nobody -- curl -sf -m 5 http://127.0.0.1:8008/_matrix/client/versions")
        # Nothing but loopback.
        laptop.fail(f"curl -sf -m 5 http://{HOST}:8008/_matrix/client/versions")

    with subtest("there are no chat passwords: Synapse refuses every other sign-in"):
        code, body = call(
            "/_matrix/client/v3/login", matus, method="POST",
            data={"type": "m.login.password", "identifier": {"type": "m.id.user", "user": "matus"},
                  "password": PASSWORD},
        )
        assert code in (400, 401, 403), (code, body)
        code, body = call(
            "/_matrix/client/v3/register", matus, method="POST",
            data={"username": "mallory", "password": "Mallory-Long-Pass-1"},
        )
        assert code in (401, 403), (code, body)

    with subtest("only LosOS Login opens chat"):
        # No sign-in: lososd makes no device and the homeserver is closed.
        code, body = call("/api/chat/session", method="POST", data={})
        assert code == 401, (code, body)
        code, body = call("/_matrix/client/v3/account/whoami")
        assert code == 401, (code, body)
        assert json.loads(body)["losos_sign_in"] == "/auth/", body
        # The spare key is not a person.
        code, body = call("/api/chat/session", method="POST", data={},
                          extra=f"-H 'Authorization: Bearer {spare}'")
        assert code == 403, (code, body)
        # Signed in, a device each.
        m = session(matus)
        e = session(eva)
        assert m["userId"] == "@matus:mattbox.local" and e["userId"] == "@eva:mattbox.local", (m, e)
        code, body = whoami(matus, m["accessToken"])
        assert code == 200 and json.loads(body)["user_id"] == m["userId"], (code, body)
        # A token alone, without the sign-in, is not enough.
        code, body = whoami(None, m["accessToken"])
        assert code == 401, (code, body)
        # Nor is someone else's token.
        code, body = whoami(eva, m["accessToken"])
        assert code == 403, (code, body)
        # The same device comes back when the page asks for it,
        again = json.loads(call("/api/chat/session", matus, method="POST",
                                data={"device": m["deviceId"]})[1])
        assert again["deviceId"] == m["deviceId"], (again, m)
        # with a new token of that device. Synapse keeps the earlier one as
        # well, so the page holds on to its own and asks for a new one only
        # once that stops working (src/chat/session.ts).
        assert again["accessToken"] != m["accessToken"], (again, m)
        assert json.loads(whoami(matus, again["accessToken"])[1])["device_id"] == m["deviceId"]
        m = again
        # Everyone on the box can be found.
        code, body = call("/api/chat/people", matus)
        people = {p["userId"]: p["displayName"] for p in json.loads(body)["people"]}
        assert set(people) == {m["userId"], e["userId"]}, people
        assert people[e["userId"]] == "Eva", people
        code, body = call("/api/me", matus)
        assert "chat" in json.loads(body)["shared"], body

    with subtest("messages, uploads and the thread list go through nginx"):
        mt, et = m["accessToken"], e["accessToken"]
        code, body = call("/_matrix/client/v3/createRoom", matus, method="POST", token=mt,
                          data={"name": "Budget", "invite": [e["userId"]], "preset": "private_chat"})
        assert code == 200, (code, body)
        room = json.loads(body)["room_id"]
        q = urllib.parse.quote(room)
        assert call(f"/_matrix/client/v3/rooms/{q}/join", eva, method="POST", token=et, data={})[0] == 200
        code, body = call(f"/_matrix/client/v3/rooms/{q}/send/m.room.message/t1", matus,
                          method="PUT", token=mt, data={"msgtype": "m.text", "body": "Rent is due Friday."})
        assert code == 200, (code, body)
        root = json.loads(body)["event_id"]
        code, body = call(f"/_matrix/client/v3/rooms/{q}/send/m.room.message/t2", eva, method="PUT",
                          token=et, data={"msgtype": "m.text", "body": "I paid my half.",
                                          "m.relates_to": {"rel_type": "m.thread", "event_id": root}})
        assert code == 200, (code, body)
        code, body = call(f"/_matrix/client/v3/rooms/{q}/messages?dir=b&limit=10", eva, token=et)
        assert "Rent is due Friday." in body, body
        code, body = call(f"/_matrix/client/v1/rooms/{q}/threads", matus, token=mt)
        assert code == 200, (code, body)
        chunk = json.loads(body)["chunk"]
        assert [ev["event_id"] for ev in chunk] == [root], body
        thread = chunk[0]["unsigned"]["m.relations"]["m.thread"]
        assert thread["count"] == 1 and thread["latest_event"]["content"]["body"] == "I paid my half.", thread
        code, body = call(f"/_matrix/client/v1/rooms/{q}/threads", matus)
        assert code in (401, 403), (code, body)
        # Media: an upload, and a download only with the sign-in.
        laptop.succeed("printf 'budget,2026\\nrent,900\\n' > /tmp/budget.csv")
        code, body = call("/_matrix/media/v3/upload?filename=budget.csv", matus, method="POST", token=mt,
                          extra="-H 'Content-Type: text/csv' --data-binary @/tmp/budget.csv")
        assert code == 200, (code, body)
        mxc = json.loads(body)["content_uri"]
        server, media = mxc.removeprefix("mxc://").split("/", 1)
        code, body = call(f"/_matrix/client/v1/media/download/{server}/{media}", eva, token=et)
        assert code == 200 and "rent,900" in body, (code, body)
        code, body = call(f"/_matrix/client/v1/media/download/{server}/{media}", token=et)
        assert code == 401, (code, body)
        assert box.succeed("find /home/shared/data/chat/synapse/media -type f | wc -l").strip() != "0"

    with subtest("a moderator hides a reported message and mutes someone in chat"):
        code, body = call(f"/_matrix/client/v3/rooms/{q}/send/m.room.message/t3", eva, method="PUT",
                          token=et, data={"msgtype": "m.text", "body": "Buy cheap watches here."})
        assert code == 200, (code, body)
        spam = json.loads(body)["event_id"]
        code, body = call("/api/reports", matus, method="POST", data={
            "app": "chat", "ref": f"{room}/{spam}", "reason": "spam", "about": "eva"})
        assert code == 200, (code, body)
        report = json.loads(body)["id"]
        code, body = call("/api/moderation", matus)
        assert "chat" in json.loads(body)["hides"], body
        code, body = call(f"/api/moderation/reports/{report}", matus, method="POST", data={"action": "hide"})
        assert code == 200, (code, body)
        # Redacted as its sender: the text is gone for everyone in the room.
        code, body = call(f"/_matrix/client/v3/rooms/{q}/event/{urllib.parse.quote(spam)}", matus, token=mt)
        assert code == 200 and "cheap watches" not in body and "redacted_because" in body, (code, body)
        code, body = call("/api/moderation/mute", matus, method="POST", data={"name": "eva", "minutes": 60})
        assert code == 200, (code, body)
        code, body = call(f"/_matrix/client/v3/rooms/{q}/send/m.room.message/t4", eva, method="PUT",
                          token=et, data={"msgtype": "m.text", "body": "Still here."})
        assert code == 403, (code, body)
        # Reading goes on.
        assert call(f"/_matrix/client/v3/rooms/{q}/messages?dir=b&limit=10", eva, token=et)[0] == 200
        assert call("/api/moderation/unmute", matus, method="POST", data={"name": "eva"})[0] == 200
        code, body = call(f"/_matrix/client/v3/rooms/{q}/send/m.room.message/t5", eva, method="PUT",
                          token=et, data={"msgtype": "m.text", "body": "Sorry about that."})
        assert code == 200, (code, body)

    with subtest("signing out of LosOS Login ends chat at the next request"):
        gone = sign_in("matus", PASSWORD)
        g = session(gone)
        assert whoami(gone, g["accessToken"])[0] == 200
        curl(f"-o /dev/null -X POST https://{HOST}/auth/api/logout -H 'Cookie: {gone}' "
             "-H 'Content-Type: application/json' -d '{}'")
        code, body = whoami(gone, g["accessToken"])
        assert code == 401, (code, body)
        assert call("/api/chat/session", gone, method="POST", data={})[0] == 401

    with subtest("removing a person signs their devices out"):
        code, body = call("/api/people", matus, method="POST",
                          data={"name": "petra", "displayName": "Petra",
                                "password": "Petra-Long-Password-8", "groups": []})
        assert code == 200, (code, body)
        box.wait_until_succeeds(f"grep -q petra {PEOPLE}")
        time.sleep(5)
        petra = sign_in("petra", "Petra-Long-Password-8")
        p = session(petra)
        token = p["accessToken"]
        box.succeed(f"curl -sf -H 'Authorization: Bearer {token}' http://127.0.0.1:8008/_matrix/client/v3/account/whoami")
        code, body = call("/api/people/remove", matus, method="POST", data={"name": "petra"})
        assert code == 200, (code, body)
        box.wait_until_succeeds(
            f"curl -s -H 'Authorization: Bearer {token}' http://127.0.0.1:8008/_matrix/client/v3/account/whoami"
            " | grep -q M_UNKNOWN_TOKEN",
            timeout=60,
        )
        assert whoami(petra, token)[0] in (401, 403)

    # The browser: nix/tests/chat-browser.mjs, run here in the driver.
    browser_env = {
        **os.environ,
        "HOME": os.environ.get("TMPDIR", "/tmp"),
        "CHAT_PLAYWRIGHT": "${playwright}",
        "PLAYWRIGHT_BROWSERS_PATH": "${browsers}",
        "PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS": "true",
        "FONTCONFIG_FILE": "${fonts}",
        "CHAT_HOST": HOST,
        "CHAT_PICTURE": "${picture}",
        "CHAT_RED": "${squares}/red.png",
        "CHAT_GREEN": "${squares}/green.png",
        "CHAT_MATUS_PASSWORD": PASSWORD,
        "CHAT_EVA_PASSWORD": EVA_PASSWORD,
    }

    def browser(phase):
        run = subprocess.Popen(
            ["${pkgs.bun}/bin/bun", "${./chat-browser.mjs}", phase, SHOTS],
            env=browser_env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        # How loaded the box is while the page works, in case a step stalls.
        began = time.monotonic()
        while run.poll() is None and time.monotonic() - began < 1800:
            print(box.execute(
                "uptime; free -m | sed -n 2p; top -bn1 -o %CPU | sed -n 8,13p; "
                "${pkgs.redis}/bin/redis-cli -s /run/redis-*/redis.sock info clients | grep connected_clients; "
                "curl -so /dev/null -w 'synapse versions %{time_total}s\\n' http://127.0.0.1:8008/_matrix/client/versions"
            )[1])
            time.sleep(15)
        out, err = run.communicate(timeout=60)
        done = subprocess.CompletedProcess(run.args, run.returncode, out, err)
        print(done.stdout)
        print(done.stderr)
        if done.returncode != 0:
            print(box.execute("journalctl -b --no-pager -o cat -n 1500 -u lososd -u losos-chat -u authelia-losos -u nginx")[1])
        assert done.returncode == 0, f"the chat page, {phase}: {done.stderr[-2000:]}"

    with subtest("with sharing off there is no chat anywhere"):
        switch("unshared")
        saved(False)
        box.wait_until_fails("systemctl is-active losos-chat.service")
        box.fail("systemctl cat losos-chat.service")
        code, body = call("/api/chat", matus)
        assert code == 404, (code, body)
        assert "chat" not in json.loads(call("/api/me", matus)[1])["shared"]
        code, _ = call("/_matrix/client/v3/account/whoami", matus)
        assert code == 404, code
        code, _ = call("/_matrix/media/v3/config", matus)
        assert code == 404, code
        loc = curl(f"-o /dev/null -w '%{{redirect_url}}' https://{HOST}/chat/")
        assert loc == f"https://{HOST}/", loc
        browser("off")

    with subtest("with sharing on, two people use the chat page"):
        switch(None)
        saved(True)
        box.wait_for_unit("losos-chat.service")
        box.wait_until_succeeds("curl -sf http://127.0.0.1:8008/_matrix/client/versions")
        assert "chat" in json.loads(call("/api/me", matus)[1])["shared"]
        browser("on")
  '';
}
