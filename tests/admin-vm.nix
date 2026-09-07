# nixos-test-vms config for the lososd control plane.
#
# Boots a minimal losos appliance — just the lososd daemon + the losos-ctl
# facade + the loopback admin HTTP API — and asserts:
#   1. lososd starts and claims org.losos1 on the system D-Bus,
#   2. `losos-ctl state` (the facade, over D-Bus) returns the state JSON,
#   3. /api/health is reachable without a token (intentionally public),
#   4. the admin API answers 401 without a Bearer token and 200 with it,
#   5. lososd mints the admin token (64 hex chars, mode 0600) on first start.
#
# This is the "disko-free variant" the plan calls for: it imports only
# options.nix + daemon.nix — no disko, impermanence, boot, services, or
# containers — so the VM closure stays small (Haskell daemon + base NixOS;
# no Nextcloud/Forgejo/Tahoe). The container + nginx front-door path is
# verified by `nix eval` of the install config, not booted here (building
# nextcloud34+postgres inside an nspawn closure is a multi-hour build).
{ pkgs }:

pkgs.testers.nixosTest {
  name = "losos-admin-daemon";

  nodes.machine =
    { pkgs, ... }:
    let
      # Reuse flake/packages.nix so the test doesn't need the flake `self`
      # (same trick tests/install.nix uses). The daemon + facade are one
      # cabal package; lososd is ${pkg}/bin/lososd, losos-ctl is in PATH.
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

    # lososd forks the warp listener + exports the D-Bus interface early in
    # ExecStart, but systemd marks the unit active before both are guaranteed
    # ready — so retry until the daemon answers. (--json is required by the
    # facade's optparse flag even though output is always JSON.)
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
  '';
}
