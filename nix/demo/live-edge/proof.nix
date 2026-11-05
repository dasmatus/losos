# nixos-test-vms config: the box half of "a file on LosOS Cloud, reached
# through the edge".
#
# A box (front vhost, LosOS Login, LosOS Cloud) is claimed, a person's
# browser (Playwright, in the laptop VM) signs in and uploads a text file in
# the Files page, the desktop client's Login Flow v2 hands out an app
# password, and the file is read back from the box's own WebDAV address the
# way the edge's tunnel delivers it: from loopback, where only an app
# password counts (modules/identity.nix, losos_lan). What the edge adds is
# its public name in front of the same request, which needs a live edge and a
# machine that reaches its tunnel port; docs/provisioning/live-edge/README.md
# has that half.
{ pkgs }:

let
  lososPkgs = import ../../packages.nix { inherit pkgs; };

  python = pkgs.python3.withPackages (ps: [ ps.requests ]);
  flows = pkgs.writeScriptBin "cloud-flows" ''
    #!${python}/bin/python3
    ${builtins.readFile ../../tests/cloud-flows.py}
  '';
  proof = pkgs.writeShellScriptBin "live-edge-proof" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./proof.mjs} "$@"
  '';
in

pkgs.testers.nixosTest {
  name = "losos-live-edge-proof";
  globalTimeout = 2 * 3600;

  nodes = {
    box = _: {
      imports = [
        ../../../modules/options.nix
        ../../../modules/mdns.nix
        ../../../modules/containers.nix
        ../../../modules/tls.nix
        ../../../modules/daemon.nix
        ../../../modules/identity.nix
        ../../../modules/cloud.nix
        ../../../modules/config-repo.nix
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
        configRepo.enable = false;
        mdns.enable = true;
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
        flows
        proof
      ];
      fonts.packages = [ pkgs.dejavu_fonts ];
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    CONTENT = "hello from a file on LosOS Cloud\n"

    # One after the other: two VMs booting at once under TCG can miss the
    # driver's wait for a shell.
    box.start()
    box.succeed("true")
    laptop.start()
    laptop.succeed("true")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-cloud.service")

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")
    laptop.wait_until_succeeds(f"getent hosts {HOST}", timeout=120)

    with subtest("the owner claims the box"):
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} {BASE}/api/setup/claim | jq -e .ready", timeout=600
        )
        out = laptop.succeed(
            f"curl -s --cacert {CA} -w '\\n%{{http_code}}' -X POST {BASE}/api/setup/claim"
            " -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"name": "matus", "password": PASSWORD})}'"
        )
        assert out.rsplit("\n", 1)[1] == "200", out
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)

    with subtest("a browser signs in and uploads a text file in LosOS Cloud"):
        laptop.succeed(f"printf '%s' '{CONTENT}' > /tmp/proof.txt")
        out = laptop.succeed(
            f"live-edge-proof {BASE} matus '{PASSWORD}' /tmp/proof.txt /tmp/files-with-proof.png",
            timeout=1800,
        )
        print(out)
        assert json.loads(out.strip().splitlines()[-1])["uploaded"] == "proof.txt", out
        laptop.copy_from_machine("/tmp/files-with-proof.png", "")
        box.succeed("test -f /var/lib/losos-cloud/notshared/files/proof.txt")

    with subtest("the file is served to an app password, as the tunnel delivers it"):
        got = json.loads(laptop.succeed(f"cloud-flows flow2 {BASE} matus '{PASSWORD}'"))
        app = got["appPassword"]
        url = f"{BASE}/remote.php/webdav/proof.txt"
        # From loopback, the address the edge's tunnel delivers from.
        local = f"--resolve {HOST}:443:127.0.0.1 --cacert /var/lib/losos-tls/cert.pem"
        body = box.succeed(f"curl -sf {local} -u 'notshared:{app}' '{url}'")
        assert body == CONTENT, body
        # The sign-in password is refused there, and so is no password.
        for auth in ("-u 'matus:" + PASSWORD + "'", ""):
            status = box.succeed(
                f"curl -s {local} {auth} -o /dev/null -w '%{{http_code}}' '{url}'"
            ).strip()
            assert status == "401", (auth, status)
        print("PROOF url:", url, "app password:", app[:4] + "…")
  '';
}
