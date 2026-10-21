# Application services:
#   * Nextcloud — the notshared user's personal cloud (admin account =
#     notshared). Native-mode config only; the same stack runs as the
#     containerised workload in container mode (see
#     modules/nextcloud-common.nix and modules/containers.nix). State in /var.
#
# LosOS Git, the box's Git server, has a module of its own (modules/git.nix).
#
# Nextcloud's state is persisted via impermanence's /var bind-mount (see impermanence.nix),
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
  signIn = import ./app-sign-in.nix { inherit lib; };

  nextcloudNative = config.losos.nextcloud.mode == "native";
  nextcloudWorkload = config.losos.nextcloud.mode == "container";
  nextcloudFederates = config.lososInternal.federation.nextcloud;
in
{
  # Nginx front door (the :80 front vhost lives in modules/containers.nix;
  # native Nextcloud uses it too). Enabled unconditionally: the admin
  # endpoint and every routed service live behind it.
  services.nginx = {
    enable = true;
    recommendedProxySettings = true;
  };

  # ── Nextcloud (for the notshared user) — native path ──────────────────
  # Only active when losos.nextcloud.mode == "native". In "container" mode
  # the same stack runs as the containerised workload and is reached through
  # the front vhost's /nextcloud route (modules/containers.nix). The lone
  # source of truth for the stack is modules/nextcloud-common.nix.
  services.nextcloud = lib.mkIf nextcloudNative config.lososInternal.nextcloudStack;

  # The module creates config/ and data/ under the datadir but not the
  # datadir itself, which LosOS puts one level below the home
  # (modules/nextcloud-stack.nix). systemd-tmpfiles then made it as root
  # under the nextcloud-owned home, refused config/ as an unsafe path
  # transition, and nextcloud-setup stopped at "config is not owned by user
  # 'nextcloud'" on a fresh box.
  systemd.tmpfiles.rules = lib.mkIf nextcloudNative [
    "d ${nc.datadir} 0750 nextcloud nextcloud - -"
  ];

  # Federation follows losos.nextcloud.federation.enable and the sharing gate
  # (lososInternal.federation), the same two halves as the pod
  # (modules/nextcloud-stack.nix, `federation`): Nextcloud's own switches,
  # set after every nextcloud-setup run, and the addresses other servers call
  # answered with 404 by the vhost while it is off. The regexes sort before
  # the module's `\.php` location (priority 500), which would otherwise hand
  # /index.php/ocm/... to php-fpm, and the exact /.well-known/ocm beats the
  # module's `^~ /.well-known` redirect to the same handler.
  #
  # The owner's box-wide settings (losos.nextcloud.site.*) ride the same
  # run: the app-config half is written after federation's, and the vhost
  # refuses the admin pages' writes to every key LosOS sets
  # (modules/nextcloud-stack.nix, `site`).
  systemd.services.nextcloud-setup.script = lib.mkIf nextcloudNative (
    lib.mkAfter (
      nc.federation.occ "${config.services.nextcloud.occ}/bin/nextcloud-occ" (
        if nextcloudFederates then "yes" else "no"
      )
      + nc.site.occ "${config.services.nextcloud.occ}/bin/nextcloud-occ" (
        pkgs.writeText "losos-nextcloud-site" (
          lib.concatLines (nc.site.appConfig config.losos.nextcloud.site)
        )
      )
      # Native Nextcloud has its own vhost that the LAN reaches directly,
      # with no nginx in front to vouch for a header, so sign-in with LosOS
      # Login stays off here (modules/app-sign-in.nix).
      + signIn.nextcloud.occ "${config.services.nextcloud.occ}/bin/nextcloud-occ" "no"
      # The apps the owner turned off on the Apps pane, off again: the module
      # has just enabled every bundled one (extraAppsEnable).
      + nc.offOcc "${config.services.nextcloud.occ}/bin/nextcloud-occ"
      # The owner's account, a standard one (modules/nextcloud-stack.nix).
      + nc.demoteOwner "${config.services.nextcloud.occ}/bin/nextcloud-occ"
    )
  );
  services.nginx.virtualHosts.${config.losos.nextcloud.hostName}.locations =
    lib.mkIf nextcloudNative
      (
        lib.mkMerge [
          (lib.genAttrs (nc.site.locations "") (_: {
            priority = 200;
            return = "403";
          }))
          (lib.mkIf (!nextcloudFederates) (
            lib.genAttrs (nc.federation.locations "") (_: {
              priority = 200;
              return = "404";
            })
            // {
              "= /.well-known/ocm".return = "404";
            }
          ))
        ]
      );

  # ── The database and cache the workload pods talk to ──────────────────────
  # These are deliberately gated on *workload* mode and nothing else.
  #
  # In native mode nothing here is needed: services.nextcloud with
  # `database.createLocally = true` provisions PostgreSQL and the nextcloud
  # role itself, `configureRedis = true` brings up
  # services.redis.servers.nextcloud. Declaring them a second time would either duplicate that work or
  # collide with it.
  #
  # In workload mode services.nextcloud is not enabled at all, so *nothing*
  # brought either of them up — and the pods do not run their own. They reach
  # the host's PostgreSQL and Redis over hostPath-mounted unix sockets, which
  # modules/workloads.nix mounts with `type: Directory`: a missing socket
  # directory is a hard pod failure, so without this block the Nextcloud pod
  # never starts. That failure
  # shows up as a crash-loop on a box with no shell, never as a rebuild error,
  # which is exactly the class of bug this appliance cannot afford.
  #
  # Peer auth over the socket is what makes this safe without a password: the
  # pod runs as uid 1002, that uid is pinned to the matching roles in
  # modules/configuration.nix, and enableTCPIP stays off so the database is
  # reachable only through the socket.
  services.postgresql = lib.mkIf nextcloudWorkload {
    enable = true;
    ensureDatabases = [ nc.database.name ];
    ensureUsers = [
      {
        name = nc.database.user;
        ensureDBOwnership = true;
      }
    ];
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
}
