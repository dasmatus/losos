# Application services:
#   * Nextcloud — the notshared user's personal cloud (admin account =
#     notshared). Native-mode config only; the same stack runs inside the
#     nspawn container in container mode (see modules/nextcloud-common.nix and
#     modules/containers.nix). State in /var.
#   * Forgejo — native-mode git host (container mode lives in
#     modules/containers.nix).
#   * Tahoe-LAFS — the shared user's storage grid (a local introducer + a
#     `shared` storage/client node). Nginx fronts its web UI on :3456. The WUI
#     itself listens on *:3457, not loopback — see the note at the vhost below.
#
# Both are persisted via impermanence's /var bind-mount (see impermanence.nix),
# so nothing is lost across the tmpfs-root reboot.
{
  pkgs,
  lib,
  config,
  ...
}:

{
  # Nginx front door (front vhost in modules/containers.nix, Tahoe vhost
  # below; native Nextcloud/Forgejo use it too). Enabled unconditionally:
  # the admin endpoint, Tahoe proxy and every routed service live behind it.
  services.nginx = {
    enable = true;
    recommendedProxySettings = true;
  };

  # Forgejo (native). Only active in native mode; in container mode the git
  # host runs inside containers.forgejo behind Nginx (see containers.nix).
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
  # the same stack runs inside containers.nextcloud and is reached through
  # the front vhost's /nextcloud route (modules/containers.nix). The lone
  # source of truth for the stack is modules/nextcloud-common.nix.
  services.nextcloud =
    lib.mkIf (config.losos.nextcloud.mode == "native")
      config.lososInternal.nextcloudStack;

  # ── Tahoe-LAFS (for the shared user) ───────────────────────────────────
  services.tahoe = {
    # A local introducer so the grid is self-contained. Its introducer.furl is
    # generated on first start; copy it into losos.tahoe.introducerFurl and
    # rebuild so the `shared` node can join.
    introducers.local = {
      nickname = "losos-introducer";
      # nixpkgs renamed tahoelafs -> tahoe-lafs; the module's default still
      # points at the old attr, so set it explicitly.
      package = pkgs.tahoe-lafs;
    };

    # The shared user's node: client + storage server for the local grid.
    nodes.shared = {
      nickname = "shared";
      # The Tahoe WUI generates absolute links and has no prefix support, so
      # it gets its own port-based vhost instead of a path route: :3457 behind
      # the public Nginx vhost on :3456 below. Note that this is *not* a
      # loopback bind — services.tahoe.nodes.<n>.web.port is a types.port int
      # and the module renders it as the bare Twisted endpoint `tcp:3457`,
      # which listens on 0.0.0.0. The only thing keeping the raw WUI off the
      # LAN is that 3457 is absent from networking.firewall.allowedTCPPorts.
      web.port = 3457;
      package = pkgs.tahoe-lafs;
      storage.enable = config.losos.sharingMyStorage;
      storage.reservedSpace = "1G";
      client.introducer = config.losos.tahoe.introducerFurl;
      # Single-node grid: one storage server must satisfy the placement
      # policy. Raise these when joining a real multi-server grid.
      client.shares.needed = 1;
      client.shares.happy = 1;
      client.shares.total = 1;
    };
  };

  # Tahoe web UI proxy: the only port-based public route. Nginx owns the
  # public :3456 and forwards to the WUI on :3457, which is firewalled off
  # rather than loopback-bound (see the web.port note above).
  services.nginx.virtualHosts."tahoe" = {
    listen = [
      {
        addr = "0.0.0.0";
        port = 3456;
      }
    ];
    locations."/" = {
      proxyPass = "http://127.0.0.1:3457";
      proxyWebsockets = true;
    };
  };

  # The `tahoe` CLI, system-wide. Not for the `shared` user — nobody can get a
  # shell on this box (no SSH, and neither data account has a password) — but
  # for the physical console and for `systemd-run`/recovery paths that need to
  # inspect the node. It costs nothing: services.tahoe already pulls the same
  # derivation into the closure. The losos-ctl facade is installed by
  # modules/daemon.nix (the sudoers bridge is gone: the facade relays to lososd
  # over the system D-Bus).
  environment.systemPackages = [ pkgs.tahoe-lafs ];

  # Open the public Nginx ports. The Tahoe web UI is reachable on the LAN at
  # :3456; the WUI's own :3457 is deliberately left closed, which is the only
  # thing keeping it off the LAN. :80 (dashboard/api/nextcloud/forgejo) is
  # opened in modules/containers.nix. The native Forgejo port (8888) is opened
  # only in native mode.
  networking.firewall.allowedTCPPorts =
    [ 3456 ]
    ++ lib.optional (config.losos.forgejo.mode == "native" && config.losos.forgejo.enable) 8888;
}
