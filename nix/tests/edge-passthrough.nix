# TLS passthrough: the box ends its own TLS, and the edge cannot read it.
#
# Three machines:
#
#   * `acme`  — Pebble, a test ACME server (nixpkgs' own test CA module), and
#               the visitor: it resolves the box's public name to the edge,
#               like the public DNS would;
#   * `edge`  — losos.edge.enable: Traefik, the registrar, the tunnel;
#   * `box`   — losos.proxy.enable with modules/public-tls.nix (on by
#               default), a minimal front vhost that says which port the
#               request reached it on.
#
# What it proves, in order:
#
#   1. Before the box holds a certificate, the edge ends TLS with its own and
#      sends the request on as plain HTTP: a secret posted by the visitor is
#      in a capture of the edge's loopback, in the clear. (This is how every
#      box worked before.)
#   2. The box gets a certificate for its name from the ACME server by
#      HTTP-01, through the edge's plain port 80, and says so on its next
#      heartbeat. The edge then routes the name by SNI with passthrough and
#      has no TLS router of its own for it.
#   3. The visitor checks the box's certificate against the ACME server's
#      root, gets the page through the edge, and the same secret posted again
#      is nowhere in the edge's capture.
#   4. The box's check of its own name from outside finds its own key; with
#      the name pointed at a machine that answers with another certificate,
#      it says so.
{ pkgs }:

let
  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  proxyToken = "/var/secrets/losos-proxy-token";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
    ];
  };
  lososPkgs = import ../packages.nix { inherit pkgs; };
  host = "mattbox.losos.cfd";
  testCa = "${pkgs.path}/nixos/tests/common/acme/server/ca.cert.pem";
  secret = "correct-horse-battery-staple-4711";
in

pkgs.testers.nixosTest {
  name = "losos-edge-passthrough";

  nodes = {
    acme =
      { nodes, ... }:
      {
        imports = [ "${pkgs.path}/nixos/tests/common/acme/server" ];
        # Pebble checks HTTP-01 where the public DNS would send it: the edge.
        networking.hosts.${nodes.edge.networking.primaryIPAddress} = [ host ];
        environment.systemPackages = [
          pkgs.curl
          pkgs.openssl
        ];
        virtualisation.memorySize = 768;
      };

    edge =
      { ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
          secretFiles
        ];
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };
        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.cfd";
          registrarApiBind = "::";
          registrarApiPort = 8443;
          tunnelBindPort = 2333;
          tunnelPortRange = "50000-50010";
          heartbeatTtl = "120s";
          reconcileInterval = "2s";
          tenants.mattbox = {
            hostname = host;
            tokenFile = proxyToken;
          };
        };
        environment.systemPackages = [
          pkgs.jq
          pkgs.tcpdump
        ];
        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };

    box =
      { nodes, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/proxy.nix
          secretFiles
        ];
        losos = {
          hostName = "mattbox";
          proxy = {
            enable = true;
            edgeTunnelEndpoint = "edge:2333";
            registrarUrl = "http://edge:8443";
            hostname = host;
            applianceId = "mattbox";
            tokenFile = proxyToken;
            heartbeatInterval = "3s";
            registrar.package = lososPkgs.losos-registrar;
            tlsPassthrough.acmeServer = "https://acme.test/dir";
          };
        };
        # Pebble's API runs under the test CA; its issuing root is made
        # fresh at each start and only the visitor needs it.
        security.pki.certificateFiles = [ testCa ];
        # The public DNS, for the box's own check of its name.
        networking.hosts.${nodes.edge.networking.primaryIPAddress} = [ host ];
        # Started by the script, so step 1 sees the box before it has one.
        systemd.timers.losos-public-tls.wantedBy = pkgs.lib.mkForce [ ];

        # The front vhost, cut down: port 80 as before, and the loopback
        # port the box's own TLS hands on to (modules/public-tls-lib.nix).
        # It echoes the port and the body, so the script can tell which way
        # a request came.
        services.nginx = {
          enable = true;
          virtualHosts."losos-front" = {
            default = true;
            listen = [
              {
                addr = "0.0.0.0";
                port = 80;
              }
              {
                addr = "127.0.0.1";
                port = 8482;
              }
            ];
            locations."/".extraConfig = ''
              return 200 "hello-losos port=$server_port\n";
            '';
          };
        };
        environment.systemPackages = [ pkgs.openssl ];
        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };
  };

  testScript =
    { nodes, ... }:
    let
      edgeIp = nodes.edge.networking.primaryIPAddress;
    in
    ''
      import json

      edge_ip = "${edgeIp}"
      host = "${host}"
      secret = "${secret}"
      resolve = f"--resolve {host}:443:{edge_ip}"

      # One at a time: three VMs under TCG can miss the driver's wait.
      acme.start()
      acme.wait_for_unit("pebble.service")
      edge.start()
      edge.wait_for_unit("losos-registrar.service")
      edge.wait_for_unit("traefik.service")
      box.start()
      box.wait_for_unit("losos-tunnel-client.service")
      box.wait_for_unit("losos-registrar-announce.service")

      # The tunnel lands on the box's splitter, not on port 80.
      assert '"127.0.0.1:8446"' in box.succeed("cat /run/losos-tunnel/client.json")
      edge.wait_until_succeeds("curl -fsS http://127.0.0.1:50000/ | grep -q 'port=80'", timeout=180)

      # A unit of its own, so the capture outlives the shell that starts it.
      def capture(name):
          edge.succeed(
              f"systemd-run --unit=capture-{name} tcpdump --immediate-mode -i lo -U -w /tmp/{name}.pcap 'tcp port 50000'"
          )
          edge.wait_until_succeeds(f"test -s /tmp/{name}.pcap", timeout=30)

      # Packets the kernel has handed over but tcpdump has not written yet
      # are lost on a stop, so give it a moment first.
      def stop(name):
          edge.sleep(3)
          edge.succeed(f"systemctl kill --signal=INT capture-{name}.service")
          edge.wait_until_fails(f"systemctl is-active capture-{name}.service", timeout=30)
          return edge.succeed(f"tcpdump -r /tmp/{name}.pcap -A 2>/dev/null | grep -c '{secret}' || true").strip()

      # 1. Before: the edge ends TLS (its own default certificate here, as
      #    Let's Encrypt cannot reach a VM) and reads the request.
      doc = json.loads(edge.wait_until_succeeds(
          "curl -fsS http://127.0.0.1:8090/traefik | grep -q 'Host(`mattbox.losos.cfd`)' && curl -fsS http://127.0.0.1:8090/traefik",
          timeout=120,
      ))
      assert "mattbox" in doc["http"]["routers"], doc
      assert "tcp" not in doc, doc
      # The HTTP-01 path is routed for every box, before it asks for anything.
      assert doc["http"]["routers"]["mattbox-acme"]["entryPoints"] == ["web"], doc
      acme.wait_until_succeeds(f"curl -sSk {resolve} https://{host}/ | grep -q 'port=80'", timeout=120)
      capture("before")
      acme.succeed(f"curl -sSk {resolve} -d 'password={secret}' https://{host}/login")
      before = stop("before")
      print("secret seen in the edge's capture before:", before)
      assert int(before) > 0, "the edge did not see the plaintext before passthrough; the capture is broken"

      # 2. The box gets its own certificate through the edge's port 80.
      box.succeed("systemctl start losos-public-tls.service")
      box.succeed(f"test -s /var/lib/losos-public-tls/live/{host}/fullchain.pem")
      assert box.succeed("cat /var/lib/losos-public-tls/ready").strip() == host
      key_mode = box.succeed(f"stat -c '%a %G' /var/lib/losos-public-tls/live/{host}/key.pem").strip()
      assert key_mode == "640 nginx", key_mode
      print(box.succeed(f"openssl x509 -in /var/lib/losos-public-tls/live/{host}/fullchain.pem -noout -issuer -subject -ext subjectAltName"))
      edge.wait_until_succeeds(
          "curl -fsS http://127.0.0.1:8090/traefik | jq -e '.tcp.routers.mattbox.tls.passthrough == true'",
          timeout=120,
      )
      doc = json.loads(edge.succeed("curl -fsS http://127.0.0.1:8090/traefik"))
      print(json.dumps(doc, indent=2))
      assert doc["tcp"]["routers"]["mattbox"]["rule"] == f"HostSNI(`{host}`)", doc
      assert doc["tcp"]["services"]["mattbox"]["loadBalancer"]["servers"][0]["address"] == "127.0.0.1:50000", doc
      websecure = [r for r in doc["http"]["routers"].values() if "websecure" in r["entryPoints"]]
      assert websecure == [], f"the edge still ends TLS for the box: {websecure}"
      # The registrar keeps it across a restart.
      edge.succeed("jq -e '.tenants.mattbox.tls == [\"${host}\"]' /var/lib/losos-registrar/registry.json")
      # And the heartbeat's answer told the box its names.
      box.wait_until_succeeds(f"grep -qx {host} /var/lib/losos-public-tls/names")
      # The first look from outside waited for the edge to switch over, so
      # it found the box's own certificate, not the edge's.
      probe = json.loads(box.succeed("cat /var/lib/losos-public-tls/probe.json"))
      assert probe["names"][host] == "ok", probe

      # 3. The visitor checks the certificate against the ACME server's
      #    root, reaches the box through the edge, and the edge reads nothing.
      acme.succeed("curl -fsS --cacert ${testCa} https://acme.test:15000/roots/0 > /tmp/root.pem")
      verified = f"curl -fsS --cacert /tmp/root.pem {resolve} https://{host}/"
      acme.wait_until_succeeds(f"{verified} | grep -q 'port=8482'", timeout=120)
      issuer = acme.succeed(
          f"openssl s_client -connect {edge_ip}:443 -servername {host} -CAfile /tmp/root.pem </dev/null 2>/dev/null"
          " | openssl x509 -noout -fingerprint -sha256"
      ).strip()
      mine = box.succeed(f"openssl x509 -in /var/lib/losos-public-tls/live/{host}/fullchain.pem -noout -fingerprint -sha256").strip()
      assert issuer == mine, f"the visitor saw {issuer}, the box holds {mine}"
      capture("after")
      acme.succeed(f"curl -fsS --cacert /tmp/root.pem {resolve} -d 'password={secret}' https://{host}/login")
      after = stop("after")
      print("secret seen in the edge's capture after:", after)
      assert int(after) == 0, "the edge could read the request after passthrough"
      edge.copy_from_machine("/tmp/before.pcap", "captures")
      edge.copy_from_machine("/tmp/after.pcap", "captures")
      # The edge never asked for a certificate for the name.
      edge.fail(f"grep -q '{host}' /var/lib/traefik/acme.json 2>/dev/null")

      # 4. The box's check of its own name from outside.
      box.succeed("systemctl start losos-public-tls.service")
      probe = json.loads(box.succeed("cat /var/lib/losos-public-tls/probe.json"))
      print(probe)
      assert probe["names"][host] == "ok", probe
      # Someone else answering for the name: here Pebble's front end, with
      # a certificate the box did not make.
      acme_ip = "${nodes.acme.networking.primaryIPAddress}"
      box.succeed(f"cp /etc/hosts /tmp/hosts && sed -i '/{host}/d' /tmp/hosts && echo '{acme_ip} {host}' >> /tmp/hosts && mount --bind /tmp/hosts /etc/hosts")
      box.succeed("systemctl start losos-public-tls.service")
      probe = json.loads(box.succeed("cat /var/lib/losos-public-tls/probe.json"))
      print(probe)
      assert probe["names"][host] == "foreign", probe
      box.succeed("journalctl -u losos-public-tls.service | grep -q 'did not make'")
      box.succeed("umount /etc/hosts")
    '';
}
