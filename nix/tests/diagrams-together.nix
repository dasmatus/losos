# nixos-test-vms config for LosOS Diagrams edited together: two boxes with
# LosOS Cloud and its mesh sharing, and a laptop whose browser is both
# people.
#
# The test asserts:
#
#   * Matus's .canvas file on mattbox, shared with Eva on kitchenbox, opens
#     in Diagrams from her Shared with you, in the file's live room;
#   * each sees the other's face in the top bar, and Matus sees Eva's
#     pointer and selection on the canvas in her colour;
#   * the card Eva writes on kitchenbox reaches Matus's page at once, and
#     mattbox writes it into the file, with the keys from other tools the
#     file came with.
#
# The pictures (`together-matus`, `together-eva`) are the pull request's.
# Like nix/tests/diagrams.nix it builds the admin UI without the GPU canvas
# (nix/tests/diagrams-common.nix); the two share that build.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  common = import ./diagrams-common.nix {
    inherit pkgs;
    shipped = lososPkgs.losos-admin-ui;
  };

  together = pkgs.writeShellScriptBin "diagrams-together" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export NODE_EXTRA_CA_CERTS=''${NODE_EXTRA_CA_CERTS:-/etc/losos-boxes.pem}
    exec ${pkgs.bun}/bin/bun ${./diagrams-together.mjs} "$@"
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
        backend.plugins = lososPkgs.losos-plugins;
        admin.ui = common.ui;
        git.enable = false;
        cloud.enable = true;
        cloud.mesh.enable = true;
      };
      networking.extraHosts = hosts nodes;
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
  name = "losos-diagrams-together";

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
          together
        ];
        environment.etc."losos-test/launch.canvas".source = common.launch;
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

    PEOPLE = {
        mattbox: ("matus", "Matus", "Correct-Horse-Battery-9"),
        kitchenbox: ("eva", "Eva", "Eva-Long-Password-3"),
    }
    CERTS = "/etc/losos-boxes.pem"
    FILE = "/var/lib/losos-cloud/notshared/files/Diagrams/launch.canvas"

    def base(m):
        return f"https://{m.name}.local"

    # Three emulated machines booting at once can starve each other; one
    # after the other boots steadily.
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

    def call(m, method, path, cookie=None, data=None):
        args = f"-X {method} '{base(m)}{path}'"
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

    with subtest("Matus's diagram from another tool goes into LosOS Cloud"):
        _, _, password = PEOPLE[mattbox]
        dav = f"{base(mattbox)}/remote.php/webdav"
        curl(f"-f -u 'matus:{password}' -X MKCOL '{dav}/Diagrams'")
        curl(f"-f -u 'matus:{password}' -T /etc/losos-test/launch.canvas '{dav}/Diagrams/launch.canvas'")
        file_id = int(mattbox.succeed(
            "sqlite3 /var/lib/losos-cloud/cloud.db"
            " \"select id from files where owner='notshared' and path='Diagrams/launch.canvas'\""
        ).strip())
        config = {
            "path": "Diagrams/launch.canvas",
            "fileId": file_id,
            "owner": {"base": base(mattbox), "name": "matus", "password": PEOPLE[mattbox][2]},
            "guest": {"base": base(kitchenbox), "name": "eva", "password": PEOPLE[kitchenbox][2]},
            "certs": ["/etc/losos-mattbox.pem", "/etc/losos-kitchenbox.pem"],
        }
        laptop.succeed(f"cat > /tmp/together.json <<'EOF'\n{json.dumps(config)}\nEOF")

    with subtest("Matus shares it with Eva and they draw in it together"):
        status, _ = laptop.execute("diagrams-together /tmp/together.json /tmp/shots >&2", timeout=2400)
        laptop.copy_from_machine("/tmp/shots", "")
        assert status == 0, "the two windows did not see each other's changes; the pictures so far are in shots"

    with subtest("mattbox wrote Eva's card into the file, other tools' keys kept"):
        mattbox.wait_until_succeeds(f"grep -q 'The van is booked for Friday' {FILE}", timeout=60)
        doc = json.loads(mattbox.succeed(f"cat {FILE}"))
        texts = [n.get("text", "") for n in doc["nodes"]]
        assert "The van is booked for Friday" in texts, texts
        assert doc["x-board-plugin"] == {"lane": "this week"}, doc.get("x-board-plugin")
        nas = next(n for n in doc["nodes"] if n["id"] == "nas")
        assert nas["x-tasks-plugin"] == {"due": "2026-10-30"}, nas
  '';
}
