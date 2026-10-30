# nixos-test-vms config for the box's one frame across the admin pages and
# LosOS Cloud (components/app-shell.tsx, components/app-sidebar.tsx,
# src/palette/host.tsx, src/cloud/app.tsx), and for LosOS Cloud taking files
# in, behind the real front vhost, LosOS Login and lososd.
#
# The laptop is a person's computer on the LAN. It
#
#   * copies an ISO-sized file of 4.5 GiB in over WebDAV, as a file manager
#     does, and reads its size and its checksum back;
#   * opens the box in Chromium (nix/tests/cloud-shell.mjs) and, on the
#     Overview, Storage, About, LosOS Cloud's Files, Recent, Notes and
#     Photos, and a document open in LosOS Office, measures the top bar,
#     the logo and the sidebar as soon as the sidebar is drawn and once the
#     page has settled: the same everywhere, and nothing moving after the
#     first paint;
#   * presses Ctrl+Shift+P and types a colon on each, and presses
#     Ctrl+Shift+P with the caret in the document: the command palette
#     opens every time;
#   * uploads a file through LosOS Cloud's Upload button and its file
#     chooser, and watches a file copied in over WebDAV appear in the open
#     folder without a reload.
#
# Its pictures are the pull request's (out/shots/). The same script on a
# box without the fix took the "before" column.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  shell = pkgs.writeShellScriptBin "cloud-shell" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    export PATH=${pkgs.curl}/bin:$PATH
    exec ${pkgs.nodejs}/bin/node ${./cloud-shell.mjs} "$@"
  '';
  # 4.5 GiB: more than any 32-bit length, about a Windows installer's size.
  bigBytes = 4831838208;
in

pkgs.testers.nixosTest {
  name = "losos-cloud-shell";
  globalTimeout = 3 * 3600;

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
        backend.plugins = lososPkgs.losos-plugins;
        admin.ui = lososPkgs.losos-admin-ui;
        cloud.enable = true;
        cloud.photos.faces = false;
        git.enable = false;
        mdns.enable = true;
      };
      environment.systemPackages = [
        pkgs.jq
        pkgs.sqlite
      ];
      virtualisation = {
        # The smallest box LosOS is made for: two cores and 4 GiB, and
        # LosOS Cloud under its own 128 MiB cap (modules/cloud.nix) while
        # gigabytes go through it.
        memorySize = 4096;
        cores = 2;
        diskSize = 12288;
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
        shell
      ];
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

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    DAV = f"{BASE}/remote.php/webdav/"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    BIG = ${toString bigBytes}

    box.start()
    box.succeed("true")
    laptop.start()
    laptop.succeed("true")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-cloud.service")

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CA} {args}")

    with subtest("the laptop finds the box, and the owner claims it"):
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

    auth = f"-u 'matus:{PASSWORD}'"

    with subtest("an ISO-sized file goes in over WebDAV"):
        # A sparse file: the laptop sends every byte, its disk holds none.
        laptop.succeed(f"truncate -s {BIG} /tmp/installer.iso")
        out = curl(
            f"{auth} -T /tmp/installer.iso -w '\\n%{{http_code}}' '{DAV}installer.iso'"
        )
        body, status = out.rsplit("\n", 1)
        print(box.succeed("journalctl -u losos-cloud -n 20 --no-pager || true"))
        assert status in ("201", "204"), (status, body[:400])
        size = int(box.succeed("stat -c %s /var/lib/losos-cloud/notshared/files/installer.iso"))
        assert size == BIG, size
        # What the box says it holds is what was sent.
        head = curl(f"{auth} -I '{DAV}installer.iso'")
        assert f"content-length: {BIG}" in head.lower(), head
        sha = box.succeed(
            "sqlite3 /var/lib/losos-cloud/cloud.db"
            " \"SELECT checksum FROM files WHERE path = 'installer.iso'\" || true"
        ).strip()
        want = laptop.succeed("sha1sum /tmp/installer.iso | cut -d' ' -f1").strip()
        assert sha in ("", f"SHA1:{want}"), (sha, want)
        # Nothing left half-written in the uploads folder.
        assert box.succeed("ls -A /var/lib/losos-cloud/notshared/uploads | wc -l").strip() == "0"
        box.succeed("systemctl is-active losos-cloud.service")
        # Room for the next one: the box's disk is not much larger.
        curl(f"-f {auth} -X DELETE '{DAV}installer.iso'")
        box.succeed("rm -rf /var/lib/losos-cloud/notshared/trash/*")

    with subtest("and one sent in chunks, with no length up front"):
        # How a file manager streams a copy whose size it does not send.
        out = curl(
            f"{auth} -H 'Transfer-Encoding: chunked' -T /tmp/installer.iso"
            f" -w '\\n%{{http_code}}' '{DAV}chunked.iso'"
        )
        body, status = out.rsplit("\n", 1)
        print(box.succeed("journalctl -u losos-cloud -n 20 --no-pager || true"))
        assert status in ("201", "204"), (status, body[:400])
        size = int(box.succeed("stat -c %s /var/lib/losos-cloud/notshared/files/chunked.iso"))
        assert size == BIG, size
        box.succeed("rm /var/lib/losos-cloud/notshared/files/chunked.iso")
        laptop.succeed("rm /tmp/installer.iso")

    with subtest("a few ordinary files"):
        for name in ["Trip plan.md", "Budget.ods", "Notes.txt"]:
            curl(f"-f {auth} -T /etc/hostname '{DAV}{name.replace(' ', '%20')}'")

    with subtest("one frame, the palette everywhere, uploads and live changes"):
        # Every page waits for its own frame, under TCG for minutes.
        laptop.succeed(
            f"cloud-shell {BASE} matus '{PASSWORD}' /tmp/shots >&2", timeout=3600
        )
        report = json.loads(laptop.succeed("cat /tmp/shots/report.json"))
        laptop.copy_from_machine("/tmp/shots", "")

        pages = report["pages"]
        ref = pages[0]["settled"]

        def near(a, b):
            # Whole pixels, rounded in the browser: a frame half a pixel
            # down rounds either way.
            return all(abs(a[k] - b[k]) <= 1 for k in ("x", "y", "w", "h"))

        for p in pages:
            for when in ("first", "settled"):
                f = p[when]
                where = f"{p['slug']} ({when})"
                # The bar's row, the logo and the sidebar sit where the
                # Overview has them, and hold the same entries.
                assert near(f["bar"], ref["bar"]), (where, f["bar"], ref["bar"])
                assert near(f["logo"], ref["logo"]), (where, f["logo"], ref["logo"])
                assert f["sidebar"] is not None, where
                # Its height is its content's.
                assert near(
                    dict(f["sidebar"], h=0), dict(ref["sidebar"], h=0)
                ), (where, f["sidebar"], ref["sidebar"])
                assert f["links"] == ref["links"], (where, f["links"], ref["links"])
                assert f["barSearch"] == 0, where
        # LosOS Cloud is in the one sidebar.
        assert "Cloud" in ref["links"], ref["links"]

        for p in report["palette"]:
            assert p["chord"], p
            if p["where"] != "office-document-text":
                assert p["colon"], p

        assert report["upload"]["listed"], report["upload"]
        box.succeed("test -s /var/lib/losos-cloud/notshared/files/picked-in-browser.bin")
        assert report["live"]["listed"] and report["live"]["sameDocument"], report["live"]
        assert report["errors"] == [], report["errors"]
  '';
}
