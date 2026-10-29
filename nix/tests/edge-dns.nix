# nixos-test for the edge's own DNS zone (losos.edge.dns.*).
#
# Two VMs:
#   * `edge`   — losos.edge with dns.enable: Knot serving the zone the
#                registrar renders, the reload path unit, port 53 open.
#   * `client` — a plain machine with kdig, asking the edge over the
#                test network as any resolver on the internet would, and
#                losos-ctl for the box's own check of the edge's name.
#
# What it asserts, in order:
#   1. Before the edge's owner has claimed it, it serves the zone (SOA, NS,
#      glue) but no box names, and `/domains/list` answers 503: an open
#      gateway nobody claimed hands out nothing.
#   2. A claim: the measured settings appear where the boot after
#      `POST /passport/settings` leaves them (the test writes them, with no
#      TPM) and the registrar restarts. It rewrites the zone with the tenant
#      hostname inside it, the path unit reloads Knot, and the client gets
#      the edge's address for `mattbox.boxes.example.test` over UDP and TCP,
#      with the SOA serial the file carries. What the settings say is the
#      boxes' to check (backend/access/src/gateway/check.rs), not this test's.
#   3. `/domains/list` now answers 200 and says why this box may not add a
#      domain: the edge runs no market, so there is no Stripe account to
#      vouch for it. It carries the box's relay pass, because the edge keeps
#      a route table (losos.edge.dns.relayRoutes).
#   4. DNSSEC and the box's wallet: Knot signs the zone, the box binds a
#      wallet with that wallet's signature (made here the way the admin page
#      makes it, with viem, for a fixed test key), and the zone then carries
#      the ENS record at the box's name, signed, and validated by `delv`
#      against the zone's own DS as the trust anchor. A signature for another
#      box is refused; unbinding takes the record away.
#   5. Encrypted Client Hello: the edge made its key (0640, group traefik),
#      every name carries an HTTPS record with it, signed and validated like
#      the wallet's, and a client that reads the key from that record
#      reaches the box's name through Traefik with ECH accepted, the box's
#      name never in the clear.
#   6. The box's side: `losos-ctl edge-domain`, the check lososd holds an
#      official edge to, passes the box's name (signed, with the key), says
#      a name the zone proves absent has no key, finds nothing signed from
#      a trust anchor the zone's key does not match, and refuses plain HTTP.
#   7. The route table's wiring: the registrar made its relay pass key
#      (0600); a spoke forwarding that pass on `/relay` is answered with no
#      routes (the box is not in this edge's mesh), but the binding is kept
#      in relay-routes.json (0600) beside the registry, and is still there
#      after the registrar restarts.
#
# The box labels and the custom-domain checks themselves (Stripe data, the
# TXT token, the CNAME, Traefik routers) are covered against a real
# registrar in edge/registrar/tests/domains.rs; this test is the NixOS
# wiring those tests cannot reach.
{ pkgs }:

let
  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  proxyToken = "/var/secrets/losos-proxy-token";
  spokeTokenValue = "test-spoke-token-0123456789abcdef0123456789abcdef";
  spokeToken = "/var/secrets/losos-tenant-acme";
  lososPkgs = import ../packages.nix { inherit pkgs; };
  registrar = "${lososPkgs.losos-registrar}/bin/losos-registrar";
  # A test wallet (private key 0x0707…07) and its personal_sign of
  # "LosOS box name v1\nzone: boxes.example.test\nbox: mattbox\naddress: <it>\n",
  # as viem's signMessage makes it in the admin page.
  walletAddress = "0x4a62316623ad457f02cdc5d997ded67a383ec569";
  walletSignature = "0xf60bb8dd608847e9d8dcc4f85fc660e541f7b9050677dac6cf472873b4d01e622718b441f44b85d93219309bb09f4ab3a19894ef759bf155655b84c51c8370301c";
  ensRecord = "ENS1 dnsname.ens.eth ${walletAddress}";
  # An ECH client: Go's crypto/tls with the ECHConfigList it is handed,
  # printing whether the server decrypted the inner hello.
  echProbe = pkgs.buildGoModule {
    pname = "ech-probe";
    version = "0";
    src = pkgs.runCommand "ech-probe-src" { } ''
      mkdir $out
      printf 'module ech-probe\n\ngo 1.24\n' > $out/go.mod
      cp ${pkgs.writeText "main.go" ''
        package main

        import (
        	"crypto/tls"
        	"encoding/base64"
        	"fmt"
        	"os"
        )

        // ech-probe ADDR NAME ECHCONFIGLIST
        func main() {
        	list, err := base64.StdEncoding.DecodeString(os.Args[3])
        	if err != nil {
        		fmt.Println("error:", err)
        		os.Exit(2)
        	}
        	c, err := tls.Dial("tcp", os.Args[1], &tls.Config{
        		ServerName:                     os.Args[2],
        		EncryptedClientHelloConfigList: list,
        		// The test edge has no certificate from Let's Encrypt.
        		InsecureSkipVerify: true,
        	})
        	if err != nil {
        		fmt.Println("error:", err)
        		os.Exit(1)
        	}
        	defer c.Close()
        	st := c.ConnectionState()
        	fmt.Printf("ech=%v tls=%x\n", st.ECHAccepted, st.Version)
        }
      ''} $out/main.go
    '';
    vendorHash = null;
  };
in

pkgs.testers.nixosTest {
  name = "losos-edge-dns";

  nodes = {
    edge =
      { config, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
        ];
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };
        systemd.tmpfiles.rules = [
          "d /var/secrets 0700 root root - -"
          "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
          "f ${spokeToken} 0600 root root - ${spokeTokenValue}"
        ];

        losos.edge = {
          enable = true;
          acmeEmail = "test@example.test";
          publicDomain = "example.test";
          registrarApiBind = "::";
          reconcileInterval = "2s";
          tenants.mattbox = {
            hostname = "mattbox.boxes.example.test";
            tokenFile = proxyToken;
          };
          # A local edge that may relay boxes under acme.example.test.
          tenants.acme = {
            hostname = "acme.example.test";
            tokenFile = spokeToken;
            relayZone = "acme.example.test";
          };
          dns = {
            enable = true;
            ipv4 = [ config.networking.primaryIPAddress ];
            # Nothing outside the test network answers; the checks are
            # covered by the Rust suite.
            checkUrl = "http://127.0.0.1:9/dns-query";
            relayRoutes.enable = true;
          };
        };
        networking.firewall.allowedTCPPorts = [
          8443
          443
        ];
        networking.firewall.allowedUDPPorts = [ 443 ];
        # Only a claimed open gateway's zone hands out names
        # (modules/edge-passport.nix). No TPM here: the test stands in for
        # the measured claim itself, below.
        losos.edge.passport.enable = true;
        environment.systemPackages = [ pkgs.jq ];
        virtualisation.memorySize = 1024;
      };

    client = _: {
      environment.systemPackages = [
        pkgs.knot-dns
        # delv: a validating lookup against a trust anchor of our choosing.
        pkgs.bind
        echProbe
        pkgs.tcpdump
        # The box's own check of an official edge's name (`edge-domain`).
        lososPkgs.losos-ctl
      ];
    };
  };

  testScript =
    { nodes, ... }:
    let
      edgeIp = nodes.edge.networking.primaryIPAddress;
    in
    ''
      import json
      import re

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

      with subtest("not claimed: the zone answers, but holds no box names"):
          edge.wait_until_succeeds("test -s /var/lib/losos-dns/boxes.example.test.zone")
          zone = edge.succeed("cat /var/lib/losos-dns/boxes.example.test.zone")
          print(zone)
          assert "mattbox" not in zone, zone
          edge.succeed("stat -c %a /var/lib/losos-dns/boxes.example.test.zone | grep -qx 644")
          client.wait_until_succeeds(
              "kdig @${edgeIp} boxes.example.test SOA +short | grep -q 'ns1.boxes.example.test.'"
          )
          assert dig("ns1.boxes.example.test") == "${edgeIp}"
          assert dig("mattbox.boxes.example.test") == "", "a name before the edge is claimed"
          body, code = domains_list()
          assert code == "503", (code, body)

      with subtest("claimed: the tenant's name is assigned and Knot reloads"):
          # The claim as the registrar sees it after the boot that measured
          # the owner's settings into the TPM. The measuring and the TPM are
          # the passport's own business; the zone only asks whether there is
          # a claim.
          edge.succeed(
              "install -d -m 0755 /run/losos-gateway",
              "echo '{\"owner\":\"test\"}' > /run/losos-gateway/settings.json",
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
          # Knot signs the zone and bumps the serial when it does.
          soa = dig("boxes.example.test", "SOA")
          assert int(soa.split()[2]) >= int(serial), (soa, serial)
          edge.succeed("systemctl show -p Result losos-dns-reload.service | grep -qx Result=success")
          edge.succeed("journalctl -u knot.service | grep -q 'boxes.example.test'")

      with subtest("claimed without a market: the box is told Stripe has not vouched"):
          body, code = domains_list()
          assert code == "200", (code, body)
          view = json.loads(body)
          assert view["eligible"] is False, view
          assert view["reason"] == "stripeAccount", view
          assert view["addresses"] == ["${edgeIp}"], view
          assert view["relay_pass"].startswith("v1."), view
          print(json.dumps(view, indent=2))

      with subtest("DNSSEC, and the box's wallet at its name"):
          signed = client.succeed(
              "kdig @${edgeIp} boxes.example.test SOA +dnssec +norec"
          )
          assert "RRSIG" in signed, signed
          client.succeed("kdig @${edgeIp} boxes.example.test DNSKEY +short | grep -q '^257 3 13 '")
          assert dig("mattbox.boxes.example.test", "TXT") == "", "a record before any wallet"

          def wallet(body):
              return edge.succeed(
                  "curl -s -o /dev/stdout -w '\\n%{http_code}' -X POST "
                  "-H 'content-type: application/json' "
                  f"-d '{json.dumps(body)}' http://127.0.0.1:8443/domains/wallet"
              ).rsplit("\n", 1)

          me = {"appliance_id": "mattbox", "token": "${proxyTokenValue}"}
          # A signature for another box binds nothing here.
          body, code = wallet({**me, "appliance_id": "acme", "token": "${spokeTokenValue}",
                               "address": "${walletAddress}", "signature": "${walletSignature}"})
          assert code == "400", (code, body)
          body, code = wallet({**me, "address": "${walletAddress}", "signature": "${walletSignature}"})
          assert code == "200", (code, body)
          view = json.loads(body)
          assert view["wallet"]["address"] == "${walletAddress}", view
          assert view["wallet"]["record"] == "${ensRecord}", view
          assert view["names"] == ["mattbox.boxes.example.test"], view
          client.wait_until_succeeds(
              "kdig @${edgeIp} mattbox.boxes.example.test TXT +short | grep -qF '${ensRecord}'"
          )
          txt = client.succeed("kdig @${edgeIp} mattbox.boxes.example.test TXT +dnssec +norec")
          print(txt)
          assert re.search(r"RRSIG\s+TXT\s+13\s", txt), txt

          # Validate it as a resolver would, from the zone's DS: the record
          # ENS reads is the one the zone's key signed.
          # keymgr prints `<zone>. DS <tag> 13 2 <digest>`: SHA-256 of the KSK.
          ds = edge.succeed("keymgr boxes.example.test ds | grep -E ' DS [0-9]+ 13 2 '").split()
          tag, alg, dtype, digest = ds[-4], ds[-3], ds[-2], ds[-1]
          client.succeed(
              "printf 'trust-anchors { boxes.example.test. static-ds %s %s %s \"%s\"; };' "
              f"{tag} {alg} {dtype} {digest} > /tmp/anchor.conf"
          )
          valid = client.succeed(
              "delv @${edgeIp} -a /tmp/anchor.conf +root=boxes.example.test "
              "mattbox.boxes.example.test TXT 2>&1"
          )
          print(valid)
          assert "fully validated" in valid, valid
          assert "${ensRecord}" in valid, valid

          body, code = wallet(me)
          assert code == "200", (code, body)
          assert json.loads(body)["wallet"] is None, body
          client.wait_until_succeeds(
              "test -z \"$(kdig @${edgeIp} mattbox.boxes.example.test TXT +short)\""
          )

      with subtest("Encrypted Client Hello: the key, the HTTPS records, a handshake"):
          edge.succeed("systemctl show -p Result losos-ech-key.service | grep -qx Result=success")
          edge.succeed("stat -c '%a %G' /var/lib/losos-ech/ech.pem | grep -qx '640 traefik'")
          edge.succeed("grep -q 'BEGIN ECHCONFIG' /var/lib/losos-ech/ech.pem")
          # The registrar reads the same key: one key, one value in the zone.
          ech = edge.succeed(
              "${registrar} ech-key --key-file /var/lib/losos-ech/ech.pem --public-name register.example.test"
          ).strip().splitlines()[-1]
          for name in ["boxes.example.test", "mattbox.boxes.example.test"]:
              client.wait_until_succeeds(
                  f"kdig @${edgeIp} {name} HTTPS +short | grep -qF '{ech}'"
              )
          https = client.succeed("kdig @${edgeIp} mattbox.boxes.example.test HTTPS +short")
          print(https)
          assert "alpn=h3,h2" in https.replace('"', ""), https
          signed = client.succeed("kdig @${edgeIp} mattbox.boxes.example.test HTTPS +dnssec +norec")
          assert re.search(r"RRSIG\s+HTTPS\s+13\s", signed), signed
          valid = client.succeed(
              "delv @${edgeIp} -a /tmp/anchor.conf +root=boxes.example.test "
              "mattbox.boxes.example.test HTTPS 2>&1"
          )
          print(valid)
          assert "fully validated" in valid, valid

          # Traefik loaded the key without complaint, and decrypts with it.
          edge.wait_for_open_port(443)
          edge.fail("journalctl -u traefik.service | grep -i 'ech key'")
          client.succeed("tcpdump -i eth1 -w /tmp/hello.pcap -U 'tcp port 443' >/dev/null 2>&1 & echo $! > /tmp/tcpdump.pid")
          client.sleep(2)
          probe = client.succeed(f"ech-probe ${edgeIp}:443 mattbox.boxes.example.test '{ech}'").strip()
          print(probe)
          assert probe == "ech=true tls=304", probe
          client.succeed("sleep 1; kill $(cat /tmp/tcpdump.pid)")
          # On the wire the hello names the public name, never the box.
          client.succeed("grep -aq 'register.example.test' /tmp/hello.pcap")
          client.fail("grep -aq mattbox /tmp/hello.pcap")
          # A key Traefik does not hold is turned down, and says so.
          # The value is the last line; making the key logs a line first.
          other = edge.succeed(
              "${registrar} ech-key --key-file /tmp/other.pem --public-name register.example.test"
          ).strip().splitlines()[-1]
          refused = client.fail(f"ech-probe ${edgeIp}:443 mattbox.boxes.example.test '{other}'")
          print(refused)
          # Turned down, the client checks the outer handshake's certificate
          # against the public name, as ECH requires; the test edge has none
          # from Let's Encrypt, so Go stops there rather than at the retry.
          assert "ECH" in refused or "not register.example.test" in refused, refused

      with subtest("a box holds an official edge to a signed name with an ECH key"):
          # The check lososd runs on a gateway that passed everything else
          # (losos.proxy.gateways.domain), pointed at this zone and its DS
          # the way modules/daemon.nix hands it the options.
          def domain(url, anchors="/tmp/anchor.conf"):
              env = (
                  "LOSOS_EDGE_DNS_SERVER=${edgeIp} "
                  f"LOSOS_EDGE_DNS_ANCHORS={anchors} "
                  "LOSOS_EDGE_DNS_ROOT=boxes.example.test"
              )
              code, out = client.execute(f"{env} losos-ctl edge-domain {url}")
              print(url, code, out)
              return code, out
          code, out = domain("https://mattbox.boxes.example.test")
          assert code == 0, out
          assert json.loads(out) == {
              "name": "mattbox.boxes.example.test", "signed": True, "ech": True,
          }, out
          # No such name: the zone proves it, signed, and there is no key.
          code, out = domain("https://nobody.boxes.example.test")
          assert code == 1 and json.loads(out) == {
              "name": "nobody.boxes.example.test", "signed": True, "ech": False,
          }, out
          # A trust anchor the zone's key does not match: nothing validates.
          wrong = client.succeed("cat /tmp/anchor.conf").replace(digest, digest[::-1])
          assert wrong != client.succeed("cat /tmp/anchor.conf"), wrong
          client.succeed(f"printf '%s' '{wrong}' > /tmp/wrong.conf")
          code, out = domain("https://mattbox.boxes.example.test", "/tmp/wrong.conf")
          assert code == 1 and json.loads(out)["signed"] is False, out
          # Plain HTTP has no name to hold it to: refused before any lookup.
          code, out = domain("http://mattbox.boxes.example.test")
          assert code == 1 and out == "", out

      with subtest("the route table: the pass key, /relay, and the table file"):
          edge.succeed("stat -c %a /var/lib/losos-registrar/relay-pass.key | grep -qx 600")
          edge.succeed("grep -Eqx '[0-9a-f]{64}' /var/lib/losos-registrar/relay-pass.key")
          relay = {
              "appliance_id": "acme",
              "token": "${spokeTokenValue}",
              "tenants": [{
                  "id": "mattbox",
                  "hostname": "mattbox.acme.example.test",
                  "pass": view["relay_pass"],
              }],
          }
          out = edge.succeed(
              "curl -sf -X POST -H 'content-type: application/json' "
              f"-d '{json.dumps(relay)}' http://127.0.0.1:8443/relay"
          )
          resp = json.loads(out)
          assert [a["id"] for a in resp["accepted"]] == ["mattbox"], resp
          assert resp["routes"] == [], resp
          table = "/var/lib/losos-registrar/relay-routes.json"
          edge.wait_until_succeeds(
              f"jq -e '.bindings.mattbox.spoke == \"acme\" and .routes == []' {table}",
              timeout=60,
          )
          edge.succeed(f"stat -c %a {table} | grep -qx 600")
          # The registrar's first pass runs before its API opens and rewrites
          # the file from the bindings it loaded, so an open port after the
          # restart means the file below is the new process's.
          edge.systemctl("restart losos-registrar.service")
          edge.wait_for_open_port(8443)
          edge.succeed(f"jq -e '.bindings.mattbox.spoke == \"acme\"' {table}")
          print(edge.succeed(f"cat {table}"))
    '';
}
