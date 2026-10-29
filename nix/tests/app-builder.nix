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
# take hours. It replays, at the real model's pace on a two-core box, the
# three answers Qwen2.5-Coder 1.5B wrote for the same request through
# lososd's own builder and a real page: the app (app-builder/chore-chart.md),
# whose choice of child starts empty, so the try keeps nothing; its fix
# (chore-chart-fix-1.md), which reads the list of children before the line
# that makes it, so the check stops it; and the fix of that
# (chore-chart-fix-2.md). Everything around the answers is real: lososd runs
# the loop, checks and compiles the JSX with oxc, asks the page to try each
# version and words what it found; the laptop's Chromium tries the app in the
# preview; nginx serves /app-frame/ under its own policy; the saved app runs
# in the sandboxed frame and writes through the page to lososd.
#
# What it asserts beyond "the screens were taken without an error": lososd
# asked for the configured model with the app prompt and the typed request,
# then twice more with what the try and the check found, in words;
# the saved app is on the box with its compiled script; what was typed into
# the app on its own page is in the app's data file; and the app frame's
# runtime and policy are what the page got.
#
#   nix build .#checks.x86_64-linux.losos-app-builder
#
# `uiSource` swaps the admin pages' source (the pull request's Before
# column is this run with main's admin-ui/app and `mode = "before"`).
{
  pkgs,
  uiSource ? null,
  mode ? "after",
  model ? "qwen2.5-coder:1.5b",
}:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # Diagrams is not on any screen here.
  adminUi = import ./admin-ui-without-lab.nix (
    lososPkgs.losos-admin-ui.overrideAttrs (
      _: pkgs.lib.optionalAttrs (uiSource != null) { src = uiSource; }
    )
  );

  # /etc/nixos as the installer leaves it, so the configuration sync that
  # makes the owner's account on LosOS Git has a repository to publish.
  configTree = pkgs.writeShellScript "losos-config-tree" ''
    [ -e /etc/nixos/.git ] && exit 0
    install -d /etc/nixos/modules
    cp ${../../modules/overrides.nix} /etc/nixos/modules/overrides.nix
    chmod 0644 /etc/nixos/modules/overrides.nix
    ${pkgs.gitMinimal}/bin/git -C /etc/nixos init -q -b main
    ${pkgs.gitMinimal}/bin/git -C /etc/nixos add -A
    ${pkgs.gitMinimal}/bin/git -C /etc/nixos -c user.name=installer -c user.email=i@x commit -qm "Installed"
  '';

  # Measured with two threads while a build kept the other cores busy:
  # Qwen2.5-Coder 1.5B reads the prompt at about this many tokens a second
  # and writes at about this many.
  promptRate = "45";
  writeRate = "7";

  # Playwright from the admin pages' own node_modules (nix/bun-deps.nix),
  # the version nix/tests/admin-ui.nix pins to nixpkgs' browsers.
  playwright = lososPkgs.losos-admin-ui.nodeModules;
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
          ../../modules/git.nix
          ../../modules/config-repo.nix
        ];
        # configuration.nix is not imported; the claim checks Host against
        # the kernel's host name.
        networking.hostName = "mattbox";
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          # The look (and with it the widget editor) is a plugin.
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = adminUi;
          # Each saved app goes to a private repository of its own there.
          git.enable = true;
          configRepo.enable = true;
        };
        systemd.services.config-tree = {
          wantedBy = [ "lososd.service" ];
          before = [ "lososd.service" ];
          serviceConfig = {
            Type = "oneshot";
            ExecStart = configTree;
          };
        };
        # The laptop's Chromium runs under TCG here; give its try time.
        systemd.services.lososd.environment.LOSOS_BUILDER_TRY_SECS = "180";
        systemd.services.lososd.path = lib.mkForce [
          pkgs.coreutils
          pkgs.curl
          pkgs.gitMinimal
        ];
        systemd.services.lososd.environment = {
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
            "${./app-builder/chore-chart-fix-1.md}"
            "${./app-builder/chore-chart-fix-2.md}"
          ];
        };
        environment.systemPackages = [
          pkgs.jq
          pkgs.gitMinimal
        ];
        virtualisation = {
          memorySize = 2048;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    laptop = _: {
      networking.hosts."192.168.1.1" = [ "mattbox.local" ];
      environment.systemPackages = [
        pkgs.curl
        pkgs.gitMinimal
        pkgs.bun
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
    box.wait_for_open_port(8082)
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
            "timeout 3600 bun shots.mjs >&2"
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

        with subtest("the try and the check sent the model back, each with what it found"):
            assert len(asked) == 3, [m["messages"][-1]["content"][:120] for m in asked]
            tried = asked[1]["messages"][-1]["content"]
            assert 'With "Child" left at its first choice, pressing "Add" keeps nothing' in tried, tried
            assert tried.endswith("It is still the app for: A chore chart for the kids"), tried
            checked = asked[2]["messages"][-1]["content"]
            assert "Line 4: children is used before line 5, which makes it" in checked, checked
            TOKEN = box.succeed("systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -").strip()
            builds = json.loads(box.succeed(f"curl -sf -H 'Authorization: Bearer {TOKEN}' 127.0.0.1:8082/api/builder"))
            build = json.loads(box.succeed(
                f"curl -sf -H 'Authorization: Bearer {TOKEN}' 127.0.0.1:8082/api/builder/builds/{builds['builds'][0]['id']}"
            ))
            kinds = [step["kind"] for step in build["steps"]]
            assert kinds == ["write", "check", "try", "fix", "check", "fix", "check", "try"], kinds
            last = build["steps"][-1]
            assert not last["skipped"] and last["problems"] == [], last
            assert build["status"] == "done" and build["problems"] == [], build

        with subtest("the saved app and what was typed into it are on the box"):
            apps = json.loads(box.succeed("cat /var/lib/losos/user-apps.json"))
            [app] = apps["apps"]
            assert app["id"] == "u1" and app["name"] == "Chores", app
            assert "function App()" in app["source"], app["source"][:200]
            assert "window.LosOS.mount(App)" in app["code"], app["code"][-200:]
            data = json.loads(box.succeed("cat /var/lib/losos/user-apps/u1.json"))
            tasks = [c["task"] for c in data["collections"]["chores"]]
            assert tasks == ["Water the plants"], tasks

        GIT_ADMIN = box.succeed("cat /var/lib/losos-git/admin-token").strip()
        BARE = "/var/lib/losos-git/repositories/notshared/app-chores.git"

        def remote_head():
            return box.succeed(f"git -c safe.directory='*' --git-dir {BARE} rev-parse main").strip()

        def app_now():
            return json.loads(box.succeed(
                f"curl -sf -H 'Authorization: Bearer {TOKEN}' 127.0.0.1:8082/api/user-apps/u1"
            ))["app"]

        with subtest("the saved app is in a private repository of its own on LosOS Git"):
            repo = json.loads(box.succeed(
                f"curl -sf -H 'Authorization: token {GIT_ADMIN}' 127.0.0.1:3000/api/v1/repos/notshared/app-chores"
            ))
            assert repo["private"] is True, repo
            local = box.succeed("git --git-dir /var/lib/losos/user-apps/u1.git rev-parse main").strip()
            assert remote_head() == local, (remote_head(), local)
            subjects = box.succeed(f"git -c safe.directory='*' --git-dir {BARE} log --format=%s main").splitlines()
            assert subjects == ["Make Chores"], subjects
            body = box.succeed(f"git -c safe.directory='*' --git-dir {BARE} log -1 --format=%b main")
            assert "> A chore chart for the kids" in body, body
            files = box.succeed(f"git -c safe.directory='*' --git-dir {BARE} ls-tree --name-only main").split()
            assert files == ["README.md", "app.jsx", "losos-app.json"], files
            assert app_now()["repo"] == {"name": "notshared/app-chores", "state": "synced"}, app_now()["repo"]

        with subtest("a change pushed from a clone on the laptop becomes the app"):
            out = box.succeed(
                f"curl -sf -X POST -H 'Authorization: token {GIT_ADMIN}' -H 'Content-Type: application/json' "
                "-d '{\"name\": \"laptop\", \"scopes\": [\"write:repository\", \"read:user\"], \"replace\": true}' "
                "127.0.0.1:3000/api/v1/admin/users/notshared/tokens"
            )
            git_token = json.loads(out)["token"]
            git = "git -c http.sslVerify=false -c user.name=Matus -c user.email=matus@laptop"
            laptop.succeed(
                f"GIT_TERMINAL_PROMPT=0 {git} clone -q https://x:{git_token}@mattbox.local/git/notshared/app-chores.git /tmp/app"
            )
            laptop.succeed("grep -q 'title=\"Chores\"' /tmp/app/app.jsx")
            laptop.succeed("sed -i 's/title=\"Chores\"/title=\"Chores at home\"/' /tmp/app/app.jsx")
            laptop.succeed(f"cd /tmp/app && {git} commit -qam 'Call it Chores at home' && GIT_TERMINAL_PROMPT=0 {git} push -q origin main")
            box.wait_until_succeeds(
                f"curl -sf -H 'Authorization: Bearer {TOKEN}' 127.0.0.1:8082/api/user-apps/u1 "
                "| jq -e '.app.source | contains(\"Chores at home\")'",
                timeout=180,
            )
            taken = app_now()
            assert "Chores at home" in taken["code"], taken["code"][-300:]
            assert taken["repo"]["state"] == "synced", taken["repo"]
            # The data stayed with the app.
            data = json.loads(box.succeed("cat /var/lib/losos/user-apps/u1.json"))
            assert [c["task"] for c in data["collections"]["chores"]] == ["Water the plants"], data
            laptop.succeed(
                "cd /tmp/run && "
                f"BOX={BOX} OWNER=matus PASSWORD={PASSWORD} OUT=/tmp/shots-pushed MODE=pushed FIRST=31 "
                "timeout 1200 bun shots.mjs >&2"
            )
            laptop.copy_from_machine("/tmp/shots-pushed", "")

        with subtest("a push that does not compile is answered in the history and the app kept"):
            laptop.succeed("echo 'function App() { return <p>; }' > /tmp/app/app.jsx")
            laptop.succeed(f"cd /tmp/app && {git} commit -qam 'Break it' && GIT_TERMINAL_PROMPT=0 {git} push -q origin main")
            box.wait_until_succeeds(
                f"curl -sf -H 'Authorization: Bearer {TOKEN}' 127.0.0.1:8082/api/user-apps/u1 "
                "| jq -e '.app.repo.state == \"refused\"'",
                timeout=180,
            )
            kept = app_now()
            assert "Chores at home" in kept["source"], kept["source"][:300]
            assert "does not compile" in kept["repo"]["detail"], kept["repo"]
            laptop.succeed(f"cd /tmp/app && GIT_TERMINAL_PROMPT=0 {git} pull -q --ff-only")
            subject = laptop.succeed("cd /tmp/app && git log -1 --format=%s").strip()
            assert subject == "Keep Chores as it was", subject
            laptop.succeed("grep -q 'Chores at home' /tmp/app/app.jsx")
  '';
}
