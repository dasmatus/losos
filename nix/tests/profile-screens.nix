# Screenshots of profiles on a booted box (nix/tests/profile-screens.mjs).
#
# The box of nix/tests/profile-box.nix with the real admin pages, and a
# laptop that opens them in Chromium, signed in to LosOS Login as the owner,
# and walks Your account: a picture and a banner through the crop dialog, a
# colour, pronouns and a line, then the top bar and the Users pane. Each
# step is a PNG at 1280 x 900 CSS pixels, twice the density, dark and then
# light, beside a manifest (nix/tests/screens.mjs). They come out of the VM
# into the test's output directory under `shots/<scheme>/`.
#
# `before = true` boots the same box with no profiles to show: Your account
# and the Users pane only, for the Before column of a pull request. Pass the
# older lososd and admin pages as `lososCtl` and `adminUi`.
#
# The picture and the banner are drawn here; nothing else goes on screen.
{
  pkgs,
  adminUi ? (import ../packages.nix { inherit pkgs; }).losos-admin-ui,
  lososCtl ? (import ../packages.nix { inherit pkgs; }).losos-ctl,
  before ? false,
}:

let
  # Pillow for the size check, and Chromium and playwright-core from
  # nixpkgs' playwright-driver (the version nix/tests/admin-ui.nix holds
  # admin-ui/app's pin to), run by Bun.
  python = pkgs.python3.withPackages (ps: [ ps.pillow ]);
  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  scripts = pkgs.runCommand "profile-screens-scripts" { } ''
    mkdir -p $out/node_modules
    ln -s ${pkgs.playwright-driver} $out/node_modules/playwright-core
    cp ${./screens.mjs} $out/screens.mjs
    cp ${./profile-screens.mjs} $out/profile-screens.mjs
  '';
  # The owner's picture, a figure drawn in the palette, and a banner: a
  # river at dusk. No brand mark and no photo, so neither reads as the logo.
  pictures =
    pkgs.runCommand "profile-screens-pictures"
      {
        nativeBuildInputs = [ (pkgs.python3.withPackages (ps: [ ps.pillow ])) ];
      }
      ''
        mkdir -p $out
        python3 - $out/picture.png <<'EOF'
        import sys
        from PIL import Image, ImageDraw
        # A head and shoulders on a blue ground, in the palette's blue, teal
        # and green, drawn at twice the size and scaled down for clean edges.
        S = 2048
        img = Image.new("RGB", (S, S))
        d = ImageDraw.Draw(img)
        for y in range(S):
            t = y / S
            d.line([(0, y), (S, y)], fill=(47 - int(20 * t), 95 + int(20 * t), 158 - int(30 * t)))
        d.ellipse([S * 0.18, S * 0.62, S * 0.82, S * 1.30], fill=(14, 110, 125))
        d.rectangle([S * 0.44, S * 0.50, S * 0.56, S * 0.68], fill=(226, 239, 241))
        d.ellipse([S * 0.32, S * 0.20, S * 0.68, S * 0.60], fill=(226, 239, 241))
        d.chord([S * 0.30, S * 0.16, S * 0.70, S * 0.48], 180, 360, fill=(46, 115, 87))
        img.resize((1024, 1024), Image.LANCZOS).save(sys.argv[1])
        EOF
        python3 - $out/banner.png <<'EOF'
        import sys
        from PIL import Image, ImageDraw
        W, H = 2400, 800
        img = Image.new("RGB", (W, H))
        d = ImageDraw.Draw(img)
        # Sky to river at dusk, in the palette's teal, with the moon up.
        for y in range(H):
            t = y / H
            d.line([(0, y), (W, y)], fill=(14 + int(40 * t), 110 - int(50 * t), 125 - int(30 * t)))
        d.ellipse([1700, 150, 1820, 270], fill=(226, 239, 241))
        # Two hills and the water line.
        d.polygon([(0, 470), (500, 330), (1000, 450), (1500, 360), (2400, 480), (2400, 800), (0, 800)], fill=(10, 46, 52))
        for i in range(9):
            y = 560 + i * 26
            d.line([(0, y), (W, y + 6)], fill=(36 + i * 3, 120 + i * 4, 132 + i * 3), width=3)
        img.save(sys.argv[1])
        EOF
      '';
  schemes =
    if before then
      [
        "light"
        "dark"
      ]
    else
      [
        "dark"
        "light"
      ];
  mode = if before then "before" else "after";
in

pkgs.testers.nixosTest {
  name = "losos-profile-screens";

  nodes = {
    box = import ./profile-box.nix { inherit pkgs adminUi lososCtl; };

    laptop = _: {
      environment.systemPackages = [
        pkgs.curl
        pkgs.bun
        python
      ];
      environment.variables.PLAYWRIGHT_BROWSERS_PATH = "${browsers}";
      networking.hosts."192.168.1.1" = [ "mattbox.local" ];
      fonts.packages = [ pkgs.inter ];
      fonts.fontconfig.defaultFonts.sansSerif = [ "Inter" ];
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

    BOX = "mattbox.local"
    PASSWORD = "Correct-Horse-Battery-9"

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def api(path, cookie, method="GET", data=None):
        args = f"-X {method} 'https://{BOX}{path}' -H 'Cookie: {cookie}'"
        if data is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        body, code = curl(f"-w '\\n%{{http_code}}' {args}").rsplit("\n", 1)
        assert code == "200", (path, code, body)
        return body

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    with subtest("the owner claims the box and adds Eva"):
        code = curl(
            f"-o /dev/null -w '%{{http_code}}' -X POST https://{BOX}/api/setup/claim "
            f"-H 'Content-Type: application/json' "
            f"-d '{json.dumps({'name': 'matus', 'password': PASSWORD})}'"
        )
        assert code == "200", code
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        laptop.wait_until_succeeds(f"curl -skf https://{BOX}/auth/api/health", timeout=120)
        matus = sign_in("matus", PASSWORD)
        api("/api/account", matus, "POST", {"displayName": "Matus", "email": "matus@mattbox.local"})
        api("/api/people", matus, "POST", {
            "name": "eva", "displayName": "Eva Novak", "email": "eva@mattbox.local",
            "password": "Eva-Long-Password-3", "groups": ["losos-account", "losos-backup"],
        })

    def reset():
        """Back to no profile, so each colour scheme starts the same way."""
        if ${if before then "True" else "False"}:
            return
        for kind in ["picture", "banner"]:
            api(f"/api/account/{kind}", matus, "DELETE")
        api("/api/account/profile", matus, "POST", {"pronouns": "", "about": "", "accent": None})

    for scheme in ${builtins.toJSON schemes}:
        with subtest(f"the profile screens, {scheme}"):
            reset()
            laptop.succeed(
                f"cd ${scripts} && bun profile-screens.mjs ${mode} {scheme} /tmp/shots/{scheme} ${pictures}"
                " >&2"
            )
            steps = json.loads(laptop.succeed(f"cat /tmp/shots/{scheme}/manifest.json"))["steps"]
            assert len(steps) >= 2, steps
            for step in steps:
                size = laptop.succeed(
                    f"python3 -c \"from PIL import Image; print(*Image.open('/tmp/shots/{scheme}/{step['file']}').size)\""
                ).split()
                assert size == ["2560", "1800"], (step, size)

    laptop.succeed("tar -C /tmp -cf /tmp/shots.tar shots")
    laptop.copy_from_machine("/tmp/shots.tar", "")
  '';
}
