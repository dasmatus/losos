# QUIC against TCP on the two paths that carry a box's traffic, measured on
# a box of the size LosOS is built for (two cores, 4 GiB):
#
#   * the front door: the box's nginx over HTTPS on TCP (HTTP/2) and over
#     HTTP/3 on UDP 443, fetched from another machine on the LAN;
#   * the edge tunnel: a request through the edge's per-box port, carried by
#     the QUIC tunnel, and again with UDP to the tunnel's port dropped, where
#     the same client falls back to TLS on TCP. The TCP fallback stands in
#     for the TCP tunnel the box used before (one TCP connection, every
#     request behind any lost packet).
#
# Each is timed on a clean link (10 ms each way) and on a lossy one (40 ms
# each way, 2 % loss), with tc netem on both machines. The test asserts what
# must hold (HTTP/3 is really used, Alt-Svc is offered over HTTPS only, the
# tunnel is on the transport it claims, every body arrives intact), prints a
# table of median times and CPU used, and writes it to `quic.md` in the
# check's output (the working directory when the driver is run by hand). It
# asserts no speed: under TCG the numbers say more about the emulator than
# about the protocols, so read them side by side, never against a real
# machine.
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

  # 4 MiB of noise, so nothing on the way can compress it.
  blob = pkgs.runCommand "quic-blob" { } ''
    mkdir $out
    head -c 4194304 /dev/urandom > $out/blob
    sha256sum $out/blob | cut -d' ' -f1 > $out/blob.sha256
  '';
in
pkgs.testers.nixosTest {
  name = "losos-quic";

  nodes = {
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
          tenants.box = {
            hostname = "box.losos.cfd";
            tokenFile = proxyToken;
          };
        };

        # The edge is also the LAN client of the front-door half. curl speaks
        # HTTP/3 out of the box.
        environment.systemPackages = [
          pkgs.curl
          pkgs.iproute2
          pkgs.jq
        ];

        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };

    box =
      { ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/proxy.nix
          ../../modules/tls.nix
          secretFiles
        ];

        losos = {
          hostName = "box";
          proxy = {
            enable = true;
            edgeTunnelEndpoint = "edge:2333";
            registrarUrl = "http://edge:8443";
            hostname = "box.losos.cfd";
            applianceId = "box";
            tokenFile = proxyToken;
            heartbeatInterval = "3s";
            registrar.package = lososPkgs.losos-registrar;
          };
        };

        # modules/tls.nix gives this vhost its listeners (80, 443, and 443
        # over QUIC); the test adds only what it serves.
        services.nginx = {
          enable = true;
          virtualHosts."losos-front" = {
            default = true;
            locations."/".extraConfig = ''
              return 200 'hello-losos\n';
            '';
            locations."= /blob".alias = "${blob}/blob";
          };
        };

        environment.systemPackages = [ pkgs.iproute2 ];
        # modules/containers.nix opens :80 on a real box; tls.nix opens 443.
        networking.firewall.allowedTCPPorts = [ 80 ];

        virtualisation = {
          memorySize = 4096;
          cores = 2;
        };
      };
  };

  testScript = ''
    import os
    import statistics

    RUNS = 5
    BLOB_SHA = open("${blob}/blob.sha256").read().strip()

    edge.start()
    box.start()
    edge.wait_for_unit("losos-tunnel.service")
    edge.wait_for_unit("losos-registrar.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("losos-tunnel-client.service")
    edge.wait_until_succeeds(
      "jq -e '.services.box.bind == \"127.0.0.1:50000\"' /etc/losos-tunnel/server.json"
    )
    edge.wait_until_succeeds("curl -fsS http://127.0.0.1:50000/ | grep -q hello-losos")
    box_v4 = box.succeed("ip -4 -o addr show eth1 | awk '{print $4}' | cut -d/ -f1").strip()
    print("box on the LAN:", box_v4)

    # What must hold before anything is timed.
    # HTTPS offers HTTP/3; plain HTTP offers nothing, it cannot be upgraded.
    head = edge.succeed(f"curl -sk --http2 -D - -o /dev/null https://{box_v4}/")
    print(head)
    assert 'alt-svc: h3=":443"; ma=86400' in head.lower(), head
    head80 = edge.succeed(f"curl -s -D - -o /dev/null http://{box_v4}/")
    assert "alt-svc" not in head80.lower(), head80
    box.succeed("ss -Hulnp 'sport = :443' | grep -q nginx")
    v = edge.succeed(f"curl -sk --http3-only -o /dev/null -w '%{{http_version}}' https://{box_v4}/").strip()
    assert v == "3", f"HTTP/3 not used: {v!r}"

    def tunnel_on(mode):
        edge.wait_until_succeeds(
          "journalctl -u losos-tunnel.service | grep 'holds box' | tail -1 "
          f"| grep -q '({mode})'",
          timeout=180,
        )
        edge.wait_until_succeeds("curl -fsS http://127.0.0.1:50000/ | grep -q hello-losos")

    def block_udp(on):
        op = "-I" if on else "-D"
        for t in ["iptables", "ip6tables"]:
            edge.succeed(f"{t} {op} INPUT -p udp --dport 2333 -j DROP")
        box.succeed("systemctl restart losos-tunnel-client.service")
        tunnel_on("tcp" if on else "quic")

    def cpu_ns(machine, unit):
        return int(machine.succeed(f"systemctl show -p CPUUsageNSec --value {unit}").strip())

    def fetch(url, flags, machine, unit):
        times = []
        before = cpu_ns(machine, unit)
        for _ in range(RUNS):
            out = edge.succeed(
              f"curl -fsS {flags} -o /tmp/got -w '%{{time_total}}\\n' {url} && sha256sum /tmp/got"
            ).strip().split("\n")
            secs, digest = float(out[0]), out[-1].split()[0]
            assert digest == BLOB_SHA, f"{url} {flags}: body changed"
            times.append(secs)
        cpu = (cpu_ns(machine, unit) - before) / 1e9 / RUNS
        return statistics.median(times), cpu

    def netem(spec):
        for m in [edge, box]:
            m.succeed("tc qdisc del dev eth1 root 2>/dev/null || true")
            m.succeed(f"tc qdisc add dev eth1 root netem {spec}")

    links = [
        ("clean (10 ms each way)", "delay 10ms"),
        ("lossy (40 ms each way, 2 % loss)", "delay 40ms loss 2%"),
    ]
    rows = []
    for name, spec in links:
        netem(spec)
        front = f"https://{box_v4}/blob"
        h2 = fetch(front, "-k --http2", box, "nginx.service")
        h3 = fetch(front, "-k --http3-only", box, "nginx.service")
        rows.append((name, "front door", "HTTP/2 over TCP", *h2))
        rows.append((name, "front door", "HTTP/3 over QUIC", *h3))
        tunnel_on("quic")
        tq = fetch("http://127.0.0.1:50000/blob", "", box, "losos-tunnel-client.service")
        block_udp(True)
        tt = fetch("http://127.0.0.1:50000/blob", "", box, "losos-tunnel-client.service")
        block_udp(False)
        rows.append((name, "edge tunnel", "TLS over TCP (fallback)", *tt))
        rows.append((name, "edge tunnel", "QUIC", *tq))

    table = [
        "| Link | Path | Transport | Median time for 4 MiB (s) | Box CPU per fetch (s) |",
        "| ---- | ---- | --------- | ------------------------- | --------------------- |",
    ]
    for name, path, transport, secs, cpu in rows:
        table.append(f"| {name} | {path} | {transport} | {secs:.2f} | {cpu:.3f} |")
    text = "\n".join(table) + "\n"
    print(text)
    with open(os.path.join(os.environ.get("out", "."), "quic.md"), "w") as f:
        f.write(text)
  '';
}
