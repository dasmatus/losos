# nixos-test-vms config for calls in LosOS chat (modules/calls.nix,
# backend/src/calls/, admin-ui/app/src/calls/, admin-ui/app/src/chat/calls.tsx).
#
# The box runs LosOS Login, lososd, nginx, Synapse and losos-calls as they
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

  # The admin UI as it ships, without LosOS Diagrams' WebAssembly (as
  # nix/tests/chat.nix): stand-ins take the place of the three packages, so
  # the bundle builds without Bevy, and only /diagrams, which no step here
  # opens, would fail.
  ui = lososPkgs.losos-admin-ui.overrideAttrs (_: {
    pname = "losos-admin-ui-without-lab";
    preBuild = ''
      for f in core-pkg/losos_lab_core virt-pkg/losos_lab_virt \
               render-pkg/webgpu/losos_lab_render render-pkg/webgl2/losos_lab_render; do
        mkdir -p "src/lab/$(dirname "$f")"
        echo 'export default function init() { throw new Error("not in this build"); } export class Lab {}' > "src/lab/$f.js"
        : > "src/lab/''${f}_bg.wasm"
      done
      cat > vite.calls-test.config.ts <<'EOF'
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
      bun --bun vite build --config vite.calls-test.config.ts
      bun --bun vite build --config vite.guest.config.ts
      runHook postBuild
    '';
  });

  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  # The build sandbox has no /etc/fonts/conf.d, so nothing tells fontconfig
  # what the generic names are, and the app's system-ui stack drew in a
  # serif. Name them here.
  fonts = pkgs.writeText "calls-fonts.conf" ''
    <?xml version="1.0"?>
    <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
    <fontconfig>
      <include>${
        pkgs.makeFontsConf {
          fontDirectories = [
            pkgs.dejavu_fonts
            pkgs.noto-fonts-color-emoji
          ];
        }
      }</include>
      ${lib.concatMapStrings
        (family: ''
          <alias binding="same"><family>${family}</family><prefer><family>DejaVu Sans</family><family>Noto Color Emoji</family></prefer></alias>
        '')
        [
          "system-ui"
          "ui-sans-serif"
          "sans-serif"
          "sans"
        ]
      }
      <alias binding="same"><family>serif</family><prefer><family>DejaVu Serif</family></prefer></alias>
      ${lib.concatMapStrings
        (family: ''
          <alias binding="same"><family>${family}</family><prefer><family>DejaVu Sans Mono</family></prefer></alias>
        '')
        [
          "ui-monospace"
          "monospace"
        ]
      }
    </fontconfig>
  '';
  # Each person's camera: a drawn stand-in, a person in a room of their own
  # (calls-faces.py), with a slow drift so the picture moves like a webcam's,
  # looped by Chromium's fake device. A room behind the person is what the
  # black background is for.
  camera =
    {
      name,
      wall,
      floor,
      skin,
      hair,
      shirt,
      style,
      room,
    }:
    pkgs.runCommand "camera-${name}.y4m" { } ''
      ${pkgs.python3}/bin/python3 -I ${./calls-faces.py} ${pkgs.imagemagick}/bin/magick still.png \
        '${wall}' '${floor}' '${skin}' '${hair}' '${shirt}' ${style} ${room}
      ${pkgs.ffmpeg-headless}/bin/ffmpeg -v error -loop 1 -i still.png \
        -vf "scale=680:510,crop=640:480:'20+12*sin(2*PI*t/4)':'15+6*sin(2*PI*t/2)',format=yuv420p" \
        -t 4 -r 15 $out
    '';
  cameras = {
    matus = camera {
      name = "matus";
      wall = "#d9cbb8";
      floor = "#8a6f55";
      skin = "#f0c7a5";
      hair = "#4a3022";
      shirt = "#2f5d8a";
      style = "short";
      room = "shelf";
    };
    eva = camera {
      name = "eva";
      wall = "#b9cfc4";
      floor = "#7d6a58";
      skin = "#e8b896";
      hair = "#a0452a";
      shirt = "#3f7d5a";
      style = "long";
      room = "plant";
    };
    jan = camera {
      name = "jan";
      wall = "#d8d2e4";
      floor = "#6f6a7d";
      skin = "#f3d1b5";
      hair = "#c9a35a";
      shirt = "#c4692e";
      style = "beard";
      room = "window";
    };
  };
  # The podcast's overlay: a transparent frame with a tag in one corner.
  overlay = pkgs.runCommand "podcast-overlay.png" { } ''
    ${pkgs.imagemagick}/bin/magick -size 1280x720 xc:none \
      -fill '#e8603c' -draw 'roundrectangle 930,28 1252,84 12,12' \
      -font ${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans-Bold.ttf \
      -fill white -pointsize 28 -annotate +952+67 'From mattbox' $out
  '';
  rtmpPort = 1935;
  tap = "/var/lib/losos-calls-tap";
  mediaPort = 7882;

  # The box's edge token, as nix/tests/edge-vm.nix provisions it.
  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  proxyToken = "/var/secrets/losos-proxy-token";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
    ];
  };
  # The edge is an open gateway with its owner's passport measured, so it
  # makes guest links (nix/tests/open-gateway.nix).
  openGateway = import ./open-gateway.nix { inherit pkgs lososPkgs; };
  # The driver's browsers send to the media port on their own loopback; it
  # arrives at the forwarding machine's user-mode address. QEMU's user-mode
  # network sends every reply from a guest UDP port to whoever wrote to it
  # last, so two browsers on one forwarded port take each other's packets.
  # Each browser's socket therefore gets a forwarded port of its own (the
  # driver's shim hands them out), each lands on a guest port of its own,
  # and the forwarding machine turns those into the media port with DNAT,
  # whose replies go back out of the guest port they came in on.
  peerPorts = 12;
  shimBase = 17882;
  peerBase = mediaPort + 100;
  forwarded = map (i: {
    from = "host";
    proto = "udp";
    host.port = shimBase + i;
    guest.port = peerBase + i;
  }) (lib.range 0 (peerPorts - 1));
  peerNat = {
    networking.firewall.extraCommands = ''
      iptables -w -t nat -A PREROUTING -p udp \
        --dport ${toString peerBase}:${toString (peerBase + peerPorts - 1)} \
        -j DNAT --to-destination :${toString mediaPort}
    '';
  };

  # The build sandbox has loopback only, and Chromium (since about 150)
  # gathers no ICE candidates on a computer with no other interface, so the
  # browsers would never send the SFU a packet. Each browser therefore runs
  # in a network namespace of its own with a veth pair beside loopback, and
  # this bridge, inside it, carries the SOCKS proxy and the media port out
  # to the driver over unix sockets in $LOSOS_BRIDGE (the filesystem is
  # shared, the network is not). Playwright drives the browser over pipes,
  # which cross namespaces as they are.
  browserBridge = pkgs.writeText "losos-calls-browser-bridge.py" ''
    import itertools
    import os
    import socket
    import sys
    import threading

    bridge, ready = sys.argv[1], sys.argv[2]


    def pipe(a, b):
        try:
            while data := a.recv(65536):
                b.sendall(data)
        except OSError:
            pass
        for s in (a, b):
            try:
                s.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass


    proxy = socket.socket()
    proxy.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    proxy.bind(("127.0.0.1", 1080))
    proxy.listen(64)


    def accept():
        while True:
            c, _ = proxy.accept()
            u = socket.socket(socket.AF_UNIX)
            try:
                u.connect(f"{bridge}/socks")
            except OSError:
                c.close()
                continue
            threading.Thread(target=pipe, args=(c, u), daemon=True).start()
            threading.Thread(target=pipe, args=(u, c), daemon=True).start()


    threading.Thread(target=accept, daemon=True).start()
    front = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    front.bind(("127.0.0.1", ${toString mediaPort}))
    open(ready, "w").close()


    # One thread per direction and socket: a browser that is slow to read
    # holds up its own packets only, never another browser's or its own
    # sending side.
    def back(up, peer):
        while True:
            try:
                front.sendto(up.recv(65535), peer)
            except OSError:
                pass


    ups = {}
    serial = itertools.count()
    while True:
        try:
            data, peer = front.recvfrom(65535)
            up = ups.get(peer)
            if up is None:
                up = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
                up.bind(f"{bridge}/p{os.getpid()}-{next(serial)}")
                up.connect(f"{bridge}/media")
                ups[peer] = up
                threading.Thread(target=back, args=(up, peer), daemon=True).start()
            up.send(data)
        except OSError:
            pass
  '';
  chromiumInOwnNetwork = pkgs.writeShellScript "chromium-in-own-network" ''
    exec ${pkgs.util-linux}/bin/unshare -rn ${pkgs.runtimeShell} -c '
      set -e
      ip=${pkgs.iproute2}/bin/ip
      $ip link set lo up
      $ip link add v0 type veth peer name v1
      $ip addr add 192.0.2.1/24 dev v0
      $ip link set v0 up
      $ip link set v1 up
      ready=$(${pkgs.coreutils}/bin/mktemp -u "$LOSOS_BRIDGE/ready.XXXXXX")
      "$LOSOS_PYTHON" ${browserBridge} "$LOSOS_BRIDGE" "$ready" &
      while [ ! -e "$ready" ]; do ${pkgs.coreutils}/bin/sleep 0.1; done
      exec "$LOSOS_CHROME" "$@"
    ' sh "$@"
  '';
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
      { config, lib, ... }:
      {
        imports = [
          (if throughEdge then { } else peerNat)
          ../../modules/options.nix
          ../../modules/containers.nix
          ../../modules/tls.nix
          ../../modules/daemon.nix
          ../../modules/identity.nix
          ../../modules/chat.nix
          ../../modules/calls.nix
          ../../modules/proxy.nix
          secretFiles
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          admin.ui = ui;
          git.enable = false;
          # The shared user's side of the box: chat's gate, and so calls'.
          sharingMyStorage = true;
          cluster.enable = true;
          calls.udpPort = mediaPort;
          proxy = lib.mkIf throughEdge {
            enable = true;
            edgeTunnelEndpoint = "edge:2333";
            registrarUrl = "http://edge:8443";
            hostname = "mattbox.losos.cfd";
            applianceId = "mattbox";
            tokenFile = proxyToken;
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
          pkgs.coreutils
          pkgs.curl
          # lososd starts and stops the tunnel units with systemctl.
          pkgs.systemd
        ];
        # ICE and DTLS in the log, for a call that does not connect. Only
        # where the unit exists: with sharing off this line alone would
        # make a unit with no ExecStart.
        systemd.services.losos-calls = lib.mkIf (config.losos.calls.enable && config.lososInternal.chat) {
          environment.RUST_LOG = "info,str0m=debug";
        };
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
        networking.extraHosts = ''
          ${nodes.box.networking.primaryIPAddress} mattbox.local
        ''
        # A guest's link names the box's public name, which the edge serves.
        + lib.optionalString throughEdge ''
          ${nodes.edge.networking.primaryIPAddress} mattbox.losos.cfd
        '';
        systemd.services.socks = {
          wantedBy = [ "multi-user.target" ];
          after = [ "network-online.target" ];
          wants = [ "network-online.target" ];
          serviceConfig = {
            ExecStart = "${pkgs.microsocks}/bin/microsocks -i 0.0.0.0 -p 1080";
            DynamicUser = true;
          };
        };
        networking.firewall.allowedTCPPorts = [
          1080
          rtmpPort
        ];
        virtualisation.forwardPorts = [
          {
            from = "host";
            host.port = 1080;
            guest.port = 1080;
          }
        ];
        # The RTMP server podcast mode streams to, as YouTube's would be.
        environment.systemPackages = [
          pkgs.curl
          pkgs.ffmpeg-headless
        ];
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
          peerNat
          (openGateway.edge "http://edge:8443")
        ];
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };
        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.cfd";
          registrarApiBind = "::";
          registrarApiPort = 8443;
          tunnelBindPort = 2333;
          tunnelPortRange = "50000-50010";
          reconcileInterval = "2s";
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
    import glob
    import json
    import os
    import re
    import struct
    import subprocess
    import tarfile
    import threading
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
    # The launch film's frames: one picture per state, numbered in the
    # order a person sees them, with the boxes the film may point at.
    FILM = f"{OUT}/shots/film"
    os.makedirs(FILM, exist_ok=True)
    film_steps = []
    # One window size for the film and the PR alike, at 2x so the film can
    # zoom in, and Matus's window recorded from his first page to the end;
    # `at` in the manifest is where each still falls in that clip.
    VIEW = {"width": 1280, "height": 900}
    clip_start = {}

    # The edge node, with throughEdge; looked up so the script reads the
    # same without one.
    EDGE = globals().get("edge")
    # The film is recorded by losos-calls alone: under emulation a 2x
    # recording of Matus's window costs the edge run, with a third machine
    # and its relay, more than it can spare.
    FILMING = EDGE is None

    start_all()
    if EDGE is not None:
        EDGE.wait_for_unit("losos-registrar.service")
        # lososd starts the tunnel once its scan finds the edge.
        box.wait_until_succeeds("systemctl is-active losos-tunnel-client.service", timeout=300)
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("losos-chat.service")
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
        # The claim names him by his sign-in name; he writes his own.
        code, body = call("/api/account", matus, method="POST",
                          data={"displayName": "Matus", "email": f"matus@{HOST}"})
        assert code == 200, (code, body)
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
        # The SFU takes the person LosOS Login signed in from nginx, and
        # nobody else: a Remote-User the client sends is no sign-in, and
        # beside a real one it is replaced.
        room = {"room": "!r:mattbox.local", "invited": [], "video": True}
        code, body = call("/calls/api/calls", method="POST", data=room,
                          extra="-H 'Remote-User: matus'")
        assert code == 401, (code, body)
        code, body = call("/calls/api/calls", matus, method="POST", data=room,
                          extra="-H 'Remote-User: eva'")
        assert code == 200, (code, body)
        made = json.loads(body)["call"]
        box.wait_until_succeeds(
            f"journalctl -u losos-calls --no-pager | grep -F '@matus:{HOST} started call {made}'",
            timeout=30,
        )

    # The browsers.
    os.environ["PLAYWRIGHT_BROWSERS_PATH"] = "${browsers}"
    os.environ["PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS"] = "true"
    os.environ["FONTCONFIG_FILE"] = "${fonts}"
    os.environ.setdefault("HOME", os.environ.get("TMPDIR", "/tmp"))
    from playwright.sync_api import sync_playwright

    import socket
    import sys
    import tempfile

    # The driver's end of each browser's bridge (browserBridge above).
    BRIDGE = tempfile.mkdtemp(prefix="br", dir=os.environ.get("TMPDIR", "/tmp"))

    def socks_relay():
        """The bridge's SOCKS connections, on to the laptop's proxy."""
        def pipe(a, b):
            try:
                while data := a.recv(65536):
                    b.sendall(data)
            except OSError:
                pass
            for s in (a, b):
                try:
                    s.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass

        listener = socket.socket(socket.AF_UNIX)
        listener.bind(f"{BRIDGE}/socks")
        listener.listen(64)
        while True:
            c, _ = listener.accept()
            try:
                u = socket.create_connection(("127.0.0.1", 1080))
            except OSError:
                c.close()
                continue
            threading.Thread(target=pipe, args=(c, u), daemon=True).start()
            threading.Thread(target=pipe, args=(u, c), daemon=True).start()

    def media_shim():
        """Each browser socket the bridges carry gets a forwarded port of its
        own (peerPorts in the Nix above), and the replies on it go back to
        that socket, each on a thread of its own so that one slow browser
        holds up nobody else."""
        front = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
        front.bind(f"{BRIDGE}/media")
        free = list(range(${toString shimBase}, ${toString (shimBase + peerPorts)}))
        ups = {}

        def back(up, peer):
            while True:
                try:
                    front.sendto(up.recv(65535), peer)
                    SHIM[peer] = (SHIM[peer][0], SHIM[peer][1] + 1)
                except OSError:
                    pass

        while True:
            try:
                data, peer = front.recvfrom(65535)
                up = ups.get(peer)
                if up is None:
                    if not free:
                        print(f"media shim: no port left for {peer}")
                        continue
                    up = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                    up.connect(("127.0.0.1", free.pop(0)))
                    ups[peer] = up
                    SHIM[peer] = (0, 0)
                    threading.Thread(target=back, args=(up, peer), daemon=True).start()
                up.send(data)
                SHIM[peer] = (SHIM[peer][0] + 1, SHIM[peer][1])
            except OSError:
                pass

    SHIM: dict[str, tuple[int, int]] = {}
    threading.Thread(target=socks_relay, daemon=True).start()
    threading.Thread(target=media_shim, daemon=True).start()
    os.environ["LOSOS_BRIDGE"] = BRIDGE
    os.environ["LOSOS_PYTHON"] = sys.executable
    os.environ["LOSOS_CHROME"] = next(iter(glob.glob(
        "${browsers}/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell")))

    def media_report():
        print("media shim, packets to the box and back per browser socket:",
              {str(k): v for k, v in SHIM.items()})
        for m in [box] + ([EDGE] if EDGE else []):
            print(m.execute("iptables -w -t nat -vnL PREROUTING; ss -uanp | grep -E '78|79'; "
                            "grep -E 'udp.*(78|79)[0-9]{2}' /proc/net/nf_conntrack | head -n 20")[1])
        print(box.execute("journalctl -u losos-calls --no-pager | grep -iE 'ice|dtls|candidate|stun' | tail -n 60")[1])

    pw = sync_playwright().start()
    # One browser per person, each with their own camera.
    CAMERAS = {
        "matus": "${cameras.matus}",
        "eva": "${cameras.eva}",
        "jan": "${cameras.jan}",
    }
    browsers = {
        name: pw.chromium.launch(
            executable_path="${chromiumInOwnNetwork}",
            chromium_sandbox=False,
            proxy={"server": "socks5://127.0.0.1:1080"},
            args=[
                "--use-fake-ui-for-media-stream",
                "--use-fake-device-for-media-stream",
                f"--use-file-for-fake-video-capture={y4m}",
                # The SFU's offered address is the driver's loopback, which
                # the bridge serves in the browser's own namespace.
                "--allow-loopback-in-peer-connection",
            ],
        )
        for name, y4m in CAMERAS.items()
    }
    problems = []
    responses = {}

    # The screen Matus shares: a headless browser has no desktop to pick,
    # so the page's getDisplayMedia draws a slide on a canvas instead.
    SLIDE = """
    navigator.mediaDevices.getDisplayMedia = async () => {
      const c = document.createElement('canvas');
      c.width = 1280; c.height = 720;
      const g = c.getContext('2d');
      let t = 0;
      const paint = () => {
        g.fillStyle = '#f4efe6'; g.fillRect(0, 0, 1280, 720);
        g.fillStyle = '#1d3557'; g.font = 'bold 68px DejaVu Sans';
        g.fillText('Holiday plan', 90, 150);
        g.font = '42px DejaVu Sans';
        ['Fly to Split on 3 July', 'Ferry to Hvar, two nights', 'Home by train through Zagreb']
          .forEach((line, i) => g.fillText('\\u2022  ' + line, 110, 280 + i * 90));
        g.fillStyle = '#e8603c'; g.fillRect(90, 640, ((t++ % 50) / 50) * 1100, 14);
      };
      paint(); setInterval(paint, 100);
      return c.captureStream(10);
    };
    """

    def person(name):
        browser = browsers[name]
        context = browser.new_context(
            viewport={"width": 1280, "height": 900},
            device_scale_factor=2,
            color_scheme="dark",
            locale="en-US",
            timezone_id="Europe/Bratislava",
            reduced_motion="reduce",
            ignore_https_errors=True,
            **({"record_video_dir": f"{FILM}/raw", "record_video_size": {"width": 2560, "height": 1800}} if name == "matus" and FILMING else {}),
        )
        if name == "matus":
            clip_start["matus"] = time.time()
        if name == "matus":
            context.add_init_script(SLIDE)
        page = context.new_page()
        page.set_default_timeout(90000)
        def on_error(err) -> None:
            problems.append(f"{name}: {err}")

        def on_console(msg) -> None:
            text = msg.text
            if msg.type == "error" and "Content Security Policy" in text and "inline style" not in text:
                problems.append(f"{name}: {text}")
            if msg.type in ("error", "warning"):
                seen.append(f"console {msg.type}: {text[:300]}")

        page.on("pageerror", on_error)
        page.on("console", on_console)
        seen = []
        responses[name] = seen

        def on_response(r) -> None:
            if r.status >= 400 or "/api/chat/session" in r.url:
                line = f"{r.status} {r.request.method} {r.url}"
                if len(seen) < 6 or "/api/chat/session" in r.url:
                    try:
                        body = r.text()
                        line += " " + re.sub(r'"(accessToken|access_token)": *"[^"]+"', r'"\\1":"…"', body)[:300]
                    except Exception as e:
                        line += f" (no body: {e})"
                seen.append(line)

        def on_failed(r) -> None:
            seen.append(f"failed {r.method} {r.url}: {r.failure}")

        page.on("response", on_response)
        page.on("requestfailed", on_failed)
        answer = context.request.post(
            f"https://{HOST}/auth/api/firstfactor",
            data=json.dumps({"username": name, "password": PASSWORDS[name]}),
            headers={"Content-Type": "application/json"},
        )
        assert answer.ok, (name, answer.status, answer.text())
        page.goto(f"https://{HOST}/chat/")
        try:
            page.get_by_role("heading", name="Conversations").wait_for()
        except Exception:
            print(f"{name} sees {page.url}:\n{page.inner_text('body')[:3000]}\n{problems}")
            print("\n".join(seen[:8] + ["…"] + seen[-20:]))
            print(box.execute("journalctl -u lososd --no-pager | grep -i 'chat\\|exchange\\|warn' | head -n 40")[1])
            print(box.execute("journalctl -u authelia-losos -u losos-chat --since -4min --no-pager | grep -v 'level=info' | tail -n 60")[1])
            raise
        return page

    def until_js(page, expression, arg=None, timeout=90000):
        """page.wait_for_function, by hand: Playwright polls that through
        eval, which the chat page's CSP refuses, and page.evaluate not."""
        deadline = time.time() + timeout / 1000
        while not (page.evaluate(expression) if arg is None else page.evaluate(expression, arg)):
            if time.time() > deadline:
                raise TimeoutError(f"timed out after {timeout} ms: {expression}")
            time.sleep(0.25)

    def settle(page):
        page.evaluate("document.fonts.ready")
        page.evaluate("document.documentElement.style.caretColor = 'transparent'")
        until_js(page, "!document.querySelector('.animate-spin')", timeout=30000)
        time.sleep(1)

    def shot(page, name, top=True):
        settle(page)
        # From the top of the page, so a Before and its After differ only
        # in what changed; a picture of the timeline's end stays put.
        if top:
            page.evaluate("window.scrollTo(0, 0)")
        page.screenshot(path=f"{SHOTS}/{name}.png")

    def shot_end(page, name):
        """The end of the room's timeline, where the newest notice is,
        with the pointer off the messages so no toolbar covers them."""
        page.mouse.move(0, 0)
        settle(page)
        page.evaluate("window.scrollTo(0, document.documentElement.scrollHeight)")
        page.get_by_role("log").first.evaluate("e => { e.scrollTop = e.scrollHeight }")
        time.sleep(1)
        page.screenshot(path=f"{SHOTS}/{name}.png")

    def film(page, slug, title, action="none", targets=None, top=True):
        settle(page)
        if top:
            page.evaluate("window.scrollTo(0, 0)")
        boxes = {}
        for key, locator in (targets or {}).items():
            b = locator.bounding_box()
            if b:
                boxes[key] = {"x": round(b["x"]), "y": round(b["y"]),
                              "w": round(b["width"]), "h": round(b["height"])}
        name = f"{len(film_steps) + 1:02d}-{slug}.png"
        page.screenshot(path=f"{FILM}/{name}")
        step = {"file": name, "url": f"{HOST}/chat/", "title": title,
                "action": action, "targets": boxes}
        if page is owner:
            step["clip"] = "calls-matus.mp4"
            step["at"] = round(time.time() - clip_start["matus"], 1)
        film_steps.append(step)
        with open(f"{FILM}/manifest.json", "w") as f:
            json.dump({"feature": "calls", "viewport": VIEW, "dpr": 2, "fps": 60,
                       "test": "losos-calls", "steps": film_steps}, f, indent=2)

    def opened(page):
        return int(page.locator("[data-e2ee]").get_attribute("data-opened") or "0")

    def live(page, others, frames=60):
        """The call view shows `others` other people, and frames from them
        are arriving and opening."""
        try:
            seen(page, others, frames)
        except Exception:
            media_report()
            print("this page's call:", page.evaluate("""() => ({
              tiles: [...document.querySelectorAll('[data-call-view] [data-pid]')].map((e) => e.getAttribute('data-pid')),
              videos: [...document.querySelectorAll('[data-call-view] video')].map((v) => ({
                muted: v.muted, width: v.videoWidth, time: v.currentTime, ready: v.readyState })),
              e2ee: { ...(document.querySelector('[data-e2ee]')?.dataset ?? {}) },
            })"""))
            for who, lines in responses.items():
                print(f"{who}'s console and failures:", *[l for l in lines if l.startswith(("console", "failed"))][-12:], sep="\n  ")
            raise

    def seen(page, others, frames):
        view = page.locator("[data-call-view]")
        view.wait_for()
        until_js(page,
            f"document.querySelectorAll('[data-call-view] [data-pid]').length === {others + 1}",
            timeout=90000,
        )
        until_js(page,
            f"Number(document.querySelector('[data-e2ee]')?.dataset.opened ?? 0) > {frames}",
            timeout=120000,
        )
        # A remote picture is playing, not only arriving.
        until_js(page,
            "[...document.querySelectorAll('[data-call-view] video')]"
            ".filter(v => !v.muted && v.videoWidth > 0 && v.currentTime > 1).length >= " + str(others),
            timeout=90000,
        )
        assert page.locator("[data-e2ee]").get_attribute("data-refused") == "0"

    def stuck(who, page):
        """What a page shows when a step times out, and its failed requests."""
        page.screenshot(path=f"{OUT}/stuck-{who}.png")
        print(f"{who} sees {page.url}:\n{page.inner_text('body')[:2000]}\n{problems}")
        print("\n".join(responses[who][-30:]))

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
        try:
            composer.wait_for()
        except Exception:
            stuck("matus", owner)
            raise
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
            page.get_by_role("heading", name="Conversations").wait_for()
        room_button(owner, "Eva").click()
        room_button(her, "Matus").click()
        owner.get_by_role("button", name="Video call").wait_for()
        shot(owner, "room-after")
        film(owner, "chat-with-eva", "a direct chat with Eva: voice and video call buttons",
             "click", {"video": owner.get_by_role("button", name="Video call")})
        owner.get_by_role("button", name="Video call").click()
        ring = her.locator("[data-ringing]")
        try:
            ring.wait_for()
        except Exception:
            stuck("matus", owner)
            stuck("eva", her)
            print(box.execute("journalctl -u losos-calls --no-pager | tail -n 40")[1])
            raise
        assert "Matus" in ring.text_content(), ring.text_content()
        shot(her, "ringing")
        film(her, "eva-rings", "Eva's browser rings with Matus's call",
             "click", {"answer": ring.get_by_role("button", name="Answer", exact=True)})
        ring.get_by_role("button", name="Answer", exact=True).click()

    with subtest("they see each other, on frames only they can open"):
        live(owner, 1)
        live(her, 1)
        shot(owner, "call-two")
        shot(her, "call-two-eva")
        film(owner, "call", "Matus and Eva in a video call, the room behind them black",
             "hover", {"lock": owner.locator("[data-e2ee]")})
        # The view says so, and says what it counted.
        owner.locator("[data-e2ee]").hover()
        owner.get_by_text(re.compile(r"frames sent encrypted")).wait_for()
        shot(owner, "call-encrypted")
        film(owner, "encrypted", "every frame is encrypted in the browser; the box cannot see it",
             "click", {"share": owner.get_by_role("button", name="Share your screen")})

    def their_tile(page):
        """The pid of the first other person's tile on `page`."""
        return page.evaluate(
            "[...document.querySelectorAll('[data-call-view] [data-pid]')]"
            ".find(t => !t.textContent.includes('You')).dataset.pid"
        )

    def corners(page, pid):
        """How bright the corners of `pid`'s picture are on `page`, 0 to 255;
        -1 while there is no picture."""
        return page.evaluate(
            """(pid) => {
              const v = document.querySelector('[data-call-view] [data-pid="' + pid + '"] video');
              if (!v || !v.videoWidth) return -1;
              const c = document.createElement('canvas');
              c.width = 64; c.height = 48;
              const g = c.getContext('2d', { willReadFrequently: true });
              g.drawImage(v, 0, 0, 64, 48);
              const d = g.getImageData(0, 0, 64, 48).data;
              const at = (x, y) => { const i = (y * 64 + x) * 4; return (d[i] + d[i + 1] + d[i + 2]) / 3; };
              return Math.max(at(2, 2), at(61, 2), at(2, 45), at(61, 45));
            }""",
            pid,
        )

    with subtest("the room behind each person is black, cut out before encryption"):
        matus_at_eva = their_tile(her)
        # Black by default: the corners of Matus's picture at Eva are dark,
        # and there is a picture.
        until_js(her,
            "(pid) => document.querySelector('[data-call-view] [data-pid=\"' + pid + '\"] video')?.videoWidth > 0",
            arg=matus_at_eva)
        deadline = time.time() + 60
        while corners(her, matus_at_eva) > 24 and time.time() < deadline:
            time.sleep(1)
        assert 0 <= corners(her, matus_at_eva) <= 24, corners(her, matus_at_eva)
        shot(her, "background-black")
        # Matus shows his room; Eva sees it.
        owner.get_by_role("button", name="Show your room behind you").click()
        deadline = time.time() + 60
        while corners(her, matus_at_eva) < 60 and time.time() < deadline:
            time.sleep(1)
        assert corners(her, matus_at_eva) >= 60, corners(her, matus_at_eva)
        shot(her, "background-room")
        owner.get_by_role("button", name="Black out your background").click()
        deadline = time.time() + 60
        while corners(her, matus_at_eva) > 24 and time.time() < deadline:
            time.sleep(1)
        assert 0 <= corners(her, matus_at_eva) <= 24, corners(her, matus_at_eva)
        assert her.locator("[data-e2ee]").get_attribute("data-refused") == "0"

    with subtest("a shared screen goes in the middle, the people beside it"):
        owner.get_by_role("button", name="Share your screen").click()
        until_js(her,
            "!!document.querySelector('[data-call-view]')?.dataset.sharing"
            " && document.querySelector('[data-share] video')?.videoWidth > 0",
            timeout=120000,
        )
        assert her.locator("[data-call-strip] [data-pid]").count() == 2
        her.get_by_text("Matus's screen").wait_for()
        shot(her, "share-eva")
        shot(owner, "share")
        film(owner, "screen", "a shared screen in the middle, the people beside it",
             "click", {"stop": owner.get_by_role("button", name="Stop sharing")})
        # The camera keeps coming beside the screen, and both open.
        before = opened(her)
        time.sleep(5)
        assert opened(her) > before + 50, (opened(her), before)
        assert her.locator("[data-e2ee]").get_attribute("data-refused") == "0"
        owner.get_by_role("button", name="Stop sharing").click()
        until_js(her, "!document.querySelector('[data-call-view]')?.dataset.sharing",
                              timeout=60000)

    with subtest("podcast mode streams the call to an RTMP address, and both see it is live"):
        laptop.succeed(
            "systemd-run --unit rtmp-sink ${pkgs.ffmpeg-headless}/bin/ffmpeg -v error -listen 1 "
            "-i rtmp://0.0.0.0:${toString rtmpPort}/live/holiday-key -c copy -y /tmp/podcast.flv"
        )
        # Not wait_for_open_port: its probe would be the one connection
        # `-listen 1` takes, and ffmpeg would give up on its handshake.
        laptop.wait_until_succeeds("ss -tln | grep -q ':${toString rtmpPort} '")
        film(owner, "call-again", "back to the two of them",
             "click", {"podcast": owner.get_by_role("button", name="Podcast mode")})
        owner.get_by_role("button", name="Podcast mode").click()
        panel = owner.locator("[data-podcast-panel]")
        panel.wait_for()
        panel.get_by_label("RTMP address with stream key").fill(
            "rtmp://laptop:${toString rtmpPort}/live/holiday-key")
        # Under a plain name: the file field shows it.
        overlay_png = os.path.join(os.environ.get("TMPDIR", "/tmp"), "holiday-overlay.png")
        with open("${overlay}", "rb") as src, open(overlay_png, "wb") as dst:
            dst.write(src.read())
        panel.get_by_label("Overlay image (PNG)").set_input_files(overlay_png)
        # Typed a word at a time, so the film can show the typing.
        for label, text in (("Title", "Holiday plans"), ("Subtitle", "Matus and Eva, live from mattbox")):
            field = panel.get_by_label(label, exact=True)
            words = text.split(" ")
            for i in range(1, len(words) + 1):
                field.fill(" ".join(words[:i]))
                film(owner, f"podcast-{label.lower()}-{i}", f"podcast mode: the {label.lower()} is typed",
                     "type", {"field": field})
        owner.wait_for_timeout(1500)
        shot(owner, "podcast-panel")
        film(owner, "podcast-ready", "podcast mode: a grid, a title and an overlay, streamed to an RTMP address",
             "click", {"live": panel.get_by_role("button", name="Go live")})
        panel.get_by_role("button", name="Go live").click()
        her.locator("[data-live]").wait_for(timeout=60000)
        owner.locator("[data-live]").wait_for()
        panel.get_by_role("button", name="End the stream").wait_for()
        shot(her, "podcast-live-eva")
        film(owner, "podcast-live", "live: everyone in the call sees that it is public")
        # The box encodes H.264 under emulation here, far slower than real
        # time, so the server keeps what arrives until the stream ends.
        try:
            laptop.wait_until_succeeds("test $(stat -c %s /tmp/podcast.flv) -gt 100000", timeout=600)
        except Exception:
            print("page:", owner.evaluate(
                "() => ({...document.querySelector('[data-podcast-preview] canvas')?.dataset})"))
            print(box.execute("for p in $(pgrep -f pipe:0); do ps -o pid,etime,args= -p $p; cat /proc/$p/io; done")[1])
            print(box.execute("journalctl -u losos-calls --no-pager | grep -iE 'ffmpeg|stream' | tail -20")[1])
            raise
        # The address carries the stream key; the box never writes it down.
        box.fail("journalctl -u losos-calls | grep -q holiday-key")
        panel.get_by_role("button", name="End the stream").click()
        her.locator("[data-live]").wait_for(state="detached", timeout=60000)
        laptop.wait_until_fails("systemctl is-active rtmp-sink", timeout=180)
        probe = json.loads(laptop.succeed(
            "${pkgs.ffmpeg-headless}/bin/ffprobe -v error -show_entries "
            "stream=codec_name,width,height:format=duration -of json /tmp/podcast.flv"))
        assert sorted(x["codec_name"] for x in probe["streams"]) == ["aac", "h264"], probe
        duration = float(probe["format"]["duration"])
        assert duration > 0.5, probe
        laptop.succeed(f"${pkgs.ffmpeg-headless}/bin/ffmpeg -v error -ss {duration / 2:.2f} -i /tmp/podcast.flv "
                       "-frames:v 1 -y /tmp/podcast.png")
        laptop.copy_from_machine("/tmp/podcast.png", "")
        os.replace(os.path.join(OUT, "podcast.png"), f"{SHOTS}/podcast-stream.png")
        panel.get_by_role("button", name="Close").click()

    with subtest("a guest link from the edge lets one guest into this call"):
        owner.get_by_role("button", name="Invite a guest").click()
        if EDGE is None:
            # No edge, no link: a box on its own makes none.
            owner.get_by_text("not connected to one").first.wait_for(timeout=60000)
            shot(owner, "invite-no-edge")
        else:
            panel = owner.locator("[data-invite-panel]")
            field = panel.locator("[data-invite-link]")
            field.wait_for(timeout=90000)
            link = field.get_attribute("data-invite-link")
            url = urllib.parse.urlsplit(link)
            assert url.scheme == "https" and url.netloc == "mattbox.losos.cfd", link
            assert url.path == "/calls/guest/", link
            assert len(url.fragment) == 43, "the secret rides after #"
            EDGE.succeed("journalctl -u losos-registrar | grep -q 'mattbox made a guest link'")
            # Neither the edge nor the box ever saw the secret.
            EDGE.fail(f"journalctl -u losos-registrar | grep -qF -- '{url.fragment}'")
            box.fail(f"journalctl | grep -qF -- '{url.fragment}'")
            shot(owner, "invite")
            panel.get_by_role("button", name="Close").click()
            # A stranger opens it from the internet: no LosOS Login, no LAN.
            guest_ctx = browsers["jan"].new_context(
                viewport={"width": 1280, "height": 900},
                device_scale_factor=2,
                color_scheme="dark",
                locale="en-US",
                ignore_https_errors=True,
            )
            guest = guest_ctx.new_page()
            guest.set_default_timeout(90000)
            def on_guest_error(err) -> None:
                problems.append(f"guest: {err}")

            guest.on("pageerror", on_guest_error)
            guest.goto(link)
            guest.get_by_label("Your name").fill("Jack")
            shot(guest, "guest-join")
            guest.get_by_role("button", name="Join the call").click()
            # Under emulation the guest's camera and its background cut take
            # minutes to open; the call view comes after them.
            guest.locator("[data-guest-call]").wait_for(timeout=360000)
            # Two others for the guest, and the guest for each of them.
            live(guest, 2)
            for page in (owner, her):
                until_js(page,
                    "document.querySelectorAll('[data-call-view] [data-pid]').length === 3", timeout=90000)
                page.get_by_text("Jack (guest)").first.wait_for()
            shot(guest, "guest-call")
            shot(owner, "call-with-guest")
            # The admin page and chat stay shut to the same browser.
            assert guest_ctx.request.get("https://mattbox.losos.cfd/api/me").status in (401, 403)
            assert guest_ctx.request.get("https://mattbox.losos.cfd/chat/").status == 403
            guest.get_by_role("button", name="Leave the call").click()
            guest.locator("[data-guest-over]").wait_for()
            for page in (owner, her):
                until_js(page,
                    "document.querySelectorAll('[data-call-view] [data-pid]').length === 2", timeout=60000)
            guest_ctx.close()

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
                                "invite": [f"@eva:{HOST}", f"@jan:{HOST}"],
                                # Calls are only for encrypted rooms: their
                                # keys go to devices the crypto tracks. The
                                # slot is the room's one call (MSC4143).
                                "initial_state": [{"type": "m.room.encryption", "state_key": "",
                                                   "content": {"algorithm": "m.megolm.v1.aes-sha2"}},
                                                  {"type": "org.matrix.msc4143.rtc.slot",
                                                   "state_key": "org.losos.call#room",
                                                   "content": {"status": "open",
                                                               "application": {"type": "org.losos.call"},
                                                               "encryption": {"type": "org.matrix.msc4143.per_member"}}}]})
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
        # Jan has just signed in: his new device reaches the others' device
        # lists a sync later, and a message sent before then is one he
        # cannot open. Talk until both of them read Matus, as people would
        # before a call.
        for attempt in range(1, 6):
            line = f"Call about the holiday in a minute ({attempt})"
            owner.get_by_role("textbox", name="Message Holiday").fill(line)
            owner.keyboard.press("Enter")
            try:
                for page in (her, jan):
                    page.get_by_role("log").get_by_text(line).wait_for(timeout=30000)
                break
            except Exception:
                if attempt == 5:
                    raise
        owner.get_by_role("button", name="Video call").click()
        for who, page in (("eva", her), ("jan", jan)):
            try:
                page.locator("[data-ringing]").get_by_role("button", name="Answer", exact=True).click()
            except Exception:
                stuck(who, page)
                raise
        for page in (owner, her, jan):
            live(page, 2)
        # What the room holds is MatrixRTC: the open slot, and a sticky
        # membership from each of them (MSC4354; encrypted, so the server
        # sees only that it is sticky), each with a delayed leave waiting on
        # the homeserver (MSC4140) for the moment its page goes quiet. A
        # leave is sent plain, since the homeserver sends it later, and
        # names only the slot and the device: earlier calls here left some.
        code, body = call(f"/_matrix/client/v3/rooms/{room}/state", cookies["matus"], token=tokens["matus"])
        assert code == 200, (code, body)
        state = json.loads(body)
        slot = [e for e in state if e["type"] == "org.matrix.msc4143.rtc.slot"]
        assert [e["state_key"] for e in slot] == ["org.losos.call#room"], slot
        assert slot[0]["content"]["status"] == "open", slot
        assert not [e for e in state if e["type"] == "org.matrix.msc3401.call.member"], state
        # The media is up before the last membership is: a page sends its
        # delayed leave first, and on a slow homeserver that takes a while.
        for _ in range(30):
            code, body = call(f"/_matrix/client/v3/rooms/{room}/messages?dir=b&limit=100", cookies["matus"],
                              token=tokens["matus"])
            assert code == 200, (code, body)
            sticky = [e for e in json.loads(body)["chunk"] if "msc4354_sticky" in e]
            sealed = [e for e in sticky if e["type"] == "m.room.encrypted"]
            if len({e["sender"] for e in sealed}) == 3:
                break
            time.sleep(2)
        assert {e["sender"] for e in sealed} == {f"@{n}:{HOST}" for n in ("matus", "eva", "jan")}, sticky
        assert all(e in sealed or (e["type"] == "org.matrix.msc4143.rtc.member"
                                   and set(e["content"]) == {"slot_id", "msc4354_sticky_key"})
                   for e in sticky), sticky
        def leaves(n):
            code, body = call("/_matrix/client/unstable/org.matrix.msc4140/delayed_events", cookies[n],
                              token=tokens[n])
            assert code == 200, (code, body)
            return [e for e in json.loads(body)["delayed_events"]
                    if urllib.parse.quote(e["room_id"]) == room]
        for n, page in (("matus", owner), ("eva", her), ("jan", jan)):
            for _ in range(20):
                if leaves(n):
                    break
                time.sleep(1)
            else:
                print(call("/_matrix/client/unstable/org.matrix.msc4140/delayed_events", cookies[n],
                           token=tokens[n])[1])
                stuck(n, page)
                raise AssertionError(f"{n} has no delayed leave in the room")
        shot(owner, "call-three")
        film(owner, "call-of-three", "a call of three in a room")
        jan.get_by_role("button", name="Leave the call").click()
        jan.locator("[data-call-view]").wait_for(state="detached")
        for page in (owner, her):
            until_js(page,
                "document.querySelectorAll('[data-call-view] [data-pid]').length === 2", timeout=60000)
        # After the leave the others rotate to a key Jan never had, and
        # still open each other's frames.
        before = {id(p): opened(p) for p in (owner, her)}
        time.sleep(8)
        for page in (owner, her):
            assert opened(page) > before[id(page)] + 30, (opened(page), before[id(page)])
            counts = page.locator("[data-e2ee]").evaluate("e => ({ ...e.dataset })")
            assert counts["refused"] == "0", counts
        notice = jan.locator("[data-call-notice]").last
        notice.get_by_role("button", name="Join", exact=True).wait_for()
        shot_end(jan, "timeline-join")
        notice.get_by_role("button", name="Join", exact=True).click()
        for page in (owner, her, jan):
            live(page, 2, frames=0)
        # The link in the notice other Matrix apps show brings Eva back.
        her.get_by_role("button", name="Leave the call").click()
        her.locator("[data-call-view]").wait_for(state="detached")
        # Her leave reaches the room before the page goes away: the
        # homeserver has no delayed leave of hers left to send.
        for _ in range(60):
            if not leaves("eva"):
                break
            time.sleep(1)
        assert not leaves("eva"), "Eva's delayed leave stayed after she left"
        her.goto(f"https://{HOST}/chat/?join={room}&call=video")
        for page in (owner, her, jan):
            live(page, 2, frames=0)

    with subtest("a closed browser leaves by itself; hanging up ends the call for the room"):
        # Jan's browser goes away without a word. The homeserver sends the
        # leave it was holding for him (MSC4140) half a minute later: a new sticky
        # event of his, and no delayed leave left.
        def sticky_from(n):
            body = call(f"/_matrix/client/v3/rooms/{room}/messages?dir=b&limit=100", cookies["matus"],
                        token=tokens["matus"])[1]
            return len([e for e in json.loads(body)["chunk"]
                        if "msc4354_sticky" in e and e["sender"] == f"@{n}:{HOST}"])
        assert leaves("jan")
        before = sticky_from("jan")
        jan.context.close()
        for _ in range(60):
            if not leaves("jan") and sticky_from("jan") > before:
                break
            time.sleep(1)
        assert not leaves("jan") and sticky_from("jan") > before, "Jan's leave never came"
        for page in (owner, her):
            until_js(page,
                "document.querySelectorAll('[data-call-view] [data-pid]').length === 2", timeout=60000)
        her.get_by_role("button", name="Leave the call").click()
        her.locator("[data-call-view]").wait_for(state="detached")
        # Left alone, Matus is out of the call too, and the SFU told his
        # page it was the last one there, so it tells the room.
        owner.locator("[data-call-view]").wait_for(state="detached", timeout=60000)
        owner.get_by_role("log").get_by_text("The call ended").last.wait_for(timeout=60000)
        until_js(her, "document.querySelectorAll('[data-call-notice] button').length === 0", timeout=30000)
        # The call rang the room once, however many joined it.
        assert her.locator("[data-call-notice]").count() == 1, her.locator("[data-call-notice]").count()

    with subtest("a call link from another app opens the direct chat and rings"):
        # Contacts and Mail link a person as /chat/?with=<name>&call=voice.
        owner.goto(f"https://{HOST}/chat/?with=eva&call=voice")
        ring = her.locator("[data-ringing]")
        ring.wait_for(timeout=60000)
        assert "Matus" in ring.text_content(), ring.text_content()
        owner.locator("[data-call-view]").wait_for()
        assert "call=" not in owner.url and "with=" not in owner.url, owner.url
        owner.get_by_role("button", name="Leave the call").click()
        owner.locator("[data-call-view]").wait_for(state="detached")
        ring.wait_for(state="detached", timeout=60000)

    with subtest("without a camera, a video call starts with it off"):
        # Eva's browser loses its camera: every request for one fails, as
        # on a desktop with none plugged in. The call goes on without it.
        her.context.add_init_script("""
          const open = navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);
          navigator.mediaDevices.getUserMedia = (c) => c && c.video
            ? Promise.reject(new DOMException('Requested device not found', 'NotFoundError'))
            : open(c);
        """)
        her.goto(f"https://{HOST}/chat/?with=matus&call=video")
        try:
            her.locator("[data-call-view]").wait_for(timeout=120000)
        except Exception:
            stuck("eva", her)
            raise
        her.get_by_text("No camera", exact=True).first.wait_for()
        her.get_by_role("button", name="Turn on camera").wait_for()
        ring = owner.locator("[data-ringing]")
        ring.wait_for(timeout=60000)
        ring.get_by_role("button", name="Answer", exact=True).click()
        owner.locator("[data-call-view]").wait_for()
        until_js(her, "() => document.querySelectorAll('[data-call-view] [data-pid]').length === 2")
        until_js(owner, "() => document.querySelectorAll('[data-call-view] [data-pid]').length === 2")
        settle(her)
        shot(her, "no-camera")
        film(her, "no-camera", "no camera: Eva joins with hers off, and still hears and is heard")
        shot(owner, "no-camera-matus")
        # Eva hangs up; Matus, left alone, leaves by himself.
        her.get_by_role("button", name="Leave the call").click()
        for page in (her, owner):
            page.locator("[data-call-view]").wait_for(state="detached", timeout=60000)

    with subtest("with sharing off there are no calls at all"):
        switch("unshared")
        box.wait_until_fails("systemctl is-active losos-calls.service")
        box.fail("systemctl cat losos-calls.service")
        box.fail("ss -uln | grep -q ':${toString mediaPort} '")
        # Nothing answers but the admin page's index.html.
        assert call("/calls/api/health", matus)[1] != "ok"

    with subtest("the pages broke no rule of their policy"):
        assert problems == [], problems

    # Matus's clip: Chromium writes it when his window closes. Under
    # emulation the page paints well below 60 frames a second, so the
    # clip is resampled to a constant 60 for the cut.
    if FILMING:
        raw = owner.video.path()
        owner.context.close()
        subprocess.run(
            ["${pkgs.ffmpeg-headless}/bin/ffmpeg", "-v", "error", "-i", raw, "-vf", "fps=60",
             "-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p",
             "-movflags", "+faststart", "-y", f"{FILM}/calls-matus.mp4"],
            check=True,
        )
        os.remove(raw)

    for b in browsers.values():
        b.close()
    pw.stop()
  '';
}
