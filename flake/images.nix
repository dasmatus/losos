# The OCI images the LOCAL k3s cluster runs.
#
# Three of them: a sandbox (pause) image, Nextcloud and Forgejo. They are
# preloaded through the cluster's `images` option, which symlinks each tarball
# into /var/lib/rancher/k3s/agent/images for the agent to import at start, so
# the pods run with `imagePullPolicy: Never` and nothing is ever fetched from a
# container registry. The image names below are under `losos.local/` for the
# same reason: if somebody ever flips that policy, the pull fails on a name
# that cannot resolve instead of quietly reaching Docker Hub.
#
# What this does relax, and it must be said plainly rather than left implied:
# the images are NOT built on the appliance. A Nextcloud image is ~2.6 GiB and
# system.autoUpgrade rebuilds at 03:00 on a repurposed mini-PC with a tmpfs
# root, three hours before an unconditional reboot. They come from this
# project's own Nix binary cache instead. Nothing is pulled from a *container
# registry*; the box does substitute from a Nix cache it trusts, and a cache
# that is unreachable at 03:00 means the rebuild fails and the previous
# generation keeps running.
#
# None of these images know anything that varies per box. The port Nextcloud
# listens on is tunable from the settings SPA, so it is not an image layer:
# modules/workloads.nix renders listen.conf and losos.config.php in module
# context and hostPath-mounts them at /etc/losos/nextcloud, which is the only
# thing the entrypoint below reads for it. Same for Forgejo's ROOT_URL and
# /etc/losos/forgejo/losos.ini. Changing a port changes the store path, changes
# the manifest, and the kubelet restarts the pod — no image rebuild ever.
#
# The pods run with hostNetwork (the local cluster has no CNI), which is why
# every listener these images configure is bound explicitly to loopback by the
# mounted config: nginx stays the only process on the box holding a public
# port.
#
# The image tags are left to dockerTools, which derives them from the output
# hash. That is deliberate: a hand-written tag like "34.0.2" would stay put
# across a content change, the manifest string would not change, and the
# kubelet — told never to pull — would happily keep running the previous image
# forever. A hash tag makes every content change a new manifest and therefore a
# restart.
{ pkgs }:

let
  inherit (pkgs) lib;

  # The one description of this appliance's Nextcloud, shared with the native
  # path (modules/nextcloud-common.nix). See that file's header for why it is
  # data in a third file rather than an option.
  nc = import ../modules/nextcloud-stack.nix { inherit pkgs lib; };

  # Every image gets these. `/bin/sh` comes from bash and is what Forgejo's
  # generated git hooks and Nextcloud's occ expect to exist; coreutils is there
  # for the entrypoints and for `crictl exec` on a box with no shell of its
  # own, which is the only way anyone will ever debug one of these pods.
  baseTools = [
    pkgs.bashInteractive
    pkgs.coreutils
  ];

  # dockerTools does not create /tmp or /usr/bin, and both are load-bearing:
  # PHP puts session and upload temp files in /tmp, and Forgejo writes git
  # hooks that start with `#!/usr/bin/env bash`. A missing /usr/bin/env is a
  # push that fails at the hook, long after anyone is still watching the
  # deployment.
  commonLayout = ''
    mkdir -p tmp usr/bin
    chmod 1777 tmp
    ln -s ${pkgs.coreutils}/bin/env usr/bin/env
  '';

  # ── Nextcloud ─────────────────────────────────────────────────────────────

  # The half of Nextcloud's configuration that belongs to the *image*: which
  # app directories exist, which cache backends are compiled in, where the
  # database socket is. It is a store path, symlinked into the config directory
  # by the entrypoint, exactly as the nixpkgs module does with its
  # override.config.php.
  #
  # It sorts before the losos.config.php modules/workloads.nix mounts ('-' is
  # 0x2D, '.' is 0x2E), and Nextcloud applies *.config.php in name order, so
  # the per-box file wins on any key both were ever to set.
  #
  # log_type = errorlog sends Nextcloud's log to stderr, which php-fpm forwards
  # to the container log. The alternative is a nextcloud.log inside the data
  # directory on a box with no shell to read it from.
  nextcloudImageConfig = pkgs.writeText "losos-image.config.php" ''
    <?php
    $CONFIG = [
      'apps_paths' => [
        [ 'path' => '${nc.webroot}/apps', 'url' => '/apps', 'writable' => false ],
        [ 'path' => '${nc.webroot}/nix-apps', 'url' => '/nix-apps', 'writable' => false ],
        [ 'path' => '${nc.webroot}/store-apps', 'url' => '/store-apps', 'writable' => true ],
      ],
      'appstoreenabled' => ${lib.boolToString nc.appstoreEnable},
      'datadirectory' => '${nc.dataDir}',
      'dbtype' => '${nc.database.type}',
      'dbname' => '${nc.database.name}',
      'dbuser' => '${nc.database.user}',
      'dbhost' => '${nc.database.socketDir}',
      'memcache.local' => '\\OC\\Memcache\\APCu',
      'memcache.distributed' => '\\OC\\Memcache\\Redis',
      'memcache.locking' => '\\OC\\Memcache\\Redis',
      'redis' => [
        'host' => '${nc.redisSocket}',
        'port' => 0,
      ],
      'log_type' => 'errorlog',
    ];
  '';

  # Apache's runtime scratch: the pid file, the mutexes, the FastCGI socket.
  # ServerRoot is a store path, so none of it can live there.
  nextcloudRunDir = "/run/nextcloud";
  nextcloudFpmSocket = "${nextcloudRunDir}/php-fpm.sock";

  # Where the front vhost's /nextcloud route lands. This literal appears in
  # three places by necessity — the nginx location in modules/containers.nix,
  # overwritewebroot in the losos.config.php modules/workloads.nix renders, and
  # the Alias here — because nginx proxies the route with the path *preserved*,
  # so Apache is asked for /nextcloud/index.php and must know that prefix.
  # There is no option for it: it is the appliance's published URL layout, not
  # a knob, and a knob would have to be turned in all three places anyway.
  nextcloudPrefix = "/nextcloud";

  # Apache, not nginx, because losos.nextcloud.apachePort's mounted listen.conf
  # is an `Listen <addr>:<port>` line — Apache syntax — and because that file is
  # the single scalar the port is allowed to live in.
  #
  # php-fpm rather than mod_php: mod_php needs a PHP built with apxs2Support
  # *and* ztsSupport, which is a full interpreter rebuild that no binary cache
  # has, on a box whose whole image story is "substitute, never build".
  #
  # No .htaccess is involved. The webroot is a store path built by symlinking
  # ${nc.package}/* into it, and that glob does not match dotfiles — so
  # Nextcloud's shipped .htaccess is not there, and Nextcloud could not rewrite
  # it in the store even if it were. The rules it would have carried are below,
  # transcribed from upstream's .htaccess and from the nginx locations the
  # nixpkgs module generates. If you add a route to one, add it here too.
  nextcloudHttpdConf = pkgs.writeText "httpd.conf" ''
    ServerRoot "${pkgs.apacheHttpd}"
    ServerName localhost
    ServerTokens Prod
    ServerSignature Off
    DefaultRuntimeDir ${nextcloudRunDir}
    PidFile ${nextcloudRunDir}/httpd.pid
    Timeout 300

    LoadModule mpm_event_module modules/mod_mpm_event.so
    LoadModule unixd_module modules/mod_unixd.so
    LoadModule authz_core_module modules/mod_authz_core.so
    LoadModule log_config_module modules/mod_log_config.so
    LoadModule mime_module modules/mod_mime.so
    LoadModule dir_module modules/mod_dir.so
    LoadModule alias_module modules/mod_alias.so
    LoadModule env_module modules/mod_env.so
    LoadModule setenvif_module modules/mod_setenvif.so
    LoadModule headers_module modules/mod_headers.so
    LoadModule rewrite_module modules/mod_rewrite.so
    LoadModule filter_module modules/mod_filter.so
    LoadModule deflate_module modules/mod_deflate.so
    LoadModule proxy_module modules/mod_proxy.so
    LoadModule proxy_fcgi_module modules/mod_proxy_fcgi.so

    # The one per-box scalar in this file: `Listen 127.0.0.1:<apachePort>`,
    # rendered by modules/workloads.nix and hostPath-mounted. Under hostNetwork
    # that loopback bind is what keeps this pod off the LAN.
    Include /etc/losos/nextcloud/listen.conf

    # A mini-PC that also runs Postgres, Redis, two Kubernetes instances and
    # nginx. These are deliberately small.
    <IfModule mpm_event_module>
      StartServers 1
      MinSpareThreads 4
      MaxSpareThreads 16
      ThreadsPerChild 16
      MaxRequestWorkers 48
    </IfModule>

    TypesConfig ${pkgs.apacheHttpd}/conf/mime.types
    AddType image/svg+xml .svg .svgz
    AddType application/wasm .wasm

    ErrorLog /dev/stderr
    LogLevel warn
    LogFormat "%h %l %u %t \"%r\" %>s %b" losos
    CustomLog /dev/stdout losos

    # Nothing is served unless a block below says so.
    <Directory />
      AllowOverride None
      Options None
      Require all denied
    </Directory>

    # Apache insists on a DocumentRoot. Everything real hangs off the Alias, so
    # this one is empty and denied.
    DocumentRoot "${pkgs.emptyDirectory}"
    <Directory "${pkgs.emptyDirectory}">
      Require all denied
    </Directory>

    Alias ${nextcloudPrefix} ${nc.webroot}

    <Directory "${nc.webroot}">
      # FollowSymLinks is not optional: every entry in this directory is a
      # symlink into /nix/store.
      Options -Indexes +FollowSymLinks
      AllowOverride None
      Require all granted
      DirectoryIndex index.php index.html
      # Nextcloud's WebDAV and OCS routes are index.php/PATH/INFO shaped.
      AcceptPathInfo On

      RewriteEngine On

      # Service discovery, as upstream's .htaccess does it.
      RewriteRule ^\.well-known/carddav ${nextcloudPrefix}/remote.php/dav/ [R=301,L]
      RewriteRule ^\.well-known/caldav ${nextcloudPrefix}/remote.php/dav/ [R=301,L]

      # Directories and entry points that must never be served as files. The
      # data directory lives outside the webroot on this appliance, so the
      # `data` arm is belt and braces rather than the only thing standing
      # between the internet and everyone's files — but it costs nothing.
      RewriteRule ^(?:build|tests|config|lib|3rdparty|templates|data)(?:$|/) - [R=404,L]
      RewriteRule ^(?:autotest|occ|issue|indie|db_|console) - [R=404,L]
      RewriteRule ^\.(?!well-known) - [R=404,L]

      # Upstream sets these from PHP as well; `set` replaces rather than
      # appends, so there is no duplicate header when it does.
      Header always set Referrer-Policy "no-referrer"
      Header always set X-Content-Type-Options "nosniff"
      Header always set X-Frame-Options "SAMEORIGIN"
      Header always set X-Permitted-Cross-Domain-Policies "none"
      Header always set X-Robots-Tag "noindex, nofollow"
    </Directory>

    # PHP goes to the FastCGI pool over a unix socket in the pod's own runtime
    # directory. Nothing listens on TCP.
    <FilesMatch "\.php$">
      SetHandler "proxy:unix:${nextcloudFpmSocket}|fcgi://localhost/"
    </FilesMatch>
  '';

  # One pool, no daemonizing, logs on stderr. `user`/`group` are deliberately
  # absent: the pod's securityContext already runs this as the nextcloud uid,
  # and php-fpm refuses to setuid when it is not root anyway.
  nextcloudFpmConf = pkgs.writeText "php-fpm.conf" ''
    [global]
    error_log = /dev/stderr
    daemonize = no
    log_limit = 8192

    [www]
    listen = ${nextcloudFpmSocket}
    listen.mode = 0660
    pm = dynamic
    pm.max_children = 32
    pm.start_servers = 2
    pm.min_spare_servers = 2
    pm.max_spare_servers = 6
    pm.max_requests = 500
    ; Nextcloud reads its configuration directory from the environment, and
    ; php-fpm scrubs the environment unless told otherwise.
    clear_env = no
    env[NEXTCLOUD_CONFIG_DIR] = ${nc.configDir}
    catch_workers_output = yes
    decorate_workers_output = no
  '';

  nextcloudEntrypoint = pkgs.writeShellApplication {
    name = "losos-nextcloud";
    runtimeInputs = [ pkgs.coreutils ];
    text = ''
      # PID 1 of the Nextcloud pod: bring the instance up to date, then run
      # Apache and php-fpm until one of them dies.
      #
      # Everything this reads from outside the image is a hostPath mount and is
      # checked before anything else runs, because the failure modes are
      # otherwise silent on a box with no shell: a missing config file makes
      # Apache exit with a line nobody reads, and a state directory owned by
      # the wrong uid makes Nextcloud reinstall itself into an empty database.

      umask 0027

      conf_dir=/etc/losos/nextcloud
      adminpass=/var/secrets/nextcloud-admin-pass

      fail() {
        echo "losos-nextcloud: $*" >&2
        exit 1
      }

      for f in "$conf_dir/listen.conf" "$conf_dir/losos.config.php" "$adminpass"; do
        [ -r "$f" ] || fail "$f is missing or unreadable. listen.conf and losos.config.php are rendered and mounted by modules/workloads.nix; the admin password is generated by losos-nextcloud-adminpass.service and mounted with hostPath type: File."
      done
      [ -s "$adminpass" ] || fail "$adminpass is empty — refusing to install Nextcloud with a blank admin password."

      mkdir -p ${nextcloudRunDir}
      chmod 0700 ${nextcloudRunDir}

      # The state root is a hostPath. The pod runs as the nextcloud uid and has
      # dropped every capability, so it cannot chown its way out of a directory
      # the host created for somebody else.
      [ -d ${nc.home} ] || fail "${nc.home} does not exist. It is a hostPath mount; the host creates it via systemd-tmpfiles."
      [ -w ${nc.home} ] || fail "${nc.home} is not writable by uid $(id -u). The host must create it owned by the nextcloud uid — see the tmpfiles rules in modules/workloads.nix."

      for d in ${nc.datadir} ${nc.configDir} ${nc.dataDir} ${nc.storeAppsDir}; do
        mkdir -p "$d" || fail "cannot create $d"
      done

      # Both halves of the configuration, linked in the way the nixpkgs module
      # links its override.config.php: a store path, replaced wholesale on
      # every activation, never edited in place.
      ln -sfn "$conf_dir/losos.config.php" ${nc.configDir}/losos.config.php
      ln -sfn ${nextcloudImageConfig} ${nc.configDir}/losos-image.config.php

      cd ${nc.webroot}
      export NEXTCLOUD_CONFIG_DIR=${nc.configDir}
      # Postgres roles are provisioned by the host (services.postgresql's
      # ensureUsers), so the installer must not try to create one itself.
      export NC_setup_create_db_user=false

      occ() {
        ${nc.php}/bin/php occ "$@"
      }

      if [ ! -s ${nc.configDir}/config.php ]; then
        echo "losos-nextcloud: no config.php — installing"
        # An empty --database-pass is correct: the connection is a peer-
        # authenticated unix socket, which is why modules/configuration.nix
        # pins the nextcloud uid.
        occ maintenance:install \
          --database "${nc.database.type}" \
          --database-name "${nc.database.name}" \
          --database-host "${nc.database.socketDir}" \
          --database-user "${nc.database.user}" \
          --database-pass "" \
          --admin-user "${nc.adminUser}" \
          --admin-pass "$(cat "$adminpass")" \
          --data-dir "${nc.dataDir}"
      fi

      # Fatal on purpose: serving an application against a database it has not
      # migrated yet corrupts data quietly.
      occ upgrade

      # Not fatal, equally on purpose: one app that went incompatible across a
      # nixpkgs bump must not take the whole box's file sync offline. It is
      # loud in the pod log and visible in the admin UI instead.
      occ app:enable ${lib.concatStringsSep " " (lib.attrNames nc.apps)} ||
        echo "losos-nextcloud: app:enable failed for at least one app; Nextcloud is still serving. Check the admin UI's app list." >&2

      ${nc.php}/bin/php-fpm --nodaemonize --fpm-config ${nextcloudFpmConf} &

      # Apache connects to the pool per request, so starting it first only
      # costs a burst of 503s to whoever is watching the dashboard while the
      # pod comes up. Waiting is cheap; ten seconds is far more than php-fpm
      # needs and short enough that a pool which will never start still fails
      # the pod quickly instead of hanging it.
      for _ in $(seq 100); do
        [ -S ${nextcloudFpmSocket} ] && break
        sleep 0.1
      done
      [ -S ${nextcloudFpmSocket} ] || fail "php-fpm did not create ${nextcloudFpmSocket} within 10s; see its output above."

      ${pkgs.apacheHttpd}/bin/httpd -f ${nextcloudHttpdConf} -DFOREGROUND &

      # Neither of these returns in normal operation, so whichever one does has
      # failed. Exiting takes the pod down with it and lets the kubelet restart
      # it, which is the honest outcome: an Apache with no FastCGI backend
      # serves 502s forever and looks alive to everything watching.
      status=0
      wait -n || status=$?
      fail "php-fpm or httpd exited with status $status; restarting the pod."
    '';
  };

  # ── Forgejo ───────────────────────────────────────────────────────────────

  forgejoStateDir = "/var/lib/forgejo";
  forgejoCustomDir = "${forgejoStateDir}/custom";

  # Everything about Forgejo that does not vary per box. The mounted
  # losos.ini — HTTP_ADDR, HTTP_PORT, ROOT_URL, DISABLE_REGISTRATION,
  # INSTALL_LOCK, actions.ENABLED — is concatenated *after* this at start, and
  # go-ini merges repeated section headers with the last value for a key
  # winning, so the per-box file overrides anything here.
  #
  # The database mirrors what modules/services.nix provisions natively: one
  # Postgres, peer auth over the socket, hence the pinned forgejo uid in
  # modules/configuration.nix.
  forgejoBaseIni = pkgs.writeText "forgejo-base.ini" ''
    [DEFAULT]
    RUN_MODE = prod
    RUN_USER = forgejo
    WORK_PATH = ${forgejoStateDir}

    [database]
    DB_TYPE = postgres
    HOST = /run/postgresql
    NAME = forgejo
    USER = forgejo
    SSL_MODE = disable

    [repository]
    ROOT = ${forgejoStateDir}/repositories

    [server]
    APP_DATA_PATH = ${forgejoStateDir}/data
    LFS_START_SERVER = true
    ; This appliance has no SSH at all — not for humans, not for git. The
    ; published route is HTTPS through the front vhost and nothing else.
    START_SSH_SERVER = false
    DISABLE_SSH = true

    [lfs]
    PATH = ${forgejoStateDir}/data/lfs

    [session]
    COOKIE_NAME = session

    [security]
    INSTALL_LOCK = true

    [log]
    MODE = console
    LEVEL = Info
  '';

  forgejoEntrypoint = pkgs.writeShellApplication {
    name = "losos-forgejo";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.forgejo
      pkgs.git
      pkgs.gnupg
    ];
    text = ''
      # PID 1 of the Forgejo pod. The shape follows nixpkgs'
      # services.forgejo preStart, because the problem is the same one: an
      # app.ini that is regenerated from the store on every start, plus a
      # handful of secrets that must survive that regeneration or every
      # session, LFS token and OAuth2 grant on the box is invalidated on
      # reboot.

      umask 0027

      conf=/etc/losos/forgejo/losos.ini

      fail() {
        echo "losos-forgejo: $*" >&2
        exit 1
      }

      [ -r "$conf" ] || fail "$conf is missing or unreadable. It is rendered and hostPath-mounted by modules/workloads.nix."
      [ -d ${forgejoStateDir} ] || fail "${forgejoStateDir} does not exist. It is a hostPath mount; the host creates it via systemd-tmpfiles."
      [ -w ${forgejoStateDir} ] || fail "${forgejoStateDir} is not writable by uid $(id -u). The host must create it owned by the forgejo uid — see the tmpfiles rules in modules/workloads.nix."

      export FORGEJO_WORK_DIR=${forgejoStateDir}
      export FORGEJO_CUSTOM=${forgejoCustomDir}
      export HOME=${forgejoStateDir}
      export USER=forgejo

      mkdir -p ${forgejoCustomDir}/conf ${forgejoStateDir}/data ${forgejoStateDir}/repositories

      # Generated once and kept. `forgejo generate secret` is the same
      # generator the NixOS module uses, and the __FILE environment variables
      # below are the documented way to get a secret into app.ini without it
      # ever being written into the store.
      for secret in SECRET_KEY INTERNAL_TOKEN JWT_SECRET LFS_JWT_SECRET; do
        file=${forgejoCustomDir}/conf/$secret
        if [ ! -s "$file" ]; then
          (
            umask 0077
            forgejo generate secret "$secret" > "$file"
          )
        fi
      done

      config=${forgejoCustomDir}/conf/app.ini
      cat ${forgejoBaseIni} "$conf" > "$config"

      FORGEJO__security__SECRET_KEY__FILE=${forgejoCustomDir}/conf/SECRET_KEY
      FORGEJO__security__INTERNAL_TOKEN__FILE=${forgejoCustomDir}/conf/INTERNAL_TOKEN
      FORGEJO__oauth2__JWT_SECRET__FILE=${forgejoCustomDir}/conf/JWT_SECRET
      FORGEJO__server__LFS_JWT_SECRET__FILE=${forgejoCustomDir}/conf/LFS_JWT_SECRET
      export FORGEJO__security__SECRET_KEY__FILE
      export FORGEJO__security__INTERNAL_TOKEN__FILE
      export FORGEJO__oauth2__JWT_SECRET__FILE
      export FORGEJO__server__LFS_JWT_SECRET__FILE

      environment-to-ini --config "$config"
      chmod 0600 "$config"

      # Both are idempotent and both are needed after an upgrade: `migrate`
      # brings the schema forward, `regenerate hooks` rewrites the store paths
      # baked into every repository's git hooks, which change on every rebuild.
      forgejo migrate
      forgejo admin regenerate hooks

      exec forgejo web
    '';
  };

  # ── Pause ─────────────────────────────────────────────────────────────────

  # The sandbox image every pod on the local cluster is built around: PID 1 of
  # the pod's namespaces, whose whole job is to hold them open and reap
  # whatever gets reparented onto it. tini + `sleep inf` is what nixpkgs' own
  # nixos/tests/rancher/single-node.nix passes to --pause-image, and this is
  # the same use.
  pauseEnv = pkgs.buildEnv {
    name = "losos-image-pause-root";
    paths = [
      pkgs.tini
      pkgs.coreutils
    ];
  };
in
{
  losos-image-pause = pkgs.dockerTools.buildLayeredImage {
    name = "losos.local/pause";
    compressor = "zstd";
    contents = pauseEnv;
    config.Entrypoint = [
      "/bin/tini"
      "-g"
      "--"
      "/bin/sleep"
      "inf"
    ];
  };

  losos-image-nextcloud = pkgs.dockerTools.buildLayeredImage {
    name = "losos.local/nextcloud";
    # zstd rather than gzip: the agent decompresses every one of these at
    # import time, on the appliance, and the mini-PC notices.
    compressor = "zstd";
    contents = pkgs.buildEnv {
      name = "losos-image-nextcloud-root";
      paths = baseTools ++ [
        pkgs.tini
        nextcloudEntrypoint
        nc.php
      ];
    };
    extraCommands = commonLayout;
    config = {
      # tini in process-group mode: it forwards the kubelet's SIGTERM to both
      # Apache and php-fpm, which are siblings under the entrypoint rather than
      # children of each other.
      Entrypoint = [
        "/bin/tini"
        "-g"
        "--"
        "/bin/losos-nextcloud"
      ];
      Env = [
        "PATH=/bin"
        "LANG=C.UTF-8"
      ];
      WorkingDir = nc.home;
    };
  };

  losos-image-forgejo = pkgs.dockerTools.buildLayeredImage {
    name = "losos.local/forgejo";
    compressor = "zstd";
    contents = pkgs.buildEnv {
      name = "losos-image-forgejo-root";
      paths = baseTools ++ [
        pkgs.tini
        forgejoEntrypoint
        pkgs.forgejo
        pkgs.git
      ];
    };
    extraCommands = commonLayout;
    config = {
      Entrypoint = [
        "/bin/tini"
        "-g"
        "--"
        "/bin/losos-forgejo"
      ];
      Env = [
        "PATH=/bin"
        "LANG=C.UTF-8"
      ];
      WorkingDir = forgejoStateDir;
    };
  };
}
