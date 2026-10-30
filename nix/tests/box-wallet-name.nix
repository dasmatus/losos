# nixos-test for paying a box by its name, from the owner's side.
#
# Three VMs on one network:
#   * `edge`   — an official edge (losos.edge with dns.enable, an open
#                gateway the box vets, nix/tests/open-gateway.nix): Knot signs the
#                zone, the registrar answers /domains*. Its API is behind TLS
#                on :8444, because lososd sends the box's token to an edge
#                only over https.
#   * `box`    — mattbox: lososd, the front vhost with the real admin UI,
#                LosOS Login, and the master proxy pointed at the edge, so the
#                edge's /domains routes reach the Network pane through lososd.
#   * `laptop` — Chromium driven by Playwright (box-wallet-name.mjs), signed
#                in through LosOS Login as the owner, plus delv.
#
# What it asserts:
#   1. the box finds the edge official and the Network pane offers to bind
#      its wallet at mattbox.boxes.losos.dasmat.us;
#   2. the owner binds the wallet made on this box: the page opens the
#      sealed phrase with its password, signs, and lososd relays the binding;
#   3. the edge's zone then carries the ENS record at the box's name, signed,
#      and `delv` validates it against the zone's DS as the trust anchor;
#   4. public DNS (a stand-in on the edge, as ENS's gateway would ask it)
#      finds nothing until the zone is delegated, an unsigned record until
#      its DS is published, and then the validated record, and the pane says
#      so for the name: wallets find it;
#   5. unbinding from the API takes the record away again.
#
# The pages it passes through are saved as PNGs (`shots/` in the test's
# output): the PR's screenshots, and the states the launch film is cut from
# (`film/name-pay/` with its manifest.json). With `before = true` the box
# runs a given older admin UI and lososd, and only the Network pane is shot.
{
  pkgs,
  # Run against another tree's packages (a `nix/packages.nix` result) for the
  # PR's "before" screenshots; the page is then only shot, not driven.
  lososPkgs ? import ../packages.nix { inherit pkgs; },
  before ? false,
}:

let
  inherit (pkgs) lib;
  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  proxyToken = "/var/secrets/losos-proxy-token";
  edgeName = "edge.losos.dasmat.us";
  registrarUrl = "https://${edgeName}:8444";
  zone = "boxes.losos.dasmat.us";
  password = "Correct-Horse-Battery-9";

  # The store keeps the fixture for as long as its inputs do not change, so
  # its certificates are valid for ten years, not 30 days after the first
  # build.
  validDays = "3650";

  # The edge is an open gateway whose passport names the URL the box uses.
  openGateway = import ./open-gateway.nix { inherit pkgs lososPkgs; };

  # A TLS certificate for the edge's name from a CA the box trusts.
  fixture =
    pkgs.runCommand "losos-box-wallet-name-fixture"
      {
        nativeBuildInputs = [ pkgs.openssl ];
      }
      ''
        mkdir -p $out
        openssl req -x509 -newkey rsa:2048 -nodes -days ${validDays} -subj "/CN=test CA" \
          -keyout $out/ca.key -out $out/ca.pem
        openssl req -newkey rsa:2048 -nodes -subj "/CN=${edgeName}" \
          -keyout $out/tls.key -out tls.csr
        printf 'subjectAltName=DNS:${edgeName}\n' > ext
        openssl x509 -req -in tls.csr -CA $out/ca.pem -CAkey $out/ca.key \
          -CAcreateserial -days ${validDays} -extfile ext -out $out/tls.pem
      '';

  # Public DNS as ENS's gateway sees it, in stand-in: a DNS-over-HTTPS JSON
  # endpoint on the edge that answers for the zone only once the parent has
  # delegated it (/run/public-dns/delegated), and sets the AD bit only once
  # the parent has published its DS (/run/public-dns/anchor.conf, which
  # delv validates against, as a resolver would from the root).
  publicDns = pkgs.writers.writePython3 "losos-public-dns" { } ''
    import json
    import os
    import subprocess
    from http.server import BaseHTTPRequestHandler, HTTPServer
    from urllib.parse import parse_qs, urlparse

    ZONE = "${zone}"
    STATE = "/run/public-dns"
    TYPES = {"A": 1, "AAAA": 28, "CNAME": 5, "TXT": 16}


    def answer(name, rtype):
        inside = name == ZONE or name.endswith("." + ZONE)
        if not inside or not os.path.exists(STATE + "/delegated"):
            return {"Status": 3, "AD": False, "Answer": []}
        out = subprocess.run(
            ["dig", "+noall", "+answer", "@127.0.0.1", name, rtype],
            capture_output=True, text=True,
        ).stdout
        rows = []
        for line in out.splitlines():
            owner, ttl, _, kind, data = line.split(None, 4)
            if kind in TYPES:
                rows.append({"name": owner, "type": TYPES[kind],
                             "TTL": int(ttl), "data": data})
        anchor = STATE + "/anchor.conf"
        valid = False
        if rows and os.path.exists(anchor):
            v = subprocess.run(
                ["delv", "@127.0.0.1", "-a", anchor, "+root=" + ZONE,
                 name, rtype],
                capture_output=True, text=True,
            )
            valid = "fully validated" in v.stdout + v.stderr
        return {"Status": 0, "AD": valid, "Answer": rows}


    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            q = parse_qs(urlparse(self.path).query)
            name = q.get("name", [""])[0].rstrip(".").lower()
            body = json.dumps(answer(name, q.get("type", ["A"])[0].upper()))
            self.send_response(200)
            self.send_header("content-type", "application/dns-json")
            self.end_headers()
            self.wfile.write(body.encode())


    HTTPServer(("127.0.0.1", 8053), Handler).serve_forever()
  '';

  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  # The owner's browser session; its PNGs land in /tmp/shots on the laptop.
  shots =
    mode:
    lib.concatStringsSep " " [
      "PLAYWRIGHT_CORE=${pkgs.playwright-driver}"
      "PLAYWRIGHT_BROWSERS_PATH=${browsers}"
      "PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true"
      "${pkgs.bun}/bin/bun ${./box-wallet-name.mjs}"
      "--mode ${mode} --out /tmp/shots --user matus --password ${password} >&2"
    ];
in

pkgs.testers.nixosTest {
  name = "losos-box-wallet-name${lib.optionalString before "-before"}";

  nodes = {
    edge =
      { config, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
          (openGateway.edge registrarUrl)
        ];
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };
        systemd.tmpfiles.rules = [
          "d /var/secrets 0700 root root - -"
          "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
        ];
        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.dasmat.us";
          publicDomain = "losos.dasmat.us";
          reconcileInterval = "2s";
          tenants.mattbox = {
            hostname = "mattbox.${zone}";
            tokenFile = proxyToken;
          };
          dns = {
            enable = true;
            ipv4 = [ config.networking.primaryIPAddress ];
            checkUrl = "http://127.0.0.1:8053/dns-query";
          };
        };
        systemd.services.losos-public-dns = {
          wantedBy = [ "multi-user.target" ];
          path = [ pkgs.bind.dnsutils ];
          serviceConfig = {
            ExecStart = publicDns;
            RuntimeDirectory = "public-dns";
            RuntimeDirectoryPreserve = true;
          };
        };
        # The registrar's API over TLS, as the box reaches a real edge.
        services.nginx = {
          enable = true;
          virtualHosts.${edgeName} = {
            listen = [
              {
                addr = "0.0.0.0";
                port = 8444;
                ssl = true;
              }
            ];
            onlySSL = true;
            sslCertificate = "${fixture}/tls.pem";
            sslCertificateKey = "${fixture}/tls.key";
            locations."/".proxyPass = "http://127.0.0.1:8443";
          };
        };
        networking.firewall.allowedTCPPorts = [ 8444 ];
        virtualisation.memorySize = 1024;
      };

    box =
      { nodes, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/containers.nix
          ../../modules/tls.nix
          ../../modules/daemon.nix
          ../../modules/identity.nix
          openGateway.box
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          admin.ui = lososPkgs.losos-admin-ui;
          proxy = {
            enable = true;
            inherit registrarUrl;
          };
        };
        systemd.tmpfiles.rules = [
          "d /var/secrets 0700 root root - -"
          "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
        ];
        security.pki.certificateFiles = [ "${fixture}/ca.pem" ];
        networking.hosts.${nodes.edge.networking.primaryIPAddress} = [ edgeName ];
        systemd.services.lososd.path = lib.mkForce [
          pkgs.coreutils
          pkgs.curl
        ];
        environment.systemPackages = [ pkgs.jq ];
        virtualisation = {
          memorySize = 1536;
          cores = 2;
        };
      };

    laptop =
      { nodes, ... }:
      {
        networking.hosts.${nodes.box.networking.primaryIPAddress} = [ "mattbox.local" ];
        environment.systemPackages = [
          pkgs.bun
          pkgs.bind
          pkgs.curl
        ];
        fonts.packages = [
          pkgs.inter
          pkgs.dejavu_fonts
        ];
        time.timeZone = "Europe/Bratislava";
        virtualisation = {
          memorySize = 3072;
          cores = 2;
        };
      };
  };

  testScript =
    { nodes, ... }:
    let
      boxIp = nodes.box.networking.primaryIPAddress;
      edgeIp = nodes.edge.networking.primaryIPAddress;
    in
    openGateway.script
    + ''
      import json

      # One after another: three machines booting at once under TCG can
      # miss the driver's wait for a shell.
      for m in (edge, box, laptop):
          m.start()
          m.succeed("true")
      edge.wait_for_unit("knot.service")
      edge.wait_for_unit("losos-registrar.service")
      edge.wait_for_open_port(8444)
      box.wait_for_unit("lososd.service")
      trust(edge, box)
      box.wait_for_unit("nginx.service")
      box.wait_for_open_port(443)
      laptop.wait_for_unit("multi-user.target")

      def curl(args):
          return laptop.succeed(f"curl -sk {args}")

      with subtest("the owner claims the box and LosOS Login starts"):
          body = json.dumps({"name": "matus", "password": "${password}"})
          claim = json.loads(curl(
              f"https://${boxIp}/api/setup/claim -H 'Host: ${boxIp}' "
              f"-H 'Content-Type: application/json' -d '{body}'"
          ))
          spare = claim["token"]
          box.wait_for_unit("authelia-losos.service")
          box.wait_for_open_port(9091)
          box.wait_until_succeeds("grep -q \"domain: 'mattbox.local'\" /run/losos-identity/session.yml")
          laptop.wait_until_succeeds("curl -sfk https://mattbox.local/auth/api/health", timeout=120)

      def api(path):
          return json.loads(box.succeed(
              f"curl -s -H 'Authorization: Bearer {spare}' 127.0.0.1:8082{path}"
          ))

      with subtest("the box finds the edge official"):
          box.wait_until_succeeds(
              f"curl -s -H 'Authorization: Bearer {spare}' 127.0.0.1:8082/api/edge"
              " | jq -e '.official == true'",
              timeout=180,
          )
    ''
    + (
      if before then
        ''
          with subtest("the Network pane, before"):
              laptop.succeed("${shots "before"}")
              laptop.copy_from_machine("/tmp/shots", "")
        ''
      else
        ''
          with subtest("the edge offers the box's name for its wallet"):
              box.wait_until_succeeds(
                  f"curl -s -H 'Authorization: Bearer {spare}' 127.0.0.1:8082/api/domains"
                  " | jq -e '.available == true and .names == [\"mattbox.${zone}\"] and .wallet == null'",
                  timeout=120,
              )
              assert laptop.succeed("dig +short @${edgeIp} mattbox.${zone} TXT").strip() == ""

          with subtest("the owner binds the wallet made on this box, in the browser"):
              laptop.succeed("${shots "after"}")
              view = api("/api/domains")
              address = view["wallet"]["address"]
              # The wallet the script sealed: the test phrase's first account.
              assert address == "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266", view

          with subtest("the zone carries the ENS record, signed and validated"):
              record = f"ENS1 dnsname.ens.eth {address}"
              laptop.wait_until_succeeds(
                  f"dig +short @${edgeIp} mattbox.${zone} TXT | grep -qF '{record}'"
              )
              ds = edge.succeed("keymgr ${zone} ds | grep -E ' DS [0-9]+ 13 2 '").split()
              tag, alg, dtype, digest = ds[-4], ds[-3], ds[-2], ds[-1]
              laptop.succeed(
                  "printf 'trust-anchors { ${zone}. static-ds %s %s %s \"%s\"; };' "
                  f"{tag} {alg} {dtype} {digest} > /tmp/anchor.conf"
              )
              valid = laptop.succeed(
                  "delv @${edgeIp} -a /tmp/anchor.conf +root=${zone} mattbox.${zone} TXT 2>&1"
              )
              print(valid)
              assert "fully validated" in valid and record in valid, valid
              laptop.succeed(f"printf %s {json.dumps(valid)} > /tmp/shots/delv.txt")

          def published(state):
              box.wait_until_succeeds(
                  f"curl -s -H 'Authorization: Bearer {spare}' 127.0.0.1:8082/api/domains"
                  f" | jq -e '.wallet.published[\"mattbox.${zone}\"] == \"{state}\"'",
                  timeout=120,
              )

          with subtest("wallets find the name once the parent zone vouches for it"):
              # Public DNS does not reach a zone the parent has not delegated.
              published("missing")
              # Delegated, but with no DS at the parent the answer is unsigned.
              edge.succeed("touch /run/public-dns/delegated")
              published("unsigned")
              # The DS published: the record validates from the parent down.
              edge.succeed(
                  "printf 'trust-anchors { ${zone}. static-ds %s %s %s \"%s\"; };' "
                  f"{tag} {alg} {dtype} {digest} > /run/public-dns/anchor.conf"
              )
              published("found")
              laptop.succeed("${shots "found"}")
              # Every shot of the run, delv's answer with them.
              laptop.copy_from_machine("/tmp/shots", "")

          import shlex

          with subtest("a body that is not an object of strings unbinds nothing"):
              for body in ["null", "not json", '{"address":1,"signature":false}', "[]"]:
                  code = box.succeed(
                      f"curl -s -o /dev/null -w '%{{http_code}}' -X POST "
                      f"-H 'Authorization: Bearer {spare}' -H 'Content-Type: application/json' "
                      f"--data-binary {shlex.quote(body)} 127.0.0.1:8082/api/domains/wallet"
                  )
                  assert code == "400", (body, code)
              assert api("/api/domains")["wallet"]["address"] == address

          with subtest("unbinding takes the record away"):
              box.succeed(
                  f"curl -sf -X POST -H 'Authorization: Bearer {spare}' "
                  "-H 'Content-Type: application/json' -d '{}' 127.0.0.1:8082/api/domains/wallet"
              )
              assert api("/api/domains")["wallet"] is None
              laptop.wait_until_succeeds(
                  "test -z \"$(dig +short @${edgeIp} mattbox.${zone} TXT)\""
              )
        ''
    );
}
