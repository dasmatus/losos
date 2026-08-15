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
      # The old `losos` Nextcloud plugin (nextcloud-app/) is retired — OS
      # settings moved to the standalone admin endpoint (see modules/daemon.nix
      # + admin-ui/).
    };
    appstoreEnable = true;
  };
}
