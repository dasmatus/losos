# The Nextcloud pod's Apache, started for real against a fixture webroot.
#
# flake/nextcloud-httpd.nix is the httpd.conf the pod runs — the Alias, the
# rewrite rules transcribed from Nextcloud's .htaccess, the php-fpm handler.
# Whether `/nextcloud/apps/tasks/` reaches index.php or Apache's "Not Found"
# page is decided entirely by that text, and nothing else exercised it: the
# image check (tests/cluster-vm.nix) imports a probe image rather than the
# 2.6 GiB Nextcloud one, and the native path is nginx. So the admin homepage
# shipped nine tiles whose links all 404ed or 403ed in container mode, and the
# first person to click one was the owner, in a VM, with a camera.
#
# Not a VM: a derivation that starts php-fpm and httpd on loopback inside the
# build sandbox, on the same config text the image bakes, with the webroot
# swapped for a handful of fixture files whose bodies say which one answered.
# index.php reports what PHP would see — SCRIPT_NAME, REQUEST_URI,
# front_controller_active — because that is what Nextcloud routes on.
#
# Runs in about two seconds. `nix build .#checks.x86_64-linux.losos-nextcloud-httpd`.
{ pkgs }:
let
  inherit (pkgs) lib;

  # The same php the pod runs, so the FastCGI side is the real one too.
  nc = import ../modules/nextcloud-stack.nix { inherit pkgs lib; };

  # Which file answered, and what it saw. Every PHP entry point in the fixture
  # prints this with its own name, so one grep tells both.
  report = name: ''
    <?php
    header('Content-Type: application/json');
    echo json_encode([
      'served_by' => '${name}',
      'script_name' => $_SERVER['SCRIPT_NAME'] ?? null,
      'request_uri' => $_SERVER['REQUEST_URI'] ?? null,
      'path_info' => $_SERVER['PATH_INFO'] ?? null,
      'front_controller_active' => getenv('front_controller_active'),
      'auth_user' => $_SERVER['PHP_AUTH_USER'] ?? null,
      'auth_pw' => $_SERVER['PHP_AUTH_PW'] ?? null,
    ]);
  '';

  webroot = pkgs.runCommand "nextcloud-httpd-fixture-webroot" { } ''
    mkdir -p $out
    cd $out
    # The entry points the rewrite rules name, each reporting itself.
    for f in index status remote public cron; do
      cat > $f.php <<'EOF'
    ${report "ENTRY"}
    EOF
      sed -i "s/ENTRY/$f.php/" $f.php
    done
    mkdir -p ocs core/ajax
    cp status.php ocs/v2.php; sed -i 's/status.php/ocs\/v2.php/' ocs/v2.php
    # Static assets: served as files, never through the front controller.
    mkdir -p core/css core/img apps/files/js
    echo 'body{}' > core/css/server.css
    echo 'app' > apps/files/js/app.js
    printf 'PNG' > core/img/logo.png
    # apps/files is a real directory with no index — the 403 of old. The
    # curated apps are not under apps/ at all (nix-apps/), hence the 404.
    mkdir -p nix-apps/tasks/appinfo
    echo '<info/>' > nix-apps/tasks/appinfo/info.xml
    # Things that must never be served.
    mkdir -p config data
    echo '<?php $CONFIG = [];' > config/config.php
    echo 'secret' > data/owner.txt
    echo '#!/bin/sh' > occ
    echo 'hidden' > .htaccess
    echo 'User-agent: *' > robots.txt
  '';

  port = 18080;
  listenConf = pkgs.writeText "listen.conf" "Listen 127.0.0.1:${toString port}\n";

  # Runtime paths live under the build's own scratch directory; the config is
  # rendered against them exactly as images.nix renders it against /run.
  runDir = "/build/run";
  fpmSocket = "${runDir}/php-fpm.sock";

  httpdConf = import ../flake/nextcloud-httpd.nix {
    inherit
      pkgs
      webroot
      listenConf
      runDir
      fpmSocket
      ;
  };

  fpmConf = pkgs.writeText "php-fpm.conf" ''
    [global]
    error_log = /dev/stderr
    daemonize = no

    [www]
    listen = ${fpmSocket}
    listen.mode = 0660
    pm = static
    pm.max_children = 2
    clear_env = no
    catch_workers_output = yes
  '';

  # One case per line: url, expected status, and a fragment the body must
  # contain (none: the status is enough). `served_by` says which entry point
  # PHP ran; a static file carries its own bytes; a 404 from the deny rules is
  # Apache's own page.
  caseLines = ''
    # The front controller: the pretty shape every homepage tile links to
    # (admin-ui/app/src/lib/apps.ts joins FILES_BASE and the app's path).
    /nextcloud/apps/tasks/ 200 "served_by":"index.php"
    /nextcloud/apps/tasks/ 200 "request_uri":"\/nextcloud\/apps\/tasks\/"
    /nextcloud/apps/tasks/ 200 "script_name":"\/nextcloud\/index.php"
    /nextcloud/apps/tasks/ 200 "front_controller_active":"true"
    /nextcloud/apps/files/ 200 "served_by":"index.php"
    /nextcloud/apps/files 200 "served_by":"index.php"
    /nextcloud/login 200 "served_by":"index.php"
    /nextcloud/settings/user 200 "served_by":"index.php"
    # The root. Without the slash — what an owner types, and what the wizard
    # frames — it is a redirect to the slash form (checked below).
    /nextcloud/ 200 "served_by":"index.php"
    /nextcloud 301
    # The old shape keeps working: every link Nextcloud generated before
    # front_controller_active was set, and the wizard's frame detection
    # (StepFirstSignIn.tsx matches /nextcloud/index.php/login too).
    /nextcloud/index.php/apps/files/ 200 "served_by":"index.php"
    /nextcloud/index.php/apps/files/ 200 "request_uri":"\/nextcloud\/index.php\/apps\/files\/"
    /nextcloud/index.php/login 200 "served_by":"index.php"
    # The other entry points are reached directly, path info and all.
    /nextcloud/status.php 200 "served_by":"status.php"
    /nextcloud/remote.php/dav/files/owner/ 200 "served_by":"remote.php"
    /nextcloud/remote.php/dav/files/owner/ 200 "request_uri":"\/nextcloud\/remote.php\/dav\/files\/owner\/"
    /nextcloud/public.php/dav/ 200 "served_by":"public.php"
    /nextcloud/ocs/v2.php/cloud/capabilities 200 "served_by":"ocs\/v2.php"
    /nextcloud/cron.php 200 "served_by":"cron.php"
    # Static assets are files, not PHP.
    /nextcloud/core/css/server.css 200 body{}
    /nextcloud/apps/files/js/app.js 200 app
    /nextcloud/core/img/logo.png 200 PNG
    /nextcloud/robots.txt 200 User-agent
    # Service discovery.
    /nextcloud/.well-known/carddav 301
    /nextcloud/.well-known/caldav 301
    # Never served, whatever exists on disk.
    /nextcloud/config/config.php 404
    /nextcloud/data/owner.txt 404
    /nextcloud/occ 404
    /nextcloud/.htaccess 404
    # Nothing outside the Alias.
    / 403
    /index.php 403
  '';
in
pkgs.runCommand "losos-nextcloud-httpd"
  {
    nativeBuildInputs = [
      pkgs.apacheHttpd
      pkgs.curl
      nc.php
    ];
    passAsFile = [ "caseLines" ];
    inherit caseLines;
  }
  ''
    mkdir -p ${runDir}
    php-fpm --nodaemonize --fpm-config ${fpmConf} &
    for _ in $(seq 100); do [ -S ${fpmSocket} ] && break; sleep 0.1; done
    [ -S ${fpmSocket} ] || { echo "php-fpm did not come up"; exit 1; }

    httpd -f ${httpdConf} -DFOREGROUND &
    for _ in $(seq 100); do
      curl -s -o /dev/null http://127.0.0.1:${toString port}/ && break
      sleep 0.1
    done

    failed=0
    while read -r url want_status want_body; do
      case "$url" in ""|"#"*) continue ;; esac
      body=$(curl -s -o - -w '\n%{http_code}' "http://127.0.0.1:${toString port}$url")
      status=''${body##*$'\n'}
      body=''${body%$'\n'*}
      if [ "$status" != "$want_status" ]; then
        echo "FAIL $url: status $status, wanted $want_status"; echo "     $body" | head -c 300; echo
        failed=1
      elif [ -n "$want_body" ] && ! grep -qF -- "$want_body" <<<"$body"; then
        echo "FAIL $url: body lacks $want_body"; echo "     $body" | head -c 300; echo
        failed=1
      else
        echo "ok   $url -> $status $want_body"
      fi
    done < "$caseLinesPath"

    # The carddav redirect lands on the DAV endpoint under the prefix, not
    # under / — which is what RewriteBase is for.
    loc=$(curl -s -o /dev/null -w '%{redirect_url}' "http://127.0.0.1:${toString port}/nextcloud/.well-known/carddav")
    case "$loc" in
      */nextcloud/remote.php/dav/) echo "ok   carddav -> $loc" ;;
      *) echo "FAIL carddav redirects to $loc"; failed=1 ;;
    esac

    loc=$(curl -s -o /dev/null -w '%{redirect_url}' "http://127.0.0.1:${toString port}/nextcloud")
    case "$loc" in
      */nextcloud/) echo "ok   /nextcloud -> $loc" ;;
      *) echo "FAIL /nextcloud redirects to '$loc'"; failed=1 ;;
    esac

    # Basic credentials reach PHP. mod_proxy_fcgi drops the Authorization
    # header unless told otherwise, and Nextcloud then answers every
    # password-authenticated request 401 — the admin sign-in probe
    # (backend/src/signin.rs), WebDAV clients, the OCS API. A browser session
    # rides a cookie, so nothing in a click-through ever noticed.
    for url in /nextcloud/ocs/v2.php/cloud/user /nextcloud/remote.php/dav/files/owner/; do
      body=$(curl -s -u 'owner:pass word!' "http://127.0.0.1:${toString port}$url")
      if grep -qF '"auth_user":"owner","auth_pw":"pass word!"' <<<"$body"; then
        echo "ok   $url sees Basic credentials"
      else
        echo "FAIL $url: PHP saw no Basic credentials"; echo "     $body" | head -c 300; echo
        failed=1
      fi
    done

    [ "$failed" = 0 ] || exit 1
    touch $out
  ''
