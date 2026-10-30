# nixos-test-vms config for the bar over a selection
# (admin-ui/app/src/components/quick-actions.tsx) on a real box: lososd,
# nginx, LosOS Login and LosOS Cloud, with a laptop on the LAN whose
# Chromium (nix/tests/quick-actions-shots.mjs) picks words in a LosOS Office
# document and in a note, and cards in a LosOS Diagrams file, and uses the
# bar on them. Then:
#
#   * the document in LosOS Cloud holds what the bar wrote, through the
#     box's shared editor: a word made bold, a word linked to a web page,
#     a word linked to the note beside it;
#   * the diagram, saved back to LosOS Cloud, has the two picked cards in
#     green and the arrow the bar drew between them, from the first card's
#     side that faces the second;
#   * with `beforeUi` given (an earlier checkout's admin UI package), the
#     box switches to it (specialisation `before`) and the same states are
#     taken again for the pictures' "before" column. The flake check runs
#     without it:
#
#   nix build --impure --expr '(import ./nix/tests/quick-actions.nix {
#     pkgs = (builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux;
#     beforeUi = (builtins.getFlake "git+file:///path?rev=<sha>").packages.x86_64-linux.losos-admin-ui; })'
#
# Diagrams draws on SVG only (nix/tests/diagrams-common.nix): the VMs have
# no GPU. nix/cluster.nix is not imported.
{
  pkgs,
  beforeUi ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  svgOnly = shipped: (import ./diagrams-common.nix { inherit pkgs shipped; }).ui;

  saturday = pkgs.writeText "Saturday.md" ''
    # Saturday at the allotment

    Saturday at the allotment, with everyone and a few tools.

    Check the planting calendar before we sow anything.
  '';

  calendar = pkgs.writeText "Planting calendar.md" ''
    # Planting calendar

    ## May

    Sow the beans after the last frost.

    ## Watering

    Water in the morning, before the sun is on the beds.
  '';

  garden = pkgs.writeText "garden.canvas" (
    builtins.toJSON {
      nodes = [
        {
          id = "seeds";
          type = "text";
          text = "Seeds";
          x = -420;
          y = -60;
          width = 220;
          height = 90;
        }
        {
          id = "seedlings";
          type = "text";
          text = "Seedlings";
          x = -60;
          y = -60;
          width = 220;
          height = 90;
        }
        {
          id = "beds";
          type = "text";
          text = "Beds";
          x = 300;
          y = -60;
          width = 220;
          height = 90;
        }
      ];
      edges = [
        {
          id = "plant-out";
          fromNode = "seedlings";
          fromSide = "right";
          toNode = "beds";
          toSide = "left";
        }
      ];
    }
  );

  shots = pkgs.writeShellScriptBin "quick-actions-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    export FFMPEG=${pkgs.ffmpeg-headless}/bin/ffmpeg
    exec ${pkgs.bun}/bin/bun ${./quick-actions-shots.mjs} "$@"
  '';
in

pkgs.testers.nixosTest {
  name = "losos-quick-actions";
  # Under TCG the two passes of pictures take most of an hour each.
  globalTimeout = 3 * 3600;

  nodes = {
    box = _: {
      imports = [
        ../../modules/options.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/daemon.nix
        ../../modules/identity.nix
        ../../modules/cloud.nix
      ];
      networking.hostName = "mattbox";
      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        backend.plugins = lososPkgs.losos-plugins;
        admin.ui = svgOnly lososPkgs.losos-admin-ui;
        cloud.enable = true;
        git.enable = false;
      };
      specialisation = lib.mkIf (beforeUi != null) {
        before.configuration.losos.admin.ui = lib.mkForce (svgOnly beforeUi);
      };
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
      ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };

    laptop = _: {
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
        shots
      ];
      environment.etc = {
        "losos-test/Saturday.md".source = saturday;
        "losos-test/Planting calendar.md".source = calendar;
        "losos-test/garden.canvas".source = garden;
      };
      fonts.packages = [ pkgs.dejavu_fonts ];
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        diskSize = 3072;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json
    import urllib.parse

    BOX = "192.168.1.1"
    BASE = f"https://{BOX}"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    BEFORE = ${if beforeUi == null then "False" else "True"}
    DAV = f"{BASE}/remote.php/webdav/"
    Q = urllib.parse.quote

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("losos-cloud.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def dav(path):
        return curl(f"-f -u 'matus:{PASSWORD}' '{DAV}{Q(path)}'")

    def put(local, path):
        code = curl(f"-u 'matus:{PASSWORD}' -T '{local}' -o /dev/null -w '%{{http_code}}' '{DAV}{Q(path)}'")
        assert code in ("201", "204"), (path, code)

    with subtest("the owner claims the box"):
        claim = json.dumps({"name": "matus", "password": PASSWORD})
        laptop.wait_until_succeeds(f"curl -sfk {BASE}/api/setup/claim | jq -e .ready", timeout=600)
        out = curl(f"-w '\\n%{{http_code}}' -X POST {BASE}/api/setup/claim -H 'Content-Type: application/json' -d '{claim}'")
        body, code = out.rsplit("\n", 1)
        assert code == "200", (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)

    with subtest("a document, a note and a diagram go into LosOS Cloud"):
        laptop.wait_until_succeeds(
            f"curl -sfk -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 0' {DAV} -o /dev/null", timeout=120
        )
        for folder in ["Notes", "Diagrams"]:
            code = curl(f"-u 'matus:{PASSWORD}' -X MKCOL -o /dev/null -w '%{{http_code}}' '{DAV}{folder}'")
            assert code in ("201", "405"), (folder, code)
        put("/etc/losos-test/Saturday.md", "Notes/Saturday.md")
        put("/etc/losos-test/Planting calendar.md", "Notes/Planting calendar.md")
        put("/etc/losos-test/garden.canvas", "Diagrams/garden.canvas")

    with subtest("Chromium uses the bar in Office, Notes and Diagrams"):
        laptop.succeed("rm -rf /tmp/shots")
        try:
            laptop.succeed(
                f"quick-actions-shots {BASE} matus '{PASSWORD}' /tmp/shots/after --test losos-quick-actions >&2",
                timeout=3600,
            )
        finally:
            laptop.copy_from_machine("/tmp/shots/after", "shots")

    with subtest("the document holds what the bar wrote"):
        doc = dav("Notes/Saturday.md")
        for want in [
            "with **everyone** and",
            "Saturday at the [allotment](https://losos.dasmat.us/handbook/), with",
            "Check the planting [[Planting calendar|calendar]] before",
        ]:
            assert want in doc, f"{want!r} not in {doc!r}"

    with subtest("the diagram holds the colour and the arrow"):
        canvas = json.loads(dav("Diagrams/garden.canvas"))
        nodes = {n["id"]: n for n in canvas["nodes"]}
        assert nodes["seeds"].get("color") == "4", nodes["seeds"]
        assert nodes["seedlings"].get("color") == "4", nodes["seedlings"]
        assert "color" not in nodes["beds"], nodes["beds"]
        drawn = [e for e in canvas["edges"] if e["fromNode"] == "seeds" and e["toNode"] == "seedlings"]
        assert len(drawn) == 1, canvas["edges"]
        assert drawn[0].get("fromSide") == "right" and drawn[0].get("toSide") == "left", drawn[0]

    if BEFORE:
        with subtest("the pictures: the same states before"):
            put("/etc/losos-test/Saturday.md", "Notes/Saturday.md")
            put("/etc/losos-test/garden.canvas", "Diagrams/garden.canvas")
            box.succeed("/run/current-system/specialisation/before/bin/switch-to-configuration test")
            laptop.succeed(
                f"quick-actions-shots {BASE} matus '{PASSWORD}' /tmp/shots/before --before >&2", timeout=3600
            )
            laptop.copy_from_machine("/tmp/shots/before", "shots")
  '';
}
