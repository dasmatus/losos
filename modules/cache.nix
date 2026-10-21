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
# losos-ctl, losos-registrar, losos-admin-ui, and the pause and Forgejo images.
# Those are exactly the paths cache.nixos.org cannot serve, because this flake
# builds them. losos-admin-ui earns its place there only since it became a real
# npm build; while it was a `cp -rT` of some static files, pushing it saved a
# target nothing worth measuring.
#
# CI also READS from this cache — setup-nix uses repository variables for the
# proxy URL and public key. The appliance carries the same two values as the
# defaults of losos.cache.* (modules/options.nix), so a stock install pulls
# from the cache without the owner configuring anything; nix/tests/invariants.nix
# pins them. Set losos.cache.substituters = [ ] to opt out.
#
# The second default substituter is the same proxy under its vercel.app name,
# which answers when the custom domain does not resolve. It shares the
# Vercel project, so it does not help once that project's usage runs out.
#
# The third is a static copy of the same paths on the GitHub Pages site (https://losos.dasmat.us/proxy, written by
# .github/scripts/pages-cache.sh and deployed with the handbook), signed with
# the same key, for when the proxy on Vercel does not answer. It carries only
# the newest main build, and its Priority of 45 puts it after both the proxy
# and cache.nixos.org.
{
  lib,
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
    # the 766 stock paths that are most of any closure here, and dropping it to
    # add ours would trade a small win for a very large loss.
    extra-substituters = cfg.substituters;
    extra-trusted-public-keys = cfg.trustedPublicKeys;

    # A cache that is unreachable must not wedge the nightly rebuild. At 03:00
    # on a box with no shell, "carry on and build it yourself" is the only
    # acceptable behaviour; the alternative is a rebuild that hangs until it is
    # killed and a generation that never switches.
    connect-timeout = 5;
    fallback = true;
  };
}
