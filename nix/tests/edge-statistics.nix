# nixos-test-vms pictures and checks of the edge's Statistics: what Traefik
# counted, read by the registrar from Traefik's own Prometheus metrics
# (edge/registrar/src/stats.rs) and drawn on the edge's manage page
# (admin-ui/app/src/manage/app.tsx, the card on the boxes view and its page).
#
# Three VMs on one network:
#   * `edge`      — losos.edge with the manage page and the real admin UI:
#                   Traefik, the tunnel server, the registrar;
#   * `appliance` — the tunnel and the announce loop of modules/proxy.nix,
#                   and an nginx that answers for the box;
#   * `laptop`    — a browser (nix/tests/screens).
#
# Asserts:
#   1. Traefik serves its metrics on the loopback entrypoint only;
#   2. the box's visits, over HTTP/2 and HTTP/3, show up in
#      /manage/api/statistics under the box's name, with its 404s counted
#      as client errors;
#   3. the API wants the manage key;
# then signs in on the page and photographs the card and the page, light,
# dark and phone. `before`, a path to an earlier admin-ui/app, builds that UI
# instead and photographs the boxes view alone:
#
#   nix build --impure --expr '(builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux.callPackage ./nix/tests/edge-statistics.nix {
#     before = /path/to/main/admin-ui/app; })'
{
  pkgs,
  before ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };
  after = before == null;
  # The same, for the test script.
  afterPy = if after then "True" else "False";

  ui = import ./admin-ui-without-lab.nix (
    lososPkgs.losos-admin-ui.overrideAttrs (_: lib.optionalAttrs (!after) { src = before; })
  );

  proxyTokenValue = "test-proxy-token-0123456789abcdef";
  proxyToken = "/var/secrets/losos-proxy-token";
  secretFiles = {
    systemd.tmpfiles.rules = [
      "d /var/secrets 0700 root root - -"
      "f ${proxyToken} 0600 root root - ${proxyTokenValue}"
    ];
  };
in

pkgs.testers.nixosTest {
  name = if after then "losos-edge-statistics" else "losos-edge-statistics-before";

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
          packages.x86_64-linux = {
            inherit (lososPkgs) losos-registrar;
            losos-admin-ui = ui;
          };
        };
        networking.hostName = "edge";
        losos.edge = {
          enable = true;
          acmeEmail = "test@losos.cfd";
          # The laptop opens the manage page straight from the LAN, as an
          # edge's operator does; there is no certificate in a VM.
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
          manage = {
            enable = true;
            inherit ui;
          };
        };
        environment.systemPackages = [ pkgs.jq ];
        time.timeZone = "UTC";
        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };

    appliance =
      { ... }:
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
            hostname = "mattbox.losos.cfd";
            applianceId = "mattbox";
            tokenFile = proxyToken;
            heartbeatInterval = "3s";
            registrar.package = lososPkgs.losos-registrar;
          };
        };
        services.nginx = {
          enable = true;
          virtualHosts."losos-front" = {
            default = true;
            listen = [
              {
                addr = "0.0.0.0";
                port = 80;
              }
            ];
            locations."/".extraConfig = ''
              return 200 'hello-losos\n';
            '';
            locations."/missing".extraConfig = ''
              return 404;
            '';
          };
        };
        time.timeZone = "UTC";
        virtualisation = {
          memorySize = 1024;
          cores = 2;
        };
      };

    laptop =
      { ... }:
      {
        imports = [ screens.browser ];
        environment.systemPackages = [ pkgs.curl ];
        time.timeZone = "UTC";
      };
  };

  testScript = ''
    import json
    import shlex

    start_all()
    edge.wait_for_unit("losos-registrar.service")
    edge.wait_for_unit("traefik.service")
    edge.wait_until_succeeds("curl -fsS http://localhost:8443/health")
    appliance.wait_for_unit("losos-tunnel-client.service")
    appliance.wait_for_unit("losos-registrar-announce.service")
    laptop.wait_for_unit("multi-user.target")

    key = edge.succeed("cat /var/lib/losos-registrar/manage-key").strip()
    assert len(key) == 64, key

    https = "curl -sSk --max-time 10 --resolve mattbox.losos.cfd:443:127.0.0.1 https://mattbox.losos.cfd"
    edge.wait_until_succeeds(f"{https}/ | grep -q hello-losos", timeout=300)

    with subtest("Traefik's metrics are on loopback only"):
        # grep reads it all: -q would close the pipe early and fail curl's write.
        edge.succeed("curl -fsS http://127.0.0.1:8091/metrics | grep traefik_entrypoint_requests_total >/dev/null")
        edge_v4 = appliance.succeed("getent ahostsv4 edge | awk 'NR==1{print $1}'").strip()
        edge_addr = edge_v4 or "[" + appliance.succeed("getent ahostsv6 edge | awk 'NR==1{print $1}'").strip() + "]"
        appliance.fail(f"curl -fsS --max-time 5 http://{edge_addr}:8091/metrics")

    def statistics():
        return json.loads(edge.succeed(
            f"curl -fsS -H 'Authorization: Bearer {key}' http://127.0.0.1:8443/manage/api/statistics"
        ))

    with subtest("the box's visits are counted under its name"):
        for _ in range(40):
            edge.succeed(f"{https}/ > /dev/null")
        for _ in range(6):
            edge.succeed(f"{https}/missing -o /dev/null")
        h3 = (
            "${pkgs.curl}/bin/curl -sSk --http3-only --max-time 10 "
            f"--resolve mattbox.losos.cfd:443:{edge_addr} https://mattbox.losos.cfd/"
        )
        for _ in range(12):
            appliance.succeed(f"{h3} > /dev/null")
        # The first scrape is the baseline; the minutes are what came after.
        edge.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: Bearer {key}' http://127.0.0.1:8443/manage/api/statistics"
            " | jq -e '[.routes[] | select(.host == \"mattbox.losos.cfd\") | .requests] | add >= 58'",
            timeout=180,
        )
        stats = statistics()
        print(json.dumps(stats, indent=2))
        assert stats["available"] is True, stats
        assert stats["codes"]["4xx"] >= 6, stats
        assert stats["totals"]["clientErrors"] >= 6, stats
        route = [r for r in stats["routes"] if r["host"] == "mattbox.losos.cfd"][0]
        assert route["p95Ms"] is not None, route

    with subtest("the statistics want the manage key"):
        code = edge.succeed(
            "curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8443/manage/api/statistics"
        ).strip()
        assert code == "401", code

    with subtest("the manage page shows the card and the page"):
        # More visits, so the chart's last minutes are not flat.
        for _ in range(30):
            edge.succeed(f"{https}/ > /dev/null")
        sign_in = [
            {"fill": {"label": "Manage key", "value": key}},
            {"click": {"role": "button", "name": "Sign in"}},
            {"waitFor": {"text": "Boxes"}},
        ]
        shots = [{
            "name": "boxes",
            "path": "/manage/",
            "waitFor": {"text": "Manage key"},
            "steps": sign_in + ([{"waitFor": {"text": "Statistics"}}] if ${afterPy} else []) + [{"sleep": 2000}],
            "fullPage": True,
        }]
        if ${afterPy}:
            shots.append({
                "name": "statistics",
                "path": "/manage/",
                "waitFor": {"text": "Manage key"},
                "steps": sign_in + [
                    {"click": {"text": "Statistics"}},
                    {"waitFor": {"text": "mattbox.losos.cfd"}},
                    {"sleep": 2000},
                ],
                "fullPage": True,
            })
        plans = {
            "light": {},
            "dark": {"colorScheme": "dark"},
            "phone": {"width": 390, "height": 844},
        }
        for name, extra in plans.items():
            plan = {"width": 1280, "height": 800, "shots": shots, **extra}
            laptop.succeed(f"mkdir -p /tmp/plans && echo {shlex.quote(json.dumps(plan))} > /tmp/plans/{name}.json")
            laptop.succeed(
                f"losos-capture --base http://edge:8443 --plan /tmp/plans/{name}.json --out /tmp/screens/{name}",
                timeout=1800,
            )

    laptop.copy_from_machine("/tmp/screens", "")
  '';
}
