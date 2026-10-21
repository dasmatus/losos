# nixos-test-vms config for calls in LosOS chat (modules/calls.nix,
# backend/src/calls/, admin-ui/app/src/calls/, admin-ui/app/src/chat/calls.tsx).
#
# The box runs LosOS Login, lososd, nginx, Dendrite and losos-calls as they
# ship. Three people call each other from the chat page in real browsers,
# and the SFU writes a copy of every frame it forwards, so the test can show
# what the box itself holds. What only a booted box can show:
#
#   * calls are there only while chat is and losos.calls.enable is on: with
#     either off there is no unit, no UDP port, no /calls/api/ and no call
#     button in a room;
#   * the SFU runs as `shared`, its socket only nginx may open, and
#     /calls/api/ wants a LosOS Login sign-in, with no spare-key bypass;
#   * a video call in a DM rings the other person, who answers, and both
#     see and hear each other through the box;
#   * every frame the SFU forwarded carries the encryption trailer and no
#     decoder can read it: the box never had the picture;
#   * a call of three in a room, someone leaving (the others take a new key
#     and carry on), and the timeline's Join bringing them back.
#
# The browsers are Chromium under Playwright in the test driver, as in
# nix/tests/chat.nix: the pages reach the box through a SOCKS proxy on the
# laptop, and the media through a UDP port forwarded to the box, which the
# SFU offers as the address to send to (the same arrangement as a port
# forwarded by a router). Screenshots land in result/shots/pr/.
#
# With `throughEdge` (the losos-calls-edge check) the box tunnels through an
# edge, as a box in a mesh does, and opens no port for calls: losos-calls
# keeps a QUIC connection to the edge's calls relay, and the forwarded port
# is the edge's. The same script then shows that the box's firewall has no
# media port, that the edge took the box's media on one, and that the
# calls still work and the frames are still unreadable.
{
  pkgs,
  throughEdge ? false,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # The admin UI as it ships, without the Lab page (as nix/tests/chat.nix).
  ui = lososPkgs.losos-admin-ui.overrideAttrs (_: {
    pname = "losos-admin-ui-without-lab";
    preBuild = ''
      cat > vite.calls-test.config.ts <<'EOF'
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
      npx vite build --config vite.calls-test.config.ts
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
  tap = "/var/lib/losos-calls-tap";
  mediaPort = 7882;

  # The box's edge token, as nix/tests/edge-vm.nix provisions it.
  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  bootstrapTokenValue = "test-bootstrap-0123456789abcdef0123";
  proxyToken = "/var/secrets/losos-proxy-token";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
      "f ${bootstrapToken} 0600 root root - ${bootstrapTokenValue}"
    ];
  };
  # The driver's browsers send to the forwarded port on their own loopback;
  # it arrives at the forwarding machine's user-mode address.
  forwarded = [
    {
      from = "host";
      proto = "udp";
      host.port = mediaPort;
      guest.port = mediaPort;
    }
  ];
in

pkgs.testers.nixosTest {
  name = if throughEdge then "losos-calls-edge" else "losos-calls";

  extraPythonPackages = p: [
    # Playwright for Python from a git checkout of its tag: the release
    # tarball nixpkgs pins no longer matches its hash.
    (p.playwright.overridePythonAttrs (_: {
      src = pkgs.fetchgit {
        name = "source";
        url = "https://github.com/microsoft/playwright-python";
        rev = "refs/tags/v1.63.0";
        hash = "sha256-RwIn+0EcHnStjORVFmT7gp4bGjl+qer1FgtI3+aPF2w=";
      };
    }))
  ];

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
          ../../modules/calls.nix
          ../../modules/proxy.nix
          secretFiles
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
          # The shared user's side of the box: chat's gate, and so calls'.
          sharingMyStorage = true;
          cluster.enable = true;
          calls.udpPort = mediaPort;
          proxy = lib.mkIf throughEdge {
            enable = true;
            edgeRatholeEndpoint = "edge:2333";
            registrarUrl = "http://edge:8443";
            hostname = "mattbox.losos.cfd";
            applianceId = "mattbox";
            tokenFile = proxyToken;
            bootstrapTokenFile = bootstrapToken;
            heartbeatInterval = "3s";
            registrar.package = lososPkgs.losos-registrar;
          };
        };
        lososInternal = {
          callsTap = tap;
          callsAddresses = lib.optional (!throughEdge) "10.0.2.15=127.0.0.1";
        };
        # The same box with calls switched off, and with sharing off; the
        # test switches to each and back (switch-to-configuration, as an
        # Apply would).
        specialisation.nocalls.configuration.losos.calls.enable = lib.mkForce false;
        specialisation.unshared.configuration.losos.sharingMyStorage = lib.mkForce false;

        # lososd takes the box's name for the claim's Host check from here.
        networking.hostName = lib.mkForce "mattbox";

        users.users.shared = {
          isNormalUser = true;
          uid = 1001;
          group = "shared";
          homeMode = "700";
        };
        users.groups.shared.gid = 1001;

        # curl for the edge scan's /health probe.
        systemd.services.lososd.path = lib.mkForce [
          stubOcc
          pkgs.coreutils
          pkgs.curl
          # lososd starts and stops the tunnel units with systemctl.
          pkgs.systemd
        ];
        systemd.services.lososd.environment.LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
        environment.systemPackages = [
          pkgs.jq
          pkgs.curl
          pkgs.iproute2
        ];
        virtualisation = {
          memorySize = 2048;
          cores = 2;
          vlans = [ 1 ];
          forwardPorts = lib.optionals (!throughEdge) forwarded;
        };
      };

    # A computer on the LAN whose SOCKS proxy carries the pages' traffic.
    #
    # (The edge, with throughEdge, is added below.)
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
        environment.systemPackages = [ pkgs.curl ];
        virtualisation.vlans = [ 1 ];
      };
  }
  // lib.optionalAttrs throughEdge {
    # The edge the box tunnels through, with its calls relay. Its media
    # port is the one forwarded to the driver's browsers.
    edge =
      { ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
          secretFiles
        ];
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };
        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.cfd";
          registrarApiBind = "::";
          registrarApiPort = 8443;
          ratholeBindPort = 2333;
          ratholePortRange = "50000-50010";
          reconcileInterval = "2s";
          bootstrapTokenFile = bootstrapToken;
          tenants.mattbox = {
            hostname = "mattbox.losos.cfd";
            tokenFile = proxyToken;
          };
          calls.relay = {
            mediaPorts = "${toString mediaPort}-${toString (mediaPort + 9)}";
            addresses = [ "10.0.2.15=127.0.0.1" ];
          };
        };
        environment.systemPackages = [ pkgs.iproute2 ];
        virtualisation = {
          memorySize = 1024;
          cores = 2;
          vlans = [ 1 ];
          forwardPorts = forwarded;
        };
      };
  };

  testScript = ''
    import json
    import os
    import re
    import struct
    import subprocess
    import tarfile
    import time
    import urllib.parse

    HOST = "mattbox.local"
    PASSWORDS = {
        "matus": "Correct-Horse-Battery-9",
        "eva": "Eva-Long-Password-3",
        "jan": "Jan-Long-Password-5",
    }
    PEOPLE = "/var/lib/losos-identity/users.yml"
    OUT = os.environ.get("out", os.getcwd())
    SHOTS = f"{OUT}/shots/pr"
    os.makedirs(SHOTS, exist_ok=True)

    # The edge node, with throughEdge; looked up so the script reads the
    # same without one.
    EDGE = globals().get("edge")

    start_all()
    if EDGE is not None:
        EDGE.wait_for_unit("losos-registrar.service")
        # lososd starts the tunnel once its scan finds the edge.
        box.wait_until_succeeds("systemctl is-active losos-rathole-client.service", timeout=300)
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("dendrite.service")
    box.wait_for_unit("losos-calls.service")
    box.wait_for_open_port(443)
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

    def sign_in(name):
        body = json.dumps({"username": name, "password": PASSWORDS[name]})
        head = curl(
            f"-D - -o /dev/null https://{HOST}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def switch(to):
        """Switch the running box, as an Apply's rebuild would."""
        path = "/run/booted-system" if to is None else f"/run/booted-system/specialisation/{to}"
        box.succeed(f"{path}/bin/switch-to-configuration test >&2")
        box.wait_for_unit("lososd.service")
        box.wait_for_unit("nginx.service")

    with subtest("Matus claims the box and adds Eva and Jan"):
        code, body = call("/api/setup/claim", method="POST",
                          data={"name": "matus", "password": PASSWORDS["matus"]})
        assert code == 200, (code, body)
        spare = json.loads(body)["token"]
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)
        box.wait_until_succeeds(f"grep -q \"domain: '{HOST}'\" /run/losos-identity/session.yml")
        matus = sign_in("matus")
        for name in ("eva", "jan"):
            code, body = call("/api/people", matus, method="POST",
                              data={"name": name, "displayName": name.title(),
                                    "email": f"{name}@{HOST}", "password": PASSWORDS[name],
                                    "groups": []})
            assert code == 200, (code, body)
            box.wait_until_succeeds(f"grep -q {name} {PEOPLE}")
        time.sleep(5)

    with subtest("the SFU runs as shared, and only LosOS Login reaches it"):
        assert box.succeed("ps -o user= -C losos-calls").split() == ["shared"]
        assert box.succeed("stat -c '%a %U %G' /run/losos-calls/http.sock").strip() == "660 shared losos-calls"
        box.fail("runuser -u nobody -- curl -sf --unix-socket /run/losos-calls/http.sock http://x/calls/api/health")
        if EDGE is not None:
            # The box opens no port for calls; the edge took one for it,
            # and holds it for the box's QUIC connection.
            box.fail("ss -uln | grep -q ':${toString mediaPort} '")
            box.fail("iptables-save | grep -q 'dport ${toString mediaPort}'")
            EDGE.wait_until_succeeds("ss -uln | grep -q ':${toString mediaPort} '")
            EDGE.wait_until_succeeds("journalctl -u losos-registrar | grep -q 'mattbox takes media'")
            box.succeed("journalctl -u losos-calls | grep -q 'media through the edge'")
        else:
            box.succeed("ss -uln | grep -q ':${toString mediaPort} '")
        # Signed out, and with the spare key: no.
        assert call("/calls/api/health")[0] == 401
        assert call("/calls/api/health", extra=f"-H 'Authorization: Bearer {spare}'")[0] == 401
        code, body = call("/calls/api/health", matus)
        assert code == 200 and body == "ok", (code, body)
        # A call is made with an OpenID token the homeserver vouches for,
        # never with a made-up one.
        code, body = call("/calls/api/calls", matus, method="POST",
                          data={"openid": {"access_token": "made-up", "matrix_server_name": HOST},
                                "room": "!r:mattbox.local", "invited": [], "video": True})
        assert code == 401, (code, body)

    # The browsers.
    os.environ["PLAYWRIGHT_BROWSERS_PATH"] = "${browsers}"
    os.environ["PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS"] = "true"
    os.environ["FONTCONFIG_FILE"] = "${fonts}"
    os.environ.setdefault("HOME", os.environ.get("TMPDIR", "/tmp"))
    from playwright.sync_api import sync_playwright

    pw = sync_playwright().start()
    browser = pw.chromium.launch(
        chromium_sandbox=False,
        proxy={"server": "socks5://127.0.0.1:1080"},
        args=[
            "--use-fake-ui-for-media-stream",
            "--use-fake-device-for-media-stream",
            # The SFU's offered address is the driver's own loopback.
            "--allow-loopback-in-peer-connection",
        ],
    )
    problems = []

    def person(name):
        context = browser.new_context(
            viewport={"width": 1280, "height": 860},
            device_scale_factor=2,
            color_scheme="dark",
            locale="en-US",
            timezone_id="Europe/Bratislava",
            reduced_motion="reduce",
            ignore_https_errors=True,
        )
        page = context.new_page()
        page.set_default_timeout(90000)
        def on_error(err) -> None:
            problems.append(f"{name}: {err}")

        def on_console(msg) -> None:
            text = msg.text
            if msg.type == "error" and "Content Security Policy" in text and "inline style" not in text:
                problems.append(f"{name}: {text}")

        page.on("pageerror", on_error)
        page.on("console", on_console)
        seen = []
        page.on("response", lambda r: r.status >= 400 and seen.append(f"{r.status} {r.request.method} {r.url}"))
        page.on("requestfailed", lambda r: seen.append(f"failed {r.method} {r.url}: {r.failure}"))
        answer = context.request.post(
            f"https://{HOST}/auth/api/firstfactor",
            data=json.dumps({"username": name, "password": PASSWORDS[name]}),
            headers={"Content-Type": "application/json"},
        )
        assert answer.ok, (name, answer.status, answer.text())
        page.goto(f"https://{HOST}/chat/")
        try:
            page.get_by_role("heading", name="LosOS chat").wait_for()
        except Exception:
            print(f"{name} sees {page.url}:\n{page.inner_text('body')[:3000]}\n{problems}")
            print("\n".join(seen[-60:]))
            print(box.execute("journalctl -u lososd -u dendrite -u authelia-losos --since -4min --no-pager | tail -n 120")[1])
            raise
        return page

    def settle(page):
        page.evaluate("document.fonts.ready")
        page.evaluate("document.documentElement.style.caretColor = 'transparent'")
        page.wait_for_function("!document.querySelector('.animate-spin')", timeout=30000)
        time.sleep(1)

    def shot(page, name):
        settle(page)
        page.screenshot(path=f"{SHOTS}/{name}.png")

    def opened(page):
        return int(page.locator("[data-e2ee]").get_attribute("data-opened") or "0")

    def live(page, others, frames=60):
        """The call view shows `others` other people, and frames from them
        are arriving and opening."""
        view = page.locator("[data-call-view]")
        view.wait_for()
        page.wait_for_function(
            f"document.querySelectorAll('[data-call-view] [data-pid]').length === {others + 1}",
            timeout=90000,
        )
        page.wait_for_function(
            f"Number(document.querySelector('[data-e2ee]')?.dataset.opened ?? 0) > {frames}",
            timeout=120000,
        )
        # A remote picture is playing, not only arriving.
        page.wait_for_function(
            "[...document.querySelectorAll('[data-call-view] video')]"
            ".filter(v => !v.muted && v.videoWidth > 0 && v.currentTime > 1).length >= " + str(others),
            timeout=90000,
        )
        assert page.locator("[data-e2ee]").get_attribute("data-refused") == "0"

    def room_button(page, name):
        return page.get_by_role("button", name=re.compile(name)).first

    with subtest("Matus writes to Eva; with calls off the room has no call buttons"):
        switch("nocalls")
        box.wait_until_fails("systemctl is-active losos-calls.service")
        box.fail("systemctl cat losos-calls.service")
        # Nothing answers but the admin page's index.html.
        assert call("/calls/api/health", matus)[1] != "ok"
        box.fail("ss -uln | grep -q ':${toString mediaPort} '")
        owner = person("matus")
        her = person("eva")
        owner.get_by_role("button", name="Message someone").first.click()
        owner.get_by_role("textbox", name="Name or @name:server").fill("eva")
        owner.get_by_role("dialog").get_by_role("button", name=re.compile("Eva")).first.click()
        composer = owner.get_by_role("textbox", name="Message Eva")
        composer.fill("Free for a quick call about the holiday?")
        owner.keyboard.press("Enter")
        owner.get_by_role("log").get_by_text("Free for a quick call about the holiday?").wait_for()
        room_button(her, "Matus").click()
        her.get_by_role("button", name="Join", exact=True).click()
        her.get_by_role("log").get_by_text("Free for a quick call about the holiday?").wait_for()
        her.get_by_role("textbox", name="Message Matus").fill("Sure, call me.")
        her.keyboard.press("Enter")
        owner.get_by_role("log").get_by_text("Sure, call me.").wait_for()
        time.sleep(3)
        assert owner.get_by_role("button", name="Video call").count() == 0
        shot(owner, "room-before")

    with subtest("with calls on, Matus video calls Eva and her browser rings"):
        switch(None)
        box.wait_for_unit("losos-calls.service")
        box.wait_until_succeeds("test -S /run/losos-calls/http.sock")
        for page in (owner, her):
            page.reload()
            page.get_by_role("heading", name="LosOS chat").wait_for()
        room_button(owner, "Eva").click()
        room_button(her, "Matus").click()
        owner.get_by_role("button", name="Video call").wait_for()
        shot(owner, "room-after")
        owner.get_by_role("button", name="Video call").click()
        ring = her.locator("[data-ringing]")
        ring.wait_for()
        assert "Matus" in ring.text_content(), ring.text_content()
        shot(her, "ringing")
        ring.get_by_role("button", name="Answer", exact=True).click()

    with subtest("they see each other, on frames only they can open"):
        live(owner, 1)
        live(her, 1)
        shot(owner, "call-two")
        shot(her, "call-two-eva")
        # The view says so, and says what it counted.
        owner.locator("[data-e2ee]").hover()
        owner.get_by_text(re.compile(r"frames sent encrypted")).wait_for()
        shot(owner, "call-encrypted")

    with subtest("the SFU's copy of every frame is ciphertext"):
        box.succeed("ls ${tap}/video-*.ivf ${tap}/audio-*.bin")
        box.succeed("tar -C ${tap} -cf /tmp/tap.tar .")
        box.copy_from_machine("/tmp/tap.tar", "")
        here = os.path.join(OUT, "tap.tar")
        tapdir = os.path.join(os.environ.get("TMPDIR", "/tmp"), "tap")
        os.makedirs(tapdir, exist_ok=True)
        with tarfile.open(here) as t:
            t.extractall(tapdir)
        os.remove(here)
        frames = 0
        for f in sorted(n for n in os.listdir(tapdir) if n.endswith((".ivf", ".bin"))):
            data = open(os.path.join(tapdir, f), "rb").read()
            chunks = []
            if f.endswith(".ivf"):
                assert data[:4] == b"DKIF", f
                at = 32
                while at + 12 <= len(data):
                    (size,) = struct.unpack_from("<I", data, at)
                    chunks.append(data[at + 12:at + 12 + size])
                    at += 12 + size
            else:
                at = 0
                while at + 4 <= len(data):
                    (size,) = struct.unpack_from("<I", data, at)
                    chunks.append(data[at + 4:at + 4 + size])
                    at += 4 + size
            # Every frame ends in the trailer: counter, key index, 0x4C.
            for c in chunks:
                assert len(c) > 22 and c[-1] == 0x4C, (f, len(c), c[-8:].hex())
            frames += len(chunks)
            if f.endswith(".ivf") and len(chunks) > 10:
                run = subprocess.run(
                    ["${pkgs.ffmpeg-headless}/bin/ffmpeg", "-v", "error", "-i", os.path.join(tapdir, f),
                     "-f", "null", "-"],
                    capture_output=True, text=True,
                )
                errors = len([l for l in run.stderr.splitlines() if l.strip()])
                assert run.returncode != 0 or errors >= len(chunks) * 0.9, (f, len(chunks), errors)
        assert frames > 200, frames

    with subtest("a call of three in a room; Jan leaves, and comes back with Join"):
        owner.get_by_role("button", name="Leave the call").click()
        her.locator("[data-call-view]").wait_for(state="detached", timeout=60000)
        owner.locator("[data-call-view]").wait_for(state="detached")
        # A room for the three of them, made over the API.
        cookies = {n: sign_in(n) for n in ("matus", "eva", "jan")}
        tokens = {n: json.loads(call("/api/chat/session", c, method="POST", data={})[1])["accessToken"]
                  for n, c in cookies.items()}
        code, body = call("/_matrix/client/v3/createRoom", cookies["matus"], method="POST",
                          token=tokens["matus"],
                          data={"name": "Holiday", "preset": "private_chat",
                                "invite": [f"@eva:{HOST}", f"@jan:{HOST}"]})
        assert code == 200, (code, body)
        room = urllib.parse.quote(json.loads(body)["room_id"])
        for n in ("eva", "jan"):
            code, body = call(f"/_matrix/client/v3/rooms/{room}/join", cookies[n], method="POST",
                              token=tokens[n], data={})
            assert code == 200, (code, body)
        jan = person("jan")
        room_button(owner, "Holiday").click()
        room_button(her, "Holiday").click()
        room_button(jan, "Holiday").click()
        owner.get_by_role("button", name="Video call").click()
        for page in (her, jan):
            page.locator("[data-ringing]").get_by_role("button", name="Answer", exact=True).click()
        for page in (owner, her, jan):
            live(page, 2)
        shot(owner, "call-three")
        jan.get_by_role("button", name="Leave the call").click()
        jan.locator("[data-call-view]").wait_for(state="detached")
        for page in (owner, her):
            page.wait_for_function(
                "document.querySelectorAll('[data-call-view] [data-pid]').length === 2", timeout=60000)
        # After the leave the others rotate to a key Jan never had, and
        # still open each other's frames.
        before = {id(p): opened(p) for p in (owner, her)}
        time.sleep(8)
        for page in (owner, her):
            assert opened(page) > before[id(page)] + 30, (opened(page), before[id(page)])
            assert page.locator("[data-e2ee]").get_attribute("data-refused") == "0"
        notice = jan.locator("[data-call-notice]").last
        notice.get_by_role("button", name="Join", exact=True).wait_for()
        shot(jan, "timeline-join")
        notice.get_by_role("button", name="Join", exact=True).click()
        for page in (owner, her, jan):
            live(page, 2, frames=0)

    with subtest("hanging up ends the call for the room"):
        for page in (jan, her, owner):
            page.get_by_role("button", name="Leave the call").click()
            page.locator("[data-call-view]").wait_for(state="detached")
        owner.get_by_role("log").get_by_text("The call ended").last.wait_for()
        jan.locator("[data-call-notice] button").wait_for(state="detached", timeout=30000)

    with subtest("with sharing off there are no calls at all"):
        switch("unshared")
        box.wait_until_fails("systemctl is-active losos-calls.service")
        box.fail("systemctl cat losos-calls.service")
        box.fail("ss -uln | grep -q ':${toString mediaPort} '")
        # Nothing answers but the admin page's index.html.
        assert call("/calls/api/health", matus)[1] != "ok"

    with subtest("the pages broke no rule of their policy"):
        assert problems == [], problems

    browser.close()
    pw.stop()
  '';
}
