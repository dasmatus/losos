# nixos-test for the edge's own DNS zone (losos.edge.dns.*).
#
# Two VMs:
#   * `edge`   — losos.edge with dns.enable: Knot serving the zone the
#                registrar renders, the reload path unit, port 53 open.
#   * `client` — a plain machine with kdig, asking the edge over the
#                test network as any resolver on the internet would.
#
# What it asserts, in order:
#   1. Before the edge holds a certificate it serves the zone (SOA, NS, glue)
#      but no box names, and `/domains/list` answers 503: an edge that is not
#      official hands out nothing.
#   2. An identity certificate is installed (signed here by a throwaway root,
#      the way `losos-registrar provision edge` would push one) and the
#      registrar restarted: the registrar rewrites the zone with the tenant
#      hostname inside it, the path unit reloads Knot, and the client gets
#      the edge's address for `mattbox.boxes.example.test` over UDP and TCP,
#      with the SOA serial the file carries.
#   3. `/domains/list` now answers 200 and says why this box may not add a
#      domain: the edge runs no market, so there is no Stripe account to
#      vouch for it.
#
# The box labels and the custom-domain checks themselves (Stripe data, the
# TXT token, the CNAME, Traefik routers) are covered against a real
# registrar in backend-registrar/tests/domains.rs; this test is the NixOS
# wiring those tests cannot reach.
{ pkgs }:

let
  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  bootstrapTokenValue = "test-bootstrap-0123456789abcdef0123";
  proxyToken = "/var/secrets/losos-proxy-token";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };
  registrar = "${lososPkgs.losos-registrar}/bin/losos-registrar";
in

pkgs.testers.nixosTest {
  name = "losos-edge-dns";

  nodes = {
    edge =
      { config, ... }:
      {
        imports = [
          ../modules/options.nix
          ../modules/edge.nix
        ];
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };
        systemd.tmpfiles.rules = [
          "d /var/secrets 0700 root root - -"
          "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
          "f ${bootstrapToken} 0600 root root - ${bootstrapTokenValue}"
        ];

        losos.edge = {
          enable = true;
          acmeEmail = "test@example.test";
          publicDomain = "example.test";
          registrarApiBind = "::";
          reconcileInterval = "2s";
          bootstrapTokenFile = bootstrapToken;
          tenants.mattbox = {
            hostname = "mattbox.boxes.example.test";
            tokenFile = proxyToken;
          };
          dns = {
            enable = true;
            ipv4 = [ config.networking.primaryIPAddress ];
            # Nothing outside the test network answers; the checks are
            # covered by the Rust suite.
            checkUrl = "http://127.0.0.1:9/dns-query";
          };
        };
        networking.firewall.allowedTCPPorts = [ 8443 ];
        environment.systemPackages = [ pkgs.jq ];
        virtualisation.memorySize = 1024;
      };

    client = _: {
      environment.systemPackages = [ pkgs.knot-dns ];
    };
  };

  testScript =
    { nodes, ... }:
    let
      edgeIp = nodes.edge.networking.primaryIPAddress;
    in
    ''
      import json

      start_all()
      edge.wait_for_unit("knot.service")
      edge.wait_for_unit("losos-registrar.service")
      edge.wait_for_open_port(53)
      edge.wait_for_open_port(8443)
      client.wait_for_unit("multi-user.target")

      def dig(name, rtype="A", extra=""):
          return client.succeed(
              f"kdig @${edgeIp} {name} {rtype} +short +norec {extra}"
          ).strip()

      def domains_list():
          return edge.succeed(
              "curl -s -o /dev/stdout -w '\\n%{http_code}' -X POST "
              "-H 'content-type: application/json' "
              "-d '{\"appliance_id\":\"mattbox\",\"token\":\"${proxyTokenValue}\"}' "
              "http://127.0.0.1:8443/domains/list"
          ).rsplit("\n", 1)

      with subtest("not official: the zone answers, but holds no box names"):
          edge.wait_until_succeeds("test -s /var/lib/losos-dns/boxes.example.test.zone")
          zone = edge.succeed("cat /var/lib/losos-dns/boxes.example.test.zone")
          print(zone)
          assert "mattbox" not in zone, zone
          edge.succeed("stat -c %a /var/lib/losos-dns/boxes.example.test.zone | grep -qx 644")
          client.wait_until_succeeds(
              "kdig @${edgeIp} boxes.example.test SOA +short | grep -q 'ns1.boxes.example.test.'"
          )
          assert dig("ns1.boxes.example.test") == "${edgeIp}"
          assert dig("mattbox.boxes.example.test") == "", "a name before the edge is official"
          body, code = domains_list()
          assert code == "503", (code, body)

      with subtest("official: the tenant's name is assigned and Knot reloads"):
          edge.succeed(
              "${registrar} identity keygen --out /root/root.key",
              "${registrar} identity show --key /var/lib/losos-registrar/identity.key > /root/edge.pub",
              "${registrar} identity sign --root-key /root/root.key --public-key \"$(cat /root/edge.pub)\" "
              "--name 'test edge' --url http://edge:8443 > /var/lib/losos-registrar/identity.cert.json",
              "systemctl restart losos-registrar.service",
          )
          edge.wait_until_succeeds(
              "grep -q '^mattbox IN A ${edgeIp}$' /var/lib/losos-dns/boxes.example.test.zone"
          )
          serial = edge.succeed(
              "awk '/^@ IN SOA/ {print $6}' /var/lib/losos-dns/boxes.example.test.zone"
          ).strip()
          client.wait_until_succeeds(
              "test \"$(kdig @${edgeIp} mattbox.boxes.example.test A +short)\" = '${edgeIp}'"
          )
          assert dig("mattbox.boxes.example.test", extra="+tcp") == "${edgeIp}"
          soa = dig("boxes.example.test", "SOA")
          assert soa.split()[2] == serial, (soa, serial)
          edge.succeed("systemctl show -p Result losos-dns-reload.service | grep -qx Result=success")
          edge.succeed("journalctl -u knot.service | grep -q 'boxes.example.test'")

      with subtest("official without a market: the box is told Stripe has not vouched"):
          body, code = domains_list()
          assert code == "200", (code, body)
          view = json.loads(body)
          assert view["eligible"] is False, view
          assert view["reason"] == "stripeAccount", view
          assert view["addresses"] == ["${edgeIp}"], view
          print(json.dumps(view, indent=2))
    '';
}
