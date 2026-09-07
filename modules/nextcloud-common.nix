# The native Nextcloud stack shared by host-native mode and the nspawn
# container. Single source of truth — no drift between deployment modes.
#
# Path-clean: the subpath deployment keys (overwritewebroot etc. — Nextcloud
# served under /nextcloud behind the front vhost) are layered on by
# modules/containers.nix when the stack runs in the container; in native mode
# the same stack serves at the vhost root, where those keys would be wrong.
{
  pkgs,
  lib,
  config,
  ...
}:
let
  inherit (lib) dirOf;
in
{
  options.lososInternal.nextcloudStack = lib.mkOption {
    type = lib.types.attrs;
    internal = true;
    description = ''
      The native Nextcloud stack (services.nextcloud attrset) shared by
      host-native mode and the nspawn container.
    '';
  };

  config.lososInternal.nextcloudStack = {
    enable = true;
    hostName = config.losos.nextcloud.hostName;
    https = config.losos.nextcloud.https;
    configureRedis = true; # recommended caching/locking
    package = pkgs.nextcloud34;

    datadir = "/var/lib/nextcloud/data";

    database.createLocally = true; # auto-provision Postgres with socket auth

    config = {
      dbtype = "pgsql"; # mandatory since nixpkgs 25.05
      adminuser = "notshared"; # the notshared user owns this instance
      adminpassFile = config.losos.nextcloud.adminpassFile;
    };

    # Enable a curated set of self-contained apps from nixpkgs (no external
    # servers / IdPs / API keys required), auto-enable them on every start,
    # and keep the App Store open so the heavier ones (onlyoffice,
    # richdocuments/Collabora, spreed/Talk, recognize, the integration_* /
    # user_saml / user_oidc / sociallogin connectors…) can be installed on
    # demand by the admin. Setting extraApps disables the App Store by
    # default; appstoreEnable = true forces it back on.
    extraAppsEnable = true;
    extraApps = with pkgs.nextcloud34Packages.apps; {
      inherit
        deck
        tasks
        notes
        bookmarks
        calendar
        contacts
        maps
        polls
        forms
        tables
        collectives
        news
        mail
        music
        memories
        groupfolders
        files_automatedtagging
        files_linkeditor
        files_retention
        previewgenerator
        checksum
        notify_push
        dav_push
        twofactor_webauthn
        twofactor_admin
        guests
        impersonate
        unroundedcorners
        ;
      # The old in-tree `losos` Nextcloud plugin is retired — OS settings
      # moved to the standalone admin endpoint (see modules/daemon.nix +
      # admin-ui/).
    };
    appstoreEnable = true;
  };

  # ── The admin password file ───────────────────────────────────────────────
  # losos.nextcloud.adminpassFile is consumed in both modes — as a systemd
  # LoadCredential natively, and as an nspawn bind *source* in container mode —
  # and nothing created it. nspawn does not create bind sources, so on a fresh
  # /persist container@nextcloud could not start at all, and the native path
  # failed on the missing credential.
  #
  # Same shape as the admin token lososd mints for itself: generate once, 0600,
  # and never touch it again (ConditionPathExists=! makes the unit a no-op on
  # every later boot). Read it out of band if you need it — there is no shell,
  # so the practical path is `occ user:resetpassword` from inside the
  # container.
  config.systemd.tmpfiles.rules = [
    "d ${dirOf (toString config.losos.nextcloud.adminpassFile)} 0700 root root -"
  ];

  config.systemd.services.losos-nextcloud-adminpass = {
    description = "Generate the Nextcloud admin password on first boot";
    wantedBy = [ "multi-user.target" ];
    before = [
      "nextcloud-setup.service"
      "phpfpm-nextcloud.service"
    ];
    unitConfig.ConditionPathExists = "!${toString config.losos.nextcloud.adminpassFile}";
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      UMask = "0077";
    };
    path = [ pkgs.coreutils ];
    script = ''
      set -eu
      install -d -m 0700 "$(dirname ${toString config.losos.nextcloud.adminpassFile})"
      umask 077
      head -c 24 /dev/urandom | base64 > ${toString config.losos.nextcloud.adminpassFile}
      chmod 0600 ${toString config.losos.nextcloud.adminpassFile}
    '';
  };
}
