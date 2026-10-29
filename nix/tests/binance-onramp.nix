# nixos-test for buying USDC with euros at Binance, from the box's wallet.
#
# Three VMs on one network:
#   * `edge`   — an official edge (an open gateway the box vets,
#                nix/tests/open-gateway.nix) with a coins-only market
#                settling USDC on Base, and Binance's onramp (losos.edge.market.usdc.binance)
#                pointed at a stand-in for Binance Connect on loopback. The
#                partner key and token are sealed with systemd-creds the way
#                an operator seals them, and the registrar reads them through
#                LoadCredentialEncrypted=. A stand-in for the Base RPC answers
#                the wallet's balance. The registrar's API is behind TLS on
#                :8444, where Binance's notifications arrive too.
#   * `box`    — mattbox: lososd, the front vhost with the real admin UI and
#                LosOS Login, with the master proxy pointed at the edge.
#   * `laptop` — Chromium driven by Playwright (binance-onramp.mjs), signed
#                in as the owner, with a wallet made on this box.
#
# Binance has no sandbox, so the stand-in plays it: it checks every call's
# client id, token and SHA256withRSA signature against the partner's public
# key and refuses anything else, answers a pre-order with a checkout link on
# binance.com, and on cue completes the order and signs a notification with
# its own key, as Binance does.
#
# What it asserts:
#   1. the sealed key and token are ciphertext on disk, and the box's market
#      account offers the purchase only once both are sealed;
#   2. the owner opens each of Binance's four documents, accepts them, and
#      creates an order for 25 euros: the edge sends Binance one signed
#      pre-order for the box's wallet on Base, and the page shows Binance's
#      checkout link;
#   3. a notification Binance did not sign is refused and changes nothing;
#   4. Binance's signed notification completes the order, and the page says
#      Binance sent the USDC, with the transaction, and the balance shows it.
#
# The states are saved as PNGs (`shots/` in the test's output), the PR's
# screenshots. With `before = true` the box runs a given older admin UI and
# lososd, the edge runs no Binance onramp, and only the wallet is shot.
{
  pkgs,
  lososPkgs ? import ../packages.nix { inherit pkgs; },
  before ? false,
}:

let
  inherit (pkgs) lib;
  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  proxyToken = "/var/secrets/losos-proxy-token";
  agentToken = "/var/secrets/losos-mesh-agent-token";
  edgeName = "edge.losos.dasmat.us";
  registrarUrl = "https://${edgeName}:8444";
  password = "Correct-Horse-Battery-9";
  clientId = "losos-test-partner";
  accessToken = "test-sign-access-token-0123456789";
  # The wallet the browser seals: the standard test phrase's first account.
  wallet = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
  validDays = "3650";

  # The edge is an open gateway whose passport names the URL the box uses.
  openGateway = import ./open-gateway.nix { inherit pkgs lososPkgs; };

  # TLS for the edge's name, the partner's key pair (Binance
  # holds the public half), Binance's own pair for its notifications, and a
  # third key a forger signs with.
  fixture =
    pkgs.runCommand "losos-binance-onramp-fixture"
      {
        nativeBuildInputs = [ pkgs.openssl ];
      }
      ''
        mkdir -p $out
        # Python's TLS checks are strict: the CA carries its key usage, the
        # leaf names its issuer's key.
        openssl req -x509 -newkey rsa:2048 -nodes -days ${validDays} -subj "/CN=test CA" \
          -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign,cRLSign" \
          -keyout $out/ca.key -out $out/ca.pem
        openssl req -newkey rsa:2048 -nodes -subj "/CN=${edgeName}" \
          -keyout $out/tls.key -out tls.csr
        printf 'subjectAltName=DNS:${edgeName}\nauthorityKeyIdentifier=keyid,issuer\nsubjectKeyIdentifier=hash\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n' > ext
        openssl x509 -req -in tls.csr -CA $out/ca.pem -CAkey $out/ca.key \
          -CAcreateserial -days ${validDays} -extfile ext -out $out/tls.pem

        for who in partner binance forger; do
          openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out $out/$who.pem
          openssl pkey -in $out/$who.pem -pubout -out $out/$who.pub.pem
        done
        openssl rand -hex 32 > $out/releaser.hex
      '';

  # Binance Connect and the Base RPC, in stand-in, on the edge's loopback.
  standIns =
    pkgs.writers.writePython3 "losos-binance-stand-in"
      {
        libraries = [ pkgs.python3Packages.cryptography ];
        flakeIgnore = [ "E501" ];
      }
      ''
        import base64
        import json
        import ssl
        import threading
        import time
        import urllib.error
        import urllib.request
        from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

        from cryptography.exceptions import InvalidSignature
        from cryptography.hazmat.primitives import hashes, serialization
        from cryptography.hazmat.primitives.asymmetric import padding

        FIX = "${fixture}"
        CLIENT = "${clientId}"
        TOKEN = "${accessToken}"
        PRE_ORDER = "/papi/v1/ramp/connect/buy/pre-order"
        ORDER = "/papi/v1/ramp/connect/order"
        TX = "0x" + "ab" * 32


        def key(name, private):
            data = open(f"{FIX}/{name}", "rb").read()
            if private:
                return serialization.load_pem_private_key(data, None)
            return serialization.load_pem_public_key(data)


        PARTNER = key("partner.pub.pem", False)
        SIGNERS = {"binance": key("binance.pem", True), "forger": key("forger.pem", True)}
        ORDERS, CALLS = {}, []
        BALANCE = [0]


        class Handler(BaseHTTPRequestHandler):
            def reply(self, doc, code=200):
                data = json.dumps(doc).encode()
                self.send_response(code)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

            def body(self):
                return self.rfile.read(int(self.headers.get("Content-Length", 0)))


        class Binance(Handler):
            def do_GET(self):
                self.reply({"calls": CALLS, "orders": ORDERS})

            def signed(self, body):
                h = self.headers
                if h.get("X-Tesla-ClientId") != CLIENT or h.get("X-Tesla-SignAccessToken") != TOKEN:
                    return False
                ts = h.get("X-Tesla-Timestamp", "")
                if not ts.isdigit() or abs(time.time() * 1000 - int(ts)) > 60_000:
                    return False
                try:
                    PARTNER.verify(base64.b64decode(h.get("X-Tesla-Signature", "")),
                                   body + ts.encode(), padding.PKCS1v15(), hashes.SHA256())
                except (InvalidSignature, ValueError):
                    return False
                return True

            def do_POST(self):
                body = self.body()
                if self.path == "/test/complete":
                    return self.reply(complete(**json.loads(body)))
                ok = self.signed(body)
                doc = json.loads(body)
                CALLS.append({"path": self.path, "signed": ok, "body": doc})
                if not ok:
                    return self.reply({"success": False, "code": "100001", "message": "signature does not verify"})
                oid = doc.get("externalOrderId", "")
                if self.path == PRE_ORDER:
                    ORDERS[oid] = {"externalOrderId": oid, "status": 0,
                                   "fiatAmount": doc["requestedAmount"],
                                   "withdrawWalletAddress": doc["address"]}
                    return self.reply({"success": True, "data": {
                        "link": f"https://app.binance.com/en/pre-connect?orderId={oid}",
                        "linkExpireTime": int(time.time() * 1000) + 3_600_000}})
                if self.path == ORDER and oid in ORDERS:
                    return self.reply({"success": True, "data": ORDERS[oid]})
                self.reply({"success": False, "code": "404", "message": "no such order"})


        def complete(order, signer):
            """Binance takes the payment, withdraws 24.61 USDC and says so."""
            event = dict(ORDERS[order], webhookEventType="connect_order_event",
                         status=20, cryptoAmount="24.61", withdrawTxHash=TX)
            body = json.dumps(event).encode()
            ts = str(int(time.time() * 1000))
            sig = SIGNERS[signer].sign(body + ts.encode(), padding.PKCS1v15(), hashes.SHA256())
            request = urllib.request.Request(
                "https://${edgeName}:8444/market/onramp/webhook", data=body, method="POST",
                headers={"Content-Type": "application/json",
                         "X-BN-Connect-For": CLIENT, "X-BN-Connect-Timestamp": ts,
                         "X-BN-Connect-Signature": base64.b64encode(sig).decode()})
            context = ssl.create_default_context(cafile=f"{FIX}/ca.pem")
            try:
                with urllib.request.urlopen(request, context=context) as r:
                    code, answer = r.status, r.read().decode()
            except urllib.error.HTTPError as e:
                code, answer = e.code, e.read().decode()
            if signer == "binance":
                ORDERS[order].update(event)
                BALANCE[0] += 24_610_000
            return {"status": code, "answer": answer}


        class Rpc(Handler):
            """Base mainnet for the wallet: its chain, gas and USDC balance."""

            def do_POST(self):
                call = json.loads(self.body())
                method = call.get("method")
                word = "0x" + "0" * 64
                result = {
                    "eth_chainId": "0x2105",
                    "eth_blockNumber": "0x1000",
                    "eth_gasPrice": "0x3b9aca00",
                    "eth_getBalance": "0x0",
                    "eth_getCode": "0x",
                    "eth_getTransactionCount": "0x0",
                    "eth_feeHistory": {"oldestBlock": "0xff7", "baseFeePerGas": ["0x3b9aca00"] * 11,
                                       "gasUsedRatio": [0.5] * 10, "reward": [["0x0"]] * 10},
                }.get(method)
                if method == "eth_call":
                    data = call["params"][0].get("data", "")
                    result = "0x%064x" % BALANCE[0] if data.startswith("0x70a08231") else word
                if result is None:
                    return self.reply({"jsonrpc": "2.0", "id": call.get("id"),
                                       "error": {"code": -32601, "message": "not here"}})
                self.reply({"jsonrpc": "2.0", "id": call.get("id"), "result": result})


        rpc = ThreadingHTTPServer(("127.0.0.1", 8545), Rpc)
        threading.Thread(target=rpc.serve_forever, daemon=True).start()
        ThreadingHTTPServer(("127.0.0.1", 9100), Binance).serve_forever()
      '';

  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  shots =
    mode:
    lib.concatStringsSep " " [
      "PLAYWRIGHT_CORE=${pkgs.playwright-driver}"
      "PLAYWRIGHT_BROWSERS_PATH=${browsers}"
      "PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true"
      "${pkgs.bun}/bin/bun ${./binance-onramp.mjs}"
      "--mode ${mode} --out /tmp/shots --user matus --password ${password} >&2"
    ];
  credDir = "/var/secrets";
in

pkgs.testers.nixosTest {
  name = "losos-binance-onramp${lib.optionalString before "-before"}";

  nodes = {
    edge =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
          ../../modules/edge-credentials.nix
          (openGateway.edge registrarUrl)
        ];
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };
        systemd.tmpfiles.rules = [
          "d /var/secrets 0700 root root - -"
          "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
          "f ${agentToken} 0600 root root - test-agent-token"
        ];
        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.dasmat.us";
          publicDomain = "losos.dasmat.us";
          reconcileInterval = "2s";
          tenants.mattbox = {
            hostname = "mattbox.boxes.losos.dasmat.us";
            tokenFile = proxyToken;
            market = true;
          };
          cluster = {
            enable = true;
            advertiseAddr = "192.0.2.1";
            agentTokenFile = agentToken;
          };
          market = {
            enable = true;
            currency = "usd";
            cards.enable = false;
            usdc = {
              enable = true;
              networks.base = {
                pay = "0x00000000000000000000000000000000000000aa";
                treasury = "0x00000000000000000000000000000000000000bb";
              };
              binance = lib.mkIf (!before) {
                enable = true;
                apiUrl = "http://127.0.0.1:9100";
                inherit clientId;
                webhookPublicKeyFile = "${fixture}/binance.pub.pem";
              };
            };
          };
        };
        # The market needs the mesh, which this test does not exercise
        # (nix/tests/market-vm.nix says why).
        services.rke2.enable = lib.mkForce false;
        systemd.services.losos-mesh-rbac.enable = false;

        systemd.services.binance-stand-in = {
          wantedBy = [ "multi-user.target" ];
          before = [ "losos-registrar.service" ];
          serviceConfig.ExecStart = standIns;
        };
        # The chain endpoint and the releaser are sealed at boot. The
        # partner's key and token are sealed by the test script, through the
        # operator's own `losos-seal-credential`.
        systemd.services.seal-chain-fixtures = {
          wantedBy = [ "multi-user.target" ];
          before = [ "losos-registrar.service" ];
          requiredBy = [ "losos-registrar.service" ];
          after = [ "systemd-tmpfiles-setup.service" ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
          };
          script = ''
            printf '%s' 'http://127.0.0.1:8545' \
              | systemd-creds encrypt --name=usdc-rpc-base - ${credDir}/losos-usdc-rpc-base.cred
            tr -d '\n' < ${fixture}/releaser.hex \
              | systemd-creds encrypt --name=usdc-releaser-key - ${credDir}/losos-usdc-releaser-key.cred
          '';
        };
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
        # Binance's notifications reach the edge by its public name.
        networking.hosts."127.0.0.1" = [ edgeName ];
        environment.systemPackages = [
          pkgs.jq
          pkgs.curl
        ];
        virtualisation = {
          memorySize = 1536;
          cores = 2;
        };
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
    in
    openGateway.script
    + ''
      import json

      # One at a time: three VMs booting at once under emulation can miss
      # the driver's wait for a shell.
      for machine in [edge, box, laptop]:
          machine.start()
          machine.succeed("true")
      edge.wait_for_unit("losos-registrar.service")
      edge.wait_for_unit("binance-stand-in.service")
      edge.wait_for_open_port(8444)
      edge.wait_for_open_port(9100)
      box.wait_for_unit("lososd.service")
      trust(edge, box)
      box.wait_for_unit("nginx.service")
      box.wait_for_open_port(443)
      laptop.wait_for_unit("multi-user.target")

      with subtest("the owner claims the box and LosOS Login starts"):
          body = json.dumps({"name": "matus", "password": "${password}"})
          claim = json.loads(laptop.succeed(
              f"curl -sk https://${boxIp}/api/setup/claim -H 'Host: ${boxIp}' "
              f"-H 'Content-Type: application/json' -d '{body}'"
          ))
          spare = claim["token"]
          box.wait_for_unit("authelia-losos.service")
          box.wait_for_open_port(9091)
          laptop.wait_until_succeeds("curl -sfk https://mattbox.local/auth/api/health", timeout=120)

      def market(query):
          box.wait_until_succeeds(
              f"curl -s -H 'Authorization: Bearer {spare}' 127.0.0.1:8082/api/market | jq -e '{query}'",
              timeout=180,
          )

      with subtest("the box finds the edge official and its USDC market on Base"):
          market('.available == true and .account.usdc.test == false')
    ''
    + (
      if before then
        ''
          with subtest("the wallet, before"):
              laptop.succeed("${shots "before"}")
              laptop.copy_from_machine("/tmp/shots", "")
        ''
      else
        ''
          with subtest("nothing is offered until the partner's key and token are sealed"):
              market('.account.usdc.onramp == null')
              edge.succeed(
                  "losos-seal-credential binance-connect-key < ${fixture}/partner.pem",
                  "printf '%s\\n' '${accessToken}' | losos-seal-credential binance-connect-token",
              )
              edge.wait_for_unit("losos-registrar.service")
              for blob in ["binance-connect-key", "binance-connect-token"]:
                  edge.fail(f"grep -q -e PRIVATE -e ${accessToken} /var/secrets/losos-{blob}.cred")
              edge.succeed(
                  "grep -q 'BEGIN PRIVATE KEY' /run/credentials/losos-registrar.service/binance-connect-key"
              )
              market('.account.usdc.onramp == "binance"')

          with subtest("the owner reads Binance's documents and creates an order"):
              laptop.succeed("${shots "order"}")
              log = json.loads(edge.succeed("curl -s 127.0.0.1:9100/"))
              pre = [c for c in log["calls"] if c["path"].endswith("/pre-order")]
              assert len(pre) == 1 and pre[0]["signed"], log
              sent = pre[0]["body"]
              assert sent["address"] == "${wallet}", sent
              assert (sent["requestedAmount"], sent["fiatCurrency"], sent["cryptoCurrency"], sent["network"], sent["amountType"]) == ("25", "EUR", "USDC", "BASE", 1), sent
              assert all(c["signed"] for c in log["calls"]), log
              order = sent["externalOrderId"]
              edge.succeed(f"jq -e '.orders[\"{order}\"].appliance_id == \"mattbox\"' /var/lib/losos-registrar/onramp.json")

          def complete(signer):
              cue = json.dumps({"order": order, "signer": signer})
              return json.loads(edge.succeed(f"curl -s -X POST 127.0.0.1:9100/test/complete -d '{cue}'"))

          with subtest("a notification Binance did not sign changes nothing"):
              forged = complete("forger")
              assert forged["status"] == 400, forged
              edge.succeed(f"jq -e '.orders[\"{order}\"].status == 0' /var/lib/losos-registrar/onramp.json")

          with subtest("Binance's notification completes the order and the page says so"):
              signed = complete("binance")
              assert signed["status"] == 200 and json.loads(signed["answer"])["returnCode"] == "SUCCESS", signed
              edge.succeed(
                  f"jq -e '.orders[\"{order}\"] | .status == 20 and .usdc == \"24.61\"' /var/lib/losos-registrar/onramp.json"
              )
              laptop.succeed("${shots "done"}")
              laptop.copy_from_machine("/tmp/shots", "")
        ''
    );
}
