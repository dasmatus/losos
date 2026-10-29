# Models split across boxes, paid per token, end to end: one edge and two
# lending boxes, with a local chain on the edge standing in for Base Sepolia.
#
#   * `edge`: the registrar with a USDC-only market (`cards.enable = false`)
#              and one model (stories260K), the tunnel and the head units of
#              modules/edge-inference.nix, and anvil on loopback with chain id
#              84532: Circle's Base Sepolia USDC address holds MockUSDC's code
#              (`anvil_setCode`) and LososEscrow is deployed with a treasury
#              and the releaser key the registrar signs with. Both contracts
#              are compiled at build time with solc 0.8.24, the version their
#              pragma pins. Base Sepolia is a test network; this file is the
#              only place the chain id is used this way.
#   * `box1`, `box2`: losos.inference.lend (1536 and 1024 MiB) and the
#              master proxy: their tunnels carry the `<id>:llm` service
#              and announce reports what they lend.
#   * `mattbox`: an owner's box: lososd with its market relay pointed at the
#              edge as a fourth tenant, and the front vhost serving the admin
#              UI. The relay talks only https and only to an official edge,
#              so the edge also runs nginx with a private CA mattbox trusts,
#              and is an open gateway whose passport names that URL
#              (nix/tests/open-gateway.nix).
#   * `laptop`: Chromium through Playwright on mattbox's LAN, signed in with
#              the spare key, driving the Market pane's AI models section
#              (nix/tests/inference-payg-shots.mjs).
#
# The buyer the assertions use is a third tenant with no VM: the registrar
# only checks its token. It pays from anvil's third account with cast. The
# owner on mattbox pays from the wallet kept in the page (anvil's first
# account), so the page signs and sends its own transactions through the
# relay.
#
# What it asserts, each a subtest:
#   1. both boxes are lending, have EVM payout addresses, and a quote for the
#      model (needMiB 2048) puts both in its group, shares by MiB;
#   2. the wallet tops up and opens the taps exactly as quoted, the meter
#      confirms, a chat answers, and the bill is the edge's own count: the
#      prompt as the head's /tokenize counts it, the answer as its usage
#      says, at the catalogue's prices;
#   3. stopping the meter closes the taps and settles them on chain: each
#      seller's USDC and the treasury's 4% are the billed split;
#   4. a tap opened below the quoted rate: confirm refused, nothing served;
#   5. a chat on an unconfirmed or stopped meter is refused;
#   6. a confirm that names another wallet than the one that opened the
#      taps is refused; two chats at once on one meter: one answers, one
#      gets 429;
#   7. the owner starts the model from mattbox's page, asks it something,
#      stops it and starts it again on a small balance (capture_ai_screens,
#      which also takes every screenshot, into $out/screenshots and
#      $out/film);
#   8. a box stops lending mid-session: the failed answer is not billed.
#      The meter ends only after the 15-minute grace (inference::GRACE_SECS),
#      which this file does not wait for.
#
# The box-local half (only root reaches the RPC port) is
# nix/tests/inference-lend.nix.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # solc as the contracts' pragma pins it, from the Solidity project's own
  # binaries (nixpkgs carries a newer one).
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

  model = pkgs.fetchurl {
    url = "https://huggingface.co/ggml-org/models/resolve/main/tinyllamas/stories260K.gguf";
    hash = "sha256-Jwy6G9UQn0LQM1D2BAYCRWBGTbFzwOOH2R8EJtO9JW0=";
  };
  # Ten base units per token either way, so every split below is whole.
  price = 10000000;

  # anvil's default accounts.
  deployerKey = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
  releaserKey = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
  releaser = "0x70997970c51812dc3a010c7d01b50e0d17dc79c8";
  buyerKey = "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a";
  seller1Key = "0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6";
  seller2Key = "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a";
  treasury = "0x9965507d1a55bcc2695c58ba16fb37d819b0a4dc";
  # The deployer's first contract.
  escrow = "0x5fbdb2315678afecb367f032d93f642f64180aa3";
  # Circle's USDC on Base Sepolia (edge/registrar/src/stablecoin.rs).
  usdc = "0x036cbd53842c5426634e7929541ec2318f3dcf7e";

  agentToken = "/var/secrets/losos-mesh-agent-token";
  proxyToken = "/var/secrets/losos-proxy-token";
  tokens = {
    box1 = "test-box1-token-0123456789abcdef0123456789";
    box2 = "test-box2-token-0123456789abcdef0123456789";
    buyer = "test-buyer-token-0123456789abcdef012345678";
    mattbox = "test-mattbox-token-0123456789abcdef01234567";
  };
  tokenFile = id: "/var/secrets/losos-tenant-${id}";

  # mattbox's market relay only talks https, and only to an official edge:
  # a private CA the box trusts, nginx on the edge terminating it in front of
  # the registrar, and an open gateway the box vets, its passport naming
  # that URL.
  edgeUrl = "https://edge:9443";
  edgeTls = pkgs.runCommand "losos-payg-edge-tls" { nativeBuildInputs = [ pkgs.openssl ]; } ''
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

  # The admin UI mattbox serves: the shipped package, with the Lab's GPU
  # canvas left as an empty module. No screen here opens /lab/, and the
  # canvas is the one part of the bundle that is a Bevy build for wasm.
  renderStub = pkgs.runCommand "losos-lab-render-empty" { } ''
    for b in webgpu webgl2; do
      mkdir -p $out/$b
      cat > $out/$b/losos_lab_render.d.ts <<'TS'
    export default function init(opts?: any): Promise<any>;
    export declare const Lab: any;
    export declare class LabCanvas { constructor(...args: any[]); [key: string]: any; static [key: string]: any; }
    TS
      cat > $out/$b/losos_lab_render.js <<'JS'
    export default async function init() { throw new Error("no GPU canvas in this build"); }
    export class Lab { constructor() { throw new Error("no GPU canvas in this build"); } }
    export class LabCanvas {}
    JS
      printf '\0asm\1\0\0\0' > $out/$b/losos_lab_render_bg.wasm
    done
  '';
  adminUi = lososPkgs.losos-admin-ui.overrideAttrs (_: {
    preBuild =
      lososPkgs.losos-admin-ui.labCorePkg
      + lososPkgs.losos-admin-ui.labVirtPkg
      + ''
        mkdir -p src/lab/render-pkg
        cp -r ${renderStub}/webgpu ${renderStub}/webgl2 src/lab/render-pkg/
        chmod -R u+w src/lab/render-pkg
      '';
  });

  # The owner's laptop: Chromium through Playwright, pinned to nixpkgs'
  # browsers as nix/tests/admin-ui.nix pins them.
  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  shots = pkgs.writeShellScript "shots" ''
    export PLAYWRIGHT_BROWSERS_PATH=${browsers}
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    export PLAYWRIGHT_CORE=${pkgs.playwright}
    exec ${pkgs.bun}/bin/bun ${./inference-payg-shots.mjs} "$@"
  '';
  # The wallet the owner keeps in mattbox's page: anvil's first account,
  # imported from its phrase.
  walletPhrase = "test test test test test test test test test test test junk";
  walletPassword = "salmon-river-2026";

  # Brings the chain up the way the test needs it, before the registrar.
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
    name: mib:
    { lib, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/configuration.nix
        ../../modules/proxy.nix
        ../../modules/inference-lend.nix
      ];
      networking.networkmanager.enable = lib.mkForce false;
      systemd.tmpfiles.rules = [
        "d /var/secrets 0700 root root - -"
        "f ${proxyToken} 0600 root root - ${tokens.${name}}"
      ];
      losos = {
        hostName = name;
        proxy = {
          enable = true;
          edgeTunnelEndpoint = "edge:2333";
          registrarUrl = "http://edge:8443";
          hostname = "${name}.losos.cfd";
          applianceId = name;
          tokenFile = proxyToken;
          heartbeatInterval = "3s";
          registrar.package = lososPkgs.losos-registrar;
        };
        inference.lend = {
          enable = true;
          memoryMiB = mib;
        };
      };
      virtualisation = {
        memorySize = 1024;
        cores = 2;
      };
    };
in
pkgs.testers.nixosTest {
  name = "losos-inference-payg";
  # Five machines under emulation, a model loaded twice and a browser: well
  # over the default hour.
  globalTimeout = 9000;

  nodes = {
    edge =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/edge.nix
          ../../modules/edge-inference.nix
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
          inference = {
            enable = true;
            models = [
              {
                id = "stories";
                name = "Stories 260K";
                file = model;
                needMiB = 2048;
                context = 512;
                priceIn = price;
                priceOut = price;
                licence = "MIT";
              }
            ];
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
        # The chain endpoint and the releaser key, sealed under the names the
        # registrar's LoadCredentialEncrypted= expects, as an operator would.
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

        # The registrar's https front, for mattbox (see edgeTls).
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
            locations."/" = {
              proxyPass = "http://127.0.0.1:8443";
              # A model's answer takes minutes under emulation.
              extraConfig = "proxy_read_timeout 900s;";
            };
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

    box1 = box "box1" 1536;
    box2 = box "box2" 1024;

    # The buyer's own box: lososd with its market relay pointed at the edge
    # as the `mattbox` tenant, and the front vhost serving the admin UI.
    mattbox =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/daemon.nix
          ../../modules/containers.nix
          ../../modules/proxy.nix
          openGateway.box
        ];
        networking.networkmanager.enable = lib.mkForce false;
        systemd.tmpfiles.rules = [
          "d /var/secrets 0700 root root - -"
          "f ${proxyToken} 0600 root root - ${tokens.mattbox}"
        ];
        security.pki.certificateFiles = [ "${edgeTls}/ca.pem" ];
        losos = {
          hostName = "mattbox";
          # Signed in with the spare key: LosOS Login is not on this box.
          identity.enable = false;
          backend.package = lososPkgs.losos-ctl;
          admin = {
            enable = true;
            ui = adminUi;
          };
          proxy = {
            enable = true;
            edgeTunnelEndpoint = "edge:2333";
            registrarUrl = edgeUrl;
            hostname = "mattbox.losos.cfd";
            applianceId = "mattbox";
            tokenFile = proxyToken;
            registrar.package = lososPkgs.losos-registrar;
          };
        };
        environment.systemPackages = [ pkgs.jq ];
        virtualisation = {
          memorySize = 1024;
          # lososd's HTTP server runs a worker per CPU, and a worker waiting
          # on a market relay call reads no new request; under emulation two
          # are not enough for the pane's reads at once.
          cores = 4;
        };
      };

    # The owner's laptop on the same LAN, which is where the front vhost
    # lets the admin pages be opened from.
    laptop =
      { lib, ... }:
      {
        networking.networkmanager.enable = lib.mkForce false;
        fonts.enableDefaultPackages = true;
        environment.systemPackages = [
          pkgs.curl
          (pkgs.writeShellScriptBin "shots" "exec ${shots} \"$@\"")
        ];
        virtualisation = {
          memorySize = 2048;
          cores = 2;
        };
      };
  };

  testScript = openGateway.script + ''
    import json
    import shlex
    from datetime import timedelta

    API = "http://127.0.0.1:8443"
    RPC = "--rpc-url http://127.0.0.1:8545"
    PRICE = ${toString price}
    TOKENS = ${builtins.toJSON tokens}
    USDC = "${usdc}"
    ESCROW = "${escrow}"
    TREASURY = "${treasury}"
    BUYER_KEY = "${buyerKey}"
    SELLER_KEYS = {"box1": "${seller1Key}", "box2": "${seller2Key}"}
    MIB = {"box1": 1536, "box2": 1024}
    WALLET_PHRASE = "${walletPhrase}"
    WALLET_PASSWORD = "${walletPassword}"
    WALLET_KEY = "${deployerKey}"


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
        assert status in (200, 204), f"{path} as {who}: {status} {out}"
        return out


    def cast(args, rpc=True):
        tail = RPC if rpc else ""
        return edge.succeed(f"cast {args} {tail}").strip()


    def address(key):
        return cast(f"wallet address --private-key {key}", rpc=False).lower()


    def balance(who):
        return int(cast(f"call {USDC} 'balanceOf(address)(uint256)' {who}").split()[0])


    def overview():
        return ok("/market/llm/models", "buyer")


    def meter_view(meter):
        return next(m for m in overview()["meters"] if m["id"] == meter)


    def open_taps(quote, tweak=None):
        ids = [t["key"] for t in quote["taps"]]
        sellers = [t["seller"] for t in quote["taps"]]
        rates = [int(t["rate"]) for t in quote["taps"]]
        if tweak:
            tweak(rates)
        cast(
            f"send {ESCROW} 'openTaps(bytes32[],address[],uint128[],uint16)' "
            f"'[{','.join(ids)}]' '[{','.join(sellers)}]' '[{','.join(map(str, rates))}]' "
            f"{quote['feeBps']} --private-key {BUYER_KEY}"
        )


    def group_ready():
        def ready(_):
            m = overview()["models"][0]
            return m.get("group") == "ready"
        retry(ready, timeout=timedelta(seconds=900))


    def head_socket():
        groups = json.loads(edge.succeed("cat /var/lib/losos-registrar/inference.json"))["groups"]
        (group,) = groups.keys()
        return f"/run/losos-llm-heads/{group}.sock"


    def head(path, body):
        sock = head_socket()
        return json.loads(edge.succeed(
            f"curl -sS --unix-socket {sock} -H 'content-type: application/json' "
            f"--data-binary {shlex.quote(json.dumps(body))} http://head{path}"
        ))


    SHOT_URL = "mattbox.local/settings/market"


    def shots(*args):
        """One call of the laptop's Chromium (nix/tests/inference-payg-shots.mjs)."""
        return laptop.succeed("shots " + " ".join(shlex.quote(str(a)) for a in args), timeout=2400)


    def capture_ai_screens(phase):
        """Every screenshot this file takes, in one place, for PR screenshots
        and the launch film alike. `before`: the Market pane while no box
        lends, then the wallet goes into the page. `after`: with the model
        ready on both boxes, the owner starts it from the page, asks it
        something, stops it, and starts it again on a small balance.

        PR shots (light, scale 1) land in /tmp/shots/screenshots/{before,after}/,
        film frames (dark, scale 2) in /tmp/shots/film/ with a manifest; both
        are copied to $out at the end of `after`."""
        pr = "/tmp/shots/screenshots"
        film = "/tmp/shots/film"
        if phase == "before":
            laptop.succeed("mkdir -p /var/lib/shots /tmp/shots/screenshots/before /tmp/shots/screenshots/after /tmp/shots/film")
            token = mattbox.succeed("systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -").strip()
            laptop.succeed(f"printf %s {shlex.quote(token)} > /var/lib/shots/token")
            shots("shot", "pr", f"{pr}/before/market.png", "ai-models")
            shots("import-wallet", WALLET_PHRASE, WALLET_PASSWORD)
            return

        wallet = address(WALLET_KEY)
        cast(f"send {USDC} 'mint(address,uint256)' {wallet} 20000000 --private-key {BUYER_KEY}")
        shots("shot", "pr", f"{pr}/after/market-ai.png", "ai-models")

        steps = []

        def frames(out, titles):
            for line, title in zip([l for l in out.splitlines() if l.startswith("{")], titles):
                step = json.loads(line)
                step["url"] = SHOT_URL
                step["title"] = title
                steps.append(step)

        out = shots("start", "0.10", "10", WALLET_PASSWORD, film)
        typed = "0.10"
        frames(out, ["the AI models section with Stories 260K and its prices",
                     "the start form under the model"]
               + [f"the hourly limit typed: {typed[:i + 1]}" for i in range(len(typed))]
               + ["the meter open on both boxes, with its balance"])
        mine = ok("/market/llm/models", "mattbox")
        (meter,) = [m for m in mine["meters"] if m["state"] == "open"]
        print("mattbox's meter:", meter)

        prompt = "Tell me a story about a little fish"
        out = shots("chat", prompt, "48", film)
        words = prompt.split(" ")
        frames(out, [f"the question typed: {' '.join(words[:i + 1])}" for i in range(len(words))]
               + ["the model's answer and what it cost"])
        billed = next(m for m in ok("/market/llm/models", "mattbox")["meters"] if m["id"] == meter["id"])
        print("after the page's chat:", billed)
        assert int(billed["billedE6"]) > 0 and billed["requests"] == 1, billed
        shots("shot", "pr", f"{pr}/after/market-ai-running.png", "ai-meters")

        out = shots("stop", film)
        frames(out, ["the running meter with its Stop button"])
        retry(lambda _: next(m for m in ok("/market/llm/models", "mattbox")["meters"]
                             if m["id"] == meter["id"])["state"] == "ended", timeout=timedelta(seconds=300))
        out = shots("settled", film)
        frames(out, ["the stopped meter, settled, and the edge's 4% share"])

        # A dollar an hour on what is left of the balance: under a day.
        shots("start", "1.00", "0", WALLET_PASSWORD, "-")
        low = next(m for m in ok("/market/llm/models", "mattbox")["wallets"])
        print("wallet at a dollar an hour:", low)
        assert low["low"] is True, low
        shots("shot", "pr", f"{pr}/after/market-ai-low.png", "ai-meters")
        shots("stop", "-")

        manifest = {
            "feature": "ai",
            "viewport": {"width": 1280, "height": 900},
            "dpr": 2,
            "commit": "",
            "test": "losos-inference-payg",
            "steps": steps,
        }
        laptop.succeed(f"printf %s {shlex.quote(json.dumps(manifest, indent=2))} > {film}/manifest.json && rm -f {film}/.next")
        laptop.copy_from_machine(pr, "")
        laptop.copy_from_machine(film, "")


    # One machine at a time until the edge answers: the driver gives each a
    # few minutes to reach its shell, and five booting at once under
    # emulation can take longer than that.
    edge.start()
    edge.wait_for_unit("multi-user.target")
    mattbox.start()
    laptop.start()
    edge.wait_for_unit("losos-test-chain.service")
    edge.wait_for_unit("losos-registrar.service")
    edge.wait_until_succeeds(f"curl -fsS {API}/health")
    edge.wait_for_unit("nginx.service")

    with subtest("mattbox finds the edge official over https, and the Market pane draws"):
        mattbox.wait_for_unit("lososd.service")
        trust(edge, mattbox)
        mattbox.wait_for_unit("nginx.service")
        mattbox.wait_until_succeeds("test -s /etc/credstore.encrypted/admin-token")
        def official(_):
            out = mattbox.succeed(
                "curl -sS -H \"Authorization: Bearer $(systemd-creds decrypt --name=admin-token /etc/credstore.encrypted/admin-token -)\" "
                "http://127.0.0.1:8082/api/edge"
            )
            doc = json.loads(out)
            return doc.get("official") is True
        retry(official, timeout=timedelta(seconds=600))
        laptop.wait_for_unit("multi-user.target")
        laptop.wait_until_succeeds("curl -fsS http://mattbox/ | grep -q '<div id=\"root\"'")
        capture_ai_screens("before")

    box1.start()
    box2.start()
    for b in [box1, box2]:
        b.wait_for_unit("losos-llm-rpc.service")
        b.wait_for_unit("losos-tunnel-client.service")
        b.wait_for_unit("losos-registrar-announce.service")

    payout = {name: address(key) for name, key in SELLER_KEYS.items()}
    buyer = address(BUYER_KEY)

    with subtest("both boxes lend, are paid on EVM, and the quote puts both in the group"):
        for name in ["box1", "box2"]:
            message = f"LosOS market payout v1\nchain: 84532\naddress: {payout[name]}\n"
            sig = cast(f"wallet sign --private-key {SELLER_KEYS[name]} {shlex.quote(message)}", rpc=False)
            ok("/market/payout", name, {"address": payout[name], "signature": sig})
        edge.wait_until_succeeds(
            "jq -e '.services[\"box1:llm\"] and .services[\"box2:llm\"]' /etc/losos-tunnel/server.json",
            timeout=600,
        )
        print(edge.succeed("jq '.services | with_entries(select(.key | endswith(\":llm\"))) | map_values(.bind)' /etc/losos-tunnel/server.json"))
        retry(lambda _: overview()["models"][0]["lentMib"] == 2560, timeout=timedelta(seconds=300))
        model = overview()["models"][0]
        print("model:", model)
        quote = ok("/market/taps", "buyer", {"model": "stories", "spendPerHour": "1000000"})
        print("quote:", quote)
        assert quote["chain"]["chain_id"] == 84532 and quote["network"] == "base-sepolia", quote
        assert len(quote["taps"]) == 2, quote
        by_seller = {t["seller"].lower(): t for t in quote["taps"]}
        assert set(by_seller) == {payout["box1"], payout["box2"]}, (by_seller, payout)
        # Shares by memory: 1536 and 1024 of 2560, of a dollar an hour.
        assert int(by_seller[payout["box1"]]["rate"]) == 600000, quote
        assert int(by_seller[payout["box2"]]["rate"]) == 400000, quote
        assert quote["feeBps"] == 400, quote

    with subtest("the wallet opens the taps as quoted, and a chat is billed the edge's count"):
        cast(f"send {USDC} 'mint(address,uint256)' {buyer} 10000000 --private-key {BUYER_KEY}")
        cast(f"send {USDC} 'approve(address,uint256)' {ESCROW} 10000000 --private-key {BUYER_KEY}")
        cast(f"send {ESCROW} 'topUp(uint256)' 5000000 --private-key {BUYER_KEY}")
        open_taps(quote)
        confirmed = ok("/market/taps/confirm", "buyer", {"meter": quote["meter"], "wallet": buyer})
        print("confirm:", confirmed)
        assert confirmed["state"] == "open", confirmed
        group_ready()
        messages = [{"role": "user", "content": "Once upon a time"}]
        answer = ok("/market/llm/chat", "buyer", {"meter": quote["meter"], "messages": messages, "maxTokens": 24})
        print("answer:", answer)
        assert answer["content"].strip(), answer
        assert 0 < answer["tokensOut"] <= 24, answer
        prompt = head("/apply-template", {"messages": messages})["prompt"]
        counted = len(head("/tokenize", {"content": prompt, "add_special": True})["tokens"])
        print("the head's /tokenize counts the prompt as", counted)
        assert answer["tokensIn"] == counted, (answer, counted)
        cost = (answer["tokensIn"] + answer["tokensOut"]) * PRICE
        assert int(answer["costE6"]) == cost, (answer, cost)
        m = meter_view(quote["meter"])
        print("meter:", m)
        assert int(m["billedE6"]) == cost and m["requests"] == 1, m
        main = quote
        main_cost = cost

    with subtest("stopping settles every tap on chain, split by memory, 4% to the treasury"):
        before = {who: balance(who) for who in [payout["box1"], payout["box2"], TREASURY]}
        assert all(v == 0 for v in before.values()), before
        stopped = ok("/market/taps/stop", "buyer", {"meter": main["meter"]})
        print("stop:", stopped)
        # Each tap's part is whole: ten base units a token, 60% and 40%.
        expect = {}
        fees = 0
        for name, share in [("box1", 600000), ("box2", 400000)]:
            amount = main_cost * share // 1000000 // 1000000
            fee = amount * 400 // 10000
            expect[payout[name]] = amount - fee
            fees += fee
        expect[TREASURY] = fees
        print("expected on chain:", expect)
        retry(lambda _: all(balance(w) == v for w, v in expect.items()), timeout=timedelta(seconds=300))
        for w in expect:
            print(f"USDC of {w}: {balance(w)}")
        retry(lambda _: meter_view(main["meter"])["state"] == "ended", timeout=timedelta(seconds=300))
        escrowed = int(cast(f"call {ESCROW} 'balances(address)(uint256)' {buyer}").split()[0])
        print("buyer's escrow balance:", escrowed)
        assert escrowed == 5000000 - main_cost // 1000000, escrowed
        print(edge.succeed("journalctl -u losos-registrar --no-pager | grep -E 'settling|closing' || true"))

    with subtest("a tap opened below the quoted rate: confirm refused, nothing served"):
        low = ok("/market/taps", "buyer", {"model": "stories", "spendPerHour": "1000000"})
        def cheaper(rates):
            rates[0] -= 1
        open_taps(low, cheaper)
        status, out = call("/market/taps/confirm", "buyer", {"meter": low["meter"], "wallet": buyer})
        print("confirm of the cheaper tap:", status, out)
        assert status == 409, (status, out)
        status, out = call("/market/llm/chat", "buyer", {"meter": low["meter"], "messages": messages, "maxTokens": 8})
        print("chat on it:", status, out)
        assert status == 409, (status, out)
        assert meter_view(low["meter"])["requests"] == 0

    with subtest("a chat on an unconfirmed or stopped meter is refused"):
        for meter in [low["meter"], main["meter"]]:
            status, out = call("/market/llm/chat", "buyer", {"meter": meter, "messages": messages, "maxTokens": 8})
            print(f"chat on {meter}:", status, out)
            assert status == 409, (status, out)
        assert meter_view(main["meter"])["requests"] == 1
        ok("/market/taps/stop", "buyer", {"meter": low["meter"]})

    with subtest("two chats at once on one meter: one answers, one gets 429"):
        live = ok("/market/taps", "buyer", {"model": "stories", "spendPerHour": "1000000"})
        open_taps(live)
        # Taps are keyed by the wallet that opened them: the same ids under
        # another wallet are not this meter's taps.
        status, out = call("/market/taps/confirm", "buyer", {"meter": live["meter"], "wallet": TREASURY})
        print("confirm naming another wallet:", status, out)
        assert status == 409, (status, out)
        assert ok("/market/taps/confirm", "buyer", {"meter": live["meter"], "wallet": buyer})["state"] == "open"
        group_ready()
        body = json.dumps({"appliance_id": "buyer", "token": TOKENS["buyer"], "meter": live["meter"],
                           "messages": messages, "maxTokens": 64})
        one = (f"curl -sS -o /tmp/chat-$1 -w '%{{http_code}}' -H 'content-type: application/json' "
               f"--data-binary {shlex.quote(body)} {API}/market/llm/chat > /tmp/status-$1")
        edge.succeed(f"bash -c {shlex.quote(f'go() {{ {one}; }}; go a & go b & wait')}")
        statuses = sorted(int(edge.succeed(f"cat /tmp/status-{x}")) for x in "ab")
        for x in "ab":
            print(f"chat {x}:", edge.succeed(f"cat /tmp/status-{x}"), edge.succeed(f"cat /tmp/chat-{x}"))
        assert statuses == [200, 429], statuses
        assert meter_view(live["meter"])["requests"] == 1

    with subtest("the owner runs the model from mattbox's Market pane"):
        capture_ai_screens("after")

    with subtest("a box stops lending mid-session: the failed answer is not billed"):
        before = meter_view(live["meter"])
        box2.succeed("systemctl stop losos-llm-rpc.service")
        status, out = call("/market/llm/chat", "buyer", {"meter": live["meter"], "messages": messages, "maxTokens": 16})
        print("chat with a box gone:", status, out)
        assert status != 200, (status, out)
        after = meter_view(live["meter"])
        print("meter before:", before, "after:", after)
        assert after["billedE6"] == before["billedE6"] and after["requests"] == before["requests"], (before, after)
        retry(lambda _: overview()["models"][0].get("group") == "down", timeout=timedelta(seconds=300))
        # The group breaks, and the meter ends, after the grace
        # (inference::GRACE_SECS, 15 minutes), which is not waited for here.
        assert meter_view(live["meter"])["state"] == "open"
        print("group:", overview()["models"][0])
  '';
}
