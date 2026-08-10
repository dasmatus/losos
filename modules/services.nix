# Application services:
#   * Nextcloud — the notshared user's personal cloud (admin account =
#     notshared). Runs as the system `nextcloud` user; state in /var.
#   * Tahoe-LAFS — the shared user's storage grid (a local introducer + a
#     `shared` storage/client node). Web UI on :3456; state in /var.
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
  # ── Nextcloud (for the notshared user) ─────────────────────────────────
  services.nextcloud = {
    enable = true;
    hostName = config.losos.nextcloud.hostName;
    https = config.losos.nextcloud.https;
    configureRedis = true; # recommended caching/locking, default true in newer nixpkgs
    package = pkgs.nextcloud34;

    datadir = "/var/lib/nextcloud/data";

    database.createLocally = true; # auto-provision Postgres with socket auth

    config = {
      dbtype = "pgsql"; # mandatory since nixpkgs 25.05
      adminuser = "notshared"; # the notshared user owns this instance
      adminpassFile = config.losos.nextcloud.adminpassFile;
    };

    # ── Apps ────────────────────────────────────────────────────────────
    # Enable a curated set of self-contained apps from nixpkgs (no external
    # servers / IdPs / API keys required), auto-enable them on every start,
    # and keep the App Store open so the heavier ones (onlyoffice,
    # richdocuments/Collabora, spreed/Talk, recognize, the integration_* /
    # user_saml / user_oidc / sociallogin connectors…) can be installed on
    # demand by the admin. Setting extraApps disables the App Store by
    # default; appstoreEnable = true forces it back on.
    extraAppsEnable = true;
    extraApps =
      with pkgs.nextcloud34Packages.apps;
      {
        # curated self-contained defaults
        inherit
          deck tasks notes bookmarks calendar contacts maps polls forms tables
          collectives news mail music memories groupfolders
          files_automatedtagging files_linkeditor files_retention previewgenerator
          checksum notify_push dav_push twofactor_webauthn twofactor_admin
          guests impersonate unroundedcorners
          ;
        # our machine-config plugin (built from this flake's ./nextcloud-app)
        losos = self.packages.x86_64-linux.losos-app;
      };
    appstoreEnable = true;
  };

  # ── Backend bridge (losos-ctl, user-authored Haskell) ───────────────────
  # When the backend package is set, grant the `nextcloud` user NOPASSWD sudo
  # for exactly that binary. No shell, no broader root — only this one command,
  # run as root. The binary itself is installed system-wide via the combined
  # environment.systemPackages line below (so it lands at
  # /run/current-system/sw/bin/losos-ctl, which the PHP app calls).
  #
  # NOTE: each command MUST be an attrset with `options = [ "NOPASSWD" ]`. A
  # bare path string is coerced by the sudo module to `{ options = []; }`,
  # which would require a password — and `nextcloud` is a passwordless system
  # user invoked via `sudo -n`, so the whole bridge would silently fail.
  security.sudo = lib.mkIf (config.losos.backend.package != null) {
    enable = true; # off by default on this appliance (SSH is off); turn on only so the rule below is effective
    extraRules = [
      {
        users = [ "nextcloud" ];
        runAs = "root";
        # Allow both the system symlink path (what PHP execs) and the canonical
        # store path (sudo may resolve the symlink before matching). No args
        # restriction: `losos-ctl` validates its own subcommands.
        commands = [
          { command = "/run/current-system/sw/bin/losos-ctl"; options = [ "NOPASSWD" ]; }
          { command = lib.getExe' config.losos.backend.package "losos-ctl"; options = [ "NOPASSWD" ]; }
        ];
      }
    ];
  };

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
      web.port = 3456; # shared user's web UI / CLI gateway
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

  # Make the `tahoe` CLI available to the shared user (and everyone) so they
  # can drive the local node from the shell. Also install the losos-ctl backend
  # system-wide when set, so the Nextcloud app reaches it at
  # /run/current-system/sw/bin/losos-ctl.
  environment.systemPackages = [ pkgs.tahoe-lafs ]
    ++ lib.optional (config.losos.backend.package != null) (
      lib.getBin config.losos.backend.package
    );

  # Open the Tahoe web UI only to the local network by default; tighten or
  # widen via firewall rules as needed.
  networking.firewall.allowedTCPPorts = [ 3456 ];
}