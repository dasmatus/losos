# nixos-test-vms config for apps that run as `notshared` on rootless Podman
# (modules/apps.nix, backend/src/compose.rs).
#
# The box runs lososd, the app job and nginx, and no cluster at all: neither
# LosOS Cloud nor LosOS Git runs here, so k3s is not even in the system. The laptop serves a Helm chart repository over https and is the
# computer on the LAN that opens the apps. Nothing comes from a registry:
# one small image, built here, is loaded from the store by the box's own
# app and used by every app after it. The test checks:
#
#   1. one of the box's own apps (losos.apps.builtin) starts at boot from
#      its Compose file and serves its page on its path of the front door;
#   2. GET /api/apps offers installs with no cluster, and `shared` is
#      refused for want of one;
#   3. an app in the LosOS app format, sent to POST /api/apps/compose with
#      its files, runs: its page on its LAN port, its two services talking
#      by name, its data written as notshared (uid 1000) into
#      /home/notshared/data/apps/<name>, its processes notshared on the host;
#   4. an app that asks for privileges is refused with the reason, before
#      anything starts;
#   5. a chart from the search, run as notshared, is rendered, turned into a
#      Compose project and runs, its config map and secret made into a file
#      and a variable;
#   6. nothing an app runs can reach lososd's API;
#   7. after a reboot every app is back, data and all;
#   8. every installed notshared app's Compose file is in notshared's own
#      Git repo, a commit per install, and nowhere a refused app's;
#   9. removing an app stops it and closes its port, its data stays, and
#      its folder leaves the repo in a commit of its own.
#
# The front door is the real one (modules/containers.nix) with the real
# admin UI, so `nix/demo/record.sh losos-podman-apps` photographs the Apps
# pane and the apps' pages at each step (nix/demo/recorder.py).
#
# Slow without KVM: rootless Podman under TCG takes a while per container.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # busybox's httpd and sh are all the apps below need.
  image = pkgs.dockerTools.buildLayeredImage {
    name = "localhost/losos-hello";
    tag = "1";
    contents = [ pkgs.busybox ];
  };

  builtinSite = pkgs.writeTextDir "site/index.html" "hello from the box's own app\n";

  # The laptop's https chart repository. The box trusts this certificate the
  # way it trusts the public ones, so Helm fetches from it as from any.
  cert = pkgs.runCommand "laptop-cert" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
      -keyout $out/key.pem -out $out/cert.pem \
      -subj /CN=laptop -addext subjectAltName=DNS:laptop
  '';

  chart = pkgs.runCommand "hello-chart-repo" { nativeBuildInputs = [ pkgs.kubernetes-helm ]; } ''
    mkdir -p src/templates $out
    cat > src/Chart.yaml <<'EOF'
    apiVersion: v2
    name: hello-chart
    version: 0.1.0
    EOF
    cat > src/values.yaml <<'EOF'
    port: 8096
    greeting: hello from a chart
    EOF
    cat > src/templates/app.yaml <<'EOF'
    apiVersion: v1
    kind: ConfigMap
    metadata:
      name: {{ .Release.Name }}-site
    data:
      index.html: |
        {{ .Values.greeting }}
    ---
    apiVersion: v1
    kind: Secret
    metadata:
      name: {{ .Release.Name }}-secret
    data:
      word: {{ "swordfish" | b64enc }}
    ---
    apiVersion: v1
    kind: Service
    metadata:
      name: {{ .Release.Name }}
    spec:
      ports:
        - port: {{ .Values.port }}
    ---
    apiVersion: apps/v1
    kind: Deployment
    metadata:
      name: {{ .Release.Name }}
    spec:
      replicas: 2
      selector:
        matchLabels: { app: hello }
      template:
        metadata:
          labels: { app: hello }
        spec:
          containers:
            - name: web
              image: localhost/losos-hello:1
              command: ["sh", "-c"]
              args:
                - cp /config/index.html /www/ && echo "$WORD" > /www/word && exec httpd -f -p $(PORT) -h /www
              env:
                - name: PORT
                  value: "{{ .Values.port }}"
                - name: WORD
                  valueFrom:
                    secretKeyRef: { name: {{ .Release.Name }}-secret, key: word }
              ports:
                - name: http
                  containerPort: {{ .Values.port }}
              volumeMounts:
                - { name: config, mountPath: /config }
                - { name: www, mountPath: /www }
          volumes:
            - name: config
              configMap: { name: {{ .Release.Name }}-site }
            - name: www
              emptyDir: {}
    EOF
    export HOME=$TMPDIR
    helm package src --destination $out
    helm repo index $out --url https://laptop/charts
  '';

  # An app in the LosOS app format, as the AI builder or an owner sends it.
  notes = builtins.toJSON {
    release = "notes";
    title = "Notes";
    compose = ''
      x-losos:
        port: 8097
      services:
        web:
          image: localhost/losos-hello:1
          command: ["httpd", "-f", "-p", "8097", "-h", "/site"]
          ports: ["8097"]
          volumes:
            - ./site:/site:ro
        writer:
          image: localhost/losos-hello:1
          command:
            - sh
            - -c
            - |
              id -u > /data/uid
              until wget -qO /data/from-web http://web:8097/; do sleep 1; done
              exec sleep 100000
          volumes:
            - data:/data
          depends_on: [web]
      volumes:
        data: {}
    '';
    files = {
      "site/index.html" = "notes from an app the owner sent\n";
    };
  };

  greedy = builtins.toJSON {
    release = "greedy";
    compose = ''
      services:
        web:
          image: localhost/losos-hello:1
          privileged: true
    '';
  };

  shared = builtins.toJSON {
    release = "elsewhere";
    runAs = "shared";
    chart = {
      repo = "https://laptop/charts";
      name = "hello-chart";
      version = "0.1.0";
    };
  };

  fromChart = builtins.toJSON {
    release = "hello";
    runAs = "notshared";
    chart = {
      repo = "https://laptop/charts";
      name = "hello-chart";
      version = "0.1.0";
    };
    values = { };
  };

  bodies = pkgs.runCommand "app-bodies" { } ''
    mkdir -p $out
    cp ${pkgs.writeText "notes.json" notes} $out/notes.json
    cp ${pkgs.writeText "greedy.json" greedy} $out/greedy.json
    cp ${pkgs.writeText "shared.json" shared} $out/shared.json
    cp ${pkgs.writeText "hello.json" fromChart} $out/hello.json
  '';
in

pkgs.testers.nixosTest {
  name = "losos-podman-apps";

  nodes = {
    box = _: {
      imports = [
        ../../modules/options.nix
        ../../modules/daemon.nix
        ../../modules/apps.nix
        ../../modules/containers.nix
      ];
      losos = {
        backend.package = lososPkgs.losos-ctl;
        admin.ui = lososPkgs.losos-admin-ui;
        # The spare key signs in; LosOS Login has a test of its own.
        identity.enable = false;
        # No modules/cluster.nix: no cluster on this box.
        forgejo.enable = false;
        apps.builtin.welcome = {
          compose = {
            "x-losos" = {
              port = 8091;
              images = [ "${image}" ];
            };
            services.web = {
              image = "localhost/losos-hello:1";
              command = [
                "httpd"
                "-f"
                "-p"
                "8091"
                "-h"
                "/site"
              ];
              volumes = [ "./site:/site:ro" ];
            };
          };
          files = builtinSite;
          path = "/welcome/";
        };
      };

      # What modules/configuration.nix gives the real box.
      users.groups.notshared.gid = 1000;
      users.users.notshared = {
        uid = 1000;
        isNormalUser = true;
        group = "notshared";
        homeMode = "700";
      };
      systemd.tmpfiles.rules = [ "d /home/notshared/data 0700 notshared notshared -" ];

      security.pki.certificateFiles = [ "${cert}/cert.pem" ];
      environment.systemPackages = [ pkgs.jq ];

      virtualisation = {
        memorySize = 3072;
        cores = 2;
        diskSize = 4096;
      };
    };

    laptop = _: {
      services.nginx = {
        enable = true;
        virtualHosts.laptop = {
          onlySSL = true;
          sslCertificate = "${cert}/cert.pem";
          sslCertificateKey = "${cert}/key.pem";
          locations."/charts/".alias = "${chart}/";
        };
      };
      networking.firewall.allowedTCPPorts = [ 443 ];
    };
  };

  testScript = builtins.readFile ../demo/recorder.py + ''
    import json
    import time

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_until_succeeds("test -s /etc/credstore.encrypted/admin-token")
    # The owner claimed this box in the first-run wizard; here the claim is
    # stated, so the admin UI opens on its panes rather than the wizard.
    box.succeed(
      "systemctl stop lososd.service; install -d -m 0700 /var/lib/losos;"
      " printf '%s' '{\"mode\":\"local\",\"sharing\":false,\"rebuild\":null,\"claimed\":true}'"
      " > /var/lib/losos/state.json; systemctl start lososd.service"
    )
    box.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    token = box.succeed("systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -").strip()
    api = f"curl -sS -H 'Authorization: Bearer {token}' http://127.0.0.1:8082"

    # What the recorder photographs: the Apps pane, the box's own app on its
    # path, and an installed app's page on its LAN port.
    record_note("box.token", token)
    record_port(box, 18080, 80)

    def pane(name, wait=None):
        return {"name": name, "port": 18080, "path": "/apps", "token": "box.token", "wait": wait}

    def page(name, host_port, path="/"):
        return {"name": name, "port": host_port, "path": path}

    def post(path, body):
        return box.succeed(
          f"{api}{path} -X POST -H 'Content-Type: application/json' "
          f"--data @${bodies}/{body} -w '\\n%{{http_code}}'"
        )

    def apps():
        return {a["release"]: a for a in json.loads(box.succeed(f"{api}/api/apps"))["apps"]}

    def repo(cmd):
        return box.succeed(
          "runuser -u notshared -- env HOME=/var/lib/losos-podman/notshared "
          f"${pkgs.gitMinimal}/bin/git -C /var/lib/losos-podman/notshared/apps {cmd}"
        )

    def wait_phase(release, phase, timeout=1200):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            app = apps().get(release)
            if app and app["phase"] == phase:
                return app
            if app and app["phase"] == "failed" and phase != "failed":
                print(box.succeed(f"cat /var/lib/losos/apps/{release}/job.log || true"))
                raise Exception(f"{release} failed: {app['message']}")
            time.sleep(5)
        print(box.succeed(f"cat /var/lib/losos/apps/{release}/job.log || true"))
        raise Exception(f"{release} never reached {phase}")

    with subtest("no cluster on this box"):
        box.fail("systemctl cat k3s.service")

    with subtest("the box's own app starts at boot and serves its path"):
        box.wait_for_unit("losos-builtin-welcome.service", timeout=900)
        box.wait_for_unit("losos-podman-app@welcome.service")
        laptop.wait_until_succeeds("curl -fsS http://box/welcome/ | grep -q \"box's own app\"", timeout=300)
        # LAN only: the box itself is turned away, as on the admin routes.
        box.fail("curl -fsS http://127.0.0.1/welcome/")
        record("builtin", "No cluster on the box. Its own app runs on Podman and installs are open.", [
          pane("apps-empty", "text=Search"),
          page("welcome", 18080, "/welcome/"),
        ])

    with subtest("installs are offered without a cluster, shared is not"):
        listing = json.loads(box.succeed(f"{api}/api/apps"))
        assert listing["available"] is True, listing
        assert listing["sharedAvailable"] is False, listing
        out = post("/api/apps/install", "shared.json")
        assert out.endswith("409"), out
        assert "kept separate" in out, out

    with subtest("an app in the LosOS app format runs as notshared"):
        out = post("/api/apps/compose", "notes.json")
        assert out.endswith("200"), out
        app = wait_phase("notes", "running")
        assert app["runtime"] == "podman" and app["kind"] == "compose", app
        assert app["appPort"] == 8097, app
        front = app["frontPort"]
        laptop.wait_until_succeeds(f"curl -fsS http://box:{front}/ | grep -q 'notes from an app'", timeout=120)
        data = "/home/notshared/data/apps/notes/data"
        # The writer reached the web service by its name, and wrote as uid
        # 1000 inside, which is notshared outside.
        box.wait_until_succeeds(f"grep -q 'notes from an app' {data}/from-web", timeout=300)
        assert box.succeed(f"cat {data}/uid").strip() == "1000"
        assert box.succeed(f"stat -c %U {data}/from-web").strip() == "notshared"
        users = box.succeed("ps -o user= -C httpd | sort -u").split()
        assert users and all(u == "notshared" for u in users), users
        record_port(box, 18097, front)
        record("compose", "An app in the LosOS app format runs as notshared.", [
          pane("apps-made", "text=Notes"),
          page("notes", 18097),
        ])

    with subtest("an app that asks for privileges is refused, with the reason"):
        out = post("/api/apps/compose", "greedy.json")
        assert out.endswith("200"), out
        app = wait_phase("greedy", "failed", timeout=300)
        assert "privileged" in app["message"], app
        box.fail("systemctl is-active losos-podman-app@greedy.service")
        record("refused", "An app that asks for privileges is refused with the reason.", [
          pane("apps-refused", "text=privileged"),
        ])

    with subtest("a chart from the search runs on Podman as notshared"):
        out = post("/api/apps/install", "hello.json")
        assert out.endswith("200"), out
        app = wait_phase("hello", "running")
        assert app["runtime"] == "podman" and app["appPort"] == 8096, app
        front = app["frontPort"]
        laptop.wait_until_succeeds(f"curl -fsS http://box:{front}/ | grep -q 'hello from a chart'", timeout=300)
        assert laptop.succeed(f"curl -fsS http://box:{front}/word").strip() == "swordfish"
        compose = json.loads(box.succeed("cat /var/lib/losos-podman/notshared/projects/hello/compose.json"))
        # Two replicas asked for, one runs: they would share a port.
        assert list(compose["services"]) == ["hello-web"], compose
        record_port(box, 18096, front)
        record("chart", "A chart from the search runs on Podman as notshared.", [
          pane("apps-chart", "text=hello-chart"),
          page("hello", 18096),
        ])

    with subtest("the apps' Compose files are in notshared's own Git repo"):
        assert box.succeed("stat -c %U /var/lib/losos-podman/notshared/apps/.git").strip() == "notshared"
        assert box.succeed("find /var/lib/losos-podman/notshared/apps ! -user notshared | wc -l").strip() == "0"
        assert repo("log --format=%s").split("\n")[:2] == ["Install hello", "Install notes"], repo("log --format=%s")
        tracked = repo("ls-files").split()
        assert "notes/compose.yaml" in tracked and "hello/compose.json" in tracked, tracked
        assert any(f.startswith("notes/files/") for f in tracked), tracked
        assert not any(f.startswith("greedy/") for f in tracked), tracked
        sent = json.loads(box.succeed("cat ${bodies}/notes.json"))["compose"]
        assert repo("show HEAD~1:notes/compose.yaml") == sent, "the stored file is not the one sent"
        repo("diff --quiet HEAD")

    podman = (
      "runuser -u notshared -- env XDG_RUNTIME_DIR=/run/user/1000 "
      "CONTAINERS_STORAGE_CONF=$(systemctl show -p Environment losos-podman-app@notes.service "
      "| grep -o 'CONTAINERS_STORAGE_CONF=[^ ]*' | cut -d= -f2) podman"
    )

    with subtest("nothing an app runs reaches lososd"):
        box.fail(f"{podman} exec losos-notes_writer_1 wget -qO- -T 5 http://127.0.0.1:8082/api/health")

    with subtest("after a reboot every app is back"):
        box.shutdown()
        box.start()
        box.wait_for_unit("losos-podman-apps.service")
        for release in ["notes", "hello"]:
            box.wait_for_unit(f"losos-podman-app@{release}.service", timeout=900)
        box.wait_for_unit("losos-builtin-welcome.service", timeout=900)
        laptop.wait_until_succeeds("curl -fsS http://box/welcome/ | grep -q \"box's own app\"", timeout=300)
        notes_front = apps()["notes"]["frontPort"]
        laptop.wait_until_succeeds(f"curl -fsS http://box:{notes_front}/ | grep -q 'notes from an app'", timeout=300)
        box.succeed("test -s /home/notshared/data/apps/notes/data/uid")
        record_port(box, 18080, 80)
        record("reboot", "After a reboot every app is back.", [pane("apps-after-reboot", "text=Notes")])

    with subtest("removing an app stops it, closes its port and keeps its data"):
        # A container of the project the Compose file no longer names, as an
        # update that renamed a service leaves behind. Removal takes it too.
        box.succeed(
          f"{podman} run --detach --name losos-notes_old_1 --network host "
          "--label io.podman.compose.project=losos-notes localhost/losos-hello:1 sleep 3600"
        )
        out = box.succeed(
          f"{api}/api/apps/remove -X POST -H 'Content-Type: application/json' "
          "--data '{\"release\": \"notes\"}' -w '\\n%{http_code}'"
        )
        assert out.endswith("200"), out
        box.wait_until_succeeds(f"! {api}/api/apps | jq -e '.apps[] | select(.release == \"notes\")'", timeout=300)
        box.fail("systemctl is-active losos-podman-app@notes.service")
        box.fail("ps -o args= -C httpd | grep -q 8097")
        laptop.fail(f"curl -fsS -m 5 http://box:{notes_front}/")
        box.succeed("test -s /home/notshared/data/apps/notes/data/uid")
        box.fail("test -e /var/lib/losos-podman/notshared/projects/notes")
        assert box.succeed(f"{podman} ps --all --quiet --filter label=io.podman.compose.project=losos-notes").strip() == ""
        assert repo("log -1 --format=%s").strip() == "Remove notes"
        tracked = repo("ls-files").split()
        assert not any(f.startswith("notes/") for f in tracked) and "hello/compose.json" in tracked, tracked
        record("removed", "Notes removed: stopped, port closed, data kept.", [pane("apps-removed", "text=hello-chart")])
  '';
}
