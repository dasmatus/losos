# nixos-test-vms config for Tor and I2P addresses (modules/hidden-services.nix,
# modules/edge-hidden.nix).
#
# Neither network is reachable from a test, so the test brings its own:
#
#   * `net` runs a private Tor network (three directory authorities and two
#     relays, each its own tor process on its own ports, as chutney does) and
#     an I2P floodfill router on network id 77, nothing like the real ones;
#   * `edge` is an edge with losos.edge.hidden.{tor,i2p}.enable;
#   * `box` is a box behind it with losos.hidden.{tor,i2p}.enable, the real
#     lososd, nginx, LosOS Login, LosOS Git and the admin UI;
#   * `client` is a visitor with a Tor client, an I2P router and a browser.
#
# Every tor and i2pd here reads one more file that points it at the private
# network (`testnet`), and the box's and edge's I2P tunnels are one hop long:
# the private network has one router to go through. The rest is the shipped
# configuration. It asserts:
#
#   * the box's .onion and .b32.i2p answer with LosOS Login over https, on a
#     certificate the box's own certificate signed, and someone can sign in
#     there and open LosOS Git;
#   * the admin pages and the API refuse both addresses, whatever Host header
#     the visitor sends;
#   * the edge makes an address on each network for the box, routes it down
#     the box's tunnel, and hands it to the box on its heartbeat; the box's
#     LosOS Login takes the edge's names too;
#   * the b32 name losos-hidden-sync computes is the one i2pd logs;
#   * Settings › Network shows the switches and the addresses (the pictures
#     land in the test's output, shots/).
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  proxyToken = "/var/secrets/losos-proxy-token";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
    ];
  };

  shooter = pkgs.writeShellScriptBin "losos-shoot" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./shoot.mjs} "$@"
  '';

  # The Network pane, scrolled to its end, where the new section is: the
  # Before picture is the same plan on the previous admin UI, whose last
  # section is Windows. One plan per language for the handbook's page.
  networkPlan =
    locale: suffix:
    pkgs.writeText "network-plan${suffix}.json" (
      builtins.toJSON {
        viewport = {
          width = 1280;
          height = 900;
        };
        dpr = 2;
        colorScheme = "light";
        inherit locale;
        shots = [
          {
            file = "tor-and-i2p${suffix}.png";
            path = "/settings/network";
            title = "Settings, Network";
            waitFor = "[data-testid=network-hidden]";
            steps = [
              # The two switches show what overrides.nix says.
              {
                kind = "wait";
                target = "[data-testid=network-hidden] input[role=switch]:checked >> nth=1";
              }
              {
                kind = "scroll";
                target = "[data-testid=network-hidden]";
              }
            ];
          }
        ];
      }
    );

  i2pTestnet = ip: {
    loglevel = "info";
    netid = 77;
    host = ip;
    port = 12345;
    reservedrange = false;
    reseed.urls = "";
    reseed.yggurls = "";
    exploratory.inbound.length = 1;
    exploratory.outbound.length = 1;
    shareddest.inbound.length = 1;
    shareddest.outbound.length = 1;
  };
  i2pPorts = {
    networking.firewall.allowedTCPPorts = [ 12345 ];
    networking.firewall.allowedUDPPorts = [ 12345 ];
  };
in

pkgs.testers.runNixOSTest (
  { config, ... }:
  let
    netIP = config.nodes.net.networking.primaryIPAddress;

    # The private Tor network. Authorities vote every 20 s (chutney's
    # numbers) and flag every relay a guard and an onion directory.
    authorities = [
      "da1"
      "da2"
      "da3"
    ];
    relays = authorities ++ [
      "relay1"
      "relay2"
    ];
    index = name: lib.lists.findFirstIndex (n: n == name) null relays + 1;
    orPort = name: 5000 + index name;
    dirPort = name: 7000 + index name;

    daKeys = lib.genAttrs authorities (
      name:
      pkgs.runCommand "tor-testnet-${name}" { nativeBuildInputs = [ pkgs.tor ]; } ''
        d=$(mktemp -d)
        mkdir -p "$d/keys"
        tor --list-fingerprint --DataDirectory "$d" --ORPort ${toString (orPort name)} \
          --Nickname ${name} --SocksPort 0 >/dev/null 2>&1
        (cd "$d/keys" && echo "" | tor-gencert --create-identity-key -m 24 \
          -a ${netIP}:${toString (dirPort name)} --passphrase-fd 0 >/dev/null 2>&1)
        mkdir -p $out/keys
        cp "$d"/keys/* $out/keys/
        cut -d' ' -f2- "$d/fingerprint" | tr -d ' \n' > $out/relay-fingerprint
        grep '^fingerprint ' $out/keys/authority_certificate | awk '{print $2}' | tr -d '\n' > $out/v3ident
      ''
    );

    # The lines every tor in the test reads: where the network is. Written
    # when a tor starts, from the keys above, so no evaluation reads a build.
    testnet = pkgs.writeShellScript "tor-testnet" ''
      set -eu
      {
        echo "TestingTorNetwork 1"
        echo "PathsNeededToBuildCircuits 0.67"
      ${lib.concatMapStringsSep "\n" (name: ''
        printf 'DirAuthority %s orport=%s v3ident=%s %s:%s %s\n' ${name} ${toString (orPort name)} \
          "$(cat ${daKeys.${name}}/v3ident)" ${netIP} ${toString (dirPort name)} \
          "$(cat ${daKeys.${name}}/relay-fingerprint)"
      '') authorities}
      } > "$1"
      chmod 0644 "$1"
    '';

    relayUnit =
      name:
      let
        authority = builtins.elem name authorities;
        torrc = pkgs.writeText "torrc-${name}" ''
          DataDirectory /var/lib/tor-net-${name}
          Nickname ${name}
          Address ${netIP}
          ORPort ${toString (orPort name)}
          DirPort ${toString (dirPort name)}
          SocksPort 0
          ContactInfo ${name}@losos.test
          AssumeReachable 1
          ExitPolicy reject *:*
          ShutdownWaitLength 2
          TestingDirAuthVoteGuard *
          TestingDirAuthVoteHSDir *
          TestingMinExitFlagThreshold 0
          V3AuthNIntervalsValid 2
          ${lib.optionalString authority ''
            AuthoritativeDirectory 1
            V3AuthoritativeDirectory 1
            V3AuthVotingInterval 20
            TestingV3AuthInitialVotingInterval 20
            TestingV3AuthInitialVoteDelay 4
            TestingV3AuthInitialDistDelay 4
            V3AuthVoteDelay 4
            V3AuthDistDelay 4
          ''}
          %include /run/tor-net-${name}/testnet.conf
        '';
      in
      lib.nameValuePair "tor-net-${name}" {
        wantedBy = [ "multi-user.target" ];
        after = [ "network-online.target" ];
        wants = [ "network-online.target" ];
        serviceConfig = {
          # Not DynamicUser: tor refuses a data directory behind a symlink.
          User = "tor-net";
          Group = "tor-net";
          StateDirectory = "tor-net-${name}";
          StateDirectoryMode = "0700";
          RuntimeDirectory = "tor-net-${name}";
          ExecStartPre = [
            "${testnet} /run/tor-net-${name}/testnet.conf"
          ]
          ++ lib.optional authority (
            pkgs.writeShellScript "tor-net-keys-${name}" ''
              set -eu
              cd /var/lib/tor-net-${name}
              mkdir -p keys
              for f in ${daKeys.${name}}/keys/*; do
                install -m 0600 "$f" keys/
              done
              touch sr-state
            ''
          );
          ExecStart = "${pkgs.tor}/bin/tor -f ${torrc}";
          Restart = "on-failure";
        };
      };

    # The box's and the edge's tor read every file in
    # /var/lib/losos-hidden/tor/ (modules/hidden-lib.nix).
    # A unit of its own: tor's ExecStartPre runs inside its sandbox, where
    # that directory is read-only.
    hiddenTestnet = {
      systemd.services.tor-testnet = {
        wantedBy = [ "tor.service" ];
        before = [ "tor.service" ];
        after = [ "systemd-tmpfiles-setup.service" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          ExecStart = "${testnet} /var/lib/losos-hidden/tor/testnet.conf";
        };
      };
      lososInternal.hidden.i2pHops = 1;
    };
  in
  {
    name = "losos-hidden-services";

    nodes = {
      net =
        { config, ... }:
        {
          networking.firewall.enable = false;
          users.users.tor-net = {
            isSystemUser = true;
            group = "tor-net";
          };
          users.groups.tor-net = { };
          systemd.services = lib.listToAttrs (map relayUnit relays);
          services.i2pd = {
            enable = true;
            settings = i2pTestnet config.networking.primaryIPAddress // {
              floodfill = true;
              # The only router there is: nobody to hop through.
              exploratory.inbound.length = 0;
              exploratory.outbound.length = 0;
              shareddest.inbound.length = 0;
              shareddest.outbound.length = 0;
            };
          };
          virtualisation = {
            memorySize = 1024;
            cores = 2;
          };
        };

      edge =
        { config, ... }:
        {
          imports = [
            ../../modules/options.nix
            ../../modules/edge.nix
            ../../modules/edge-hidden.nix
            secretFiles
            hiddenTestnet
            i2pPorts
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
              hostname = "mattbox.losos.cfd";
              tokenFile = proxyToken;
            };
            hidden.tor.enable = true;
            hidden.i2p.enable = true;
          };
          services.i2pd.settings = i2pTestnet config.networking.primaryIPAddress;
          environment.systemPackages = [
            pkgs.curl
            pkgs.jq
          ];
          virtualisation = {
            memorySize = 1536;
            cores = 2;
          };
        };

      box =
        { config, ... }:
        {
          imports = [
            ../../modules/options.nix
            ../../modules/containers.nix
            ../../modules/tls.nix
            ../../modules/daemon.nix
            ../../modules/identity.nix
            ../../modules/config-repo.nix
            ../../modules/git.nix
            ../../modules/proxy.nix
            ../../modules/hidden-services.nix
            secretFiles
            hiddenTestnet
            i2pPorts
          ];
          losos = {
            hostName = "mattbox";
            backend.package = lososPkgs.losos-ctl;
            backend.plugins = lososPkgs.losos-plugins;
            admin.ui = lososPkgs.losos-admin-ui;
            git.enable = true;
            hidden.tor.enable = true;
            hidden.i2p.enable = true;
            proxy = {
              enable = true;
              edgeTunnelEndpoint = "edge:2333";
              registrarUrl = "http://edge:8443";
              hostname = "mattbox.losos.cfd";
              applianceId = "mattbox";
              tokenFile = proxyToken;
              heartbeatInterval = "3s";
              registrar.package = lososPkgs.losos-registrar;
            };
          };
          services.i2pd.settings = i2pTestnet config.networking.primaryIPAddress;
          # The claim checks the Host against the kernel's hostname.
          networking.hostName = lib.mkForce "mattbox";
          # The configuration repository the installer leaves behind.
          systemd.services.config-tree = {
            wantedBy = [ "multi-user.target" ];
            before = [ "lososd.service" ];
            unitConfig.ConditionPathExists = "!/etc/nixos/.git";
            serviceConfig.Type = "oneshot";
            path = [ pkgs.git ];
            script = ''
              mkdir -p /etc/nixos/modules
              cd /etc/nixos
              # Both networks on, as the Network pane writes them; the
              # configuration above sets the same, since the test does not
              # rebuild.
              printf '{ ... }:\n{\n  losos.hidden.tor.enable = true;\n  losos.hidden.i2p.enable = true;\n}\n' > modules/overrides.nix
              git init -q -b main
              git add -A
              git -c user.name=LosOS -c user.email=losos@mattbox.local commit -qm "The box as installed"
            '';
          };
          environment.systemPackages = [
            pkgs.jq
            pkgs.openssl
          ];
          # It boots alone, first; under emulation it needs every core to
          # reach a shell within the driver's five minutes.
          virtualisation = {
            memorySize = 2048;
            cores = 4;
          };
        };

      client =
        { config, nodes, ... }:
        {
          imports = [ i2pPorts ];
          services.tor = {
            enable = true;
            client.enable = true;
            settings."%include" = "/run/tor/testnet.conf";
          };
          systemd.services.tor.serviceConfig.ExecStartPre = lib.mkBefore [
            "+${testnet} /run/tor/testnet.conf"
          ];
          services.i2pd = {
            enable = true;
            settings = i2pTestnet config.networking.primaryIPAddress // {
              http.enabled = false;
              httpproxy.enabled = false;
              socksproxy.inbound.length = 1;
              socksproxy.outbound.length = 1;
            };
          };
          environment.systemPackages = [
            pkgs.curl
            pkgs.jq
            pkgs.openssl
            shooter
          ];
          environment.etc = {
            "losos-test/network-plan.json".source = networkPlan "en-US" "";
            "losos-test/network-plan-sk.json".source = networkPlan "sk-SK" "-sk";
            "losos-test/network-plan-de.json".source = networkPlan "de-DE" "-de";
          };
          networking.hosts.${nodes.box.networking.primaryIPAddress} = [ "mattbox.local" ];
          fonts.packages = [ pkgs.dejavu_fonts ];
          virtualisation = {
            memorySize = 2048;
            cores = 2;
          };
        };
    };

    testScript = ''
      import json
      import re

      PASSWORD = "Correct-Horse-Battery-9"
      TOR = "--socks5-hostname 127.0.0.1:9050"
      I2P = "--socks5-hostname 127.0.0.1:4447"

      # One at a time, the heaviest first: four emulated machines booting at
      # once can miss the driver's five minutes for a shell. Even one at a
      # time, a busy host can take longer than that for the third, so each
      # waits for its shell on the console first, with no such limit. The
      # box's tunnel and Tor wait for the others and try again.
      for m in [box, client, edge, net]:
          m.start()
          m.wait_for_console_text("Started backdoor.service", timeout=1800)
          m.wait_for_unit("multi-user.target")
      # Every tor from a network that is there now.
      net.wait_until_succeeds("ss -Htln | grep -q ':7003 '")
      for m in [edge, box, client]:
          m.systemctl("restart tor.service")

      with subtest("the private I2P network: every router knows the floodfill"):
          for m in [net, edge, box, client]:
              m.wait_for_file("/var/lib/i2pd/router.info")
          net.copy_from_machine("/var/lib/i2pd/router.info", "net-identity")
          seed = str(net.out_dir / "net-identity" / "router.info")
          for name, m in [("edge", edge), ("box", box), ("client", client)]:
              m.copy_from_machine("/var/lib/i2pd/router.info", f"{name}-identity")
              m.succeed("mkdir -p /var/lib/i2pd/netDb/r0")
              m.copy_from_host(seed, "/var/lib/i2pd/netDb/r0/net.dat")
              net.copy_from_host(
                  str(m.out_dir / f"{name}-identity" / "router.info"),
                  f"/var/lib/i2pd/netDb/r0/{name}.dat",
              )
          for m in [net, edge, box, client]:
              m.systemctl("restart i2pd.service")
          for m in [net, edge, box, client]:
              m.wait_for_unit("i2pd.service")

      with subtest("the box makes its addresses"):
          box.wait_until_succeeds(
              "jq -e '.front.tor and .front.i2p' /var/lib/losos-hidden/addresses.json", timeout=300
          )
          own = json.loads(box.succeed("cat /var/lib/losos-hidden/addresses.json"))["front"]
          ONION, B32 = own["tor"], own["i2p"]
          assert re.fullmatch(r"[a-z2-7]{56}\.onion", ONION), ONION
          assert re.fullmatch(r"[a-z2-7]{52}\.b32\.i2p", B32), B32
          assert box.succeed("cat /var/lib/tor/onion/losos-front/hostname").strip() == ONION
          # The name losos-hidden-sync worked out from the keys file is the
          # one i2pd itself announces.
          box.wait_until_succeeds(
              f"journalctl -u i2pd -o cat | grep -F 'Local address {B32.removesuffix('.b32.i2p')}'"
          )
          # The torrc fragment and the tunnels, as written.
          conf = box.succeed("cat /var/lib/losos-hidden/tor/services.conf")
          assert "HiddenServicePort 80 127.0.0.1:8480" in conf, conf
          assert "HiddenServicePort 443 127.0.0.1:8443" in conf, conf
          tunnels = box.succeed("cat /var/lib/losos-hidden/i2pd/services.conf")
          assert "[losos-front-https]" in tunnels and "inport = 443" in tunnels, tunnels
          # Nothing of it on the LAN: nginx's two listeners and the TLS
          # one are loopback only, and neither daemon has a port open to it.
          listen = box.succeed("ss -Htln")
          for port in ["8480", "8481", "8443"]:
              assert f"127.0.0.1:{port}" in listen and f"0.0.0.0:{port}" not in listen, listen
          box.fail("ss -Htln | grep -E ':(9050|4444|4447|7070|7656) '")
          # The certificate names both, and the box's own signed it.
          box.wait_until_succeeds(f"grep -qxF '{ONION}' /var/lib/losos-hidden/tls/names")
          box.succeed(
              "openssl verify -CAfile /var/lib/losos-tls/cert.pem /var/lib/losos-hidden/tls/cert.pem"
          )
          sans = box.succeed("openssl x509 -noout -ext subjectAltName -in /var/lib/losos-hidden/tls/cert.pem")
          assert ONION in sans and B32 in sans, sans
          box.copy_from_machine("/var/lib/losos-tls/cert.pem", "box-ca")
          client.copy_from_host(str(box.out_dir / "box-ca" / "cert.pem"), "/tmp/box-ca.pem")

      with subtest("Matus claims the box on the LAN"):
          body = json.dumps({"name": "matus", "password": PASSWORD})
          client.succeed(
              "curl -skf -X POST https://mattbox.local/api/setup/claim"
              f" -H 'Content-Type: application/json' -d '{body}'"
          )
          box.wait_for_unit("authelia-losos.service")
          box.wait_for_open_port(9091)
          # LosOS Login sets cookies for the box's own addresses too.
          box.wait_until_succeeds(f"grep -qF \"domain: '{ONION}'\" /run/losos-identity/session.yml")
          box.wait_until_succeeds(f"grep -qF \"domain: '{B32}'\" /run/losos-identity/session.yml")

      def fetch(proxy, url, extra="", timeout=900):
          """Retries until the network has carried the request once."""
          client.wait_until_succeeds(
              f"curl -s -o /dev/null --max-time 60 {proxy} --cacert /tmp/box-ca.pem {extra} {url}",
              timeout=timeout,
          )
          out = client.succeed(
              f"curl -s --max-time 120 {proxy} --cacert /tmp/box-ca.pem -w '\\n%{{http_code}}' {extra} {url}"
          )
          text, code = out.rsplit("\n", 1)
          return int(code), text

      with subtest("the box over Tor"):
          client.wait_for_unit("tor.service")
          code, page = fetch(TOR, f"https://{ONION}/auth/")
          assert code == 200 and "<title>" in page, (code, page[:400])
          # Plain http on port 80 reaches the same door.
          code, _ = fetch(TOR, f"http://{ONION}/auth/")
          assert code in (200, 301, 302), code
          # The admin pages and the API stay on the LAN, whoever the
          # visitor says they are.
          for host in [ONION, "mattbox.local", "losos-manage.invalid", "localhost"]:
              code, _ = fetch(TOR, f"http://{ONION}/", f"-H 'Host: {host}'")
              assert code == 403, (host, code)
          code, _ = fetch(TOR, f"https://{ONION}/api/state")
          assert code in (401, 403), code

      with subtest("signing in over Tor and opening LosOS Git"):
          body = json.dumps({"username": "matus", "password": PASSWORD})
          head = client.succeed(
              f"curl -s --max-time 120 {TOR} --cacert /tmp/box-ca.pem -D - -o /dev/null "
              f"https://{ONION}/auth/api/firstfactor -H 'Content-Type: application/json' -d '{body}'"
          )
          m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+);.*domain=" + re.escape(ONION), head)
          assert m, head
          code, page = fetch(TOR, f"https://{ONION}/git/", f"-H 'Cookie: {m.group(1)}'")
          assert code == 200 and "LosOS Git" in page, (code, page[:400])

      with subtest("the box over I2P"):
          code, page = fetch(I2P, f"https://{B32}/auth/")
          assert code == 200 and "<title>" in page, (code, page[:400])
          code, _ = fetch(I2P, f"http://{B32}/")
          assert code == 403, code

      with subtest("the edge gives the box addresses of its own"):
          edge.wait_until_succeeds(
              "jq -e '.mattbox.tor and .mattbox.i2p' /var/lib/losos-hidden/addresses.json", timeout=300
          )
          made = json.loads(edge.succeed("cat /var/lib/losos-hidden/addresses.json"))["mattbox"]
          # Traefik reads them from the registrar's provider document.
          routes = edge.succeed("curl -fsS http://127.0.0.1:8090/traefik")
          assert made["tor"] in routes and made["i2p"] in routes, routes
          # The box hears them on its heartbeat and keeps them.
          box.wait_until_succeeds(
              f"jq -e '.tor == \"{made['tor']}\" and .i2p == \"{made['i2p']}\"' /var/lib/losos-hidden/edge.json"
          )
          box.wait_until_succeeds(f"grep -qF \"domain: '{made['tor']}'\" /run/losos-identity/session.yml")
          # The public entry points know nothing of them.
          code = edge.succeed(
              f"curl -sk -o /dev/null -w '%{{http_code}}' -H 'Host: {made['tor']}' https://127.0.0.1/"
          ).strip()
          assert code == "404", code
          # Through Tor and I2P to the edge, down the tunnel to the box.
          code, page = fetch(TOR, f"https://{made['tor']}/auth/", "-k")
          assert code == 200 and "<title>" in page, (code, page[:400])
          code, page = fetch(I2P, f"https://{made['i2p']}/auth/", "-k")
          assert code == 200 and "<title>" in page, (code, page[:400])
          code, _ = fetch(TOR, f"http://{made['tor']}/")
          assert code == 403, code

      with subtest("Settings, Network on the LAN"):
          head = client.succeed(
              "curl -sk -D - -o /dev/null https://mattbox.local/auth/api/firstfactor "
              f"-H 'Content-Type: application/json' -d '{body}'"
          )
          found = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
          assert found, head
          cookie = found.group(1)
          shown = json.loads(
              client.succeed(f"curl -skf https://mattbox.local/api/hidden -H 'Cookie: {cookie}'")
          )
          assert shown == {
              "box": {"tor": ONION, "i2p": B32},
              "edge": {"tor": made["tor"], "i2p": made["i2p"]},
          }, shown
          client.succeed("mkdir -p /tmp/shots")
          missed = []
          for plan in ["network-plan", "network-plan-sk", "network-plan-de"]:
              status, _ = client.execute(
                  f"losos-shoot /etc/losos-test/{plan}.json /tmp/shots https://mattbox.local matus"
                  f" '{PASSWORD}' >&2",
                  timeout=1800,
              )
              if status != 0:
                  missed.append(plan)
          client.copy_from_machine("/tmp/shots", "")
          assert not missed, f"not taken: {missed}; see the log"
    '';
  }
)
