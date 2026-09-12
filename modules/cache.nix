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
#
# WHAT FILLS IT. `.forgejo/workflows/ci.yml` pushes after each build job:
# losos-ctl, losos-registrar, losos-admin-ui, and the pause and Forgejo images.
# Those are exactly the paths cache.nixos.org cannot serve, because this flake
# builds them. losos-admin-ui earns its place there only since it became a real
# npm build; while it was a `cp -rT` of some static files, pushing it saved a
# target nothing worth measuring.
#
# CI also READS from this cache — the same URL and key are in that workflow's
# NIX_CONFIG as `extra-substituters`. So an unchanged losos-ctl is fetched
# rather than recompiled, which is ~9 minutes against a 10-minute cap. Keep the
# key here and there in step; this file is the one that matters, because it is
# the one a real box uses.
#
# The Nextcloud image is the exception, and it is the one that matters most
# here — it is the 2.3 GiB closure named above. It is NOT built on every push:
# that job would fetch roughly 3.5x what the others do inside a 10-minute cap,
# and nobody has measured whether it fits. It has its own manual job
# (`image-nextcloud`, workflow_dispatch) that builds it, pushes it and prints
# its own timing. The image only changes when modules/nextcloud-stack.nix or
# flake/images.nix does, so running it by hand after those change is enough to
# keep the cache honest.
#
# If that job is never run, this substituter simply does not have the image and
# the appliance builds it — the behaviour from before the cache existed, not a
# new failure. But it is the difference between a cache that saves the install
# and one that saves everything except the expensive part.
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
