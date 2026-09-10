# The Nextcloud stack, as plain data.
#
# Two deployment paths need the same facts about this appliance's Nextcloud —
# which server package, which apps, where the state lives, which database and
# which cache it talks to:
#
#   native mode    modules/nextcloud-common.nix turns them into the
#                  services.nextcloud attrset (lososInternal.nextcloudStack)
#                  that modules/services.nix hands to the nixpkgs module.
#   workload mode  flake/images.nix bakes them into the OCI image the local
#                  k3s cluster runs as a static pod.
#
# The second one is why this is a separate file instead of a `let` block inside
# the module. A package definition is evaluated from the flake, where there is
# no module system and therefore no option to read; a NixOS module file, in
# turn, evaluates to a module and to nothing else, so it cannot also export an
# attrset for a package to import. Anything both need has to live in a third
# file or be written down twice — and written down twice is exactly what this
# file exists to prevent. A drifted app list or a drifted datadir is invisible
# until somebody flips losos.nextcloud.mode and finds their files gone.
#
# Nothing here may read `config`: there is none where the image is built.
# Anything that varies per box — hostname, the Apache port, proxy URLs, the
# admin password path — stays in the module, or in the config that
# modules/workloads.nix renders and hostPath-mounts into the pod. This file is
# for the facts that are the same on every losos box.
{ pkgs, lib }:

let
  # Nextcloud's own layout, as the nixpkgs module lays it out, so a box can be
  # flipped between the two modes without moving a byte:
  #   home      services.nextcloud.home     — store-apps and the state root
  #   datadir   services.nextcloud.datadir  — holds config/ and data/
  # (datadir is not the *data* directory: the module puts config/ and data/
  # underneath it. Hence the nesting below, which reads odd and is correct.)
  home = "/var/lib/nextcloud";
  datadir = "${home}/data";

  # Where Nextcloud actually keeps things, spelled out once so the image's
  # entrypoint and the module cannot disagree about which directory has to
  # exist before the first request.
  configDir = "${datadir}/config";
  dataDir = "${datadir}/data";
  storeAppsDir = "${home}/store-apps";

  package = pkgs.nextcloud34;

  # A curated set of self-contained apps from nixpkgs (no external servers,
  # IdPs or API keys required), auto-enabled on every start. The App Store
  # stays open on top of this (appstoreEnable below) so the heavier ones —
  # onlyoffice, richdocuments/Collabora, spreed/Talk, recognize, the
  # integration_* / user_saml / user_oidc / sociallogin connectors — can be
  # installed on demand by the admin.
  apps = with pkgs.nextcloud34Packages.apps; {
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
    # The old in-tree `losos` Nextcloud plugin is retired — OS settings moved
    # to the standalone admin endpoint (modules/daemon.nix + admin-ui/).
  };

  # The interpreter, built explicitly because pkgs.nextcloud34 has no
  # phpPackage passthru to borrow one from (its passthru is { tests; packages; }
  # and nothing else). `enabled` is the base php84 extension set — it already
  # carries bcmath, exif, gd, intl, pdo_pgsql, pgsql and sodium, so listing
  # those again would only make this look more authoritative than it is. What
  # is added here is precisely what services.nextcloud adds on top for *this*
  # stack: bz2 and intl as its "recommended" pair (intl is already in the base),
  # apcu because caching.apcu defaults on, and redis because configureRedis is
  # on below. Keep the two in step: the native path computes its own
  # interpreter from services.nextcloud.phpPackage and would not notice this one
  # rotting.
  php = pkgs.php84.buildEnv {
    extensions =
      { all, enabled }:
      enabled
      ++ (with all; [
        apcu
        bz2
        redis
      ]);
    # Mirrors services.nextcloud's defaults: maxUploadSize (512M) drives all
    # three limits, and apc.enable_cli is what lets `occ` use APCu at all —
    # without it every occ run warns and the memcache.local setting is a lie on
    # the CLI side.
    extraConfig = ''
      memory_limit = 512M
      upload_max_filesize = 512M
      post_max_size = 512M
      output_buffering = 0
      short_open_tag = Off
      expose_php = Off
      apc.enable_cli = 1
      opcache.interned_strings_buffer = 16
      opcache.max_accelerated_files = 10000
      opcache.memory_consumption = 128
      opcache.revalidate_freq = 1
    '';
  };
in
{
  inherit
    package
    apps
    php
    home
    datadir
    configDir
    dataDir
    storeAppsDir
    ;

  # The `notshared` user owns this instance — see modules/configuration.nix for
  # the two-domain split.
  adminUser = "notshared";

  # Postgres over the peer-authenticated unix socket, which is why
  # modules/configuration.nix pins the nextcloud uid: peer auth matches the
  # connecting uid against the database role, and an unpinned uid moves under
  # us. These are the values services.nextcloud derives from
  # database.createLocally = true; they are written out so the pod, which has
  # no NixOS module to derive them, uses the same ones.
  database = {
    type = "pgsql";
    name = "nextcloud";
    user = "nextcloud";
    socketDir = "/run/postgresql";
  };

  # services.redis.servers.nextcloud's default socket. Native mode gets this
  # wired by configureRedis; the pod mounts the directory and reads the same
  # path out of the image's config snippet.
  redisSocket = "/run/redis-nextcloud/redis.sock";

  # The App Store stays reachable so the heavy apps can be installed on demand.
  # Setting extraApps alone would disable it, which is why the native path also
  # sets appstoreEnable = true.
  appstoreEnable = true;

  # The served tree: the package, plus the two app stores. Written out here
  # rather than reused because the nixpkgs module's `webroot` is a `let`
  # binding inside nextcloud.nix and not an output — but it is deliberately the
  # *same* recipe, guards and all, so that both modes instantiate one
  # derivation and the equality is checkable rather than asserted:
  #
  #   nix eval --impure --expr 'let f = builtins.getFlake (toString ./.); \
  #     lib = f.inputs.nixpkgs.lib; \
  #     pkgs = f.inputs.nixpkgs.legacyPackages.x86_64-linux; in \
  #     (import ./modules/nextcloud-stack.nix { inherit pkgs lib; }).webroot.drvPath \
  #     == (f.nixosConfigurations.install.extendModules { \
  #          modules = [ { losos.nextcloud.mode = lib.mkForce "native"; } ]; \
  #        }).config.services.nextcloud.finalPackage.drvPath'
  #
  # If that goes false after a nixpkgs bump, nextcloud.nix changed how it lays
  # the webroot out and this needs to follow — `apps_paths` in the image's
  # config points into this tree.
  #
  # `ln -sfv "${package}"/*` does not match dotfiles, so the .htaccess Nextcloud
  # ships never lands in the webroot. That is not an oversight to fix: the
  # webroot is a store path, Nextcloud cannot rewrite an .htaccess there, and
  # the Apache config in flake/images.nix carries those rules instead.
  webroot = pkgs.runCommand "${package.name}-with-apps" { preferLocalBuild = true; } ''
    mkdir $out
    ln -sfv "${package}"/* "$out"
    if [ -e "$out"/nix-apps ]; then
      echo "Didn't expect nix-apps already in $out!"
      exit 1
    fi
    ln -sfTv ${
      pkgs.linkFarm "nix-apps" (lib.mapAttrsToList (name: path: { inherit name path; }) apps)
    } "$out"/nix-apps
    if [ -e "$out"/store-apps ]; then
      echo "Didn't expect store-apps already in $out!"
      exit 1
    fi
    ln -sfTv ${storeAppsDir} "$out"/store-apps

  '';
}
