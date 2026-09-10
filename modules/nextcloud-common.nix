# The native Nextcloud stack — the services.nextcloud attrset, assembled from
# the shared truth in modules/nextcloud-stack.nix.
#
# There are two ways this appliance runs Nextcloud and exactly one description
# of what it runs. modules/nextcloud-stack.nix holds that description as plain
# data (package, apps, php, paths, database, cache); this file turns it into a
# services.nextcloud attrset for the native path, and flake/images.nix bakes
# the same data into the OCI image the local k3s cluster runs as a static pod.
# Neither mode may grow a fact the other cannot see — see the header of
# nextcloud-stack.nix for why the data lives in a third file.
#
# Path-clean: the subpath deployment keys (overwritewebroot and friends —
# Nextcloud served under /nextcloud behind the front vhost) are NOT here. In
# workload mode modules/workloads.nix renders them into the losos.config.php it
# mounts into the pod; in native mode the same stack serves at the vhost root,
# where those keys would be wrong.
{
  pkgs,
  lib,
  config,
  ...
}:
let
  inherit (lib) dirOf;

  nc = import ./nextcloud-stack.nix { inherit pkgs lib; };
in
{
  options.lososInternal.nextcloudStack = lib.mkOption {
    type = lib.types.attrs;
    internal = true;
    description = ''
      The native Nextcloud stack (services.nextcloud attrset), built from
      modules/nextcloud-stack.nix and consumed by modules/services.nix when
      losos.nextcloud.mode == "native". In "container" mode the same data goes
      into the workload image instead.
    '';
  };

  config.lososInternal.nextcloudStack = {
    enable = true;
    hostName = config.losos.nextcloud.hostName;
    https = config.losos.nextcloud.https;
    configureRedis = true; # recommended caching/locking
    inherit (nc) package datadir;

    database.createLocally = true; # auto-provision Postgres with socket auth

    config = {
      dbtype = nc.database.type; # pgsql is mandatory since nixpkgs 25.05
      dbname = nc.database.name;
      dbuser = nc.database.user;
      # A dbhost that starts with a slash is a socket directory, not a host.
      # The module would default to this value under createLocally; it is
      # spelled out so the pod — which has no module to default anything — can
      # read the same string out of nextcloud-stack.nix.
      dbhost = nc.database.socketDir;
      adminuser = nc.adminUser;
      adminpassFile = config.losos.nextcloud.adminpassFile;
    };

    # Auto-enable the curated apps on every start, and keep the App Store open
    # so the heavier ones can still be installed on demand. Setting extraApps
    # disables the App Store by default; appstoreEnable forces it back on.
    extraAppsEnable = true;
    extraApps = nc.apps;
    inherit (nc) appstoreEnable;
  };

  # ── The admin password file ───────────────────────────────────────────────
  # losos.nextcloud.adminpassFile is consumed in both modes — as a systemd
  # LoadCredential natively, and as a hostPath mount in workload mode — and
  # nothing created it. On a fresh /persist the native path failed on the
  # missing credential, and the workload path fails harder: the pod mounts it
  # with `type: File`, so a missing file is a pod that never starts rather than
  # a path the kubelet quietly invents a directory at.
  #
  # Same shape as the admin token lososd mints for itself: generate once, 0600,
  # and never touch it again (ConditionPathExists=! makes the unit a no-op on
  # every later boot). Read it out of band if you need it — there is no shell,
  # so the practical path is `occ user:resetpassword` inside the pod.
  config.systemd.tmpfiles.rules = [
    "d ${dirOf (toString config.losos.nextcloud.adminpassFile)} 0700 root root -"
  ];

  config.systemd.services.losos-nextcloud-adminpass = {
    description = "Generate the Nextcloud admin password on first boot";
    wantedBy = [ "multi-user.target" ];
    # Three consumers, two of which exist in only one mode: nextcloud-setup and
    # phpfpm-nextcloud on the native path, k3s (whose kubelet mounts the file
    # into the static pod) on the workload path. systemd silently ignores an
    # ordering dependency on a unit that does not exist — unlike Requires — so
    # one list covers both modes. Keep all three.
    before = [
      "nextcloud-setup.service"
      "phpfpm-nextcloud.service"
      "k3s.service"
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
