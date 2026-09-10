# Application services:
#   * Nextcloud — the notshared user's personal cloud (admin account =
#     notshared). Native-mode config only; the same stack runs as the
#     containerised workload in container mode (see
#     modules/nextcloud-common.nix and modules/containers.nix). State in /var.
#   * Forgejo — native-mode git host (container mode lives in
#     modules/containers.nix).
#
# Both are persisted via impermanence's /var bind-mount (see impermanence.nix),
# so nothing is lost across the tmpfs-root reboot.
#
# Tahoe-LAFS used to be the third service here: a local introducer plus a
# `shared` storage/client node, fronted by a port-based Nginx vhost on :3456
# because the Tahoe WUI writes absolute links and has no path-prefix support.
# It is gone. The shared user's storage domain is served by Longhorn on the
# mesh rke2 cluster now (docs/superpowers/specs/2026-09-09-k3s-mesh-design.md),
# and `shared`'s data directory is an fscrypt policy under /persist rather than
# a Tahoe node directory. Nothing here replaces it, and that is the point: the
# :3456 vhost was the only firewall hole this module punched, and the WUI it
# fronted bound 0.0.0.0:3457 — kept off the LAN by nothing but the absence of a
# second firewall entry. Both are gone rather than ported.
{
  lib,
  config,
  pkgs,
  ...
}:

let
  # The shared description of the Nextcloud stack. The database name, role and
  # socket paths come from here rather than being retyped, because keeping the
  # native path and the workload path from drifting is that file's entire
  # reason to exist.
  nc = import ./nextcloud-stack.nix { inherit pkgs lib; };

  nextcloudWorkload = config.losos.nextcloud.mode == "container";
  forgejoWorkload = config.losos.forgejo.mode == "container" && config.losos.forgejo.enable;
in
{
  # Nginx front door (the :80 front vhost lives in modules/containers.nix;
  # native Nextcloud/Forgejo use it too). Enabled unconditionally: the admin
  # endpoint and every routed service live behind it.
  services.nginx = {
    enable = true;
    recommendedProxySettings = true;
  };

  # Forgejo (native). Only active in native mode; in container mode the git
  # host runs as a workload behind Nginx (see containers.nix).
  services.forgejo = lib.mkIf (config.losos.forgejo.mode == "native" && config.losos.forgejo.enable) {
    enable = true;
    lfs.enable = true;
    database.type = "postgres";
    settings = {
      server.HTTP_PORT = 8888;
      # Forgejo's default is open sign-up. Native mode is LAN-only (:8888 is
      # not routed through the master-proxy tunnel), but "anyone on the LAN"
      # is still not who should be creating accounts on the appliance.
      service.DISABLE_REGISTRATION = true;
      # A declarative deployment never runs the first-run wizard, so the
      # installer page stays unlocked unless this is set — and that page
      # rewrites the database and the admin credentials.
      security.INSTALL_LOCK = true;
      # Kept on here, unlike the container path (modules/containers.nix),
      # because native mode is not published through the tunnel.
      actions.ENABLED = true;
    };
  };

  # ── Nextcloud (for the notshared user) — native path ──────────────────
  # Only active when losos.nextcloud.mode == "native". In "container" mode
  # the same stack runs as the containerised workload and is reached through
  # the front vhost's /nextcloud route (modules/containers.nix). The lone
  # source of truth for the stack is modules/nextcloud-common.nix.
  services.nextcloud = lib.mkIf (
    config.losos.nextcloud.mode == "native"
  ) config.lososInternal.nextcloudStack;

  # ── The database and cache the workload pods talk to ──────────────────────
  # These are deliberately gated on *workload* mode and nothing else.
  #
  # In native mode nothing here is needed: services.nextcloud with
  # `database.createLocally = true` provisions PostgreSQL and the nextcloud
  # role itself, `configureRedis = true` brings up
  # services.redis.servers.nextcloud, and services.forgejo creates its own
  # database. Declaring them a second time would either duplicate that work or
  # collide with it.
  #
  # In workload mode services.nextcloud is not enabled at all, so *nothing*
  # brought either of them up — and the pods do not run their own. They reach
  # the host's PostgreSQL and Redis over hostPath-mounted unix sockets, which
  # modules/workloads.nix mounts with `type: Directory`: a missing socket
  # directory is a hard pod failure, so without this block the Nextcloud pod
  # never starts and the Forgejo pod cannot open its database. That failure
  # shows up as a crash-loop on a box with no shell, never as a rebuild error,
  # which is exactly the class of bug this appliance cannot afford.
  #
  # Peer auth over the socket is what makes this safe without a password: the
  # pod runs as uid 1002/1003, those uids are pinned to the matching roles in
  # modules/configuration.nix, and enableTCPIP stays off so the database is
  # reachable only through the socket.
  services.postgresql = lib.mkIf (nextcloudWorkload || forgejoWorkload) {
    enable = true;
    ensureDatabases =
      lib.optional nextcloudWorkload nc.database.name ++ lib.optional forgejoWorkload "forgejo";
    ensureUsers =
      lib.optional nextcloudWorkload {
        name = nc.database.user;
        ensureDBOwnership = true;
      }
      ++ lib.optional forgejoWorkload {
        name = "forgejo";
        ensureDBOwnership = true;
      };
  };

  # The cache. Only Nextcloud uses it, so it follows that mode alone.
  # unixSocketPerm 0660 keeps the socket to root and the redis-nextcloud group
  # — see the fixed gid in modules/configuration.nix for why that group must be
  # nameable at eval time.
  services.redis.servers.nextcloud = lib.mkIf nextcloudWorkload {
    enable = true;
    unixSocket = nc.redisSocket;
    unixSocketPerm = 660;
  };

  # Open the public Nginx ports. :80 (dashboard/api/nextcloud/forgejo) is
  # opened in modules/containers.nix; the native Forgejo port (8888) only in
  # native mode. This list is empty in the default (container) mode — the
  # Tahoe WUI's :3456 was its one unconditional entry, so the appliance now
  # opens nothing from this module.
  networking.firewall.allowedTCPPorts = lib.optional (
    config.losos.forgejo.mode == "native" && config.losos.forgejo.enable
  ) 8888;
}
