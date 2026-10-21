# nixos-test-vms config for LosOS chat (modules/chat.nix, backend/src/chat.rs,
# admin-ui/app/src/chat/).
#
# The box runs LosOS Login, lososd, nginx and Dendrite as they ship; a laptop
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
#   * Dendrite runs as `shared`, keeps everything in the shared data domain,
#     and only root, nginx and `shared` may open its port;
#   * messages, uploads and the thread list (which lososd answers, because
#     Dendrite has none) work through nginx;
#   * the chat page itself: two people in two browsers, an end-to-end
#     encrypted conversation, a reply, a reaction, a thread, a picture and an
#     edit, and a recovery key that lets a second device read the past.
#
# The browser is Chromium under Playwright in the test driver, talking to the
# box through a SOCKS proxy on the laptop, so the address bar says
# https://mattbox.local/ and LosOS Login's cookie is the box's own. Every
# screenshot lands in the test's output: `nix build
# .#checks.x86_64-linux.losos-chat` and look in result/shots/. The launch
# film's states are numbered there with a manifest.json; the pull request's
# pictures are under result/shots/pr/.
#
# LosOS cloud and LosOS Git are stand-ins, as in nix/tests/identity.nix: the
# claim asks LosOS cloud one question, and nothing here is about either app.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # The admin UI as it ships, without the Lab page, which this test never
  # opens and whose WebAssembly takes longer to build than the test to run.
  ui = lososPkgs.losos-admin-ui.overrideAttrs (_: {
    pname = "losos-admin-ui-without-lab";
    preBuild = ''
      cat > vite.chat-test.config.ts <<'EOF'
      import base from "./vite.config";
      import { join } from "node:path";
      import { fileURLToPath } from "node:url";
      const root = fileURLToPath(new URL(".", import.meta.url));
      const config = base as Record<string, any>;
      config.build.rolldownOptions.input = {
        main: join(root, "index.html"),
        chat: join(root, "chat/index.html"),
        manage: join(root, "manage/index.html"),
      };
      export default config;
      EOF
    '';
    buildPhase = ''
      runHook preBuild
      npx vite build --config vite.chat-test.config.ts
      runHook postBuild
    '';
  });

  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';
  standIn =
    name:
    pkgs.hello
    // {
      imageName = name;
      imageTag = "test";
    };

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

  extraPythonPackages = p: [ p.playwright ];

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
          ../../modules/workloads.nix
          ../../modules/chat.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          admin.ui = ui;
          nextcloud.mode = "container";
          forgejo.enable = true;
          forgejo.mode = "container";
          workloads.nextcloudImage = standIn "losos-nextcloud-stand-in";
          workloads.forgejoImage = standIn "losos-forgejo-stand-in";
          # The shared user's side of the box: chat's gate.
          sharingMyStorage = true;
          cluster.enable = true;
        };
        # The same box, sharing switched off; the test switches to it and
        # back (switch-to-configuration, as an Apply would).
        specialisation.unshared.configuration.losos.sharingMyStorage = lib.mkForce false;

        # configuration.nix's two data users, without the rest of it.
        users.users.shared = {
          isNormalUser = true;
          uid = 1001;
          group = "shared";
          homeMode = "700";
        };
        users.groups.shared.gid = 1001;

        systemd.services.lososd.path = lib.mkForce [
          stubOcc
          pkgs.coreutils
        ];
        systemd.services.lososd.environment.LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
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
    import time
    import urllib.parse

    HOST = "mattbox.local"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA_PASSWORD = "Eva-Long-Password-3"
    PEOPLE = "/var/lib/losos-identity/users.yml"
    OUT = os.environ.get("out", os.getcwd())
    SHOTS = f"{OUT}/shots"
    os.makedirs(f"{SHOTS}/film", exist_ok=True)
    os.makedirs(f"{SHOTS}/pr", exist_ok=True)

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("dendrite.service")
    box.wait_for_open_port(443)
    box.wait_for_open_port(8008)
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

    with subtest("the claim makes Matus, and an administrator adds Eva"):
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
        code, body = call(
            "/api/people",
            matus,
            method="POST",
            data={"name": "eva", "displayName": "Eva", "email": "eva@mattbox.local",
                  "password": EVA_PASSWORD, "groups": []},
        )
        assert code == 200, (code, body)
        box.wait_until_succeeds(f"grep -q eva {PEOPLE}")
        time.sleep(5)
        eva = sign_in("eva", EVA_PASSWORD)

    with subtest("Dendrite runs as shared, inside the shared data domain"):
        assert box.succeed("ps -o user= -C dendrite").split() == ["shared"]
        assert box.succeed("stat -c '%U %a' /home/shared/data/chat").strip() == "shared 700"
        box.wait_until_succeeds("test -s /home/shared/data/chat/roomserver.db")
        assert "shared" in box.succeed("stat -c '%U' /home/shared/data/chat/roomserver.db")
        # Only root, nginx and shared may open its port on loopback.
        box.succeed("curl -sf http://127.0.0.1:8008/_matrix/client/versions")
        box.fail("runuser -u nobody -- curl -sf -m 5 http://127.0.0.1:8008/_matrix/client/versions")
        # Nothing but loopback.
        laptop.fail(f"curl -sf -m 5 http://{HOST}:8008/_matrix/client/versions")

    with subtest("there are no chat passwords: Dendrite refuses every other sign-in"):
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
        # The same device comes back when the page asks for it.
        again = json.loads(call("/api/chat/session", matus, method="POST",
                                data={"device": m["deviceId"]})[1])
        assert again["deviceId"] == m["deviceId"], (again, m)
        # Everyone on the box can be found.
        code, body = call("/api/chat/people", matus)
        people = {p["userId"]: p["displayName"] for p in json.loads(body)["people"]}
        assert set(people) == {m["userId"], e["userId"]}, people
        assert people[e["userId"]] == "Eva", people
        code, body = call("/api/me", matus)
        assert json.loads(body)["shared"] == ["chat"], body

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
        # Dendrite has no thread list; lososd answers it from Dendrite's tables.
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
        assert box.succeed("find /home/shared/data/chat/media -type f | wc -l").strip() != "0"

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

    # The browser. Everything below drives the chat page as a person would.
    os.environ["PLAYWRIGHT_BROWSERS_PATH"] = "${browsers}"
    os.environ["PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS"] = "true"
    os.environ["FONTCONFIG_FILE"] = "${fonts}"
    os.environ.setdefault("HOME", os.environ.get("TMPDIR", "/tmp"))
    from playwright.sync_api import sync_playwright

    pw = sync_playwright().start()
    browser = pw.chromium.launch(
        chromium_sandbox=False,
        proxy={"server": "socks5://127.0.0.1:1080"},
        args=["--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream"],
    )
    problems = []
    steps = []

    def person(name, password, pages=None):
        context = browser.new_context(
            viewport={"width": 1280, "height": 900},
            device_scale_factor=2,
            color_scheme="dark",
            locale="en-US",
            timezone_id="Europe/Bratislava",
            reduced_motion="reduce",
            ignore_https_errors=True,
            accept_downloads=True,
        )
        page = context.new_page()
        page.set_default_timeout(60000)
        page.on("pageerror", lambda err: problems.append(f"{name}: {err}"))
        page.on("console", lambda msg: msg.type == "error" and "Content Security Policy" in msg.text
                and "inline style" not in msg.text and problems.append(f"{name}: {msg.text}"))
        answer = context.request.post(
            f"https://{HOST}/auth/api/firstfactor",
            data=json.dumps({"username": name, "password": password}),
            headers={"Content-Type": "application/json"},
        )
        assert answer.ok, (name, answer.status, answer.text())
        return page

    def settle(page):
        page.evaluate("document.fonts.ready")
        page.evaluate("document.documentElement.style.caretColor = 'transparent'")
        # No spinner, no toast on its way in or out.
        page.wait_for_function("!document.querySelector('.animate-spin')", timeout=30000)
        time.sleep(1)

    def box_of(locator):
        b = locator.bounding_box()
        return {k: round(v) for k, v in b.items()} if b else None

    def film(page, slug, url, title, action="none", targets=None):
        """One state of the launch film: a PNG and its line in manifest.json."""
        settle(page)
        name = f"{len(steps) + 1:02d}-{slug}.png"
        page.screenshot(path=f"{SHOTS}/film/{name}")
        steps.append({"file": name, "url": url, "title": title, "action": action,
                      "targets": {k: box_of(v) for k, v in (targets or {}).items()}})

    def shot(page, name):
        settle(page)
        page.screenshot(path=f"{SHOTS}/pr/{name}.png")

    sidebar_chat = lambda page: page.get_by_role("link", name="Chat", exact=True)

    with subtest("with sharing off there is no chat anywhere"):
        switch("unshared")
        box.wait_until_fails("systemctl is-active dendrite.service")
        box.fail("systemctl cat dendrite.service")
        code, body = call("/api/chat", matus)
        assert code == 404, (code, body)
        assert json.loads(call("/api/me", matus)[1])["shared"] == []
        code, _ = call("/_matrix/client/v3/account/whoami", matus)
        assert code == 404, code
        code, _ = call("/_matrix/media/v3/config", matus)
        assert code == 404, code
        loc = curl(f"-o /dev/null -w '%{{redirect_url}}' https://{HOST}/chat/")
        assert loc == f"https://{HOST}/", loc
        owner = person("matus", PASSWORD)
        owner.goto(f"https://{HOST}/")
        owner.get_by_role("link", name="Overview", exact=True).wait_for()
        time.sleep(3)
        assert sidebar_chat(owner).count() == 0
        film(owner, "sharing-off", f"{HOST}/", "the admin pages with sharing off: no Chat")
        shot(owner, "sidebar-before")
        owner.context.close()

    with subtest("sharing on brings Chat into the sidebar"):
        switch(None)
        box.wait_for_unit("dendrite.service")
        box.wait_until_succeeds(f"curl -sf http://127.0.0.1:8008/_matrix/client/versions")
        owner = person("matus", PASSWORD)
        owner.goto(f"https://{HOST}/")
        sidebar_chat(owner).wait_for()
        film(owner, "sharing-on", f"{HOST}/", "the admin pages with sharing on: Chat in the sidebar",
             action="click", targets={"chat": sidebar_chat(owner)})
        shot(owner, "sidebar-after")
        sidebar_chat(owner).click()
        owner.wait_for_url(f"https://{HOST}/chat/**")
        owner.get_by_role("heading", name="LosOS chat").wait_for()

    with subtest("secure messages: a recovery key on the first device"):
        owner.get_by_role("button", name="Set up secure messages").click()
        owner.get_by_role("button", name="Make my recovery key").click()
        key = owner.get_by_test_id("recovery-key").text_content().strip()
        assert len(key.split()) >= 10, key
        shot(owner, "recovery-key")
        with owner.expect_download():
            owner.get_by_role("button", name="Download").click()
        owner.get_by_role("button", name="I saved it").click()
        owner.get_by_role("button", name="Set up secure messages").wait_for(state="detached")

    with subtest("Matus writes to Eva, end to end encrypted"):
        her = person("eva", EVA_PASSWORD)
        her.goto(f"https://{HOST}/chat/")
        her.get_by_role("heading", name="LosOS chat").wait_for()
        owner.get_by_role("button", name="Message someone").first.click()
        owner.get_by_role("textbox", name="Name or @name:server").fill("eva")
        owner.get_by_role("dialog").get_by_role("button", name=re.compile("Eva")).first.click()
        composer = owner.get_by_role("textbox", name="Message Eva")
        composer.wait_for()
        for text in ["Hi Eva, chat is up on the box.", "It's end to end encrypted, and nobody needs a chat password."]:
            composer.fill(text)
            owner.keyboard.press("Enter")
            owner.get_by_role("log").get_by_text(text).wait_for()
        her.get_by_role("button", name=re.compile("Matus")).first.click()
        her.get_by_role("button", name="Join", exact=True).click()
        her.get_by_role("log").get_by_text("Hi Eva, chat is up on the box.").wait_for()
        reply = her.get_by_role("textbox", name="Message Matus")
        reply.fill("Nice. Can we plan the holiday here?")
        her.keyboard.press("Enter")
        owner.get_by_role("log").get_by_text("Nice. Can we plan the holiday here?").wait_for()
        film(owner, "room-with-eva", f"{HOST}/chat/", "a conversation with Eva",
             action="type", targets={"composer": composer})
        shot(owner, "conversation")
        shot(her, "conversation-eva")

    with subtest("Matus types a reply, word by word"):
        words = "Yes, I'll make a room for it tonight.".split(" ")
        typed = ""
        composer.click()
        for i, word in enumerate(words):
            typed = word if i == 0 else f"{typed} {word}"
            composer.fill(typed)
            film(owner, f"typing-{i + 1}", f"{HOST}/chat/", f"typing: {typed}",
                 action="type", targets={"composer": composer})
        owner.keyboard.press("Enter")
        owner.get_by_role("log").get_by_text(typed).wait_for()
        her.get_by_role("log").get_by_text(typed).wait_for()
        film(owner, "sent", f"{HOST}/chat/", "the reply sent")

    with subtest("a reaction, a thread, a picture and an edit"):
        mine = her.get_by_role("log").get_by_text("Hi Eva, chat is up on the box.", exact=True)
        mine.hover()
        her.get_by_role("button", name="React", exact=True).click()
        her.get_by_role("menuitem", name="👍").click()
        owner.get_by_role("log").get_by_role("button", name=re.compile("👍")).wait_for()
        hers = owner.get_by_role("log").get_by_text("Nice. Can we plan the holiday here?")
        hers.hover()
        owner.get_by_role("button", name="Reply in a thread").first.click()
        owner.get_by_role("textbox", name="Reply in the thread").fill("Somewhere with a lake.")
        owner.keyboard.press("Enter")
        owner.get_by_label("Thread messages").get_by_text("Somewhere with a lake.").wait_for()
        owner.get_by_label("Thread messages").get_by_text("Sending…").wait_for(state="detached")
        shot(owner, "thread")
        her.get_by_role("log").get_by_role("button", name=re.compile("1 reply")).wait_for()
        her.get_by_role("button", name="Threads", exact=True).click()
        her.get_by_label("Threads in this room").get_by_text("Somewhere with a lake.").wait_for()
        shot(her, "thread-list")
        her.get_by_role("button", name="Close threads").click()
        owner.get_by_role("main").locator("input[type=file]").first.set_input_files("${picture}")
        her.get_by_role("log").locator("img[src^='blob:']").last.wait_for()
        mine = owner.get_by_role("log").get_by_text("Hi Eva, chat is up on the box.", exact=True)
        mine.hover()
        owner.get_by_role("button", name="Edit", exact=True).first.click()
        owner.get_by_role("textbox", name="Message Eva").fill("Hi Eva, chat is up on mattbox.")
        owner.keyboard.press("Enter")
        her.get_by_role("log").get_by_text("Hi Eva, chat is up on mattbox.").wait_for()
        her.get_by_role("log").get_by_text("(edited)").wait_for()
        time.sleep(2)
        shot(her, "picture-and-edit")

    with subtest("a second device reads the past with the recovery key"):
        laptop_two = person("matus", PASSWORD)
        laptop_two.goto(f"https://{HOST}/chat/")
        laptop_two.get_by_role("heading", name="LosOS chat").wait_for()
        laptop_two.get_by_role("button", name=re.compile("Eva")).first.click()
        laptop_two.get_by_role("log").get_by_text(re.compile("no key for this message")).first.wait_for()
        laptop_two.get_by_role("button", name="Unlock this device to read encrypted messages").click()
        laptop_two.get_by_role("textbox", name="Recovery key").fill(key)
        laptop_two.get_by_role("button", name="Unlock", exact=True).click()
        laptop_two.get_by_role("log").get_by_text("Hi Eva, chat is up on mattbox.").wait_for()
        shot(laptop_two, "second-device")

    with subtest("the page broke no rule of its policy"):
        assert problems == [], problems

    with open(f"{SHOTS}/film/manifest.json", "w") as f:
        json.dump({"feature": "chat", "viewport": {"width": 1280, "height": 900}, "dpr": 2,
                   "test": "losos-chat", "steps": steps}, f, indent=2)
    browser.close()
    pw.stop()
  '';
}
