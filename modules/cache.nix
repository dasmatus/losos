# The binary cache the appliance substitutes from.
#
# Imported by BOTH systems, and that is the whole point of the file. A
# substituter the target only learns about once it is installed does nothing for
# the install itself, which is where the time actually goes:
#
#   losos.nextcloud.mode defaults to "container", so the install closure
#   contains losos-image-nextcloud — a 2.3 GiB content closure over 244 store
#   paths — and losos-ctl install runs a bare `nixos-install` with no
#   substituter flags. With nothing serving that path, a fresh install *builds*
#   the image on a repurposed mini-PC.
#
# Until this file existed, eight files in this repo discussed "our own binary
# cache" and not one configured a substituter. `substituters`, `nixConfig`,
# `trusted-public-keys` and `nix.settings` had zero hits across every .nix file.
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
