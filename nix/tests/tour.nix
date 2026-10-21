# nixos-test-vms config for the first-run tour (admin-ui/app/src/tour/).
#
# The tour opens over the overview once the wizard is done. Six of its steps
# are on every box; each other step shows a feature and is there only when
# the box reports that feature (src/tour/features.ts), so the tour never
# shows something the owner cannot open. Those reports are lososd's real
# answers on a booted box: GET /api/options (the option document
# modules/config-repo.nix builds from this generation), GET /api/me and
# GET /api/domains. What is asserted:
#
#   * a claimed box, as the wizard leaves it, opens the tour over the
#     overview, served by the real nginx under the real Content-Security-
#     Policy, with the admin UI signed in by the spare key the claim gave;
#   * Build with AI is on by default (losos.builder.enable), so this box's
#     tour has seven steps: the six and Build with AI, in English, Slovak
#     and German, light and dark, with no Chat anywhere;
#   * sharing its storage and joining a mesh adds no chat step here: this
#     lososd lists no shared apps in /api/me, and the step follows that, not
#     the two settings;
#   * with losos.builder.enable off (the specialisation) the tour has the
#     six steps;
#   * Office, lending to AI models, a name on an edge and profiles are not
#     on this box, and none of their steps shows;
#   * every step draws with no page error and no CSP violation beyond the
#     one every load logs (Sonner's stylesheet).
#
# Each step is photographed: `nix build .#checks.x86_64-linux.losos-tour`
# and look in result/shots/. The pictures are taken under reduced motion,
# so each scene shows its composed still.
#
# The browser is Chromium under Playwright (nixpkgs' playwright-core) in the
# test driver, reaching the box through a forwarded port: QEMU delivers it
# from the user-mode gateway, which the LAN guard counts as the LAN.
# LosOS Login is off here (losos.identity.enable): the spare key is the
# sign-in the admin UI keeps in the browser after the wizard, and nothing in
# this test is about LosOS Login. Build with AI's model is not part of this
# box, only its option: the step is about what the option document says.
# LosOS cloud is a stand-in, as in nix/tests/setup.nix: the claim asks it
# one question.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # The admin UI as it ships, without the Lab page, which this test never
  # opens and whose WebAssembly takes longer to build than the test to run.
  ui = lososPkgs.losos-admin-ui.overrideAttrs (_: {
    pname = "losos-admin-ui-without-lab";
    preBuild = ''
      cat > vite.tour-test.config.ts <<'EOF'
      import base from "./vite.config";
      import { join } from "node:path";
      import { fileURLToPath } from "node:url";
      const root = fileURLToPath(new URL(".", import.meta.url));
      const config = base as Record<string, any>;
      config.build.rolldownOptions.input = {
        main: join(root, "index.html"),
        manage: join(root, "manage/index.html"),
      };
      export default config;
      EOF
    '';
    buildPhase = ''
      runHook preBuild
      npx vite build --config vite.tour-test.config.ts
      runHook postBuild
    '';
  });

  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';

  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  fonts = pkgs.makeFontsConf {
    fontDirectories = [
      pkgs.inter
      pkgs.dejavu_fonts
    ];
  };
  driver = ./tour-browser.mjs;
in

pkgs.testers.nixosTest {
  name = "losos-tour";

  nodes.box =
    { config, ... }:
    {
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
        nextcloud.mode = "container";
        forgejo.enable = false;
      };

      # LosOS cloud's stand-in on the port the front vhost proxies to.
      services.nginx.virtualHosts."stub-nextcloud" = {
        listen = [
          {
            addr = "127.0.0.1";
            port = config.losos.nextcloud.apachePort;
          }
        ];
        locations."/".extraConfig = ''return 200 "stub-nextcloud\n";'';
      };

      systemd.services.lososd.path = lib.mkForce [
        stubOcc
        pkgs.coreutils
      ];
      systemd.services.lososd.environment.LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
      ];
      virtualisation = {
        memorySize = 1536;
        cores = 2;
      };

      specialisation.noBuilder.configuration.losos.builder.enable = false;
    };

  testScript = ''
    import json
    import os
    import subprocess

    PASSWORD = "Correct-Horse-Battery-9"
    OVERRIDES = "/etc/nixos/modules/overrides.nix"
    OUT = os.environ.get("out", os.getcwd())
    SHOTS = f"{OUT}/shots"
    os.makedirs(SHOTS, exist_ok=True)
    PORT = 18080

    box.start()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(80)
    box.wait_until_succeeds("curl -fsS localhost:8082/api/health")
    box.forward_port(host_port=PORT, guest_port=80)

    with subtest("the claim makes the owner and hands back the spare key"):
        claim = json.dumps({"name": "matus", "password": PASSWORD})
        body = box.succeed(
            "curl -fsS -X POST 127.0.0.1:8082/api/setup/claim "
            f"-H 'Content-Type: application/json' -d '{claim}'"
        )
        token = json.loads(body)["token"]
        assert len(token) == 64, body

    def share(sharing, mesh):
        """The box's settings as lososd reads them: the overrides file an Apply writes."""
        box.succeed(
            f"mkdir -p $(dirname {OVERRIDES}) && cat > {OVERRIDES} <<'EOF'\n"
            "{ ... }:\n{\n"
            f"  losos.sharingMyStorage = {'true' if sharing else 'false'};\n"
            f"  losos.cluster.enable = {'true' if mesh else 'false'};\n"
            "}\nEOF"
        )
        settings = json.loads(box.succeed(f"curl -fsS -H 'Authorization: Bearer {token}' localhost:8082/api/settings"))
        assert settings["sharingMyStorage"] == sharing and settings["clusterEnable"] == mesh, settings

    def browse(name, **plan):
        """One run of the browser against the box (nix/tests/tour-browser.mjs)."""
        plan = {"origin": f"http://127.0.0.1:{PORT}", "token": token, "out": f"{SHOTS}/{name}", **plan}
        env = dict(os.environ)
        env.update({
            "PLAYWRIGHT_BROWSERS_PATH": "${browsers}",
            "PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS": "true",
            "FONTCONFIG_FILE": "${fonts}",
            "HOME": os.environ.get("TMPDIR", "/tmp"),
            "LOSOS_PLAYWRIGHT": "${pkgs.playwright-driver}/index.mjs",
        })
        run = subprocess.run(
            ["${pkgs.nodejs}/bin/node", "${driver}", json.dumps(plan)],
            env=env, capture_output=True, text=True, timeout=1800,
        )
        print(run.stdout)
        print(run.stderr)
        assert run.returncode == 0, f"{name}: the browser run failed"
        return json.loads(run.stdout.strip().splitlines()[-1])

    STOCK = [
        "Your apps, on your box",
        "Add more apps",
        "See where the space goes",
        "Lend it while you sleep",
        "It updates itself at night",
        "Find this tour again",
    ]
    WITH_BUILDER = STOCK[:4] + ["Describe a widget, get a widget"] + STOCK[4:]

    def options_on(name):
        doc = json.loads(box.succeed(f"curl -fsS -H 'Authorization: Bearer {token}' localhost:8082/api/options"))
        return [o["current"] for o in doc["options"] if o["name"] == name]

    with subtest("this box has Build with AI and nothing else: seven steps, no Chat anywhere"):
        assert options_on("builder.enable") == [True], options_on("builder.enable")
        assert options_on("office.enable") == [] and options_on("inference.lend.enable") == []
        share(False, False)
        seen = browse("local", locales=["en-US", "sk-SK", "de-DE"], themes=["light", "dark"], expect=len(WITH_BUILDER))
        assert seen["titles"] == WITH_BUILDER, seen
        assert not seen["chatInSidebar"], seen

    with subtest("sharing in a mesh adds no chat step where lososd lists no chat"):
        share(True, True)
        me = json.loads(box.succeed(f"curl -fsS -H 'Authorization: Bearer {token}' localhost:8082/api/me"))
        assert "chat" not in me.get("shared", []), me
        seen = browse("shared", locales=["en-US"], themes=["light", "dark"], expect=len(WITH_BUILDER))
        assert seen["titles"] == WITH_BUILDER, seen
        assert not seen["chatInSidebar"], seen

    with subtest("with Build with AI off, the six steps"):
        box.succeed("/run/booted-system/specialisation/noBuilder/bin/switch-to-configuration test")
        box.wait_for_unit("lososd.service")
        box.wait_until_succeeds("curl -fsS localhost:8082/api/health")
        assert options_on("builder.enable") == [False], options_on("builder.enable")
        seen = browse("no-builder", locales=["en-US", "sk-SK", "de-DE"], themes=["light", "dark"], expect=len(STOCK))
        assert seen["titles"] == STOCK, seen

    with subtest("Take the tour under Settings, About opens it again"):
        browse("about", locales=["en-US"], themes=["light"], expect=len(STOCK), about=True, shots=False)
  '';
}
