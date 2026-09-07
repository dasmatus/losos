# Project defaults. Previously this file set `sharingMyStorage = false;` as a
# free-floating attribute that did nothing in a module context. It now sets the
# real losos.sharingMyStorage option.
#
# Defaulting to true makes the `shared` Tahoe node provide storage, so the
# self-contained single-node grid can actually store and retrieve data. Flip
# to false if you join a remote grid and don't want to contribute storage.
#
# losos.backend.package defaults to the Rust losos-ctl/lososd built from this
# flake (./backend), so the admin endpoint (daemon + facade) works out of the
# box: modules/daemon.nix runs lososd system-wide and installs the facade for
# root. Override to null to run without a backend.
{ self, lib, ... }:

{
  # Non-tunable defaults: the losos-ctl/lososd control plane, the static admin
  # UI wired into the front vhost, the master-proxy registrar client (appliance
  # side), and the Forgejo enable flag (consulted in both modes — the container
  # path in modules/containers.nix and the native path in modules/services.nix).
  # The user-tunable losos.* options (sharing, modes, hostname, https, gpu,
  # container port, proxy) live in modules/overrides.nix so lososd can rewrite
  # them.
  #
  # mkDefault, not a bare value: these are defaults, and at normal priority
  # they collide with the documented escape hatches — setting
  # `losos.backend.package = null` to run without a control plane produced
  # "conflicting definition values" instead of a backend-less system.
  losos.forgejo.enable = lib.mkDefault true;
  losos.backend.package = lib.mkDefault self.packages.x86_64-linux.losos-ctl;
  losos.admin.ui = lib.mkDefault self.packages.x86_64-linux.losos-admin-ui;
  losos.proxy.registrar.package = lib.mkDefault self.packages.x86_64-linux.losos-registrar;
}
