# Application services:
#   * Forgejo — native-mode git host (container mode lives in
#     modules/containers.nix).
#   * the PostgreSQL the Forgejo pod talks to.
# LosOS Cloud, the owner's files, calendars and contacts, is modules/cloud.nix.
#
# State is persisted via impermanence's /var bind-mount (see impermanence.nix),
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
  # The LosOS look for Forgejo (admin-ui/themes/), shared with the workload
  # image in nix/images.nix.
  themes = import ../admin-ui/themes { inherit pkgs; };
  forgejoNative = config.losos.forgejo.mode == "native" && config.losos.forgejo.enable;
  forgejoManagedDirs = [
    "public/assets/css"
    "public/assets/img"
    "templates"
  ];

  forgejoWorkload = config.losos.forgejo.mode == "container" && config.losos.forgejo.enable;
in
{
  # Nginx front door (the :80 front vhost lives in modules/containers.nix;
  # native Forgejo uses it too). Enabled unconditionally: the admin
  # endpoint and every routed service live behind it.
  services.nginx = {
    enable = true;
    recommendedProxySettings = true;
  };

  # Forgejo (native). Only active in native mode; in container mode the git
  # host runs as a workload behind Nginx (see containers.nix).
  services.forgejo = lib.mkIf forgejoNative {
    enable = true;
    lfs.enable = true;
    database.type = "postgres";
    # LosOS Git's name, theme, <meta> tags and footer (admin-ui/themes/),
    # under this box's own settings.
    # The owner's box-wide settings (losos.forgejo.site.*,
    # lososInternal.forgejoSite) go under those, and LosOS's own on top.
    settings =
      lib.recursiveUpdate (lib.recursiveUpdate themes.forgejo.settings config.lososInternal.forgejoSite)
        {
          server.HTTP_PORT = 8888;
          # Forgejo's default is open sign-up. Native mode is LAN-only (:8888 is
          # not routed through the master-proxy tunnel), but "anyone on the LAN"
          # is still not who should be creating accounts on the appliance.
          service.DISABLE_REGISTRATION = true;
          # A declarative deployment never runs the first-run wizard, so the
          # installer page stays unlocked unless this is set — and that page
          # rewrites the database and the admin credentials.
          security.INSTALL_LOCK = true;
          # Actions is remote code execution by design and nothing on the box
          # runs jobs: the on-box runner that existed for the official-edge key
          # ceremony is gone (the ceremony runs on the operator's machine,
          # `losos-registrar provision`, behind a GitHub sign-in). Enabled with
          # no runner it would only expose the runner-registration API. Same
          # setting as the container path (modules/workloads.nix).
          actions.ENABLED = false;
          # ActivityPub, the same two lines as the container path
          # (modules/workloads.nix explains both). Native mode is LAN-only,
          # so this federates with other boxes on the LAN and nothing further,
          # and only while the box shares its disk (lososInternal.federation).
          federation = {
            ENABLED = config.lososInternal.federation.forgejo;
            SHARE_USER_STATISTICS = false;
          };
        };
  };

  # Replace each LosOS-managed tree as a unit before Forgejo starts. This keeps
  # removed or renamed assets from surviving in the persistent custom directory;
  # unrelated custom data, especially conf/ secrets, is untouched.
  # modules/config-repo.nix appends the LosOS Git bootstrap (mkAfter) to the
  # same preStart; this half is mkBefore so the order is theirs, not ours.
  systemd.services.forgejo.preStart = lib.mkIf forgejoNative (
    let
      customDir = config.services.forgejo.customDir;
      replaceTree = dir: ''
        rm -rf -- ${lib.escapeShellArg "${customDir}/${dir}"}
        ln -s -- ${lib.escapeShellArg "${themes.forgejo.customDir}/${dir}"} ${lib.escapeShellArg "${customDir}/${dir}"}
      '';
    in
    lib.mkBefore ''
      install -d ${lib.escapeShellArg "${customDir}/public/assets"}
      ${lib.concatMapStrings replaceTree forgejoManagedDirs}
    ''
  );

  # ── The database the Forgejo pod talks to ─────────────────────────────────
  # Gated on *workload* mode and nothing else: natively, services.forgejo
  # creates its own database, and declaring it a second time would collide.
  #
  # In workload mode nothing else brings PostgreSQL up, and the pod does not
  # run its own. It reaches the host's over a hostPath-mounted unix socket,
  # which modules/workloads.nix mounts with `type: Directory`: a missing socket
  # directory is a hard pod failure, which shows up as a crash-loop on a box
  # with no shell, never as a rebuild error.
  #
  # Peer auth over the socket is what makes this safe without a password: the
  # pod runs as uid 1003, pinned to the matching role in
  # modules/configuration.nix, and enableTCPIP stays off so the database is
  # reachable only through the socket.
  services.postgresql = lib.mkIf forgejoWorkload {
    enable = true;
    ensureDatabases = [ "forgejo" ];
    ensureUsers = [
      {
        name = "forgejo";
        ensureDBOwnership = true;
      }
    ];
  };

  # Open the public Nginx ports. :80 (dashboard/api/cloud/forgejo) is
  # opened in modules/containers.nix; the native Forgejo port (8888) only in
  # native mode. This list is empty in the default (container) mode — the
  # Tahoe WUI's :3456 was its one unconditional entry, so the appliance now
  # opens nothing from this module.
  networking.firewall.allowedTCPPorts = lib.optional (
    config.losos.forgejo.mode == "native" && config.losos.forgejo.enable
  ) 8888;
}
