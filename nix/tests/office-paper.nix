# nixos-test-vms config for LosOS Office's own documents and presentations
# (admin-ui/app/src/cloud/office/paper/): `.paper` and `.slides`, MDX
# edited as it looks, behind the real front vhost and LosOS Login, with
# lososd claiming the box. The box runs LosOS Cloud and no Collabora.
#
# The laptop is the owner's computer on the LAN. Chromium
# (nix/tests/office-paper-shots.mjs) starts a document and a presentation
# from the files bar, writes into them with the bar above the page and its
# Insert menu, sets the page up, plays the presentation full screen, and
# opens a document full of script; then
#
#   * the box holds both files, written by its shared editor: the document
#     with its page in the front matter, a callout, a table, a figure, a
#     footnote and a footer with the page number, the presentation with
#     three slides and speaker notes;
#   * the script ran nowhere (the page says so) and the file is unchanged;
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
  # document's picture is the vault's tomato drawn as a PNG.
  picture = pkgs.runCommand "tomato.png" { } ''
    ${pkgs.librsvg}/bin/rsvg-convert -w 640 ${../../admin-ui/app/tests/fixtures/vault/attachments/tomato.svg} -o $out
  '';

  # A document with script in every place MDX and HTML could carry it.
  hostile = pkgs.writeText "Hostile.paper" ''
    # Hello

    <script>window.__ran = 1</script>

    <img src="x" onerror="window.__ran = 2" />

    {window.__ran = 3}

    <Callout type="note" onClick="window.__ran = 4">Boo</Callout>

    <a href="javascript:window.__ran = 5">click</a>

    export const x = (window.__ran = 6)
  '';

  shots = pkgs.writeShellScriptBin "office-paper-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./office-paper-shots.mjs} "$@"
  '';
in

pkgs.testers.nixosTest {
  name = "losos-office-paper";

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

    with subtest("a document full of script arrives over WebDAV"):
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 0' {BASE}/remote.php/webdav/ -o /dev/null",
            timeout=120,
        )
        # The picture stays on the laptop: the page uploads it.
        laptop.succeed("cp ${picture} /tmp/tomato.png")
        laptop.succeed(f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -T ${hostile} {BASE}/remote.php/webdav/Hostile.paper")
        # The cloud calls it its own kind of text.
        listing = curl(f"-f -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 1' {BASE}/remote.php/webdav/")
        assert "text/vnd.losos.paper" in listing, listing

    with subtest("Chromium writes a document and a presentation as they look, and plays it"):
        laptop.succeed("rm -rf /tmp/shots")
        try:
            laptop.succeed(
                f"office-paper-shots {BASE} matus '{PASSWORD}' /tmp/shots/after --test losos-office-paper >&2"
            )
        finally:
            laptop.copy_from_machine("/tmp/shots/after", "shots")

    with subtest("the box saved the MDX the page wrote"):
        doc = dav("Untitled document.paper")
        for want in [
            "size: Letter",
            "# Garden day",
            "with **everyone**",
            '<Callout type="tip">',
            "\n| Bed | Crop |",
            '<Figure src="tomato.png"',
            "[^1]",
            "<Footer>",
            "<PageNumber />",
        ]:
            assert want in doc, f"{want!r} not in {doc!r}"
        assert doc.startswith("---\n"), doc
        # The table, picture and callout are the page's, in that order, and
        # the footnote's text comes after all of them.
        order = [doc.index(w) for w in ["\n| Bed | Crop |", "<Figure", '<Callout type="tip">', "\n[^1]:"]]
        assert order == sorted(order), doc
        deck = dav("Untitled presentation.slides")
        assert deck.startswith('<Slide layout="title">\n# Untitled presentation\n'), deck
        assert deck.count("<Slide") == 3, deck
        assert "## This year" in deck, deck
        assert "<Notes>" in deck, deck

    with subtest("the document full of script is as it was"):
        assert dav("Hostile.paper") == box.succeed("cat ${hostile}")

    if BEFORE:
        with subtest("the pictures: the same places before"):
            box.succeed("/run/current-system/specialisation/before/bin/switch-to-configuration test")
            laptop.succeed(f"office-paper-shots {BASE} matus '{PASSWORD}' /tmp/shots/before --before >&2")
            laptop.copy_from_machine("/tmp/shots/before", "shots")
  '';
}
