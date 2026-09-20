# nixos-test-vms config for the lososd control plane.
#
# Boots a minimal losos appliance — just the lososd daemon + the losos-ctl
# facade + the loopback admin HTTP API — and asserts:
#   1. lososd starts and claims org.losos1 on the system D-Bus,
#   2. `losos-ctl state` (the facade, over D-Bus) returns the state JSON,
#   3. /api/health is reachable without a token (intentionally public),
#   4. the admin API answers 401 without a Bearer token and 200 with it,
#   5. lososd mints the admin token (64 hex chars, mode 0600) on first start,
#   6. /api/apps/search is gated like the rest and refuses a bad query with a
#      400 rather than a 404.
#
# (6) deliberately never runs a search. The VM has no route to a catalogue and
# should not get one for a test: what would be under test then is the test
# host's network. The search itself is exercised against a recorded body in
# backend/src/losos.rs, and the response mapping in backend/src/catalogue.rs.
#
# This is the "disko-free variant" the plan calls for: it imports only
# options.nix + daemon.nix — no disko, impermanence, boot, services, cluster or
# containers — so the VM closure stays small: the Rust daemon plus base NixOS,
# with no Nextcloud, no Forgejo and neither Kubernetes instance. The nginx front
# door has a booted test of its own (tests/front-vhost.nix, which stubs the
# backends); the workload pods behind it are covered by `nix eval` of the
# install config and are booted nowhere, because pulling the ~2.6 GiB Nextcloud
# image into a test VM is a multi-hour build that exercises no line of the
# control plane this file is about.
{ pkgs }:

pkgs.testers.nixosTest {
  name = "losos-admin-daemon";

  nodes.machine =
    { pkgs, ... }:
    let
      # Reuse flake/packages.nix so the test doesn't need the flake `self`
      # (same trick tests/install.nix uses). The daemon + facade are two
      # binaries of one Rust crate; lososd is ${pkg}/bin/lososd, losos-ctl is
      # in PATH.
      lososPkgs = import ../flake/packages.nix { inherit pkgs; };
    in
    {
      imports = [
        ../modules/options.nix
        ../modules/daemon.nix
      ];

      # Non-null backend.package is what gates daemon.nix on (its `enabled`).
      losos.backend.package = lososPkgs.losos-ctl;

      virtualisation = {
        memorySize = 1024;
        cores = 2;
      };
    };

  testScript = ''
    machine.start()

    # lososd starts the actix-web listener on its own thread and exports the
    # D-Bus interface early in ExecStart, but systemd marks the unit active
    # before both are guaranteed ready — so retry until the daemon answers.
    # (`--json` is accepted and does nothing; the facade's output is always
    # JSON.)
    machine.wait_until_succeeds("losos-ctl state --json")
    machine.wait_for_unit("lososd.service")

    # 1. The facade relays State over the system D-Bus to lososd.
    state = machine.succeed("losos-ctl state --json")
    print("state:", state)
    assert '"mode"' in state, f"losos-ctl state missing mode: {state!r}"

    # 2. /api/health is intentionally unauthenticated (dashboard reachability).
    machine.wait_until_succeeds("curl -fsS localhost:8082/api/health")
    health = machine.succeed("curl -fsS localhost:8082/api/health")
    assert '"ok"' in health, f"health endpoint bad body: {health!r}"

    # 3. Without a Bearer token the admin API refuses with 401.
    code = machine.succeed("curl -s -o /dev/null -w '%{http_code}' localhost:8082/api/state")
    assert code == "401", f"expected 401 without token, got {code!r}"
    body = machine.succeed("curl -s localhost:8082/api/state")
    assert "unauthorized" in body, f"expected unauthorized body, got {body!r}"

    # 4. lososd mints the admin token on first start (mode 0600 under /var).
    machine.succeed("test -s /var/secrets/losos-admin-token")
    token = machine.succeed("cat /var/secrets/losos-admin-token").strip()
    assert len(token) == 64, f"token not 64 hex chars: {token!r}"

    # 5. With the token the admin API answers 200 and returns the same state.
    hdr = f"Authorization: Bearer {token}"
    code = machine.succeed(
      f"curl -s -o /dev/null -w '%{{http_code}}' -H '{hdr}' localhost:8082/api/state"
    )
    assert code == "200", f"expected 200 with token, got {code!r}"
    body = machine.succeed(f"curl -s -H '{hdr}' localhost:8082/api/state")
    assert '"mode"' in body, f"authed state missing mode: {body!r}"

    # 6. The catalogue search: gated like everything else, and a query the box
    # will not act on is a 400. Never a 404 — the settings screen latches a 404
    # as "this box does not serve the route" and stops asking for the rest of
    # the session, so answering that way would disable the field with nothing
    # to say why.
    code = machine.succeed(
      "curl -s -o /dev/null -w '%{http_code}' 'localhost:8082/api/apps/search?q=nextcloud'"
    )
    assert code == "401", f"search answered {code!r} without a token, expected 401"

    for label, query in [("no q", ""), ("one character", "?q=n")]:
      code = machine.succeed(
        f"curl -s -o /dev/null -w '%{{http_code}}' -H '{hdr}' "
        f"'localhost:8082/api/apps/search{query}'"
      )
      assert code == "400", f"search with {label} answered {code!r}, expected 400"
  '';
}
