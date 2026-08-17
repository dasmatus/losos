# Application services:
#   * Nextcloud — the notshared user's personal cloud (admin account =
#     notshared). Native-mode config only; the same stack runs inside the
#     nspawn container in container mode (see modules/nextcloud-common.nix and
#     modules/containers.nix). State in /var.
#   * Forgejo — native-mode git host (container mode lives in
#     modules/containers.nix).
#   * Tahoe-LAFS — the shared user's storage grid (a local introducer + a
#     `shared` storage/client node). Its web UI binds 127.0.0.1:3457; Nginx
#     fronts it publicly on :3456.
#
# Both are persisted via impermanence's /var bind-mount (see impermanence.nix),
# so nothing is lost across the tmpfs-root reboot.
{
  pkgs,
  lib,
  config,
  self,
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
      # it gets its own port-based vhost instead of a path route: loopback
      # :3457 behind the public Nginx vhost on :3456 below.
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

  # Tahoe web UI proxy: the only port-based public route. The WUI binds
  # loopback :3457; Nginx owns the public :3456.
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

  # Make the `tahoe` CLI available to the shared user (and everyone) so they
  # can drive the local node from the shell. The losos-ctl facade is
  # installed by modules/daemon.nix (the sudoers bridge is gone: the facade
  # relays to lososd over the system D-Bus).
  environment.systemPackages = [ pkgs.tahoe-lafs ];

  # Open the public Nginx ports. The Tahoe web UI is reachable on the LAN at
  # :3456 (backend is loopback-only); :80 (dashboard/api/nextcloud/forgejo)
  # is opened in modules/containers.nix. The native Forgejo port (8888) is
  # opened only in native mode.
  networking.firewall.allowedTCPPorts =
    [ 3456 ]
    ++ lib.optional (config.losos.forgejo.mode == "native" && config.losos.forgejo.enable) 8888;
}
