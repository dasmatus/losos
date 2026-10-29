# nixos-test-vms config for Markdown documents and presentations in LosOS
# Office (admin-ui/app/src/cloud/office/markdown/), behind the real front
# vhost and LosOS Login, with lososd claiming the box. The box runs LosOS
# Cloud and no Collabora: Markdown is Office's own.
#
# The laptop is the owner's computer on the LAN. Chromium
# (nix/tests/office-markdown-shots.mjs) starts a document and a presentation
# from the files bar, writes into them with the formatting bar, and plays
# the presentation full screen; then
#
#   * the box holds both files, written by its shared editor: the document
#     with the Markdown the bar wrote (bold, highlight, a task, a callout, a
#     table, a formula), the presentation with its properties, its slides
#     and its picture;
#   * with `beforeUi` given, the box switches to the admin UI as it was
#     before (specialisation `before`) for the pictures' "before" column.
#     The flake check runs without it.
#
# nix/cluster.nix is not imported. The admin UI is built without LosOS Lab,
# as in nix/tests/notes.nix.
{
  pkgs,
  beforeUi ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  noLab = import ./admin-ui-without-lab.nix;

  # The cloud serves an SVG as a download (it can carry script), so the
  # slide's picture is the vault's tomato drawn as a PNG.
  picture = pkgs.runCommand "tomato.png" { } ''
    ${pkgs.librsvg}/bin/rsvg-convert -w 640 ${../../admin-ui/app/tests/fixtures/vault/attachments/tomato.svg} -o $out
  '';

  shots = pkgs.writeShellScriptBin "office-markdown-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./office-markdown-shots.mjs} "$@"
  '';
in

pkgs.testers.nixosTest {
  name = "losos-office-markdown";

  nodes = {
    box = _: {
      imports = [
        ../../modules/options.nix
        ../../modules/mdns.nix
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
        admin.ui = noLab lososPkgs.losos-admin-ui;
        cloud.enable = true;
        forgejo.enable = false;
        mdns.enable = true;
      };
      specialisation = lib.mkIf (beforeUi != null) {
        before.configuration.losos.admin.ui = lib.mkForce (noLab beforeUi);
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
      services.avahi = {
        enable = true;
        nssmdns4 = true;
      };
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
        shots
      ];
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

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    BEFORE = ${if beforeUi == null then "False" else "True"}
    Q = urllib.parse.quote

    box.succeed("true")
    laptop.succeed("true")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-cloud.service")
    # No Collabora on this box.
    box.fail("systemctl cat coolwsd.service")

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CA} {args}")

    def dav(name):
        return curl(f"-f -u 'matus:{PASSWORD}' '{BASE}/remote.php/webdav/{Q(name)}'")

    with subtest("the owner claims the box"):
        laptop.wait_until_succeeds(f"getent hosts {HOST}", timeout=120)
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} {BASE}/api/setup/claim | jq -e .ready", timeout=600
        )
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST {BASE}/api/setup/claim"
            " -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"name": "matus", "password": PASSWORD})}'"
        )
        assert out.rsplit("\n", 1)[1] == "200", out
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)

    with subtest("a picture for the slides arrives over WebDAV"):
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 0' {BASE}/remote.php/webdav/ -o /dev/null",
            timeout=120,
        )
        laptop.succeed(f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -T ${picture} {BASE}/remote.php/webdav/tomato.png")

    with subtest("Chromium writes a document and a presentation, and plays it"):
        laptop.succeed("rm -rf /tmp/shots")
        try:
            laptop.succeed(
                f"office-markdown-shots {BASE} matus '{PASSWORD}' /tmp/shots/after --test losos-office-markdown >&2"
            )
        finally:
            laptop.copy_from_machine("/tmp/shots/after", "shots")

    with subtest("the box saved what the bar wrote"):
        doc = dav("Untitled document.md")
        for want in [
            "# Untitled document",
            "**everyone**",
            "==tools==",
            "[[Planting calendar]] #garden",
            "- [ ] Bring gloves",
            "- [x] Order the compost",
            "> [!tip]",
            "| 1 | Tomatoes |",
            "$$\nA = w \\times l\n$$",
        ]:
            assert want in doc, f"{want!r} not in {doc!r}"
        deck = dav("Untitled presentation.md")
        assert deck.startswith("---\nslides: true\n---\n\n# Untitled presentation\n"), deck
        assert "![A tomato plant](tomato.png)" in deck, deck
        assert deck.count("\n---\n") == 3, deck
        assert "## This year" in deck, deck

    if BEFORE:
        with subtest("the pictures: the same places before"):
            box.succeed("/run/current-system/specialisation/before/bin/switch-to-configuration test")
            laptop.succeed(f"office-markdown-shots {BASE} matus '{PASSWORD}' /tmp/shots/before --before >&2")
            laptop.copy_from_machine("/tmp/shots/before", "shots")
  '';
}
