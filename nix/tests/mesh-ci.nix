# nixos-test-vms config for Mesh CI (modules/ci.nix, backend/src/ci/): an
# edge with the job queue and two boxes of its mesh, each with LosOS Git,
# lososd and a runner that starts its jobs' containers through pm with Mesh
# CI's pm plugin.
#
#   * a push to a public repository on boxa plans the workflow's two jobs,
#     hands them to the edge's queue, and the two boxes run one each;
#   * a job's service starts beside it, healthy before the steps, reachable
#     by name and on localhost; a container action runs once in the job's
#     network; a step builds a signed pm recipe in pm's jail and gets its
#     archive back as an output;
#   * no step reaches a LAN address, the edge's included;
#   * each job's result is a check on the commit in LosOS Git;
#   * a private repository's job stays on its own box;
#   * a run started by hand is cancelled, and its containers are gone;
#   * pm loads nothing but Mesh CI's signed plugin, and the plugin touches
#     no container but Mesh CI's.
#
# Without KVM the boxes are slow; every wait here is generous for that.
#
# `screens = true` (the check losos-mesh-ci-screens) also gives boxa the
# real admin UI behind LosOS Login and puts a laptop with a browser on the
# LAN, which takes the pull request's pictures of the Mesh CI pages once the
# runs are done. `before`, a path to an earlier admin-ui/app, builds that UI
# instead, boots only boxa and the laptop, and takes the same addresses,
# for the Before column:
#
#   nix build --impure --expr '(import ./nix/tests/mesh-ci.nix {
#     pkgs = (builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux;
#     pmSrc = (builtins.getFlake (toString ./.)).inputs.pm;
#     before = /path/to/main/admin-ui/app; })'
{
  pkgs,
  pmSrc,
  screens ? false,
  before ? null,
}:

let
  lososPkgs = import ../packages.nix { inherit pkgs pmSrc; };
  inherit (lososPkgs) pm;

  stubUi = pkgs.runCommand "stub-admin-ui" { } ''
    mkdir -p $out
    echo "admin ui" > $out/index.html
  '';

  after = before == null;
  shoot = screens || !after;
  shots = import ./screens { inherit pkgs; };
  PASSWORD = "Correct-Horse-Battery-9";

  # The admin UI as it ships, without the Lab page, which this test never
  # opens and whose WebAssembly takes longer to build than the test to run.
  ui = import ./admin-ui-without-lab.nix (
    # The earlier UI shares this one's lock file, so its node_modules too.
    lososPkgs.losos-admin-ui.overrideAttrs (_: lib.optionalAttrs (!after) { src = before; })
  );

  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';

  # The pictures. The run's address is known only once it exists, so the
  # test script writes the run's shots itself (see `shoot_run` below).
  plan = pkgs.writeText "plan-pages.json" (
    builtins.toJSON {
      width = 1280;
      height = 800;
      shots = [
        {
          name = "overview";
          path = "/";
          waitFor = {
            role = "heading";
            name = "mattbox";
          };
        }
        {
          name = "ci";
          path = "/ci";
          waitFor =
            if after then
              {
                role = "button";
                name = "Run workflow";
              }
            else
              { selector = "main"; };
          steps = [ { sleep = 3000; } ];
        }
      ];
    }
  );

  # A service and a container action, loaded into the shared user's
  # Podman on both boxes: nothing is pulled in a VM test. It answers
  # "pong" on port 7000 and says it is ready with a file, once it listens.
  svcImage = pkgs.dockerTools.buildImage {
    name = "lci-test-svc";
    tag = "1";
    copyToRoot = pkgs.buildEnv {
      name = "svc-root";
      paths = [
        pkgs.bashInteractive
        pkgs.coreutils
        pkgs.python3Minimal
        pkgs.dockerTools.binSh
      ];
    };
    extraCommands = ''
      mkdir -p srv tmp
      chmod 1777 tmp
      echo pong > srv/index.html
    '';
    config.Cmd = [
      "python3"
      "-c"
      (lib.concatStringsSep "; " [
        "import functools, http.server, pathlib"
        "h = functools.partial(http.server.SimpleHTTPRequestHandler, directory='/srv')"
        "s = http.server.ThreadingHTTPServer(('', 7000), h)"
        "pathlib.Path('/tmp/ready').touch()"
        "s.serve_forever()"
      ])
    ];
  };

  # A pm recipe, signed with a key made here; the workflow trusts it.
  recipe = pkgs.runCommand "lci-test-recipe" { nativeBuildInputs = [ pm ]; } ''
    mkdir -p $out/pkg
    cat > $out/pkg/build.rhai <<'EOF'
    package(#{
        name: "hello",
        version: "0.1.0",
        dependencies: [],
        steps: [
            step(#{
                stage: Install,
                name: "stage",
                run: [
                    "install -d /dest/share/hello",
                ],
            }),
        ],
    });
    EOF
    export HOME=$TMPDIR XDG_CONFIG_HOME=$TMPDIR/config
    pm sign $out/pkg/build.rhai --key $TMPDIR/key --trust-dir $TMPDIR/trust
    ls $TMPDIR/trust | sed 's/\.pub$//' > $out/key
  '';

  workflows = {
    ci = pkgs.writeText "ci.yml" ''
      name: CI
      on: [push]
      jobs:
        services:
          runs-on: ubuntu-latest
          services:
            web:
              image: localhost/lci-test-svc:1
              options: --health-cmd "test -e /tmp/ready" --health-interval 2s --health-retries 60
          steps:
            - run: curl -fsS http://web:7000/ | grep pong
            - run: curl -fsS http://localhost:7000/ | grep pong
            - run: echo "services ran on $RUNNER_NAME"; sleep 30
            - name: The LAN is out of reach
              run: |
                for target in 192.168.1.1 192.168.1.2 192.168.1.3:8443; do
                  if curl -sS --max-time 5 -o /dev/null "http://$target/"; then
                    echo "reached $target"; exit 1
                  fi
                done
            - uses: docker://localhost/lci-test-svc:1
              with:
                entrypoint: /bin/sh
                args: -c "echo from a container action in $PWD"
        recipe:
          runs-on: ubuntu-latest
          steps:
            - run: echo "recipe ran on $RUNNER_NAME"; sleep 30
            - uses: losos-project/pm@v0
              id: pm
              with:
                recipe: pkg/build.rhai
                trust: KEY
            - run: test -f "''${{ steps.pm.outputs.archive }}" && echo "archive ''${{ steps.pm.outputs.archive }}"
    '';
    slow = pkgs.writeText "slow.yml" ''
      name: Slow
      on: [workflow_dispatch]
      jobs:
        wait:
          runs-on: ubuntu-latest
          steps:
            - run: echo started; sleep 900
    '';
    private = pkgs.writeText "private.yml" ''
      name: Private
      on: [push]
      jobs:
        home:
          runs-on: ubuntu-latest
          steps:
            - run: echo "private ran on $RUNNER_NAME"
    '';
  };

  edgeNode = _: {
    networking.firewall.allowedTCPPorts = [ 8443 ];
    environment.etc."losos-test/tenants.json".text = builtins.toJSON {
      boxa = {
        hostname = "mattbox";
        token_file = "/var/lib/reg/boxa.token";
        cluster = true;
      };
      boxb = {
        hostname = "evabox";
        token_file = "/var/lib/reg/boxb.token";
        cluster = true;
      };
    };
    systemd.tmpfiles.rules = [
      "d /var/lib/reg 0700 root root -"
      "f /var/lib/reg/boxa.token 0600 root root - token-of-boxa-0123456789abcdef01234567"
      "f /var/lib/reg/boxb.token 0600 root root - token-of-boxb-0123456789abcdef01234567"
    ];
    # The registrar as modules/edge.nix starts it, less what this test does
    # not need: no Traefik, no tunnel, no mesh server.
    systemd.services.registrar = {
      wantedBy = [ "multi-user.target" ];
      after = [ "systemd-tmpfiles-setup.service" ];
      environment.RUST_LOG = "info";
      serviceConfig.ExecStart = lib.escapeShellArgs [
        "${lososPkgs.losos-registrar}/bin/losos-registrar"
        "serve"
        "--listen"
        "0.0.0.0:8443"
        "--port-range"
        "20000-20010"
        "--registry"
        "/var/lib/reg/registry.json"
        "--traefik-provider-listen"
        "127.0.0.1:8090"
        "--tunnel-config"
        "/var/lib/reg/tunnel.json"
        "--tenants-file"
        "/etc/losos-test/tenants.json"
        "--compute-windows-file"
        "/var/lib/reg/windows.json"
        "--ci-dir"
        "/var/lib/reg/ci"
      ];
    };
    environment.systemPackages = [
      pkgs.gitMinimal
      pkgs.curl
      pkgs.jq
    ];
    virtualisation = {
      memorySize = 768;
      vlans = [ 1 ];
    };
  };

  inherit (pkgs) lib;

  boxNode =
    {
      id,
      hostName,
      real ? false,
    }:
    { config, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/containers.nix
        ../../modules/daemon.nix
        ../../modules/git.nix
        ../../modules/ci.nix
      ]
      ++ lib.optionals real [
        ../../modules/tls.nix
        ../../modules/identity.nix
      ];
      # The real UI signs in through LosOS Login, which claims the box with
      # a password lososd also gives LosOS Cloud: a stand-in answers that.
      systemd.services.lososd = lib.mkIf real {
        path = lib.mkBefore [ stubOcc ];
        environment.LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
      };
      # lososd reads the owner's choices from overrides.nix, as the admin UI
      # saves them; without it the pages would say the box is in no mesh.
      environment.etc."nixos/modules/overrides.nix" = lib.mkIf real {
        mode = "0644";
        text = ''
          { ... }:
          {
            losos.sharingMyStorage = true;
            losos.cluster.enable = true;
          }
        '';
      };
      losos = {
        inherit hostName;
        backend.package = lososPkgs.losos-ctl;
        backend.plugins = lososPkgs.losos-plugins;
        admin.ui = if real then ui else stubUi;
        nextcloud.mode = lib.mkIf real "native";
        git.enable = true;
        identity.enable = real;
        sharingMyStorage = true;
        # In a mesh as far as the gate goes; rke2 is not imported.
        cluster.enable = true;
        shared.fscrypt.enable = false;
        proxy = {
          enable = true;
          applianceId = id;
          registrarUrl = "http://192.168.1.3:8443";
          tokenFile = "/var/secrets/edge-token";
        };
        ci = {
          inherit pm;
          pmPlugin = lososPkgs.losos-ci-pm-plugin;
          runner.memory = "768m";
          runner.cpus = "1";
        };
      };
      users.groups.shared.gid = 1001;
      users.users.shared = {
        uid = 1001;
        isNormalUser = true;
        group = "shared";
        homeMode = "700";
      };
      systemd.tmpfiles.rules = [
        "d /var/secrets 0700 root root -"
        "f /var/secrets/edge-token 0600 root root - token-of-${id}-0123456789abcdef01234567"
      ];
      # The test's service image, in the runner's own Podman storage.
      systemd.services.lci-test-image = {
        wantedBy = [ "losos-ci-runner.service" ];
        before = [ "losos-ci-runner.service" ];
        after = [ "user@1001.service" ];
        wants = [ "user@1001.service" ];
        inherit (config.systemd.services.losos-ci-runner) path;
        environment = removeAttrs config.systemd.services.losos-ci-runner.environment [ "PATH" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          User = "shared";
          ExecStart = "${config.virtualisation.podman.package}/bin/podman load --quiet --input ${svcImage}";
        };
      };
      environment.systemPackages = [
        pkgs.gitMinimal
        pkgs.curl
        pkgs.jq
      ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };

  # What the picture-taking runs share: the owner claims boxa through the
  # laptop, and losos-capture signs in as them.
  shootDefs = lib.optionalString shoot ''
    BOX = "192.168.1.1"
    PASSWORD = "${PASSWORD}"

    ${shots.handbookScript}

    def claim():
        laptop.wait_for_unit("multi-user.target")
        boxa.wait_for_open_port(443)
        # lososd's unit is active a moment before its API answers.
        boxa.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
        body = json.dumps({"name": "matus", "password": PASSWORD})
        out = laptop.succeed(
            f"curl -sk -w '\\n%{{http_code}}' -X POST https://{BOX}/api/setup/claim "
            f"-H 'Host: {BOX}' -H 'Content-Type: application/json' -d '{body}'"
        )
        reply, code = out.rsplit("\n", 1)
        assert code == "200", (code, reply)
        boxa.wait_for_unit("authelia-losos.service")
        boxa.systemctl("start losos-identity-setup.service")
        boxa.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        boxa.wait_until_succeeds("systemctl is-active authelia-losos.service")
        boxa.wait_for_open_port(9091)

    def capture(name, plan="/etc/losos-test/plan-pages.json", timeout=1800):
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan {plan} "
            f"--out /tmp/screens/{name} --user matus --password '{PASSWORD}'",
            timeout=timeout,
        )
  '';

  # After the runs: the list, the run with a job's log open, and the dialog
  # that starts a workflow by hand.
  shootRun = ''
    with subtest("pictures of the Mesh CI pages"):
        claim()
        capture("after")
        run_plan = {
            "width": 1280,
            "height": 800,
            "shots": [
                {
                    "name": "run",
                    "path": f"/ci/runs/{site_run}",
                    "waitFor": {"text": "services"},
                    "steps": [
                        {"click": {"text": "services"}},
                        # A step that passed stays folded, as on GitHub.
                        {"click": {"text": "Run curl -fsS http://web:7000/ | grep pong"}},
                        {"waitFor": {"selector": "li:has-text('pong')"}},
                        {"sleep": 1500},
                    ],
                },
                {
                    "name": "recipe",
                    "path": f"/ci/runs/{site_run}",
                    "waitFor": {"text": "recipe"},
                    "steps": [
                        {"click": {"text": "recipe"}},
                        {"click": {"text": "Run losos-project/pm@v0"}},
                        {"waitFor": {"selector": "li:has-text('building hello version 0.1.0')"}},
                        {"sleep": 1500},
                    ],
                },
                {
                    "name": "dispatch",
                    "path": "/ci",
                    "waitFor": {"role": "button", "name": "Run workflow"},
                    "steps": [
                        {"click": {"role": "button", "name": "Run workflow"}},
                        {"waitFor": {"role": "dialog"}},
                        {"sleep": 1000},
                    ],
                },
            ],
        }
        laptop.succeed(f"cat > /tmp/plan-run.json <<'EOF'\n{json.dumps(run_plan)}\nEOF")
        capture("after-run", "/tmp/plan-run.json")

        # The launch film's states, one picture per state and per typed
        # character, dark, at twice the pixels, with the boxes of what the
        # film points at.
        run_page = f"/ci/runs/{site_run}"
        services_log = [
            {"click": {"text": "services"}},
            {"click": {"text": "Run curl -fsS http://web:7000/ | grep pong"}},
            {"waitFor": {"selector": "li:has-text('pong')"}},
        ]
        recipe_log = [
            {"click": {"text": "recipe"}},
            {"click": {"text": "Run losos-project/pm@v0"}},
            {"waitFor": {"selector": "li:has-text('building hello version 0.1.0')"}},
        ]
        dialog = [
            {"click": {"role": "button", "name": "Run workflow"}},
            {"waitFor": {"role": "dialog"}},
        ]
        def shot(path, wait, title, targets, action=None, steps=None) -> dict:
            return {
                "path": path, "waitFor": wait, "steps": steps or [],
                "film": {"title": title, "action": action or {"kind": "none"}, "targets": targets},
            }

        ci_button = {"role": "button", "name": "Run workflow"}
        film: list[dict] = [
            shot("/", {"role": "link", "name": "Mesh CI"},
                 "the overview: Mesh CI in the sidebar",
                 {"ci": {"role": "link", "name": "Mesh CI"}},
                 {"kind": "click", "target": "ci"}),
            shot("/ci", {"text": "CI #1"},
                 "Mesh CI: this box's runner and the runs of the mesh",
                 {"run": {"text": "CI #1"}, "runner": {"text": "This box's runner"}},
                 {"kind": "click", "target": "run"}),
            shot(run_page, {"text": "services"},
                 "a push's run: two jobs, one on each box",
                 {"services": {"text": "services"}, "recipe": {"text": "recipe"},
                  "step": {"text": "Run curl -fsS http://web:7000/ | grep pong"}},
                 {"kind": "click", "target": "step"},
                 [{"click": {"text": "services"}}]),
            shot(run_page, {"text": "services"},
                 "a step's log: the service answers by name",
                 {"log": {"selector": "li:has-text('pong')"}, "recipe": {"text": "recipe"}},
                 {"kind": "click", "target": "recipe"},
                 services_log),
            shot(run_page, {"text": "recipe"},
                 "the recipe job, run on the other box",
                 {"step": {"text": "Run losos-project/pm@v0"}},
                 {"kind": "click", "target": "step"},
                 [{"click": {"text": "recipe"}}]),
            shot(run_page, {"text": "recipe"},
                 "pm builds the recipe in its jail",
                 {"log": {"selector": "li:has-text('building hello version 0.1.0')"}},
                 steps=recipe_log),
            shot("/ci", ci_button, "back on Mesh CI",
                 {"dispatch": ci_button},
                 {"kind": "click", "target": "dispatch"}),
            # The dialog's way in, 200 ms, is also a clip at 60 fps.
            dict(shot("/ci", ci_button, "the Run workflow dialog",
                      {"repo": {"selector": "#ci-dispatch-repo"}},
                      {"kind": "type", "target": "repo"},
                      [{"freeze": True}] + dialog),
                 clip={"fps": 60, "ms": 300}),
        ]
        typed = "matus/site"
        for i in range(1, len(typed) + 1):
            last = i == len(typed)
            film.append(shot(
                "/ci", ci_button, f"typing the repository: {typed[:i]}",
                {"repo": {"selector": "#ci-dispatch-repo"},
                 "find": {"role": "button", "name": "Find workflows"}},
                {"kind": "click", "target": "find"} if last else {"kind": "type", "target": "repo"},
                dialog + [{"fill": {"label": "Repository", "value": typed[:i]}}],
            ))
        find = dialog + [
            {"fill": {"label": "Repository", "value": typed}},
            {"click": {"role": "button", "name": "Find workflows"}},
            {"waitFor": {"selector": "#ci-dispatch-workflow"}},
        ]
        film.append(shot(
            "/ci", ci_button, "the workflows that start by hand",
            {"workflow": {"selector": "#ci-dispatch-workflow"},
             "run": {"role": "button", "name": "Run"}},
            {"kind": "click", "target": "run"},
            find,
        ))
        film.append(shot(
            "/ci", ci_button, "the run it started, already on a box",
            {"job": {"text": "wait"}},
            # A box takes the job from the edge's queue first.
            steps=find + [
                {"click": {"role": "button", "name": "Run"}},
                {"waitFor": {"text": "Run echo started; sleep 900"}, "timeout": 900000},
            ],
        ))
        for n, s in enumerate(film, 1):
            s["name"] = f"{n:02d}-" + re.sub(r"[^a-z0-9]+", "-", s["film"]["title"].split(":")[0].lower()).strip("-")
            s["steps"] = s["steps"] + [{"sleep": 1000}]
        film_plan = {
            "width": 1280,
            "height": 900,
            "variants": [{"suffix": None, "colorScheme": "dark", "dpr": 2,
                          "timezoneId": "Europe/Bratislava", "hideCaret": True}],
            "shots": film,
        }
        laptop.succeed(f"cat > /tmp/plan-film.json <<'EOF'\n{json.dumps(film_plan)}\nEOF")
        capture("film", "/tmp/plan-film.json", timeout=5400)
        laptop.copy_from_machine("/tmp/screens", "")
        # The clips' frames become MP4s here, beside the driver: an encoder
        # on the emulated laptop is slow, and ffmpeg there reads the test
        # shell's input as its own.
        film_dir = driver.out_dir / "screens" / "film"
        for first in sorted(film_dir.glob("*-clip-000.png")):
            n = first.name.removesuffix("-clip-000.png")
            subprocess.run(
                ["${pkgs.ffmpeg-headless}/bin/ffmpeg", "-nostdin", "-v", "error",
                 "-framerate", "60", "-i", str(film_dir / f"{n}-clip-%03d.png"),
                 "-c:v", "libx264", "-crf", "16", "-pix_fmt", "yuv420p",
                 "-movflags", "+faststart", "-y", str(film_dir / f"{n}.mp4")],
                check=True,
            )
            for frame in film_dir.glob(f"{n}-clip-*.png"):
                frame.unlink()

    with subtest("the handbook's pictures"):
        handbook_pictures(laptop, f"https://{BOX}", "matus", PASSWORD, [
            {"name": "mesh-ci", "path": "/ci",
             "waitFor": {"selector": f"a[href='/ci/runs/{site_run}']"}, "settle": 1500},
            {"name": "mesh-ci-run", "path": f"/ci/runs/{site_run}", "waitFor": {"text": "services"},
             "steps": run_plan["shots"][0]["steps"]},
        ])
  '';

  mainScript = ''
    BOXES = {"boxa": boxa, "boxb": boxb}

    # One after another: three emulated machines booting at once can miss
    # the driver's wait for a shell.
    for m in [edge, boxa, boxb]:
        m.start()
        m.succeed("true")
    edge.wait_for_unit("registrar.service")
    edge.wait_for_open_port(8443)
    for b in BOXES.values():
        b.wait_for_unit("losos-git.service")
        b.wait_for_unit("lososd.service")
        b.wait_for_unit("losos-ci.service")
        b.wait_for_unit("lci-test-image.service", timeout=600)
        b.wait_for_unit("losos-ci-runner.service")
        b.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")

    # lososd keeps its admin token in the credential store.
    ADMIN_TOKEN = "systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -"

    def api(b, method, path, body=None):
        token = b.succeed(ADMIN_TOKEN).strip()
        args = f"-fsS -X {method} -H 'Authorization: Bearer {token}'"
        if body is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(body)}'"
        return json.loads(b.succeed(f"curl {args} 127.0.0.1:8082{path}"))

    def forge(b, method, path, body=None, token=None):
        token = token or b.succeed("cat /var/lib/losos-git/admin-token").strip()
        args = f"-fsS -X {method} -H 'Authorization: token {token}'"
        if body is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(body)}'"
        out = b.succeed(f"curl {args} http://127.0.0.1:3000/api/v1{path}")
        return json.loads(out) if out.strip() else None

    def diagnose():
        for name, m in [("boxa", boxa), ("boxb", boxb)]:
            print(f"── {name} ──")
            print(m.execute("journalctl -b --no-pager -o cat -u losos-ci -u losos-ci-runner | tail -n 150")[1])
            print(m.execute("runuser -u shared -- env $(systemctl show -p Environment --value"
                            " losos-ci-runner) podman ps --all 2>&1")[1])
        print(edge.execute("journalctl -b --no-pager -o cat -u registrar | grep -i ci | tail -n 80")[1])

    def run_done(b, run_id, timeout=3000):
        try:
            return wait_run(b, run_id, timeout)
        except Exception:
            print(json.dumps(api(b, "GET", f"/api/ci/runs/{run_id}"), indent=1))
            diagnose()
            raise

    def wait_run(b, run_id, timeout):
        b.wait_until_succeeds(
            "curl -fsS -H \"Authorization: Bearer $(" + ADMIN_TOKEN + ")\""
            f" 127.0.0.1:8082/api/ci/runs/{run_id}"
            " | jq -e '.state as $s | [\"success\",\"failure\",\"cancelled\",\"error\"] | index($s)'",
            timeout=timeout,
        )
        return api(b, "GET", f"/api/ci/runs/{run_id}")

    def log_of(b, job_id):
        return api(b, "GET", f"/api/ci/jobs/{job_id}/log")["text"]

    with subtest("pm loads Mesh CI's signed plugin and nothing else"):
        plugins = boxa.succeed(
            "XDG_CONFIG_HOME=${lososPkgs.losos-ci-pm-plugin} RUST_LOG=warn ${pm}/bin/pm plugins"
        )
        assert "losos-ci" in plugins, plugins
        # The plugin answers for lci-* containers only.
        refused = boxa.fail(
            "runuser -u shared -- env XDG_CONFIG_HOME=${lososPkgs.losos-ci-pm-plugin}"
            " ${pm}/bin/pm container stop nextcloud 2>&1"
        )
        assert "not a Mesh CI container" in refused, refused
        # Unsigned, it does not load: a copy without the signature is refused.
        boxa.succeed(
            "mkdir -p /tmp/unsigned/pm/plugins /tmp/unsigned/pm/trusted"
            " && cp ${lososPkgs.losos-ci-pm-plugin}/pm/plugins/losos-ci.wasm /tmp/unsigned/pm/plugins/"
            " && cp ${lososPkgs.losos-ci-pm-plugin}/pm/trusted/* /tmp/unsigned/pm/trusted/"
        )
        boxa.fail("XDG_CONFIG_HOME=/tmp/unsigned ${pm}/bin/pm plugins")

    for name, b in BOXES.items():
        assert api(b, "PUT", "/api/ci/runner", {"lend": "always"})["lend"] == "always"
        b.wait_until_succeeds(
            "curl -fsS -H \"Authorization: Bearer $(" + ADMIN_TOKEN + ")\""
            " 127.0.0.1:8082/api/ci | jq -e '.mesh == true'",
            timeout=180,
        )

    # Matus and two repositories on boxa, a public one and a private one.
    forge(boxa, "PUT", "/admin/users/matus", {"full_name": "Matus"})
    forge(boxa, "PUT", "/admin/users/matus/repos/site", {"private": False})
    forge(boxa, "PUT", "/admin/users/matus/repos/notes", {"private": True})
    token = forge(boxa, "POST", "/admin/users/matus/tokens",
                  {"name": "edge", "scopes": ["write:repository"], "replace": True})["token"]
    key = "${recipe}/key"
    edge.succeed(
        "mkdir -p /tmp/site/.github/workflows /tmp/notes/.github/workflows",
        "cp -r ${recipe}/pkg /tmp/site/pkg",
        f"sed \"s/KEY/$(cat {key})/\" ${workflows.ci} > /tmp/site/.github/workflows/ci.yml",
        "cp ${workflows.slow} /tmp/site/.github/workflows/slow.yml",
        "cp ${workflows.private} /tmp/notes/.github/workflows/private.yml",
    )

    def push(repo):
        edge.succeed(
            f"cd /tmp/{repo} && git init -q -b main && git add -A"
            " && git -c user.name=Matus -c user.email=m@x commit -qm 'Workflows'"
            f" && git push -q http://matus:{token}@192.168.1.1/git/matus/{repo}.git main"
        )
        return edge.succeed(f"git -C /tmp/{repo} rev-parse HEAD").strip()

    with subtest("a push runs the workflow's jobs on both boxes of the mesh"):
        sha = push("site")
        boxa.wait_until_succeeds(
            "curl -fsS -H \"Authorization: Bearer $(" + ADMIN_TOKEN + ")\""
            " '127.0.0.1:8082/api/ci/runs?owner=matus&repo=site' | jq -e '.runs | length == 1'",
            timeout=300,
        )
        run_id = api(boxa, "GET", "/api/ci/runs?owner=matus&repo=site")["runs"][0]["id"]
        site_run = run_id
        run = run_done(boxa, run_id)
        logs = {j["name"]: log_of(boxa, j["id"]) for j in run["jobs"]}
        for name, text in logs.items():
            print(f"── {name} ──\n{text}")
        runners = sorted(j["runner"] for j in run["jobs"])
        if run["state"] != "success" or runners != ["boxa", "boxb"]:
            diagnose()
        assert run["state"] == "success", run
        assert runners == ["boxa", "boxb"], runners
        assert all(j["placement"] == "mesh" for j in run["jobs"]), run["jobs"]

    with subtest("services, a container action and a pm recipe, in the job's containers"):
        services = logs["services"]
        assert "web is healthy" in services, services
        assert services.count("pong") >= 2, services
        assert "from a container action in /github/workspace" in services, services
        recipe = logs["recipe"]
        assert "archive /github/workspace/" in recipe and ".cpkg" in recipe, recipe

    with subtest("each job is a check on the commit"):
        statuses = forge(boxa, "GET", f"/repos/matus/site/commits/{sha}/statuses")
        got = {s["context"]: s["state"] for s in statuses["statuses"]}
        assert got.get("CI / services (push)") == "success", got
        assert got.get("CI / recipe (push)") == "success", got

    with subtest("a private repository's job stays on its own box"):
        push("notes")
        boxa.wait_until_succeeds(
            "curl -fsS -H \"Authorization: Bearer $(" + ADMIN_TOKEN + ")\""
            " '127.0.0.1:8082/api/ci/runs?owner=matus&repo=notes' | jq -e '.runs | length == 1'",
            timeout=300,
        )
        run_id = api(boxa, "GET", "/api/ci/runs?owner=matus&repo=notes")["runs"][0]["id"]
        run = run_done(boxa, run_id, timeout=900)
        assert run["state"] == "success", run
        job = run["jobs"][0]
        assert job["placement"] == "box" and job["runner"] == "boxa", job
        assert "private ran on boxa" in log_of(boxa, job["id"])

    with subtest("a run started by hand is cancelled, and its containers go"):
        run_id = api(boxa, "POST", "/api/ci/dispatch",
                     {"owner": "matus", "repo": "site", "workflow": ".github/workflows/slow.yml"})["id"]
        boxa.wait_until_succeeds(
            "curl -fsS -H \"Authorization: Bearer $(" + ADMIN_TOKEN + ")\""
            f" 127.0.0.1:8082/api/ci/runs/{run_id} | jq -e '.jobs[0].state == \"running\"'",
            timeout=900,
        )
        job = api(boxa, "GET", f"/api/ci/runs/{run_id}")["jobs"][0]
        where = BOXES[job["runner"]]
        where.wait_until_succeeds(
            "runuser -u shared -- env XDG_CONFIG_HOME=${lososPkgs.losos-ci-pm-plugin}"
            " $(systemctl show -p Environment --value losos-ci-runner)"
            " ${pm}/bin/pm container list | grep -q lci-",
            timeout=300,
        )
        api(boxa, "POST", f"/api/ci/runs/{run_id}/cancel")
        run = run_done(boxa, run_id, timeout=300)
        assert run["state"] == "cancelled", run
        where.wait_until_succeeds(
            "test -z \"$(runuser -u shared -- env XDG_CONFIG_HOME=${lososPkgs.losos-ci-pm-plugin}"
            " $(systemctl show -p Environment --value losos-ci-runner)"
            " ${pm}/bin/pm container list)\"",
            timeout=300,
        )
  '';
in
pkgs.testers.nixosTest {
  name =
    if !after then
      "losos-mesh-ci-before"
    else if screens then
      "losos-mesh-ci-screens"
    else
      "losos-mesh-ci";

  # Under emulation the film's twenty states at twice the pixels take most
  # of an hour on their own, after the hour the jobs and pictures take.
  globalTimeout = if screens then 4 * 3600 else 3600;

  nodes = {
    boxa = boxNode {
      id = "boxa";
      hostName = "mattbox";
      real = shoot;
    };
  }
  // lib.optionalAttrs after {
    boxb = boxNode {
      id = "boxb";
      hostName = "evabox";
    };
    edge = edgeNode;
  }
  // lib.optionalAttrs shoot {
    laptop = _: {
      imports = [ shots.browser ];
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
      ];
      environment.etc."losos-test/plan-pages.json".source = plan;
      virtualisation.vlans = [ 1 ];
    };
  };

  testScript = ''
    import json
    import re
    import subprocess

  ''
  + shootDefs
  + (
    if after then
      mainScript + lib.optionalString screens shootRun
    else
      ''
        start_all()
        boxa.wait_for_unit("nginx.service")
        boxa.wait_for_unit("lososd.service")
        claim()
        capture("before")
        laptop.copy_from_machine("/tmp/screens", "")
      ''
  );
}
