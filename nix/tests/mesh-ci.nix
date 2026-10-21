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
{
  pkgs,
  pmSrc,
}:

let
  lososPkgs = import ../packages.nix { inherit pkgs pmSrc; };
  inherit (lososPkgs) pm;
  git = "${pkgs.gitMinimal}/bin/git";

  stubUi = pkgs.runCommand "stub-admin-ui" { } ''
    mkdir -p $out
    echo "admin ui" > $out/index.html
  '';

  # A service and a container action, loaded into the shared user's
  # Podman on both boxes: nothing is pulled in a VM test. It answers
  # "pong" on port 7000 and says it is ready with a file.
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
      "/bin/sh"
      "-c"
      "touch /tmp/ready; exec python3 -m http.server 7000 --directory /srv"
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
      "f /var/lib/reg/boxa.token 0600 root root - token-of-boxa"
      "f /var/lib/reg/boxb.token 0600 root root - token-of-boxb"
      "f /var/lib/reg/bootstrap 0600 root root - bootstrap-token-0123456789"
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
        "--traefik-dir"
        "/var/lib/reg/traefik"
        "--rathole-config"
        "/var/lib/reg/rathole.toml"
        "--bootstrap-token-file"
        "/var/lib/reg/bootstrap"
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
    }:
    { config, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/containers.nix
        ../../modules/daemon.nix
        ../../modules/git.nix
        ../../modules/ci.nix
      ];
      losos = {
        inherit hostName;
        backend.package = lososPkgs.losos-ctl;
        backend.plugins = lososPkgs.losos-plugins;
        admin.ui = stubUi;
        git.enable = true;
        identity.enable = false;
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
        "f /var/secrets/edge-token 0600 root root - token-of-${id}"
      ];
      # The test's service image, in the runner's own Podman storage.
      systemd.services.lci-test-image = {
        wantedBy = [ "losos-ci-runner.service" ];
        before = [ "losos-ci-runner.service" ];
        after = [ "user@1001.service" ];
        wants = [ "user@1001.service" ];
        inherit (config.systemd.services.losos-ci-runner) environment path;
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
in
pkgs.testers.nixosTest {
  name = "losos-mesh-ci";

  nodes = {
    boxa = boxNode {
      id = "boxa";
      hostName = "mattbox";
    };
    boxb = boxNode {
      id = "boxb";
      hostName = "evabox";
    };
    edge = edgeNode;
  };

  testScript = ''
    import json, time

    BOXES = {"boxa": boxa, "boxb": boxb}

    start_all()
    edge.wait_for_unit("registrar.service")
    edge.wait_for_open_port(8443)
    for b in BOXES.values():
        b.wait_for_unit("losos-git.service")
        b.wait_for_unit("lososd.service")
        b.wait_for_unit("losos-ci.service")
        b.wait_for_unit("lci-test-image.service", timeout=600)
        b.wait_for_unit("losos-ci-runner.service")
        b.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")

    def api(b, method, path, body=None):
        token = b.succeed("cat /var/secrets/losos-admin-token").strip()
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

    def run_done(b, run_id, timeout=1500):
        b.wait_until_succeeds(
            "curl -fsS -H \"Authorization: Bearer $(cat /var/secrets/losos-admin-token)\""
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
            "curl -fsS -H \"Authorization: Bearer $(cat /var/secrets/losos-admin-token)\""
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
            "curl -fsS -H \"Authorization: Bearer $(cat /var/secrets/losos-admin-token)\""
            " '127.0.0.1:8082/api/ci/runs?owner=matus&repo=site' | jq -e '.runs | length == 1'",
            timeout=300,
        )
        run_id = api(boxa, "GET", "/api/ci/runs?owner=matus&repo=site")["runs"][0]["id"]
        run = run_done(boxa, run_id)
        logs = {j["name"]: log_of(boxa, j["id"]) for j in run["jobs"]}
        for name, text in logs.items():
            print(f"── {name} ──\n{text}")
        assert run["state"] == "success", run
        runners = sorted(j["runner"] for j in run["jobs"])
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
            "curl -fsS -H \"Authorization: Bearer $(cat /var/secrets/losos-admin-token)\""
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
            "curl -fsS -H \"Authorization: Bearer $(cat /var/secrets/losos-admin-token)\""
            f" 127.0.0.1:8082/api/ci/runs/{run_id} | jq -e '.jobs[0].state == \"running\"'",
            timeout=900,
        )
        job = api(boxa, "GET", f"/api/ci/runs/{run_id}")["jobs"][0]
        where = BOXES[job["runner"]]
        where.wait_until_succeeds(
            "runuser -u shared -- env XDG_CONFIG_HOME=${lososPkgs.losos-ci-pm-plugin}"
            " XDG_RUNTIME_DIR=/run/user/1001 CONTAINERS_STORAGE_CONF=$(systemctl show -p Environment losos-ci-runner"
            " | tr ' ' '\\n' | sed -n 's/^CONTAINERS_STORAGE_CONF=//p')"
            " ${pm}/bin/pm container list | grep -q lci-",
            timeout=300,
        )
        api(boxa, "POST", f"/api/ci/runs/{run_id}/cancel")
        run = run_done(boxa, run_id, timeout=300)
        assert run["state"] == "cancelled", run
        where.wait_until_succeeds(
            "test -z \"$(runuser -u shared -- env XDG_CONFIG_HOME=${lososPkgs.losos-ci-pm-plugin}"
            " XDG_RUNTIME_DIR=/run/user/1001 CONTAINERS_STORAGE_CONF=$(systemctl show -p Environment losos-ci-runner"
            " | tr ' ' '\\n' | sed -n 's/^CONTAINERS_STORAGE_CONF=//p')"
            " ${pm}/bin/pm container list)\"",
            timeout=300,
        )
  '';
}
