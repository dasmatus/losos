# Project defaults. Previously this file set `sharingMyStorage = false;` as a
# free-floating attribute that did nothing in a module context. It now sets the
# real losos.sharingMyStorage option.
#
# Defaulting to true makes the `shared` Tahoe node provide storage, so the
# self-contained single-node grid can actually store and retrieve data. Flip
# to false if you join a remote grid and don't want to contribute storage.
#
# losos.backend.package defaults to the Haskell losos-ctl/lososd built from
# this flake (./backend), so the admin endpoint (daemon + facade) works out of
# the box: modules/daemon.nix runs lososd system-wide and installs the facade
# for root. Override to null to run without a backend.
{ self, ... }:

{
  # Non-tunable defaults: the losos-ctl/lososd control plane, the static admin
  # UI wired into the front vhost, the master-proxy registrar client (appliance
  # side), and the native-Forgejo enable flag (only consulted in native mode).
  # The user-tunable losos.* options (sharing, modes, hostname, https, gpu,
  # container port, proxy) live in modules/overrides.nix so lososd can rewrite
  # them.
  losos.forgejo.enable = true;
  losos.backend.package = self.packages.x86_64-linux.losos-ctl;
  losos.admin.ui = self.packages.x86_64-linux.losos-admin-ui;
  losos.proxy.registrar.package = self.packages.x86_64-linux.losos-registrar;
}
