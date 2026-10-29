# Two of three keys move an escrowed USDC payment, end to end: one edge and
# two boxes running lososd, with a local chain on the edge standing in for
# Base Sepolia.
#
#   * `edge`: the registrar with a USDC-only market (`cards.enable = false`)
#              and anvil on loopback with chain id 84532: Circle's Base
#              Sepolia USDC address holds MockUSDC's code and LososEscrow is
#              deployed with a treasury and the releaser key the registrar
#              signs with, both compiled with solc 0.8.24 as their pragma
#              pins. nginx serves the registrar over https with a private CA
#              the boxes trust, and the edge is an open gateway whose passport
#              names that URL (nix/tests/open-gateway.nix), so both boxes find
#              it official.
#   * `sellerbox`, `buyerbox`: lososd with the market relay pointed at the
#              edge. Each makes its `market-escrow-key` credential at first
#              boot and registers the keys derived from it with the edge
#              (`losos::cmd_escrow_pass`).
#
# The buyer's wallet is anvil's third account and pays with cast, the way the
# page's wallet would; the seller's payout address is anvil's fourth. The
# seller sells compute, which needs no mesh: the edge's registry is given the
# compute window `/cluster/join` would have written.
#
# What it asserts, each a subtest:
#   1. both boxes make their escrow key and register it; the two differ;
#   2. an order through the buyer's lososd is invoiced with both boxes'
#      keys, and the payment names them on chain;
#   3. a second payment for the paid order is a stray the edge decides to
#      give back, but while the buyer's box is off nothing moves: the edge's
#      key alone is one of three. Once the box is back it agrees and the
#      buyer has the money again;
#   4. a month on, the seller asks to be paid. While the seller's box is off
#      the deposit stays held; once it is back it agrees, and the seller and
#      the treasury hold exactly the invoice's split.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  solc = pkgs.fetchurl {
    name = "solc-0.8.24";
    url = "https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.8.24+commit.e11b9ed9";
    hash = "sha256-+wOimlF0UrnxK89FnvN9ClQ3Zbs7vJEecKh9ajfDDV8=";
  };
  contracts = pkgs.runCommand "losos-test-contracts" { } ''
    mkdir $out
    ${pkgs.glibc}/lib/ld-linux-x86-64.so.2 --library-path ${pkgs.glibc}/lib ${solc} \
      --evm-version cancun --optimize --optimize-runs 200 --bin --bin-runtime \
      -o $out ${../../edge/contracts/src}/LososEscrow.sol ${../../edge/contracts/src}/MockUSDC.sol
  '';

  # anvil's default accounts.
  deployerKey = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
  releaserKey = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
  releaser = "0x70997970c51812dc3a010c7d01b50e0d17dc79c8";
  buyerKey = "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a";
  sellerKey = "0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6";
  treasury = "0x9965507d1a55bcc2695c58ba16fb37d819b0a4dc";
  escrow = "0x5fbdb2315678afecb367f032d93f642f64180aa3";
  usdc = "0x036cbd53842c5426634e7929541ec2318f3dcf7e";

  agentToken = "/var/secrets/losos-mesh-agent-token";
  proxyToken = "/var/secrets/losos-proxy-token";
  tokens = {
    sellerbox = "test-sellerbox-token-0123456789abcdef012345";
    buyerbox = "test-buyerbox-token-0123456789abcdef0123456";
  };
  tokenFile = id: "/var/secrets/losos-tenant-${id}";

  edgeUrl = "https://edge:9443";
  edgeTls = pkgs.runCommand "losos-escrow-edge-tls" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    cd $out
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 3650 \
      -subj "/CN=LosOS edge CA" -keyout ca.key -out ca.pem
    openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes \
      -subj "/CN=edge" -keyout edge.key -out edge.csr
    printf 'subjectAltName=DNS:edge\nbasicConstraints=CA:FALSE\n' > ext
    openssl x509 -req -in edge.csr -CA ca.pem -CAkey ca.key -CAcreateserial -days 3650 \
      -extfile ext -out edge.pem
  '';
  openGateway = import ./open-gateway.nix { inherit pkgs lososPkgs; };

  chainSetup = pkgs.writeShellScript "losos-test-chain" ''
    set -eu
    rpc=http://127.0.0.1:8545
    for _ in $(seq 1 120); do
      cast chain-id --rpc-url $rpc >/dev/null 2>&1 && break
      sleep 1
    done
    cast rpc --rpc-url $rpc anvil_setCode ${usdc} "0x$(cat ${contracts}/MockUSDC.bin-runtime)"
    addr=$(cast send --rpc-url $rpc --private-key ${deployerKey} --json \
      --create "0x$(cat ${contracts}/LososEscrow.bin)" \
      "constructor(address,address,address)" ${usdc} ${treasury} ${releaser} \
      | jq -r .contractAddress)
    if [ "''${addr,,}" != "${escrow}" ]; then
      echo "the escrow landed at $addr, not ${escrow}" >&2
      exit 1
    fi
  '';

  box =
    name:
    { lib, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/daemon.nix
        ../../modules/proxy.nix
        openGateway.box
      ];
      networking.networkmanager.enable = lib.mkForce false;
      systemd.tmpfiles.rules = [
        "d /var/secrets 0700 root root - -"
        "f ${proxyToken} 0600 root root - ${tokens.${name}}"
      ];
      security.pki.certificateFiles = [ "${edgeTls}/ca.pem" ];
      losos = {
        hostName = name;
        identity.enable = false;
        backend.package = lososPkgs.losos-ctl;
        proxy = {
          enable = true;
          edgeTunnelEndpoint = "edge:2333";
          registrarUrl = edgeUrl;
          hostname = "${name}.losos.cfd";
          applianceId = name;
          tokenFile = proxyToken;
          registrar.package = lososPkgs.losos-registrar;
        };
      };
      environment.systemPackages = [ pkgs.jq ];
      virtualisation = {
        memorySize = 1024;
        cores = 2;
      };
    };
in
pkgs.testers.nixosTest {
  name = "losos-market-escrow";
  # Each box signs on a one-minute pass, and the test waits on it four times.
  globalTimeout = 5400;

  nodes = {
    edge =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
          (openGateway.edge edgeUrl)
        ];
        _module.args.self = {
          packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
        };

        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.cfd";
          registrarApiBind = "::";
          tunnelPortRange = "50000-50010";
          reconcileInterval = "2s";
          tenants = lib.genAttrs (builtins.attrNames tokens) (id: {
            hostname = "${id}.losos.cfd";
            tokenFile = tokenFile id;
            market = true;
          });
          cluster = {
            enable = true;
            advertiseAddr = "192.0.2.1";
            agentTokenFile = agentToken;
          };
          market = {
            enable = true;
            cards.enable = false;
            currency = "usd";
            usdc = {
              enable = true;
              networks.base-sepolia = {
                pay = escrow;
                inherit treasury;
              };
            };
          };
        };

        # The market requires the mesh, which this file does not exercise
        # (see nix/tests/market-vm.nix).
        services.rke2.enable = lib.mkForce false;
        systemd.services.losos-mesh-rbac.enable = false;

        systemd.tmpfiles.rules = [
          "d /var/secrets 0700 root root - -"
          "f ${agentToken} 0600 root root - test-agent-token"
        ]
        ++ lib.mapAttrsToList (id: t: "f ${tokenFile id} 0600 root root - ${t}") tokens;

        systemd.services.anvil = {
          wantedBy = [ "multi-user.target" ];
          serviceConfig.ExecStart = "${pkgs.foundry}/bin/anvil --chain-id 84532 --host 127.0.0.1 --port 8545 --silent";
        };
        systemd.services.losos-test-chain = {
          wantedBy = [ "multi-user.target" ];
          after = [ "anvil.service" ];
          requires = [ "anvil.service" ];
          before = [ "losos-registrar.service" ];
          path = [
            pkgs.foundry
            pkgs.jq
            pkgs.coreutils
          ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            ExecStart = chainSetup;
          };
        };
        systemd.services.seal-usdc-fixtures = {
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
              | systemd-creds encrypt --name=usdc-rpc-base-sepolia - /var/secrets/losos-usdc-rpc-base-sepolia.cred
            printf '%s' '${releaserKey}' \
              | systemd-creds encrypt --name=usdc-releaser-key - /var/secrets/losos-usdc-releaser-key.cred
          '';
        };
        systemd.services.losos-registrar.after = [
          "losos-test-chain.service"
          "seal-usdc-fixtures.service"
        ];

        services.nginx = {
          enable = true;
          virtualHosts.edge = {
            onlySSL = true;
            listen = [
              {
                addr = "0.0.0.0";
                port = 9443;
                ssl = true;
              }
              {
                addr = "[::]";
                port = 9443;
                ssl = true;
              }
            ];
            sslCertificate = "${edgeTls}/edge.pem";
            sslCertificateKey = "${edgeTls}/edge.key";
            locations."/".proxyPass = "http://127.0.0.1:8443";
          };
        };
        networking.firewall.allowedTCPPorts = [ 9443 ];

        environment.systemPackages = [
          pkgs.foundry
          pkgs.jq
        ];
        virtualisation = {
          memorySize = 2048;
          cores = 2;
        };
      };

    sellerbox = box "sellerbox";
    buyerbox = box "buyerbox";
  };

  testScript = openGateway.script + ''
    import json
    import shlex
    import time
    from datetime import timedelta

    API = "http://127.0.0.1:8443"
    RPC = "--rpc-url http://127.0.0.1:8545"
    TOKENS = ${builtins.toJSON tokens}
    USDC = "${usdc}"
    ESCROW = "${escrow}"
    TREASURY = "${treasury}"
    BUYER_KEY = "${buyerKey}"
    SELLER_KEY = "${sellerKey}"
    LEDGER = "/var/lib/losos-registrar/market-test.json"
    REGISTRY = "/var/lib/losos-registrar/registry.json"
    HELD, RELEASED, REFUNDED = 1, 2, 3
    # Longer than two of a box's escrow passes (losos::ESCROW_PASS_EVERY).
    QUIET = 150


    def call(path, who, body=None):
        data = dict(body or {}, appliance_id=who, token=TOKENS[who])
        out = edge.succeed(
            "curl -sS -o /tmp/resp -w '%{http_code}' -H 'content-type: application/json' "
            f"--data-binary {shlex.quote(json.dumps(data))} {API}{path}; echo; cat /tmp/resp"
        )
        status, _, text = out.partition("\n")
        try:
            parsed = json.loads(text) if text.strip() else None
        except ValueError:
            parsed = text
        return int(status), parsed


    def ok(path, who, body=None):
        status, out = call(path, who, body)
        assert status in (200, 201, 204), f"{path} as {who}: {status} {out}"
        return out


    def lososd(machine, path, body):
        """A POST to the box's own API with its spare key, as the page sends it."""
        out = machine.succeed(
            "curl -sS -o /tmp/resp -w '%{http_code}' -H 'content-type: application/json' "
            "-H \"Authorization: Bearer $(systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -)\" "
            f"--data-binary {shlex.quote(json.dumps(body))} http://127.0.0.1:8082{path}; echo; cat /tmp/resp"
        )
        status, _, text = out.partition("\n")
        return int(status), (json.loads(text) if text.strip() else None)


    def cast(args, rpc=True):
        tail = RPC if rpc else ""
        return edge.succeed(f"cast {args} {tail}").strip()


    def address(key):
        return cast(f"wallet address --private-key {key}", rpc=False).lower()


    def balance(who):
        return int(cast(f"call {USDC} 'balanceOf(address)(uint256)' {who}").split()[0])


    def deposit_state(n):
        out = cast(
            f"call {ESCROW} 'deposits(uint256)(bytes32,address,address,uint256,uint256,uint64,uint8,address,address)' {n}"
        )
        return int(out.splitlines()[6].split()[0])


    def ledger():
        return json.loads(edge.succeed(f"cat {LEDGER}"))


    def box_keys():
        return ledger().get("escrow_keys", {})


    def pay(invoice):
        (chain,) = [c for c in invoice["chains"] if c.get("network") == "base-sepolia"]
        sent = json.loads(cast(
            f"send --json {ESCROW} 'pay(bytes32,address,uint256,uint256,uint64,address,address)' "
            f"{invoice['order_id_hash']} {chain['seller']} {invoice['gross']} {invoice['fee']} "
            f"{invoice['deadline']} {chain['buyer_box']} {chain['seller_box']} --private-key {BUYER_KEY}"
        ))
        assert sent["status"] in ("0x1", 1), sent
        # The edge counts a payment once a block is on top of it.
        cast("rpc anvil_mine 1")
        return sent["transactionHash"], chain


    def boot_lososd(machine):
        machine.succeed("systemctl start lososd.service")
        machine.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
        official(machine)


    def official(machine):
        """Wait until the box's own scan finds the edge official again: it
        checks every 20 s, and a restarted registrar or lososd starts over."""
        def check(_):
            out = machine.succeed(
                "curl -sS -H \"Authorization: Bearer $(systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -)\" "
                "http://127.0.0.1:8082/api/edge"
            )
            return json.loads(out).get("official") is True
        retry(check, timeout=timedelta(seconds=600))


    def restart_registrar():
        edge.succeed("systemctl start losos-registrar.service")
        edge.wait_until_succeeds(f"curl -fsS {API}/health")
        for b in [sellerbox, buyerbox]:
            if b.succeed("systemctl is-active lososd.service || true").strip() == "active":
                official(b)


    edge.start()
    sellerbox.start()
    buyerbox.start()
    edge.wait_for_unit("losos-test-chain.service")
    edge.wait_for_unit("losos-registrar.service")
    edge.wait_until_succeeds(f"curl -fsS {API}/health")
    edge.wait_for_unit("nginx.service")

    with subtest("both boxes find the edge official, make an escrow key and register it"):
        for b in [sellerbox, buyerbox]:
            b.wait_for_unit("lososd.service")
        trust(edge, sellerbox, buyerbox)
        for b in [sellerbox, buyerbox]:
            official(b)
        for b in [sellerbox, buyerbox]:
            b.wait_until_succeeds("test -s /etc/credstore.encrypted/market-escrow-key")
        retry(lambda _: set(box_keys()) >= {"sellerbox", "buyerbox"}, timeout=timedelta(seconds=600))
        keys = box_keys()
        print("registered:", keys)
        assert keys["sellerbox"]["evm"] != keys["buyerbox"]["evm"], keys
        for k in keys.values():
            assert k["evm"].startswith("0x") and len(k["evm"]) == 42, k
            assert k["evm"] not in (TREASURY, "${releaser}"), k

    buyer = address(BUYER_KEY)
    seller = address(SELLER_KEY)

    with subtest("the seller lists compute and is paid on EVM"):
        edge.succeed("systemctl stop losos-registrar.service")
        edge.succeed(
            f"(cat {REGISTRY} 2>/dev/null || echo '{{\"tenants\":{{}}}}') "
            "| jq '.compute_windows.sellerbox = {share_compute: true, window_start: \"00:00\", window_end: \"00:00\", tz: \"UTC\"}' "
            f"> /tmp/registry && install -m 600 /tmp/registry {REGISTRY}"
        )
        restart_registrar()
        message = f"LosOS market payout v1\nchain: 84532\naddress: {seller}\n"
        sig = cast(f"wallet sign --private-key {SELLER_KEY} {shlex.quote(message)}", rpc=False)
        ok("/market/payout", "sellerbox", {"address": seller, "signature": sig})
        listing = ok("/market/listings", "sellerbox", {"kind": "compute", "unit_price": 5, "capacity": 100})["listing_id"]

    with subtest("an order from the buyer's box is invoiced with both boxes' keys, and paid to them"):
        status, out = lososd(buyerbox, "/api/market/orders", {"listing_id": listing, "quantity": 40})
        print("order:", status, out)
        assert status in (200, 201), (status, out)
        invoice = out.get("stablecoin", out)
        keys = box_keys()
        for c in invoice["chains"]:
            assert c["buyer_box"] == keys["buyerbox"]["evm"], (c, keys)
            assert c["seller_box"] == keys["sellerbox"]["evm"], (c, keys)
        order_id = invoice["order_id"]
        gross, fee = int(invoice["gross"]), int(invoice["fee"])
        cast(f"send {USDC} 'mint(address,uint256)' {buyer} {3 * gross} --private-key {BUYER_KEY}")
        cast(f"send {USDC} 'approve(address,uint256)' {ESCROW} {3 * gross} --private-key {BUYER_KEY}")
        tx, chain = pay(invoice)
        status, out = lososd(buyerbox, "/api/market/confirm", {"order_id": order_id, "tx": tx, "network": "base-sepolia"})
        print("confirm:", status, out)
        assert status in (200, 204), (status, out)
        assert deposit_state(1) == HELD
        on_chain = cast(
            f"call {ESCROW} 'deposits(uint256)(bytes32,address,address,uint256,uint256,uint64,uint8,address,address)' 1"
        ).lower().splitlines()
        assert on_chain[7] == chain["buyer_box"] and on_chain[8] == chain["seller_box"], on_chain
        assert balance(buyer) == 2 * gross

    with subtest("a second payment goes back only once the buyer's box agrees"):
        buyerbox.succeed("systemctl stop lososd.service")
        tx2, _ = pay(invoice)
        status, out = call("/market/confirm", "buyerbox", {"order_id": order_id, "tx": tx2, "network": "base-sepolia"})
        print("second confirm:", status, out)
        assert status == 409 and "already paid" in json.dumps(out), (status, out)
        time.sleep(QUIET)
        assert deposit_state(2) == HELD, "the edge's key alone gave a deposit back"
        assert balance(buyer) == gross
        boot_lososd(buyerbox)
        retry(lambda _: deposit_state(2) == REFUNDED, timeout=timedelta(seconds=600))
        assert balance(buyer) == 2 * gross
        assert deposit_state(1) == HELD, "the paid order's deposit stays held"

    with subtest("a month on, the seller is paid only once the seller's box agrees"):
        sellerbox.succeed("systemctl stop lososd.service")
        edge.succeed("systemctl stop losos-registrar.service")
        month = 30 * 24 * 3600 + 60
        shift = " | ".join(
            f"(if .{k} then .{k} - {month} else .{k} end) as $v | .{k} = $v" for k in ["created_at", "paid_at", "expires_at"]
        )
        edge.succeed(f"jq '.orders |= map_values({shift})' {LEDGER} > /tmp/ledger && install -m 600 /tmp/ledger {LEDGER}")
        restart_registrar()
        released = ok("/market/sales/release", "sellerbox")
        print("release:", released)
        # The buyer's box is not asked to agree to the seller being paid.
        asked = ok("/market/escrow/decisions", "buyerbox", {"evm": box_keys()["buyerbox"]["evm"]})
        assert asked == [], asked
        time.sleep(QUIET)
        assert deposit_state(1) == HELD, "the edge's key alone released a deposit"
        assert balance(seller) == 0 and balance(TREASURY) == 0
        boot_lososd(sellerbox)
        retry(lambda _: deposit_state(1) == RELEASED, timeout=timedelta(seconds=600))
        # The edge records the release once a block is on top of it.
        def recorded(_):
            cast("rpc anvil_mine 1")
            return ledger()["orders"][order_id]["escrow"]["step"] == "released"
        retry(recorded, timeout=timedelta(seconds=300))
        print(f"seller {balance(seller)}, treasury {balance(TREASURY)}, gross {gross}, fee {fee}")
        assert balance(seller) == gross - fee
        assert balance(TREASURY) == fee
        print(sellerbox.succeed("journalctl -u lososd --no-pager | grep -i escrow || true"))
  '';
}
