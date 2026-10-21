# nixos-test-vms config for Your apps and the builder that writes them
# (backend/src/{builder,user_apps,jsx}.rs, admin-ui/app/src/apps/,
# admin-ui/app/src/app-frame/).
#
# A box with lososd, LosOS Login and nginx, and a laptop on the LAN whose
# Chromium does what the owner does: signs in at /auth/, opens Your apps,
# describes an app, watches the box write it, tries it in the preview,
# saves it and uses it on its own page. Every screen on the way is a PNG
# (nix/tests/app-builder/shots.mjs), the pull request's and the launch
# film's, so what they show is what this run did.
#
# The model is a stand-in (app-builder/ollama-stand-in.py): the VM has no
# network to pull Qwen2.5-Coder from, and the real model under TCG would
# take hours. It replays, at the real model's pace on a two-core box, an
# answer Qwen2.5-Coder 1.5B wrote for the same request through lososd's own
# builder (app-builder/chore-chart.md). Everything after the answer is real:
# lososd parses it, checks and compiles the JSX with oxc, keeps the app and
# its data; nginx serves /app-frame/ under its own policy; the app runs in
# the sandboxed frame and writes through the page to lososd.
#
# What it asserts beyond "the screens were taken without an error": lososd
# asked for the configured model with the app prompt and the typed request;
# the saved app is on the box with its compiled script; what was typed into
# the app on its own page is in the app's data file; and the app frame's
# runtime and policy are what the page got.
#
#   nix build .#checks.x86_64-linux.losos-app-builder
#
# `ui` swaps the admin pages (the pull request's Before column is this run
# with main's pages and `mode = "before"`).
{
  pkgs,
  ui ? null,
  mode ? "after",
  model ? "qwen2.5-coder:1.5b",
}:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # LosOS Lab is not on any screen here, and its Rust core, GPU canvas and
  # libvirt client take most of an hour to build: stand-ins with the same
  # module names keep `tsc` and Vite happy, and the Lab page is never
  # opened.
  labStubs = ''
    mkdir -p src/lab/core-pkg src/lab/virt-pkg src/lab/render-pkg/webgl2 src/lab/render-pkg/webgpu
    stub() {
      echo 'export default async function init() {} export class Lab {} export class LabCanvas {} export class VirtClient {} export class VirtConsole {}' > "$1.js"
      echo 'export default function init(...a: any[]): Promise<any>; export class Lab { static [k: string]: any; constructor(...a: any[]); [k: string]: any } export class LabCanvas { static [k: string]: any; constructor(...a: any[]); [k: string]: any } export class VirtClient { static [k: string]: any; constructor(...a: any[]); [k: string]: any } export class VirtConsole { static [k: string]: any; constructor(...a: any[]); [k: string]: any }' > "$1.d.ts"
      : > "$1_bg.wasm"
    }
    stub src/lab/core-pkg/losos_lab_core
    stub src/lab/virt-pkg/losos_lab_virt
    stub src/lab/render-pkg/webgl2/losos_lab_render
    stub src/lab/render-pkg/webgpu/losos_lab_render
  '';
  adminUi =
    if ui != null then ui else lososPkgs.losos-admin-ui.overrideAttrs (_: { preBuild = labStubs; });

  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';
  standIn =
    name:
    pkgs.hello
    // {
      imageName = name;
      imageTag = "test";
    };

  # Measured on two cores of a mini PC's class of processor (4 threads of
  # the same machine halved): Qwen2.5-Coder 1.5B reads the prompt at about
  # this many tokens a second and writes at about this many.
  promptRate = "47";
  writeRate = "9";

  # Playwright for Node from the admin pages' own lock file, the version
  # nix/tests/admin-ui.nix pins to nixpkgs' browsers.
  playwright = pkgs.buildNpmPackage {
    pname = "losos-shots-playwright";
    version = "0.1.0";
    src = ../../admin-ui/app;
    inherit (lososPkgs.losos-admin-ui) npmDeps;
    npmFlags = [ "--ignore-scripts" ];
    dontNpmBuild = true;
    installPhase = ''
      mkdir -p $out/node_modules
      cp -r node_modules/playwright node_modules/playwright-core $out/node_modules/
    '';
  };
  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
in
pkgs.testers.nixosTest {
  name = "losos-app-builder";

  nodes = {
    box =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/containers.nix
          ../../modules/tls.nix
          ../../modules/daemon.nix
          ../../modules/identity.nix
          ../../modules/workloads.nix
        ];
        # configuration.nix is not imported; the claim checks Host against
        # the kernel's host name.
        networking.hostName = "mattbox";
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          admin.ui = adminUi;
          nextcloud.mode = "container";
          forgejo.enable = true;
          forgejo.mode = "container";
          workloads.nextcloudImage = standIn "losos-nextcloud-stand-in";
          workloads.forgejoImage = standIn "losos-forgejo-stand-in";
        };
        systemd.services.lososd.path = lib.mkForce [
          stubOcc
          pkgs.coreutils
          pkgs.curl
        ];
        systemd.services.lososd.environment = {
          LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
          # What modules/builder.nix sets, pointed at the stand-in.
          LOSOS_BUILDER_URL = "http://127.0.0.1:11434";
          LOSOS_BUILDER_MODEL = model;
          LOSOS_BUILDER_CONTEXT = "12288";
        };
        systemd.services.ollama-stand-in = {
          wantedBy = [ "multi-user.target" ];
          before = [ "lososd.service" ];
          serviceConfig.ExecStart = lib.concatStringsSep " " [
            "${pkgs.python3}/bin/python3"
            "${./app-builder/ollama-stand-in.py}"
            "11434"
            model
            promptRate
            writeRate
            "${./app-builder/chore-chart.md}"
            "${./app-builder/chore-chart-fixed.md}"
          ];
        };
        environment.systemPackages = [ pkgs.jq ];
        virtualisation = {
          memorySize = 2048;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    laptop =
      { ... }:
      {
        networking.hosts."192.168.1.1" = [ "mattbox.local" ];
        environment.systemPackages = [
          pkgs.curl
          pkgs.nodejs
        ];
        environment.variables.PLAYWRIGHT_BROWSERS_PATH = "${browsers}";
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

    BOX = "https://mattbox.local"
    PASSWORD = "Correct-Horse-Battery-9"
    MODE = "${mode}"

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("ollama-stand-in.service")
    box.wait_for_open_port(443)
    box.wait_for_open_port(11434)
    laptop.wait_for_unit("multi-user.target")

    with subtest("the owner claims the box and LosOS Login starts"):
        body = json.dumps({"name": "matus", "password": PASSWORD})
        out = laptop.succeed(
            f"curl -sk -X POST {BOX}/api/setup/claim -H 'Content-Type: application/json' -d '{body}'"
        )
        assert json.loads(out)["name"] == "matus", out
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds("grep -q \"domain: 'mattbox.local'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)

    if MODE == "after":
        with subtest("the app frame is the box's own runtime under its own policy"):
            head = laptop.succeed(f"curl -sk -D - -o /dev/null {BOX}/app-frame/")
            assert "script-src 'self' 'unsafe-inline'" in head, head
            laptop.succeed(f"curl -skf {BOX}/app-frame/runtime.js | grep -q LosOS")
            laptop.succeed(f"curl -skf {BOX}/app-frame/runtime.css -o /dev/null")

    with subtest("Chromium on the laptop takes the screens"):
        laptop.succeed(
            "mkdir -p /tmp/run && ln -sfn ${playwright}/node_modules /tmp/run/node_modules "
            "&& cp ${./app-builder/shots.mjs} /tmp/run/shots.mjs && cd /tmp/run && "
            f"BOX={BOX} OWNER=matus PASSWORD={PASSWORD} OUT=/tmp/shots MODE={MODE} "
            "timeout 3600 node shots.mjs >&2"
        )
        laptop.copy_from_machine("/tmp/shots", "")

    if MODE == "after":
        with subtest("lososd asked the model for an app, with the request typed"):
            asked = [json.loads(l) for l in box.succeed("cat /tmp/ollama-requests.jsonl").splitlines()]
            assert asked, "the model was never asked"
            first = asked[0]
            assert first["model"] == "${model}", first["model"]
            assert first["messages"][0]["role"] == "system", first["messages"][0]
            assert "small apps for LosOS" in first["messages"][0]["content"]
            assert first["messages"][-1]["content"].startswith("A chore chart for the kids"), first["messages"][-1]

        with subtest("the saved app and what was typed into it are on the box"):
            apps = json.loads(box.succeed("cat /var/lib/losos/user-apps.json"))
            [app] = apps["apps"]
            assert app["id"] == "u1" and app["name"] == "Chores", app
            assert "function App()" in app["source"], app["source"][:200]
            assert "window.LosOS.mount(App)" in app["code"], app["code"][-200:]
            data = json.loads(box.succeed("cat /var/lib/losos/user-apps/u1.json"))
            tasks = [c["task"] for c in data["collections"]["chores"]]
            assert tasks == ["Water the plants"], tasks
  '';
}
