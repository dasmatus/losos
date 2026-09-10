# Project defaults: the wiring that points losos.* options at this flake's own
# outputs, plus the one enable flag that is not user-tunable from the SPA.
#
# Nothing here is a user preference. The tunable values — sharing, deployment
# modes, hostname, https, gpu, ports, proxy, the mesh toggles — live in
# modules/overrides.nix, which lososd rewrites from the settings page. If a
# value can be changed from the admin UI, it does not belong in this file.
#
# mkDefault, not a bare value: these are defaults, and at normal priority they
# collide with the documented escape hatches — setting
# `losos.backend.package = null` to run without a control plane produced
# "conflicting definition values" instead of a backend-less system.
{ self, lib, ... }:

let
  pkgs' = self.packages.x86_64-linux;
in
{
  # The git host is on by default. Consulted in both deployment modes: the
  # workload path in modules/workloads.nix and the native path in
  # modules/services.nix.
  losos.forgejo.enable = lib.mkDefault true;

  # The Rust control plane (lososd + the losos-ctl facade) and the static admin
  # SPA the front vhost serves, so the admin endpoint works out of the box.
  losos.backend.package = lib.mkDefault pkgs'.losos-ctl;
  losos.admin.ui = lib.mkDefault pkgs'.losos-admin-ui;

  # The appliance side of the master proxy.
  losos.proxy.registrar.package = lib.mkDefault pkgs'.losos-registrar;

  # ── Workload images ───────────────────────────────────────────────────────
  # The OCI images the local k3s cluster runs. modules/cluster.nix lists them
  # in services.k3s.images, which symlinks each into the agent's airgap image
  # directory, so the pods run with imagePullPolicy: Never and nothing is
  # fetched from a container registry at runtime.
  #
  # They are NOT built on the appliance. A Nextcloud image is ~2.6 GiB, and
  # system.autoUpgrade would rebuild it at 03:00 on a repurposed mini-PC with a
  # tmpfs root, three hours before the unconditional 00:07 reboot. They are
  # substituted from this project's own Nix binary cache instead — a deliberate,
  # documented relaxation of the old "nothing is pulled at runtime" claim:
  # nothing is pulled from a *container* registry, but the box does trust a
  # binary cache. If the cache is unreachable at 03:00 the rebuild fails and the
  # previous generation keeps running.
  losos.workloads.pauseImage = lib.mkDefault pkgs'.losos-image-pause;
  losos.workloads.nextcloudImage = lib.mkDefault pkgs'.losos-image-nextcloud;
  losos.workloads.forgejoImage = lib.mkDefault pkgs'.losos-image-forgejo;
}
