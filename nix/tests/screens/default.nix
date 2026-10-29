# Screenshots of a booted box for any VM test: a NixOS module that turns a
# node into a laptop with a browser and `losos-capture` (capture.mjs says
# what a plan looks like). The pictures in a pull request and the frames of
# a film come from here, so they show the real box: lososd, nginx, LosOS
# Login and the built admin UI, never a dev server or a mocked API.
#
#   nodes.laptop = { imports = [ (import ./screens { inherit pkgs; }).browser ]; … };
#   laptop.succeed("losos-capture --base https://192.168.1.1 --plan /etc/plan.json --out /tmp/shots …")
#   laptop.copy_from_machine("/tmp/shots", "")
{ pkgs }:

let
  driver = pkgs.playwright-driver;
  # One face whether the browser runs in a VM or in the driver
  # (nix/tests/fonts.nix).
  fonts = import ../fonts.nix { inherit pkgs; };
  # node with Playwright's own driver as its one module, and Chromium only.
  capture = pkgs.writeShellScriptBin "losos-capture" ''
    export PLAYWRIGHT_BROWSERS_PATH=${driver.browsers-chromium}
    export FONTCONFIG_FILE=${fonts}
    export NODE_PATH=${
      pkgs.linkFarm "losos-capture-modules" [
        {
          name = "playwright-core";
          path = driver;
        }
      ]
    }
    exec ${pkgs.nodejs}/bin/node ${./capture.mjs} "$@"
  '';
in
{
  inherit capture;

  # The handbook's pictures (handbook/docs/img/) for a test script: put
  # `capture` on the laptop and this text in the script, then call
  #
  #   handbook_pictures(laptop, BASE, "matus", PASSWORD, [ {shot}, … ])
  #
  # with shots as in capture.mjs, named as the handbook names its files.
  # They are taken only when the driver runs with LOSOS_HANDBOOK_SHOTS=1,
  # so the test costs nothing more otherwise: light, 1280 x 900 at twice
  # the pixels, once in each language the handbook has, into handbook/en,
  # handbook/sk and handbook/de of the output directory
  # (LOSOS_HANDBOOK_LANGS="sk de" takes only those).
  #
  #   nix build .#checks.x86_64-linux.losos-cloud.driver
  #   LOSOS_HANDBOOK_SHOTS=1 ./result/bin/nixos-test-driver --no-interactive -o out
  #
  # With `proxy="socks5://127.0.0.1:1080"` the browser is the driver's own,
  # as in the tests whose pages are driven from the host through a SOCKS
  # proxy on the laptop, and `machine` only names the output directory.
  handbookScript = ''
    def handbook_pictures(machine, base, user, password, shots, proxy=None):
        import json
        import os
        import subprocess
        if os.environ.get("LOSOS_HANDBOOK_SHOTS") != "1":
            return
        # LOSOS_HANDBOOK_LANGS="sk de" takes only those.
        langs = os.environ.get("LOSOS_HANDBOOK_LANGS", "en sk de").split()
        if proxy is None:
            # The driver's shared directory outlives a run.
            machine.succeed("rm -rf /tmp/shared/handbook")
        for lang, locale in (("en", "en-US"), ("sk", "sk-SK"), ("de", "de-DE")):
            if lang not in langs:
                continue
            def take():
                plan = json.dumps({
                    "width": 1280,
                    "height": 900,
                    "variants": [{
                        "locale": locale,
                        "colorScheme": "light",
                        "dpr": 2,
                        "timezoneId": "Europe/Bratislava",
                        "hideCaret": True,
                    }],
                    "shots": shots,
                })
                if proxy is not None:
                    out = os.path.join(machine.out_dir, "handbook", lang)
                    os.makedirs(out, exist_ok=True)
                    with open(os.path.join(out, "plan.json"), "w") as f:
                        f.write(plan)
                    run = subprocess.run(
                        ["${capture}/bin/losos-capture", "--base", base, "--plan", f.name,
                         "--out", out, "--user", user, "--password", password, "--proxy", proxy],
                        capture_output=True, text=True, timeout=3600,
                    )
                    print(run.stdout, run.stderr)
                    assert run.returncode == 0, f"handbook pictures, {lang}: {run.stderr[-2000:]}"
                    return
                machine.succeed(f"cat > /tmp/handbook-{lang}.json << 'PLAN'\n{plan}\nPLAN")
                machine.succeed(
                    f"losos-capture --base {base} --plan /tmp/handbook-{lang}.json"
                    f" --out /tmp/handbook/{lang} --user {user} --password '{password}'",
                    timeout=3600,
                )
                # Each language as soon as it is taken, so a run cut short keeps it.
                machine.copy_from_machine(f"/tmp/handbook/{lang}", "handbook")
            # A picture the handbook lacks is no reason to fail the test.
            try:
                take()
            except Exception as e:
                print(f"handbook pictures, {lang}: not all taken: {e}")
                if proxy is None:
                    # The ones that were.
                    machine.execute(f"mkdir -p /tmp/handbook/{lang}")
                    machine.copy_from_machine(f"/tmp/handbook/{lang}", "handbook")
  '';

  browser = _: {
    environment.systemPackages = [ capture ];
    # The admin UI's own fonts are bundled; these cover anything else.
    fonts.packages = [ pkgs.dejavu_fonts ];
    # Chromium under TCG wants room.
    virtualisation = {
      memorySize = 2048;
      cores = 2;
    };
  };
}
