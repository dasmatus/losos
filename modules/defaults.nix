# Project defaults. Previously this file set `sharingMyStorage = false;` as a
# free-floating attribute that did nothing in a module context. It now sets the
# real losos.sharingMyStorage option.
#
# Defaulting to true makes the `shared` Tahoe node provide storage, so the
# self-contained single-node grid can actually store and retrieve data. Flip
# to false if you join a remote grid and don't want to contribute storage.
#
# losos.backend.package defaults to the Haskell losos-ctl built from this
# flake (./backend), so the Nextcloud app's toggle works out of the box:
# services.nix installs it system-wide and grants the nextcloud user sudo for
# it. Override to null to run without a backend.
{ self, ... }:

{
  # Non-tunable defaults: the losos-ctl backend (the admin app's privileged
  # bridge) and the native-Forgejo enable flag (only consulted in native mode).
  # The user-tunable losos.* options (sharing, modes, hostname, https, gpu,
  # AIO ports) live in modules/overrides.nix so the admin app can rewrite them.
  losos.forgejo.enable = true;
  losos.backend.package = self.packages.x86_64-linux.losos-ctl;
}
