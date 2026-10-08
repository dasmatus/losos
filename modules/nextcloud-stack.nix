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

  themes = import ../admin-ui/themes { inherit pkgs; };

  # Stock Nextcloud plus the LosOS theme folder (admin-ui/themes/), copied
  # into the package rather than linked beside it. Nextcloud looks for
  # themes/<name>/ under its real server root — lib/base.php derives it from
  # __DIR__, which PHP resolves through symlinks — and the webroot below is a
  # tree of symlinks into *this* derivation, so a theme anywhere else would be
  # silently never found. Defining it here, once, is what puts the same theme
  # under both modes: native hands `package` to services.nextcloud, and the
  # image serves `webroot`, which is built from it.
  #
  # themes/ is excluded from Nextcloud's integrity check by design
  # (IntegrityCheck/Iterator/ExcludeFoldersByPathFilterIterator.php), so the
  # extra folder does not turn the admin overview red.
  #
  # A few core/img/ files are then overwritten with the theme's: the theming
  # app reads those by absolute path rather than through the theme folder,
  # for the favicons the web-app manifest names (admin-ui/themes/default.nix,
  # coreImages, says which and why). That *does* change files the integrity
  # check covers, so the image disables it, as services.nextcloud always has
  # for the same package: the store is read-only and Nix checks it already.
  package = pkgs.nextcloud34.overrideAttrs (old: {
    postInstall =
      (old.postInstall or "")
      + ''
        mkdir -p "$out/themes"
        cp -r --no-preserve=mode ${themes.nextcloud.dir} "$out/themes/${themes.nextcloud.name}"
      ''
      + lib.concatMapStrings (f: ''
        install -m0644 ${themes.nextcloud.dir}/core/img/${f} "$out/core/img/${f}"
      '') themes.nextcloud.coreImages;
  });

  # A curated set of self-contained apps from nixpkgs (no external servers,
  # IdPs or API keys required), auto-enabled on every start. The App Store
  # stays open on top of this (appstoreEnable below) so the heavier ones —
  # onlyoffice, richdocuments/Collabora, spreed/Talk, recognize, the
  # integration_* / user_saml / user_oidc / sociallogin connectors — can be
  # installed on demand by the admin.
  #
  # `mail` is deliberately NOT here, and "self-contained" is why. Every other
  # app in this list works the moment it is enabled; the mail client is a
  # client, so it is an empty window until it is pointed at an IMAP and SMTP
  # server the appliance does not run. Shipping it enabled would put a broken-
  # looking app in front of the owner on first login with nothing they can do
  # about it. It comes back when this box grows a mail server of its own
  # (Stalwart is the intended one), at which point it can be configured rather
  # than merely present. Until then an admin who wants it can install it from
  # the App Store, which is open.
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
      ;
    # `unroundedcorners` used to be here and is not, on purpose: it squares
    # off Nextcloud's corners by rewriting the same radius variables the LosOS
    # theme sets (admin-ui/themes/nextcloud/server.css), so the two fought
    # over every container. The theme's radii are the homepage's — 9px cards,
    # 6px controls — and that consistency is what the theme is for.
    #
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

  # config.php keys for the LosOS cloud branding, set in both modes —
  # services.nextcloud.settings natively, the image's config snippet in
  # workload mode — from this one attrset. `theme` loads the folder under the
  # package's themes/; the rest (admin-ui/themes/default.nix) take out what
  # a theme folder cannot reach.
  brandSettings = {
    theme = themes.nextcloud.name;
  }
  // themes.nextcloud.settings;

  # Federation, the facts both modes need to switch it as one thing
  # (losos.nextcloud.federation.enable, gated on losos.sharingMyStorage in
  # modules/options.nix). Two halves, because neither is enough alone:
  #
  #   locations  the addresses other servers call, as nginx regex locations
  #              under `prefix` ("/nextcloud" behind the front vhost, "" on
  #              the native vhost). The vhost answers 404 there while
  #              federation is off. OCM discovery (/ocm-provider/ and
  #              /.well-known/ocm) answers `enabled: true` whatever the
  #              settings below say, and the share endpoints of
  #              cloud_federation_api and federatedfilesharing exist on every
  #              instance: both apps are in core/shipped.json's alwaysEnabled
  #              list, so `app:disable` cannot remove them.
  #   occ        the settings that stop the box itself: no federated share
  #              out or in (files_sharing's four server2server keys; the two
  #              group ones default to no and are only ever forced to no), no
  #              calendar federation, and the trusted-servers app
  #              (`federation`, defaultEnabled, so it can go) disabled. Without
  #              these an owner could still share a folder *to* another server,
  #              which then reads it through public.php like any public link,
  #              a path the vhost cannot tell apart from a link share.
  #
  # `occ` takes the command to run occ with and a shell word that expands to
  # yes or no (anything else counts as no), because the image decides at start from a mounted file and the
  # native path at eval time. Non-fatal, like the app list in the entrypoint:
  # a Nextcloud that will not take a setting still serves the owner's files,
  # and the vhost half holds either way.
  federation = {
    locations = prefix: [
      "~* ^${prefix}/(?:index\\.php/)?(?:ocm-provider|ocs-provider|ocm|\\.well-known/ocm|apps/federation|apps/federatedfilesharing)(?:$|/)"
      "~* ^${prefix}/ocs/v[12]\\.php/(?:cloud/shares|apps/federation|apps/federatedfilesharing)(?:$|/)"
    ];
    occ = occ: yes: ''
      losos_federation=${yes}
      if [ "$losos_federation" = yes ]; then
        ${occ} app:enable federation ||
          echo "losos-nextcloud: could not enable the federation app" >&2
        losos_calendar=true
      else
        losos_federation=no
        ${occ} app:disable federation ||
          echo "losos-nextcloud: could not disable the federation app" >&2
        for key in outgoing_server2server_group_share_enabled incoming_server2server_group_share_enabled; do
          ${occ} config:app:set files_sharing "$key" --value=no >/dev/null ||
            echo "losos-nextcloud: could not set files_sharing $key to no" >&2
        done
        losos_calendar=false
      fi
      for key in outgoing_server2server_share_enabled incoming_server2server_share_enabled; do
        ${occ} config:app:set files_sharing "$key" --value="$losos_federation" >/dev/null ||
          echo "losos-nextcloud: could not set files_sharing $key to $losos_federation" >&2
      done
      ${occ} config:app:set dav enableCalendarFederation --value="$losos_calendar" --type=boolean >/dev/null ||
        echo "losos-nextcloud: could not set dav enableCalendarFederation to $losos_calendar" >&2
    '';
  };

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

  # The uid the Nextcloud process runs as, in both modes. Pinned (and matched
  # by modules/configuration.nix's users.users.nextcloud) because Postgres
  # peer auth maps the *kernel* uid of the connecting process to a role and
  # the pod shares the host's user namespace; modules/workloads.nix writes it
  # into the pod's runAsUser, and flake/images.nix bakes the one directory the
  # entrypoint needs outside its hostPath mounts (/run/nextcloud) owned by it,
  # because a pod that has dropped every capability cannot create a directory
  # under a root-owned /run. gid == uid by construction.
  uid = 1002;

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
