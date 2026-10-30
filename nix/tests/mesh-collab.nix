# nixos-test-vms config for sharing across boxes and editing together
# (losos.cloud.mesh, modules/cloud.nix, backend/src/cloud/{mesh,collab}/).
#
# Two boxes, each with its owner, and a laptop on the same LAN:
#
#   mattbox     Matus's box. He keeps "Summer trip.md" in LosOS Cloud.
#   kitchenbox  Eva's box.
#   laptop      Chromium with a window for each of them.
#
# Both boxes run the real front vhost, LosOS Login and LosOS Cloud with the
# mesh on. The test checks what nothing at build time can:
#
#   * /mesh/v1/link answers other boxes outside LosOS Login and the LAN
#     guard, and the service can dial the other box but not loopback;
#   * Matus shares the note with eva@kitchenbox.local from the web app, the
#     offer crosses to kitchenbox, Eva accepts it and both type into it at
#     once, with faces and named cursors, and the file on mattbox is what
#     they wrote;
#   * with the boxes cut apart both keep writing, and when they meet again
#     both lines are in both windows and in the file;
#   * a view-only share takes no typing;
#   * no page meets a refusal from the page's security policy
#     (`style-src 'self'`), the editor included.
#
# The pictures for the pull request and the launch film come from the same
# run (nix/tests/mesh-collab-shots.mjs). `beforeUi`, when given, is the
# admin UI as it was before this feature: the boxes switch to it at the end
# for the "before" pictures. The flake check runs without it.
#
# The admin UI is built without LosOS Diagrams, whose
# WebAssembly takes longer to build than this whole test runs and is not
# part of anything tested here; the shipped package builds it.
{
  pkgs,
  beforeUi ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  noLab = import ./admin-ui-without-lab.nix;
  adminUi = noLab lososPkgs.losos-admin-ui;

  # Matus's profile picture, a figure drawn in the palette (no photo, no
  # logo), so the editor shows a picture beside Eva's initials.
  picture =
    pkgs.runCommand "mesh-collab-picture"
      {
        nativeBuildInputs = [ (pkgs.python3.withPackages (ps: [ ps.pillow ])) ];
      }
      ''
        python3 - $out <<'EOF'
        import sys
        from PIL import Image, ImageDraw
        S = 1024
        img = Image.new("RGB", (S, S))
        d = ImageDraw.Draw(img)
        for y in range(S):
            t = y / S
            d.line([(0, y), (S, y)], fill=(47 - int(20 * t), 95 + int(20 * t), 158 - int(30 * t)))
        d.ellipse([S * 0.18, S * 0.62, S * 0.82, S * 1.30], fill=(14, 110, 125))
        d.rectangle([S * 0.44, S * 0.50, S * 0.56, S * 0.68], fill=(226, 239, 241))
        d.ellipse([S * 0.32, S * 0.20, S * 0.68, S * 0.60], fill=(226, 239, 241))
        d.chord([S * 0.30, S * 0.16, S * 0.70, S * 0.48], 180, 360, fill=(46, 115, 87))
        img.resize((512, 512), Image.LANCZOS).save(sys.argv[1], "PNG")
        EOF
      '';

  shots = pkgs.writeShellScriptBin "mesh-collab-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    # Signing in goes through Node's TLS, not Chromium's: the boxes'
    # certificates, which the test collects on the laptop.
    export NODE_EXTRA_CA_CERTS=''${NODE_EXTRA_CA_CERTS:-/etc/losos-boxes.pem}
    exec ${pkgs.bun}/bin/bun ${./mesh-collab-shots.mjs} "$@"
  '';

  box =
    name:
    { nodes, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/daemon.nix
        ../../modules/identity.nix
        ../../modules/cloud.nix
      ];
      networking.hostName = name;
      losos = {
        hostName = name;
        backend.package = lososPkgs.losos-ctl;
        admin.ui = adminUi;
        git.enable = false;
        cloud.enable = true;
        # What a box in a mesh has on by default.
        cloud.mesh.enable = true;
      };
      networking.extraHosts = hosts nodes;
      specialisation = lib.mkIf (beforeUi != null) {
        before.configuration.losos.admin.ui = lib.mkForce (noLab beforeUi);
      };
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
        pkgs.sqlite
      ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };

  # The boxes by their LAN names, as mDNS would give them.
  hosts = nodes: ''
    ${nodes.mattbox.networking.primaryIPAddress} mattbox.local
    ${nodes.kitchenbox.networking.primaryIPAddress} kitchenbox.local
  '';
in

pkgs.testers.nixosTest {
  name = "losos-mesh-collab";

  nodes = {
    mattbox = box "mattbox";
    kitchenbox = box "kitchenbox";

    laptop =
      { nodes, ... }:
      {
        networking.extraHosts = hosts nodes;
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
          shots
        ];
        environment.variables = {
          PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers.override {
            withFirefox = false;
            withWebkit = false;
          }}";
          PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
        };
        fonts.packages = [ pkgs.dejavu_fonts ];
        virtualisation = {
          memorySize = 3072;
          cores = 2;
          vlans = [ 1 ];
        };
      };
  };

  testScript = ''
    import json
    import re

    BEFORE = ${if beforeUi == null then "False" else "True"}
    FILE = "Summer trip.md"
    NOTE = "# Summer trip\n\n- Train to Vienna on Friday\n- Book the flat by Tuesday\n"
    PEOPLE = {
        mattbox: ("matus", "Matus", "Correct-Horse-Battery-9"),
        kitchenbox: ("eva", "Eva", "Eva-Long-Password-3"),
    }
    CERTS = "/etc/losos-boxes.pem"
    SHOTS = "/tmp/shots"

    def base(m):
        return f"https://{m.name}.local"

    # Three emulated machines booting at once can starve each other past the
    # driver's five minutes for a shell; one after the other boots steadily.
    for m in (mattbox, kitchenbox, laptop):
        m.succeed("true")
    for m in (mattbox, kitchenbox):
        m.wait_for_unit("nginx.service")
        m.wait_for_unit("lososd.service")
        m.wait_for_unit("losos-cloud.service")
        cert = m.succeed("cat /var/lib/losos-tls/cert.pem")
        laptop.succeed(f"cat > /etc/losos-{m.name}.pem <<'EOF'\n{cert}EOF")
    laptop.succeed(f"cat /etc/losos-mattbox.pem /etc/losos-kitchenbox.pem > {CERTS}")

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CERTS} {args}")

    def call(m, method, path, cookie=None, data=None, extra=""):
        args = f"-X {method} '{base(m)}{path}' {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        body, code = curl(f"-w '\\n%{{http_code}}' {args}").rsplit("\n", 1)
        return int(code), body

    def sign_in(m):
        name, _, password = PEOPLE[m]
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null {base(m)}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        found = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert found, f"no session cookie on {m.name}: {head}"
        return found.group(1)

    with subtest("each owner claims their box and gives their name"):
        for m in (mattbox, kitchenbox):
            name, display, password = PEOPLE[m]
            laptop.wait_until_succeeds(
                f"curl -sf --cacert {CERTS} {base(m)}/api/setup/claim | jq -e .ready", timeout=900
            )
            code, body = call(m, "POST", "/api/setup/claim", data={"name": name, "password": password})
            assert code == 200, (m.name, code, body)
            m.wait_for_unit("authelia-losos.service")
            m.wait_for_open_port(9091)
            mark = m.succeed(
                "journalctl -u authelia-losos -n 0 --show-cursor | sed -n 's/^-- cursor: //p'"
            ).strip()
            code, body = call(
                m, "POST", "/api/account", sign_in(m),
                {"displayName": display, "email": f"{name}@{m.name}.local"},
            )
            assert code == 200, (m.name, code, body)
            m.wait_until_succeeds(
                f"journalctl -u authelia-losos --after-cursor='{mark}' | grep -q 'Reloaded successfully'",
                timeout=120,
            )

    with subtest("Matus has a profile picture; Eva keeps the colour her name gives"):
        code, body = call(
            mattbox, "PUT", "/api/account/picture", sign_in(mattbox),
            extra="-H 'Content-Type: image/png' --data-binary @${picture}",
        )
        assert code == 200, (code, body)
        assert json.loads(body)["picture"].startswith("/api/profiles/matus/picture?v="), body

    with subtest("other boxes reach the link without signing in; people do not reach the rest"):
        # A plain GET is not a WebSocket: the service, not LosOS Login,
        # answers, and only for the upgrade.
        code, _ = call(mattbox, "GET", "/mesh/v1/link")
        assert code == 400, code
        code, _ = call(mattbox, "GET", "/cloud/api/mesh/box")
        assert code == 401, code
        # Sandboxed: the network, but not the box's own loopback services.
        deny = mattbox.succeed("systemctl show -p IPAddressDeny --value losos-cloud").split()
        assert "127.0.0.0/8" in deny and "::1/128" in deny, deny
        cookie = sign_in(mattbox)
        code, body = call(mattbox, "GET", "/cloud/api/mesh/box", cookie)
        assert code == 200, (code, body)
        me = json.loads(body)
        assert me["name"] == "mattbox.local" and me["me"] == "matus@mattbox.local", me

    with subtest("Matus's note"):
        _, _, password = PEOPLE[mattbox]
        laptop.succeed(f"cat > /tmp/note.md <<'EOF'\n{NOTE}EOF")
        curl(
            f"-f -u 'matus:{password}' -T /tmp/note.md"
            f" '{base(mattbox)}/remote.php/webdav/Summer%20trip.md'"
        )
        file_id = int(mattbox.succeed(
            "sqlite3 /var/lib/losos-cloud/cloud.db"
            " \"select id from files where owner='notshared' and path='Summer trip.md'\""
        ).strip())
        config = {
            "file": FILE,
            "fileId": file_id,
            "owner": {"base": base(mattbox), "name": "matus", "password": PEOPLE[mattbox][2]},
            "guest": {"base": base(kitchenbox), "name": "eva", "password": PEOPLE[kitchenbox][2]},
            "certs": ["/etc/losos-mattbox.pem", "/etc/losos-kitchenbox.pem"],
            "test": "losos-mesh-collab",
        }
        laptop.succeed(f"cat > /tmp/shots.json <<'EOF'\n{json.dumps(config)}\nEOF")

    def on_disk():
        return mattbox.succeed("cat '/var/lib/losos-cloud/notshared/files/Summer trip.md'")

    def phase(name):
        status, _ = laptop.execute(f"mesh-collab-shots {name} /tmp/shots.json {SHOTS}/after >&2")
        if status != 0:
            laptop.copy_from_machine(f"{SHOTS}/after", "shots")
            raise Exception(f"the {name} phase failed; its pictures are in shots/after")

    with subtest("Matus shares the note with Eva on kitchenbox and they write in it together"):
        phase("share")
        mattbox.wait_until_succeeds(
            "grep -q 'Pack the tent and two sleeping bags' '/var/lib/losos-cloud/notshared/files/Summer trip.md'",
            timeout=60,
        )
        text = on_disk()
        assert "I'll bring the camping stove" in text and text.startswith("# Summer trip"), text
        # Each box pinned the other's key.
        for m, other in ((mattbox, "kitchenbox.local"), (kitchenbox, "mattbox.local")):
            m.succeed(
                f"sqlite3 /var/lib/losos-cloud/.mesh/mesh.db \"select 1 from pins where name='{other}'\" | grep -q 1"
            )

    def cut(on):
        """Drop every packet between the two boxes, connections left open."""
        rule = "add" if on else "del"
        for m, other in ((mattbox, kitchenbox), (kitchenbox, mattbox)):
            m.succeed(f"ip route {rule} blackhole $(getent hosts {other.name}.local | cut -d' ' -f1)")

    with subtest("cut apart, both keep writing; together again, both lines are everywhere"):
        cut(True)
        # The link notices the silence within half a minute.
        kitchenbox.wait_until_succeeds(
            "journalctl -u losos-cloud | grep -q 'the other box went silent'", timeout=120
        )
        phase("apart")
        text = on_disk()
        assert "Leave at seven" in text and "Wachau" not in text, text
        cut(False)
        phase("merged")
        mattbox.wait_until_succeeds(
            "grep -q 'Wachau' '/var/lib/losos-cloud/notshared/files/Summer trip.md'", timeout=120
        )
        text = on_disk()
        assert "Leave at seven" in text and "Pack the tent" in text, text

    with subtest("a view-only share takes no typing"):
        phase("readonly")
        assert "Sneaky" not in on_disk()

    laptop.copy_from_machine(f"{SHOTS}/after", "shots")

    if BEFORE:
        with subtest("the pictures: the same addresses before"):
            for m in (mattbox, kitchenbox):
                m.succeed("/run/current-system/specialisation/before/bin/switch-to-configuration test")
            laptop.succeed(f"mesh-collab-shots before /tmp/shots.json {SHOTS}/before >&2")
            laptop.copy_from_machine(f"{SHOTS}/before", "shots")
  '';
}
