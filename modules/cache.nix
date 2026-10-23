# The binary cache the appliance substitutes from.
#
# Imported by BOTH systems, and that is the whole point of the file. A
# substituter the target only learns about once it is installed does nothing for
# the install itself, which is where the time actually goes:
#
#   the install closure carries the LosOS packages and the workload images,
#   which cache.nixos.org cannot serve, and losos-ctl install runs a bare
#   `nixos-install` with no substituter flags. With nothing serving them, a
#   fresh install *builds* them on a repurposed mini-PC.
#
# Until this file existed, eight files in this repo discussed "our own binary
# cache" and not one configured a substituter. `substituters`, `nixConfig`,
# `trusted-public-keys` and `nix.settings` had zero hits across every .nix file.
#
# WHAT FILLS IT. `.github/workflows/ci.yml` publishes signed GHCR artifacts
# through .github/actions/proxy-push after each build job:
# losos-ctl, losos-registrar, losos-admin-ui, and the pause and Nextcloud images.
# Those are exactly the paths cache.nixos.org cannot serve, because this flake
# builds them. losos-admin-ui earns its place there only since it became a real
# npm build; while it was a `cp -rT` of some static files, pushing it saved a
# target nothing worth measuring.
#
# The appliance and installer ISO use the same local GHCR-backed substituter,
# configured by losos.cache.* (modules/options.nix), so a stock install pulls
# prebuilt paths without a hosted cache proxy. CI may use its own local proxy
# process; nix/tests/invariants.nix pins the appliance defaults. Set
# losos.cache.substituters = [ ] to opt out.
#
# The local substituter fetches individual signed OCI cache artifacts directly
# from GHCR with oras. The GitHub Pages copy remains a remote fallback and is
# signed with the same key.
{
  lib,
  pkgs,
  config,
  ...
}:

let
  cfg = config.losos.cache;
  enabled = cfg.substituters != [ ];
in
{
  # A substituter with no key is worse than no substituter: nix declines the
  # signature, silently falls back to building from source, and the operator
  # sees "the install is still slow" rather than an error. Fail at eval instead.
  assertions = [
    {
      assertion = enabled -> cfg.trustedPublicKeys != [ ];
      message =
        "losos.cache.substituters is set but losos.cache.trustedPublicKeys is empty. "
        + "Nix would refuse the signatures and build from source anyway, which is "
        + "the slow install the cache exists to avoid.";
    }
  ];

  nix.settings = lib.mkIf enabled {
    # extra-* rather than replacing the defaults: cache.nixos.org still serves
    # upstream NixOS paths, and dropping it to add ours would be a regression.
    extra-substituters = cfg.substituters;
    extra-trusted-public-keys = cfg.trustedPublicKeys;
    connect-timeout = 5;
    fallback = true;
  };

  # Nix speaks its ordinary binary-cache HTTP protocol to localhost. On a
  # cache miss this service uses oras to pull the matching per-store-path OCI
  # artifact from GHCR, rewrites only the unsigned URL field in narinfo, and
  # serves the NAR locally. Nix still verifies the original narinfo signature.
  systemd.services.losos-ghcr-cache = lib.mkIf enabled {
    description = "Local GHCR-backed Nix binary cache";
    wantedBy = [ "multi-user.target" ];
    after = [ "network.target" ];
    path = [ pkgs.oras ];
    environment = {
      LOSOS_GHCR_CACHE = "ghcr.io/dasmatus/losos/nix-cache";
      LOSOS_GHCR_CACHE_DIR =
        if config.losos.installer.autorun then
          "/mnt/persist/var/cache/losos-ghcr-cache"
        else
          "/var/cache/losos-ghcr-cache";
    };
    serviceConfig = {
      ExecStart = "${pkgs.python3}/bin/python3 ${../tools/losos-ghcr-cache.py}";
      Restart = "on-failure";
      RestartSec = 3;
    };
  };
}
