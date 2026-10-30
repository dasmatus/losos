# nixos-test-vms config for the first-run wizard's last step, "Your desktop"
# (admin-ui/app/src/screens/wizard/StepDesktop.tsx, App.tsx's desktop stage).
#
# A box nobody owns, set up the way its owner does it, in a browser, over
# https at the box's name. What is asserted:
#
#   * step 2 claims the box, and step 3 signs the owner in to the box's real
#     LosOS Login with that password from the page itself, waiting for LosOS
#     Login to start after the claim: no second tab, nothing typed twice;
#   * the tab lets go of the spare key the claim gave it: sessionStorage has
#     no token and lososd knows the tab as `matus` by the session cookie;
#   * the real desktop is drawn behind the card, out of reach (`inert`),
#     with the card docked at its foot;
#   * "Let's go" grows it into the window, setup is over, the tour opens
#     over it, and a reload lands on the desktop signed in, not in setup;
#   * a tab reloaded on step 3 asks for the password once more and goes on
#     the same way, in light and dark, on a desktop and a phone, in Slovak
#     and German, with no page error, no Content-Security-Policy violation
#     beyond the one every load logs (Sonner's stylesheet) and no sideways
#     scroll on a phone.
#
# The pictures and the frames of both motions are in result/shots/:
# `nix build .#checks.x86_64-linux.losos-setup-desktop`.
#
# The browser is Chromium under Playwright (nixpkgs' playwright-core) in the
# test driver, reaching the box's https port through a forwarded port and a
# host-resolver rule, so the page is at mattbox.local, a name LosOS Login
# keeps a cookie for. QEMU delivers the forward from the user-mode gateway,
# which the LAN guard counts as the LAN.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # The admin UI as it ships, without the Lab page, which this test never
  # opens and whose WebAssembly takes longer to build than the test to run.
  ui = import ./admin-ui-without-lab.nix lososPkgs.losos-admin-ui;

  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  # fonts.nix: the generic families mapped, or the pictures come out serif.
  fonts = import ./fonts.nix { inherit pkgs; };
  driver = ./setup-desktop-browser.mjs;
in

pkgs.testers.nixosTest {
  name = "losos-setup-desktop";

  nodes.box = _: {
    imports = [
      ../../modules/options.nix
      ../../modules/containers.nix
      ../../modules/tls.nix
      ../../modules/setup.nix
      ../../modules/daemon.nix
      ../../modules/identity.nix
      ../../modules/cloud.nix
      # The option document, which the wizard asks about the TPM.
      ../../modules/config-repo.nix
    ];
    networking.hostName = "mattbox";
    losos = {
      hostName = "mattbox";
      backend.package = lososPkgs.losos-ctl;
      # The look and the pins, which the desktop asks for.
      backend.plugins = lososPkgs.losos-plugins;
      admin.ui = ui;
      cloud.enable = true;
      git.enable = false;
    };
    # The statistics plugin reads Traefik's metrics there; modules/apps.nix
    # sets it on a full box, and this one has no apps.
    systemd.services.lososd.environment.LOSOS_TRAEFIK_URL = "http://127.0.0.1:8089";
    environment.systemPackages = [
      pkgs.curl
      pkgs.jq
    ];
    virtualisation = {
      memorySize = 2048;
      cores = 2;
      diskSize = 4096;
    };
  };

  testScript = ''
    import json
    import os
    import subprocess

    PASSWORD = "Correct-Horse-Battery-9"
    OUT = os.environ.get("out", os.getcwd())
    SHOTS = f"{OUT}/shots"
    os.makedirs(SHOTS, exist_ok=True)
    PORT = 18443
    # Every load logs Sonner's refused stylesheet; anything else is new.
    SONNER = "style-src-elem"

    box.start()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    box.wait_until_succeeds("curl -fsS localhost:8082/api/health")
    box.forward_port(host_port=PORT, guest_port=443)

    with subtest("nobody owns the box and LosOS Login waits for its first person"):
        claim = json.loads(box.succeed("curl -fsS localhost:8082/api/setup/claim"))
        assert claim["claimed"] is False, claim
        box.fail("systemctl is-active authelia-losos.service")

    with subtest("the owner sets the box up in a browser, and step 3 signs them in"):
        env = dict(os.environ)
        env.update({
            "PLAYWRIGHT_BROWSERS_PATH": "${browsers}",
            "PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS": "true",
            "FONTCONFIG_FILE": "${fonts}",
            "HOME": os.environ.get("TMPDIR", "/tmp"),
            "LOSOS_PLAYWRIGHT": "${pkgs.playwright-driver}/index.mjs",
        })
        plan = {"origin": "https://mattbox.local", "map": f"127.0.0.1:{PORT}", "password": PASSWORD, "out": SHOTS}
        run = subprocess.run(
            ["${pkgs.bun}/bin/bun", "${driver}", json.dumps(plan)],
            env=env, capture_output=True, text=True, timeout=5400,
        )
        print(run.stdout)
        print(run.stderr)
        assert run.returncode == 0, "the browser run failed"
        seen = json.loads(run.stdout.strip().splitlines()[-1])

    with subtest("the claim made the owner, and LosOS Login knows them"):
        box.succeed("systemctl is-active authelia-losos.service")
        people = json.loads(box.succeed("cat /var/lib/losos-identity/users.yml"))
        assert "matus" in people["users"], people
        assert len(seen["token"]) == 64, seen

    with subtest("the preview is the signed-in desktop, out of reach, the spare key let go"):
        p = seen["preview"]
        assert p["stage"] == "preview", p
        assert p["inert"] is True, p
        assert p["docked"] == "true", p
        assert p["token"] is None, p
        assert p["me"] == "matus", p
        assert "signed in as matus" in p["text"], p

    with subtest("Let's go grows it into the window and setup is over"):
        assert seen["growing"] == "live", seen
        a = seen["after"]
        assert a["stage"] is None and a["wizard"] == 0 and a["me"] == "matus", a
        r = seen["reload"]
        assert r["wizard"] == 0 and r["me"] == "matus", r
        assert seen["errors"] == [], seen["errors"]
        assert all(v.startswith(SONNER) for v in seen["violations"]), seen["violations"]

    with subtest("a tab reloaded on step 3 signs in the same way, in every window and language"):
        assert [v["suffix"] for v in seen["variants"]] == [
            "dark-desktop", "light-phone", "dark-phone", "sk-light-desktop", "de-light-desktop",
        ], seen["variants"]
        for v in seen["variants"]:
            assert v["stage"] == "preview" and v["me"] == "matus" and v["after"] is None, v
            assert v["errors"] == [], v
            assert all(x.startswith(SONNER) for x in v["violations"]), v
            assert v["pageWidth"] <= v["width"], v
  '';
}
