# The Apache configuration the Nextcloud pod runs (flake/images.nix).
#
# A function rather than a file in images.nix, because tests/nextcloud-httpd.nix
# starts a real httpd and php-fpm on this exact text against a fixture webroot
# and asserts what every URL shape lands on. The pod's webroot is the 2.6 GiB
# Nextcloud closure, which no test wants to build to find out whether
# `/nextcloud/apps/tasks/` answers — and whether it does is a question about
# the rewrite rules, not about Nextcloud.
#
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
{
  pkgs,
  # The Nextcloud webroot the `/nextcloud` prefix is aliased onto.
  webroot,
  # The file carrying the one per-box scalar, `Listen 127.0.0.1:<apachePort>`.
  listenConf,
  # Apache's runtime scratch: the pid file, the mutexes. ServerRoot is a store
  # path, so none of it can live there.
  runDir,
  # php-fpm's socket, which the pool in images.nix listens on.
  fpmSocket,
}:

let
  # Where the front vhost's /nextcloud route lands. This literal appears in
  # three places by necessity — the nginx location in modules/containers.nix,
  # overwritewebroot in the losos.config.php modules/workloads.nix renders, and
  # the Alias here — because nginx proxies the route with the path *preserved*,
  # so Apache is asked for /nextcloud/index.php and must know that prefix.
  # There is no option for it: it is the appliance's published URL layout, not
  # a knob, and a knob would have to be turned in all three places anyway.
  prefix = "/nextcloud";
in
pkgs.writeText "httpd.conf" ''
  ServerRoot "${pkgs.apacheHttpd}"
  ServerName localhost
  ServerTokens Prod
  ServerSignature Off
  DefaultRuntimeDir ${runDir}
  PidFile ${runDir}/httpd.pid
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
  Include ${listenConf}

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

  Alias ${prefix} ${webroot}

  # `/nextcloud` with no slash is what an owner types into the address bar and
  # what the wizard frames (StepFirstSignIn.tsx). mod_dir used to add the slash;
  # DirectorySlash Off below stops it, so the redirect lives here instead.
  RewriteEngine On
  RewriteRule ^${prefix}$ ${prefix}/ [R=301,L]

  <Directory "${webroot}">
    # FollowSymLinks is not optional: every entry in this directory is a
    # symlink into /nix/store. -MultiViews as upstream's .htaccess has it: a
    # content-negotiated `index` would shadow the front controller.
    Options -Indexes +FollowSymLinks -MultiViews
    AllowOverride None
    Require all granted
    DirectoryIndex index.php index.html
    # Nextcloud's WebDAV and OCS routes are index.php/PATH/INFO shaped.
    AcceptPathInfo On

    RewriteEngine On
    # Per-directory rewriting strips this directory's filesystem path off the
    # URL before matching and would put it back afterwards — as a filesystem
    # path, which is not a URL. RewriteBase makes it put the Alias back
    # instead, so a relative substitution below means `${prefix}/…`.
    RewriteBase ${prefix}

    # Service discovery, as upstream's .htaccess does it.
    RewriteRule ^\.well-known/carddav ${prefix}/remote.php/dav/ [R=301,L]
    RewriteRule ^\.well-known/caldav ${prefix}/remote.php/dav/ [R=301,L]

    # Directories and entry points that must never be served as files. The
    # data directory lives outside the webroot on this appliance, so the
    # `data` arm is belt and braces rather than the only thing standing
    # between the internet and everyone's files — but it costs nothing.
    RewriteRule ^(?:build|tests|config|lib|3rdparty|templates|data)(?:$|/) - [R=404,L]
    RewriteRule ^(?:autotest|occ|issue|indie|db_|console) - [R=404,L]
    RewriteRule ^\.(?!well-known) - [R=404,L]

    # The front controller: pretty URLs.
    #
    # This is the block `occ maintenance:update:htaccess` appends to upstream's
    # .htaccess when `htaccess.RewriteBase` is set — which the losos.config.php
    # modules/workloads.nix renders does set, and which nothing could act on
    # with the .htaccess absent. Without it `/nextcloud/apps/tasks/` is a
    # filesystem lookup: `apps/tasks` does not exist (the curated apps live in
    # nix-apps/), so Apache answered "Not Found", and `/nextcloud/apps/files/`
    # is a real directory with no index, so that one was 403. Every tile on
    # the admin homepage links the pretty shape (admin-ui/app/src/lib/apps.ts),
    # and the native path serves it already — the nixpkgs nginx module sets
    # front_controller_active and falls back to index.php the same way. Now
    # anything that is not a static asset or one of Nextcloud's other entry
    # points goes to index.php, which routes on REQUEST_URI; and with
    # front_controller_active set, Nextcloud generates the pretty shape itself
    # instead of /nextcloud/index.php/… links.
    #
    # tests/nextcloud-httpd.nix asserts the map. Two shapes it pins that are
    # easy to break by "tidying": `/nextcloud/index.php/apps/files/` must keep
    # working (every link Nextcloud generated before this block existed, and
    # the wizard's frame detection in StepFirstSignIn.tsx), and `/nextcloud`
    # without the slash must still reach index.php.
    RewriteRule ^core/js/oc\.js$ index.php [PT,E=PATH_INFO:$1]
    RewriteRule ^core/preview\.png$ index.php [PT,E=PATH_INFO:$1]
    RewriteCond %{REQUEST_FILENAME} !\.(css|js|mjs|svg|gif|ico|jpg|jpeg|png|webp|html|ttf|woff2?|map|webm|mp4|mp3|ogg|wav|wasm|tflite)$
    RewriteCond %{REQUEST_FILENAME} !/remote\.php
    RewriteCond %{REQUEST_FILENAME} !/public\.php
    RewriteCond %{REQUEST_FILENAME} !/cron\.php
    RewriteCond %{REQUEST_FILENAME} !/core/ajax/update\.php
    RewriteCond %{REQUEST_FILENAME} !/status\.php
    RewriteCond %{REQUEST_FILENAME} !/ocs/v1\.php
    RewriteCond %{REQUEST_FILENAME} !/ocs/v2\.php
    RewriteCond %{REQUEST_FILENAME} !/robots\.txt
    RewriteCond %{REQUEST_FILENAME} !/\.well-known/(acme-challenge|pki-validation)/.*
    RewriteCond %{REQUEST_FILENAME} !/ocm-provider/.*
    RewriteCond %{REQUEST_FILENAME} !/ocs-provider/.*
    RewriteCond %{REQUEST_URI} !^${prefix}/\.well-known/(acme-challenge|pki-validation)/.*
    RewriteRule . index.php [PT,E=PATH_INFO:$1]
    SetEnv front_controller_active true
    # mod_dir would otherwise answer `/nextcloud/apps/files` (a real directory,
    # no slash) with a 301 to the slash form before the rule above ran.
    DirectorySlash Off

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
    SetHandler "proxy:unix:${fpmSocket}|fcgi://localhost/"
  </FilesMatch>
''
