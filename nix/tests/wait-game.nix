# nixos-test-vms config for "Play while you wait" (admin-ui/app/src/games/,
# src/components/play-while-waiting.tsx): the game a long wait offers.
#
# The wait is the box's own, not a mocked API: a rebuild that lososd starts
# from Apply as a real `losos-rebuild-<job>` unit and watches like any
# other. A stand-in makes it last as long as the test needs: the
# `nixos-rebuild` lososd runs waits for the script before it exits 0. (The
# wizard's wait is tests/games.browser.mjs's: nothing on a real box has to
# start before the first password any more.) What is asserted:
#
#   * on a claimed box, Apply starts a rebuild unit and the apply bar offers
#     the game; each of the five games plays by keyboard, pointer or touch;
#     Home's applying alert offers it too; dark and phone widths draw it
#     with no sideways scroll;
#   * when the rebuild unit exits, lososd reports done and the game gives
#     way to the line that says the wait is over;
#   * nothing trips the admin page's Content-Security-Policy, served by the
#     real nginx, and nothing on the page throws.
#
# Pictures and a clip land in result/shots/: `nix build
# .#checks.x86_64-linux.losos-wait-game`. The browser is Chromium under
# Playwright in the test driver, reaching the box through a forwarded port
# (the LAN guard counts QEMU's user-mode gateway as the LAN), as in
# nix/tests/tour.nix, whose box this one borrows.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # The admin UI as it ships, without the Lab page, which this test never
  # opens and whose WebAssembly takes longer to build than the test to run.
  ui = import ./admin-ui-without-lab.nix lososPkgs.losos-admin-ui;

  # What lososd's rebuild unit runs: a rebuild that takes as long as the
  # script wants, then succeeds.
  stubRebuild = pkgs.writeShellScriptBin "nixos-rebuild" ''
    echo "building the system for $*"
    while [ ! -e /run/wait-game/release ]; do sleep 1; done
    echo "activating the new system"
  '';

  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  # fonts.nix: the generic families mapped, or the pictures come out serif.
  fonts = import ./fonts.nix { inherit pkgs; };
  driver = ./wait-game-browser.mjs;
in

pkgs.testers.nixosTest {
  name = "losos-wait-game";

  nodes.box = _: {
    imports = [
      ../../modules/options.nix
      ../../modules/containers.nix
      ../../modules/tls.nix
      ../../modules/setup.nix
      ../../modules/daemon.nix
      ../../modules/config-repo.nix
    ];
    losos = {
      hostName = "mattbox";
      backend.package = lososPkgs.losos-ctl;
      admin.ui = ui;
      identity.enable = false;
      git.enable = false;
    };

    # The stand-in first, so it shadows the real nixos-rebuild; git and
    # the rest of lososd's path stay for the settings commit.
    systemd.services.lososd.path = lib.mkBefore [ stubRebuild ];
    systemd.tmpfiles.rules = [ "d /run/wait-game 0755 root root -" ];
    environment.systemPackages = [
      pkgs.curl
      pkgs.jq
    ];
    virtualisation = {
      memorySize = 1536;
      cores = 2;
    };
  };

  testScript = ''
    import json
    import os
    import subprocess
    import time

    PASSWORD = "Correct-Horse-Battery-9"
    OUT = os.environ.get("out", os.getcwd())
    SHOTS = f"{OUT}/shots"
    SYNC = os.path.join(os.environ.get("TMPDIR", "/tmp"), "wait-game-sync")
    os.makedirs(SHOTS, exist_ok=True)
    os.makedirs(SYNC, exist_ok=True)
    PORT = 18081

    box.start()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(80)
    box.wait_until_succeeds("curl -fsS localhost:8082/api/health")
    box.forward_port(host_port=PORT, guest_port=80)

    def browse(mode, on_step, **plan):
        """One run of the browser (nix/tests/wait-game-browser.mjs). It asks
        for each step by writing a file into SYNC; on_step answers it."""
        for name in os.listdir(SYNC):
            os.remove(os.path.join(SYNC, name))
        plan = {"origin": f"http://127.0.0.1:{PORT}", "mode": mode, "out": f"{SHOTS}/{mode}", "sync": SYNC, **plan}
        env = dict(os.environ)
        env.update({
            "PLAYWRIGHT_BROWSERS_PATH": "${browsers}",
            "PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS": "true",
            "FONTCONFIG_FILE": "${fonts}",
            "HOME": os.environ.get("TMPDIR", "/tmp"),
            "LOSOS_PLAYWRIGHT": "${pkgs.playwright-driver}/index.mjs",
        })
        run = subprocess.Popen(
            ["${pkgs.bun}/bin/bun", "${driver}", json.dumps(plan)],
            env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        answered = set()
        deadline = time.time() + 1800
        while run.poll() is None and time.time() < deadline:
            for step in sorted(set(os.listdir(SYNC)) - answered):
                answered.add(step)
                on_step(step)
            time.sleep(0.5)
        if run.poll() is None:
            run.kill()
        stdout, stderr = run.communicate()
        print(stdout)
        print(stderr)
        assert run.returncode == 0, f"{mode}: the browser run failed"
        return json.loads(stdout.strip().splitlines()[-1])

    with subtest("the owner claims the box"):
        body = box.succeed(
            "curl -fsS -X POST 127.0.0.1:8082/api/setup/claim "
            "-H 'Content-Type: application/json' "
            f"-d '{json.dumps({'name': 'matus', 'password': PASSWORD})}'"
        )
        token = json.loads(body)["token"]
        assert len(token) == 64, body

    with subtest("Apply starts a rebuild unit; every game plays while it runs; its end stops the game"):
        box.succeed("mkdir -p /etc/nixos/modules")

        def status():
            return json.loads(box.succeed(f"curl -fsS -H 'Authorization: Bearer {token}' localhost:8082/api/status"))

        def rebuild_step(step):
            if step == "applied":
                box.wait_until_succeeds("systemctl list-units --plain --no-legend 'losos-rebuild-*' | grep -q running", timeout=60)
                assert status()["state"] == "building", status()
            elif step == "release":
                assert status()["state"] == "building", status()
                box.succeed("touch /run/wait-game/release")
            else:
                raise AssertionError(step)

        seen = browse("rebuild", rebuild_step, token=token)
        print(json.dumps(seen, indent=2))
        for name in ["upstream", "eel", "merge", "bricks", "pairs"]:
            assert name in seen["games"], (name, seen)
        assert seen["games"]["upstream"]["score"] > 0, seen
        assert seen["games"]["merge"]["score"] > 0, seen
        assert seen["games"]["pairs"]["score"] == 3, seen
        assert seen["phoneTries"] == 1 and not seen["phoneSideways"], seen
        assert seen["rebuildReady"] and seen["clip"], seen
        assert seen["violations"] == [], seen
        box.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: Bearer {token}' localhost:8082/api/status | jq -e '.state == \"done\"'",
            timeout=60,
        )
        assert "fishbox" in box.succeed("cat /etc/nixos/modules/overrides.nix")

    with subtest("the clip, as an MP4 and as a GIF for the pull request"):
        clip = f"{SHOTS}/rebuild/clip.webm"
        subprocess.run(["${pkgs.ffmpeg}/bin/ffmpeg", "-y", "-loglevel", "error", "-i", clip,
                        "-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart",
                        f"{SHOTS}/rebuild/clip.mp4"], check=True)
        subprocess.run(["${pkgs.ffmpeg}/bin/ffmpeg", "-y", "-loglevel", "error", "-i", clip,
                        "-vf", "fps=12,scale=720:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96[p];[b][p]paletteuse=dither=bayer",
                        f"{SHOTS}/rebuild/clip.gif"], check=True)
  '';
}
