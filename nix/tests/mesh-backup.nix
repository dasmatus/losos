# Backups on storage bought on the mesh: encrypted on the owner's box, and
# nothing but ciphertext on the edge or the volume.
#
# Two VMs:
#
#   * `edge` runs the registrar with the market on (a Stripe gate in test
#     mode, so no Stripe is reached), its API behind TLS as a real edge has
#     it, a ledger seeded with one paid, bound storage order of mattbox's,
#     and a stand-in for the mesh's apiserver. The stand-in records what the
#     registrar creates (the order's Secret, Deployment and Service) and
#     answers the Service's address once it exists. The test then does what
#     the kubelet on the edge's node would: it runs the Deployment's command,
#     with the Secret's file and a directory for the claim. That directory is
#     what Longhorn would replicate onto lending boxes.
#   * `box` is mattbox: lososd and the backup units, laid out like the real
#     box (tmpfs root, /persist), with the market relay pointed at the edge.
#
# What is asserted:
#
#   1. choosing the order keeps no secret on the box: the sealed target names
#      the repository and the order, and the door's key is made from the
#      recovery code on every run;
#   2. no official edge is in reach of a VM (official means a TPM-attested
#      gateway), so lososd keeps the choice and says the door is not keyed.
#      The test keys it as lososd's relay would, with the box's tenant token
#      and a key it derives with sha256sum, not with lososd's code;
#   3. the first request starts the order's server with a password of its
#      own, and that server refuses anyone without it, as every mesh node
#      that can reach a Service's address would be;
#   4. the door refuses a wrong key before anything reaches the server;
#   5. a backup through lososd reaches the volume, and neither the volume nor
#      the edge's own hop to the server carries a sentence from the owner's
#      files, while the box does (the positive control);
#   6. a door keyed to another code turns a backup away in plain words;
#   7. the recovery code brings the files back from the bought volume.
{ pkgs, impermanence }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  registrar = "${lososPkgs.losos-registrar}/bin/losos-registrar";
  lososPersistence =
    (import ../../modules/impermanence.nix {
      config = { };
      inherit lib pkgs;
    }).environment.persistence."/persist";
  persistedDirs = builtins.filter (d: d != "/nix") lososPersistence.directories;

  order = "ord_0a1b2c3d4e5f60718293a4b5";
  edgeUrl = "https://edge";
  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  kubeTokenValue = "test-kube-token-0123456789abcdef";
  image = "losos.local/rest-server:${pkgs.restic-rest-server.version}";

  # A CA the box trusts and a certificate for the edge's name. The store
  # keeps the fixture while its inputs stay the same, so it is valid for
  # ten years rather than days after the first build.
  tls = pkgs.runCommand "losos-mesh-backup-tls" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=test CA" \
      -keyout $out/ca.key -out $out/ca.pem
    openssl req -newkey rsa:2048 -nodes -subj "/CN=edge" -keyout $out/tls.key -out tls.csr
    printf 'subjectAltName=DNS:edge\n' > ext
    openssl x509 -req -in tls.csr -CA $out/ca.pem -CAkey $out/ca.key \
      -CAcreateserial -days 3650 -extfile ext -out $out/tls.pem
  '';

  # Shaped like the real thing, because the gate checks both before it
  # serves anything. A `sk_test_` key puts the market in test mode.
  stripeKey = pkgs.writeText "stripe-key" "sk_test_vm_0123456789abcdefghijklmnop";
  webhookSecret = pkgs.writeText "webhook-secret" "whsec_vm_0123456789abcdefghijklmnopqrstuv";

  stateDir = "/var/lib/losos-registrar";
  tenants = pkgs.writeText "tenants.json" (
    builtins.toJSON {
      mattbox = {
        hostname = "mattbox.edge.test";
        token_file = "${stateDir}/mattbox.token";
        cluster = true;
        market = true;
        relay_zone = null;
      };
    }
  );
  # The test-mode ledger with mattbox's paid storage order, its claim bound.
  ledger = pkgs.writeText "market-test.json" (
    builtins.toJSON {
      mode = "test";
      orders.${order} = {
        id = order;
        listing_id = "lst_0a1b2c3d4e5f6071";
        buyer = "mattbox";
        seller = "lender";
        kind = "storage";
        quantity = 5;
        unit_price = 100;
        amount = 500;
        fee = 20;
        currency = "eur";
        status = "paid";
        session_id = null;
        created_at = 1760000000;
        paid_at = 1760000100;
        expires_at = 4102444800;
        volume = "market-mattbox/ord-0a1b2c3d4e5f60718293a4b5";
        claim = "market-mattbox/ord-0a1b2c3d4e5f60718293a4b5";
      };
    }
  );

  # The mesh's apiserver, as far as the backup door uses it: creates are
  # recorded and answered 201, a Service is found once it was created, and
  # a claim is Bound. GET /debug/created hands the records to the test.
  kubeStub = pkgs.writers.writePython3 "losos-kube-stub" { } ''
    import json
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

    TOKEN = "Bearer ${kubeTokenValue}"
    CREATED = {}


    class Stub(BaseHTTPRequestHandler):
        def reply(self, code, body):
            data = json.dumps(body).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            path = self.path.split("?")[0]
            if path == "/debug/created":
                return self.reply(200, CREATED)
            if self.headers.get("Authorization") != TOKEN:
                return self.reply(401, {"kind": "Status", "code": 401})
            parts = path.strip("/").split("/")
            if len(parts) == 6 and parts[4] == "services":
                made = CREATED.get("/".join([""] + parts[:5]), [])
                if any(m["metadata"]["name"] == parts[5] for m in made):
                    return self.reply(200, {"spec": {"clusterIP": "127.0.0.1"}})
            if len(parts) == 6 and parts[4] == "persistentvolumeclaims":
                return self.reply(200, {"status": {"phase": "Bound"}})
            return self.reply(404, {"kind": "Status", "code": 404})

        def do_POST(self):
            if self.headers.get("Authorization") != TOKEN:
                return self.reply(401, {"kind": "Status", "code": 401})
            length = int(self.headers.get("Content-Length", "0"))
            body = json.loads(self.rfile.read(length))
            CREATED.setdefault(self.path.split("?")[0], []).append(body)
            return self.reply(201, body)

        def log_message(self, *args):
            pass


    ThreadingHTTPServer(("127.0.0.1", 6444), Stub).serve_forever()
  '';
in

pkgs.testers.nixosTest {
  name = "losos-mesh-backup";

  nodes.edge = {
    systemd.tmpfiles.rules = [
      "d ${stateDir} 0700 root root - -"
      "f ${stateDir}/mattbox.token 0600 root root - ${proxyTokenValue}"
      "f ${stateDir}/kube-token 0600 root root - ${kubeTokenValue}"
      "C ${stateDir}/market-test.json 0600 root root - ${ledger}"
      # The claim, as the server sees it mounted.
      "d /var/lib/volume 0700 nobody nogroup - -"
      "d /run/rest-auth 0750 root nogroup - -"
    ];

    systemd.services.losos-kube-stub = {
      description = "a stand-in for the mesh apiserver";
      wantedBy = [ "multi-user.target" ];
      serviceConfig.ExecStart = kubeStub;
    };

    systemd.services.losos-stripe-gate = {
      description = "the Stripe gate, in test mode";
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        ExecStart = lib.escapeShellArgs [
          registrar
          "stripe-gate"
          "--socket"
          "/run/losos-stripe-gate/gate.sock"
          "--stripe-key-file"
          stripeKey
          "--webhook-secret-file"
          webhookSecret
        ];
        RuntimeDirectory = "losos-stripe-gate";
      };
    };

    systemd.services.losos-registrar = {
      description = "the registrar with the market and the backup door";
      wantedBy = [ "multi-user.target" ];
      after = [
        "losos-stripe-gate.service"
        "losos-kube-stub.service"
        "systemd-tmpfiles-setup.service"
      ];
      wants = [
        "losos-stripe-gate.service"
        "losos-kube-stub.service"
      ];
      environment.RUST_LOG = "info";
      serviceConfig.ExecStart = lib.escapeShellArgs [
        registrar
        "serve"
        "--listen"
        "127.0.0.1:8443"
        "--registry"
        "${stateDir}/registry.json"
        "--tunnel-config"
        "${stateDir}/tunnel.json"
        "--port-range"
        "50000-50010"
        "--tenants-file"
        tenants
        "--reconcile-interval"
        "5s"
        "--compute-windows-file"
        "${stateDir}/compute-windows.json"
        "--kube-api"
        "http://127.0.0.1:6444"
        "--kube-token-file"
        "${stateDir}/kube-token"
        "--market-gate-socket"
        "/run/losos-stripe-gate/gate.sock"
        "--market-return-url"
        "https://edge/market/return"
        "--market-state-file"
        "${stateDir}/market.json"
        "--market-storage-image"
        image
      ];
    };

    # The registrar's API over TLS, as the box reaches a real edge, with a
    # body limit over restic's 16 MiB packs.
    services.nginx = {
      enable = true;
      clientMaxBodySize = "80m";
      virtualHosts.edge = {
        onlySSL = true;
        sslCertificate = "${tls}/tls.pem";
        sslCertificateKey = "${tls}/tls.key";
        locations."/".proxyPass = "http://127.0.0.1:8443";
      };
    };
    networking.firewall.allowedTCPPorts = [ 443 ];

    environment.systemPackages = [
      pkgs.curl
      pkgs.restic-rest-server
      pkgs.tcpdump
      pkgs.jq
    ];
    virtualisation = {
      memorySize = 1024;
      cores = 2;
    };
  };

  nodes.box =
    { lib, pkgs, ... }:
    {
      imports = [
        impermanence.nixosModules.impermanence
        ../../modules/options.nix
        ../../modules/configuration.nix
        ../../modules/daemon.nix
        ../../modules/fscrypt.nix
        ../../modules/backup.nix
      ];

      environment.persistence."/persist" = {
        inherit (lososPersistence) hideMounts files;
        directories = persistedDirs;
      };

      networking.hostName = "mattbox";
      losos.backend.package = lososPkgs.losos-ctl;
      losos.sharingMyStorage = true;
      losos.tpm.enable = false;
      losos.gpu.enable = false;
      networking.networkmanager.enable = lib.mkForce false;
      losos.mdns.enable = lib.mkForce false;

      # The market relay's three settings, which modules/daemon.nix sets
      # from losos.proxy.* on a box behind an edge. The tunnel itself is not
      # what this test is about.
      systemd.services.lososd.environment = {
        LOSOS_REGISTRAR_URL = edgeUrl;
        LOSOS_APPLIANCE_ID = "mattbox";
        LOSOS_PROXY_TOKEN_FILE = "/var/secrets/losos-proxy-token";
      };
      systemd.tmpfiles.rules = [
        "d /var/secrets 0700 root root - -"
        "f /var/secrets/losos-proxy-token 0600 root root - ${proxyTokenValue}"
      ];
      security.pki.certificateFiles = [ "${tls}/ca.pem" ];

      environment.systemPackages = [
        pkgs.jq
        pkgs.sqlite
      ];

      virtualisation = {
        memorySize = 2048;
        cores = 2;
        diskSize = 4096;
        fileSystems."/" = lib.mkForce {
          device = "tmpfs";
          fsType = "tmpfs";
          options = [ "mode=755" ];
        };
        fileSystems."/persist" = {
          device = "/dev/vda";
          fsType = "ext4";
          neededForBoot = true;
        };
      };

      # What disko and the installer do on a real box (see erase.nix): ext4
      # with the encrypt feature fscrypt needs, and the directories
      # impermanence binds from.
      boot.initrd.systemd.initrdBin = [ pkgs.e2fsprogs ];
      boot.initrd.systemd.services.losos-test-persist = {
        description = "Give /persist the encrypt feature";
        wantedBy = [ "initrd.target" ];
        after = [ "dev-vda.device" ];
        requires = [ "dev-vda.device" ];
        before = [
          "sysroot-persist.mount"
          "systemd-fsck@dev-vda.service"
        ];
        unitConfig.DefaultDependencies = false;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = ''
          if ! features=$(tune2fs -l /dev/vda 2>/dev/null); then
            mkfs.ext4 -q -O encrypt /dev/vda
          else
            case "$features" in
              *"Filesystem features:"*" encrypt"*) ;;
              *) tune2fs -O encrypt /dev/vda ;;
            esac
          fi
        '';
      };
      boot.initrd.systemd.services.losos-test-persist-dirs = {
        description = "Seed /persist with the dirs nixos-install would have made";
        wantedBy = [ "initrd.target" ];
        requires = [ "sysroot-persist.mount" ];
        after = [ "sysroot-persist.mount" ];
        before = [ "sysroot-var.mount" ];
        unitConfig.DefaultDependencies = false;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = "mkdir -p /sysroot/persist/var /sysroot/persist/etc/keys";
      };
    };

  testScript = ''
    import json
    import shlex

    ORDER = "${order}"
    REPO = "${edgeUrl}/market/storage/" + ORDER + "/"
    TOKEN = "${proxyTokenValue}"
    NOTE = "the plate of salmon stays in the family"
    ROW = "a row the database must keep"

    def token():
        return box.succeed("systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -").strip()

    def api(method, path, body=None, expect=200):
        args = f"-s -o /tmp/api.out -w '%{{http_code}}' -X {method} -H 'Authorization: Bearer {token()}'"
        if body is not None:
            args += f" -H 'Content-Type: application/json' --data-binary {shlex.quote(json.dumps(body))}"
        code = box.succeed(f"curl {args} http://127.0.0.1:8082{path}").strip()
        out = box.succeed("cat /tmp/api.out")
        assert code == str(expect), f"{method} {path} answered {code}, expected {expect}: {out}"
        return json.loads(out) if out.strip() else None

    def wait_job(state, timeout=600):
        box.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: Bearer {token()}' http://127.0.0.1:8082/api/backup "
            f"| jq -e '.job.state == \"{state}\" or .job.state == \"failed\"'",
            timeout=timeout,
        )
        job = api("GET", "/api/backup")["job"]
        print("job:", job)
        assert job["state"] == state, f"job ended {job['state']}: {job.get('message')}"
        return job

    def sha256(text):
        return box.succeed(f"printf %s {shlex.quote(text)} | sha256sum | cut -c1-64").strip()

    def set_door(door):
        body = json.dumps({"appliance_id": "mattbox", "token": TOKEN, "order_id": ORDER, "door": door})
        code = box.succeed(
            "curl -s -o /tmp/door.out -w '%{http_code}' -H 'Content-Type: application/json' "
            f"--data-binary {shlex.quote(body)} ${edgeUrl}/market/storage-door"
        ).strip()
        assert code == "204", f"storage-door answered {code}: {box.succeed('cat /tmp/door.out')}"

    def through_door(secret, path="config"):
        return box.succeed(
            f"curl -s --path-as-is -o /dev/null -w '%{{http_code}}' -u {ORDER}:{secret} {REPO}{path}"
        ).strip()

    # What the registrar made in mattbox's namespace (the reconciler may ask
    # the stand-in for other things; those are not the door's).
    def created():
        made = json.loads(edge.succeed("curl -fsS http://127.0.0.1:6444/debug/created"))
        return {k: v for k, v in made.items() if "/namespaces/market-mattbox/" in k}

    start_all()
    edge.wait_for_unit("losos-registrar.service")
    edge.wait_for_unit("nginx.service")
    edge.wait_for_open_port(443)
    box.wait_for_unit("multi-user.target")
    box.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    box.wait_until_succeeds("test -s /etc/credstore.encrypted/admin-token")
    box.wait_until_succeeds("curl -fsS ${edgeUrl}/health")

    with subtest("the box holds an owner's data"):
        box.succeed(
            "install -d /var/lib/losos-cloud/notshared/files",
            f"echo '{NOTE}' > /var/lib/losos-cloud/notshared/files/note.txt",
            "install -d /var/lib/losos-git",
            f"sqlite3 /var/lib/losos-git/git.db \"create table notes (body text); insert into notes values ('{ROW}');\"",
        )

    code = api("GET", "/api/recovery")["code"]

    with subtest("choosing the bought storage keeps no secret on the box"):
        api("POST", "/api/backup/target/bought", {"order": "ord_NOT_HEX"}, expect=400)
        out = api("POST", "/api/backup/target/bought", {"order": ORDER})
        print("bought:", out)
        assert out["target"]["bought"] == {"order": ORDER, "edge": "${edgeUrl}"}, out
        # A VM has no TPM-attested gateway, so no official edge: the choice
        # is kept and the door is not keyed from here.
        assert out["keyed"] is False, out
        sealed = box.succeed("systemd-creds decrypt --name=backup-bucket /etc/credstore.encrypted/backup-bucket -")
        assert f"RESTIC_REPOSITORY=rest:{REPO}" in sealed, sealed
        assert f"LOSOS_DOOR_ORDER={ORDER}" in sealed, sealed
        assert "PASSWORD" not in sealed and code not in sealed, sealed
        assert api("GET", "/api/backup")["target"]["bought"]["order"] == ORDER

    # What lososd's relay sends once an official edge answers: the SHA-256
    # of the secret the backup job makes from the recovery code.
    secret = sha256(f"losos-backup-door\n{ORDER}\n{code}")
    door = sha256(secret)

    with subtest("the door opens only to the key the recovery code makes"):
        assert through_door(secret) == "401", "the door opened before it was keyed"
        set_door(door)
        assert through_door("0" * 64) == "401"
        assert created() == {}, "a wrong key reached the cluster"
        # The first request starts the order's server.
        assert through_door(secret) == "503"

    with subtest("the order's server has a password of its own"):
        edge.wait_until_succeeds(
            "curl -fsS http://127.0.0.1:6444/debug/created"
            " | jq -e '[keys[] | select(contains(\"/namespaces/market-mattbox/\"))] | length == 3'",
            timeout=60,
        )
        made = created()
        ns = "/api/v1/namespaces/market-mattbox"
        deployment = made["/apis/apps/v1/namespaces/market-mattbox/deployments"][0]
        auth = made[ns + "/secrets"][0]
        assert made[ns + "/services"][0]["metadata"]["name"] == "rest-ord-0a1b2c3d4e5f60718293a4b5"
        pod = deployment["spec"]["template"]["spec"]
        container = pod["containers"][0]
        assert container["image"] == "${image}", container
        assert pod["volumes"][0]["persistentVolumeClaim"]["claimName"] == "ord-0a1b2c3d4e5f60718293a4b5"
        assert "--no-auth" not in container["command"], container
        line = auth["stringData"]["htpasswd"]
        assert line.startswith("edge:{SHA}"), line
        # The kubelet's part: the Secret as a file, the claim as a folder,
        # and the Deployment's own command line, as nobody.
        edge.succeed(
            f"printf %s {shlex.quote(line)} > /run/rest-auth/htpasswd",
            "chgrp nogroup /run/rest-auth/htpasswd && chmod 0440 /run/rest-auth/htpasswd",
        )
        command = [
            "/var/lib/volume" if a == "/data" else
            "/run/rest-auth/htpasswd" if a == "/etc/rest-server/htpasswd" else
            "${pkgs.restic-rest-server}/bin/rest-server" if a == "/bin/rest-server" else a
            for a in container["command"]
        ]
        edge.succeed(
            "systemd-run --unit=rest-server -p User=nobody -p Group=nogroup "
            + " ".join(shlex.quote(a) for a in command)
        )
        edge.wait_for_open_port(8000)
        # Where a mesh node reaching the Service's address would knock.
        assert edge.succeed("curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8000/config").strip() == "401"
        # Through the door the repository answers: there is none yet.
        box.wait_until_succeeds(f"test \"$(curl -s -o /dev/null -w '%{{http_code}}' -u {ORDER}:{secret} {REPO}config)\" = 404", timeout=60)
        # A path that climbs out of the repository is refused before the
        # server is asked, whichever way it is spelled.
        assert through_door(secret, "%2e%2e/%2e%2e/etc/passwd") == "404"
        assert through_door(secret, "data/%2e%2e/config") == "404"

    with subtest("a backup reaches the bought volume as ciphertext"):
        edge.succeed("systemd-run --unit=capture tcpdump -i lo -U -w /tmp/hop.pcap 'tcp port 8000'")
        edge.sleep(2)
        api("POST", "/api/backup/run")
        wait_job("done")
        last = api("GET", "/api/backup")["last"]
        assert last and len(last["snapshot"]) == 64, last
        edge.succeed("systemctl stop capture")
        edge.succeed("test -s /var/lib/volume/config", "ls /var/lib/volume/data | grep -c .")
        # Positive controls first, so a grep that cannot read proves nothing.
        box.succeed(f"grep -rqF '{NOTE}' /var/lib/losos-cloud")
        box.succeed(f"grep -rqaF '{ROW}' /var/lib/losos-git")
        edge.succeed("grep -qai 'authorization: basic' /tmp/hop.pcap")
        edge.succeed("grep -qa 'POST /data/' /tmp/hop.pcap")
        for text in (NOTE, ROW):
            edge.fail(f"grep -rqaF '{text}' /var/lib/volume")
            edge.fail(f"grep -qaF '{text}' /tmp/hop.pcap")
        edge.fail(f"grep -rqaF '{code}' /var/lib/volume /var/lib/losos-registrar /tmp/hop.pcap")
        edge.fail(f"grep -rqaF '{secret}' /var/lib/losos-registrar")
        edge.succeed(f"grep -qF '{door}' /var/lib/losos-registrar/market-test.json")

    with subtest("a door keyed to another code turns the backup away"):
        set_door("ab" * 32)
        api("POST", "/api/backup/run")
        job = wait_job("failed", timeout=1200)
        assert job["message"] == "the edge does not open this storage to this recovery code", job
        set_door(door)

    with subtest("the recovery code brings the files back"):
        box.succeed("rm /var/lib/losos-cloud/notshared/files/note.txt")
        api("POST", "/api/backup/restore", {"code": code})
        wait_job("done")
        box.succeed(f"grep -qF '{NOTE}' /var/lib/losos-cloud/notshared/files/note.txt")
        row = box.succeed("sqlite3 /var/lib/losos-git/git.db 'select body from notes'")
        assert row.strip() == ROW, row
  '';
}
