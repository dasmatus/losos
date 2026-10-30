# LosOS Cloud: people's files, calendars, contacts and boards, in the
# browser at /cloud/ and to the Nextcloud desktop and phone apps, calendar
# apps and file managers over the addresses they already know
# (backend/src/cloud/, the losos-cloud binary).
#
#   /cloud/         the web app (admin-ui/app/cloud/), behind LosOS Login
#   /cloud/api/     its JSON API
#   /cloud/feed/    calendars their owners published, for any calendar app
#                   holding the address (LosOS Calendar)
#   /status.php, /remote.php/, /ocs/, /index.php/, /public.php/
#                   what the Nextcloud apps call: discovery, sign-in (Login
#                   Flow v2, which opens /cloud/connect/<token>), WebDAV with
#                   chunked uploads, trash and versions, CalDAV and CardDAV
#   /nextcloud/     the same, where a box's Nextcloud used to answer, so
#                   apps set up against it keep working
#
# nginx is the only way in. The service listens on a Unix socket only
# nginx's group may open, which is what makes the person nginx names in
# X-Losos-Cloud-User believable: LosOS Login's (modules/identity.nix) for
# the page and its API, lososd's check of a LosOS Login password on the LAN
# for WebDAV (backend/src/cloud_share.rs). The apps sign in with a password
# of their own per device, which the service checks itself, from the LAN
# and through the tunnel alike.
#
# With losos.cloud.mesh.enable, people share with people on other boxes
# and edit text together live (backend/src/cloud/mesh/, collab/): the box
# answers other boxes at /mesh/v1/link, outside LosOS Login and the LAN
# guard, because the link's handshake proves which box is calling. The
# service then needs the network to dial them, but never loopback, where
# lososd and the other local services listen. nix/tests/mesh-collab.nix
# boots two boxes editing one file.
#
# A folder of notes can be published as a knowledge base, read at
# /kb/<name>/ (backend/src/cloud/notes/kb.rs, admin-ui/app/src/kb/), only
# while the box shares and is in a mesh, as chat, mail and calls are.
# Without both the service is started without --knowledge-bases: the menu
# item is gone, the owner's list says so, and /kb/api/ answers 404. A site
# for anyone is read without signing in, so /kb/ is outside the LAN guard;
# a site for people on the box takes the person LosOS Login names.
# nix/tests/notes.nix boots both states.
#
# Files live under /var/lib/losos-cloud/<login>/files, persisted with /var.
# The owner is `notshared`, the account that held their files in Nextcloud
# too (modules/app-sign-in.nix), so `losos-cloud import` moves them over
# under the same name. nix/tests/cloud.nix boots it, and
# nix/tests/cloud-from-nextcloud.nix moves a real Nextcloud into it.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.cloud;
  inherit (cfg) mesh;
  enabled = config.lososInternal.appSignIn.cloud;
  pkg = config.losos.backend.package;
  # The same gate as the shared side's chat, mail and calls.
  knowledgeBases = config.lososInternal.sideLive;
  # Office edits the cloud's files unless it was given a folder of its own.
  office = config.losos.office.enable && config.losos.office.folder == null;
  signIn = import ./app-sign-in.nix { inherit lib; };
  faceModels = import ../nix/face-models.nix { inherit pkgs; };

  stateDir = "/var/lib/losos-cloud";
  runDir = "/run/losos-cloud";
  socket = "${runDir}/http.sock";
  # The fetcher that reads subscribed calendars, which the cloud, with no
  # network of its own, asks over this socket.
  webcalSocket = "/run/losos-webcal/fetch.sock";
  upstream = "http://unix:${socket}";

  passSignIn = authz: ''
    auth_request ${authz};
    auth_request_set $losos_user $upstream_http_remote_user;
    auth_request_set $losos_groups $upstream_http_remote_groups;
    auth_request_set $losos_person_name $upstream_http_remote_name;
    auth_request_set $losos_person_email $upstream_http_remote_email;
    proxy_set_header ${signIn.header.user} $losos_app_user;
    proxy_set_header ${signIn.header.name} $losos_person_name;
    proxy_set_header ${signIn.header.email} $losos_person_email;
    proxy_set_header X-Losos-Cloud-Login $losos_user;
  '';

  # Where Nextcloud kept everything, on every box that ran it, in the pod
  # and natively alike: the files under data/<login>/files, its config.php,
  # and its database in the box's Postgres. The move below takes the files
  # (a rename, so no second copy), reads the rest once and leaves it.
  ncData = "/var/lib/nextcloud/data/data";
  ncConfig = "/var/lib/nextcloud/data/config/config.php";
  marker = "${stateDir}/.from-nextcloud";
  report = "${stateDir}/from-nextcloud.json";

  # The Postgres the box runs, if it still runs one, for its client tools.
  # A data folder no server has open is started with the server of its own
  # major version (PG_VERSION), from this list: 18 is what nixpkgs gives
  # stateVersion 26.11, the others are there in case a box was moved on.
  pg =
    if config.services.postgresql.enable then
      config.services.postgresql.package
    else
      pkgs.postgresql_18;
  pgByMajor = {
    "16" = pkgs.postgresql_16;
    "17" = pkgs.postgresql_17;
    "18" = pkgs.postgresql_18;
  };

  # The box's own containerd (modules/cluster.nix, `localCriSocket`), where
  # Nextcloud's pod ran.
  localCri = "unix:///run/containerd/containerd.sock";
  staticPod = "/var/lib/losos/k3s-static-pods/nextcloud.yaml";

  # The tables the move reads: every row of each, as JSON.
  ncTables = [
    "users"
    "calendars"
    "calendarobjects"
    "calendarsubscriptions"
    "addressbooks"
    "cards"
    "authtoken"
    "files_trash"
    "deck_boards"
    "deck_stacks"
    "deck_cards"
    "deck_labels"
    "deck_assigned_labels"
    "deck_assigned_users"
    "deck_board_acl"
  ];

  fromNextcloud = pkgs.writeShellScript "losos-cloud-from-nextcloud" ''
    set -euo pipefail
    work=$(mktemp -d /var/lib/losos-cloud-import.XXXXXX)
    chmod 0711 "$work"
    started=
    cleanup() {
      if [ -n "$started" ]; then
        runuser -u postgres -- "$started/bin/pg_ctl" -D "$work/pg" -m fast -w stop >/dev/null || true
      fi
      rm -rf "$work"
    }
    trap cleanup EXIT
    mkdir -p "$work/dump" ${stateDir}
    chmod 0700 ${stateDir}

    # Nextcloud first stops for good, so nothing writes while its files
    # move. The pod's manifest link goes (modules/cluster.nix removes it
    # too, but that may run after this on the boot that switches), and a
    # pod still running is stopped through the runtime. Natively there is
    # nothing to do: the switch to this system stopped php-fpm already.
    rm -f ${staticPod}
    if [ -S ${lib.removePrefix "unix://" localCri} ]; then
      for pod in $(crictl --runtime-endpoint ${localCri} pods --name '^nextcloud' --quiet 2>/dev/null); do
        echo "losos-cloud: stopping Nextcloud's pod"
        crictl --runtime-endpoint ${localCri} stopp "$pod" >/dev/null || true
      done
    fi

    # Nextcloud's database: the running server if there is one, else a
    # private copy of its data folder on a private socket, started with
    # the Postgres that made it. Never the original files, which stay as
    # they are.
    has_nextcloud() {
      # grep reads to the end (no -q), so psql never dies of a closed pipe
      # under pipefail.
      runuser -u postgres -- "$1/bin/psql" -h "$2" -XAtq -l | cut -d'|' -f1 | grep -x nextcloud >/dev/null
    }
    sock=
    bin=${pg}
    if [ -S /run/postgresql/.s.PGSQL.5432 ] && has_nextcloud ${pg} /run/postgresql; then
      sock=/run/postgresql
    else
      for v in $(ls -d /var/lib/postgresql/*/PG_VERSION 2>/dev/null | sort -rV); do
        data=$(dirname "$v")
        major=$(cat "$v")
        case "$major" in
          ${lib.concatStrings (lib.mapAttrsToList (m: p: "${m}) bin=${p} ;;
          ") pgByMajor)}*)
            echo "losos-cloud: $data is Postgres $major, which this box cannot open; skipped" >&2
            continue
            ;;
        esac
        rm -rf "$work/pg" "$work/sock"
        cp -a "$data" "$work/pg"
        # NixOS links postgresql.conf (and may link pg_hba.conf) into the
        # data folder from the store; replace the links, never write
        # through them.
        rm -f "$work/pg/postmaster.pid" "$work/pg/postgresql.conf" "$work/pg/pg_hba.conf"
        : > "$work/pg/postgresql.conf"
        printf 'local all all trust\n' > "$work/pg/pg_hba.conf"
        mkdir -p "$work/sock"
        # The work folder is root's (0711), so Postgres gets its log ready
        # made.
        : > "$work/pg.log"
        chown -R postgres "$work/pg" "$work/sock" "$work/pg.log"
        runuser -u postgres -- "$bin/bin/pg_ctl" -D "$work/pg" -w -l "$work/pg.log" \
          -o "-c listen_addresses= -c unix_socket_directories=$work/sock -c hba_file=$work/pg/pg_hba.conf" start
        started=$bin
        if has_nextcloud "$bin" "$work/sock"; then
          sock="$work/sock"
          break
        fi
        runuser -u postgres -- "$bin/bin/pg_ctl" -D "$work/pg" -m fast -w stop >/dev/null
        started=
      done
    fi

    if [ -n "$sock" ]; then
      q() { runuser -u postgres -- "$bin/bin/psql" -h "$sock" -d nextcloud -XAtq -v ON_ERROR_STOP=1 -c "$1"; }
      for t in ${lib.concatStringsSep " " ncTables}; do
        if [ "$(q "select to_regclass('oc_$t') is not null")" = t ]; then
          q "select coalesce(json_agg(row_to_json(x)), '[]') from oc_$t x" > "$work/dump/$t.json"
        fi
      done
      if [ "$(q "select to_regclass('oc_comments') is not null")" = t ]; then
        q "select coalesce(json_agg(row_to_json(x)), '[]') from oc_comments x where object_type = 'deckCard'" > "$work/dump/comments.json"
      fi
    else
      echo "losos-cloud: no Nextcloud database found; moving the files only" >&2
    fi

    ${pkg}/bin/losos-cloud --root ${stateDir} import \
      --from ${ncData} --dump "$work/dump" --config ${ncConfig} --report ${report}
    chown -R losos-cloud:losos-cloud ${stateDir}
    touch ${marker}
  '';

  # What the service is told about the request, by hand rather than by
  # recommendedProxySettings: behind the edge the request arrives as http
  # from loopback, and the scheme the apps are given back in sign-in and
  # poll addresses must be the browser's, `$losos_proto`
  # (modules/identity.nix), not nginx's own `$scheme`.
  forwarded = ''
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Proto $losos_proto;
    proxy_set_header X-Forwarded-Host $host;
  '';

  # Uploads of any size, streamed, and sync runs that take all day. The
  # body goes on in pieces of up to client_body_buffer_size; nginx's 16 KiB
  # default costs a write per piece on both sides, and 1 MiB took a large
  # upload from 24 s to 20 s on a two-core box (nix/tests/upload-speed.nix).
  # A smaller body gets a buffer of its own size.
  files = ''
    client_max_body_size 0;
    client_body_buffer_size 1m;
    proxy_request_buffering off;
    proxy_buffering off;
    proxy_read_timeout 86400s;
    proxy_send_timeout 86400s;
  '';
in
{
  config = lib.mkIf enabled {
    users.groups.losos-cloud = { };
    users.users.losos-cloud = {
      isSystemUser = true;
      group = "losos-cloud";
      home = stateDir;
    };
    # nginx opens the socket.
    users.users.${config.services.nginx.user}.extraGroups = [ "losos-cloud" ];

    # LosOS Contacts is LosOS Cloud's address books (backend/src/cloud/contacts.rs):
    # lososd lists it in /api/me and asks the book for the people pickers.
    systemd.services.lososd.environment.LOSOS_CLOUD_CONTACTS = "1";

    # Once, on the first boot without Nextcloud: everything it held comes
    # into LosOS Cloud before LosOS Cloud starts. Should it fail, the cloud
    # does not start on an empty folder, and the next boot or rebuild tries
    # again; nothing of Nextcloud's is deleted either way.
    systemd.services.losos-cloud-from-nextcloud = {
      description = "Move Nextcloud's files, calendars, contacts and boards into LosOS Cloud";
      requiredBy = [ "losos-cloud.service" ];
      before = [ "losos-cloud.service" ];
      after = [
        "local-fs.target"
        "postgresql.service"
        "k3s.service"
      ];
      unitConfig.ConditionPathExists = [
        ncData
        "!${marker}"
      ];
      path = [
        pkgs.coreutils
        pkgs.cri-tools
        pkgs.gnugrep
        pkgs.util-linux
      ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = fromNextcloud;
        UMask = "0077";
      };
    };
    users.users.postgres = lib.mkIf (!config.services.postgresql.enable) {
      isSystemUser = true;
      group = "postgres";
      uid = config.ids.uids.postgres;
    };
    users.groups.postgres = lib.mkIf (!config.services.postgresql.enable) {
      gid = config.ids.gids.postgres;
    };

    systemd.services.losos-cloud = {
      description = "LosOS Cloud";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];
      before = [ "nginx.service" ];
      serviceConfig = {
        ExecStart =
          lib.concatStringsSep " " [
            "${pkg}/bin/losos-cloud"
            "--root ${stateDir}"
            "serve"
            "--socket ${socket}"
            "--trash-days ${toString cfg.trashDays}"
            "--webcal-socket ${webcalSocket}"
            # People on the box edit text together either way; with the mesh
            # they also share with, and edit with, people on other boxes.
            "--box-name ${lib.escapeShellArg mesh.name}"
          ]
          + lib.optionalString mesh.enable (
            " --mesh" + lib.optionalString mesh.selfSigned " --mesh-self-signed"
          )
          + lib.optionalString knowledgeBases " --knowledge-bases"
          # Office documents open in Collabora (modules/office.nix): coolwsd
          # reaches /wopi/files/ through nginx on the WOPI port, and the types
          # it opens come from the copy of its discovery saved under /run.
          + lib.optionalString office (
            " --office-wopi http://127.0.0.1:${toString config.losos.office.wopiPort}"
            + " --coolwsd /run/losos-office-discovery/discovery.xml"
          )
          # Files the Windows share writes, into the table as they land
          # (modules/windows.nix).
          + lib.optionalString cfg.windows.enable " --watch";
        User = "losos-cloud";
        Group = "losos-cloud";
        StateDirectory = "losos-cloud";
        StateDirectoryMode = "0700";
        RuntimeDirectory = "losos-cloud";
        RuntimeDirectoryMode = "0750";
        Restart = "always";
        RestartSec = 2;
        UMask = "0077";
        NoNewPrivileges = true;
        CapabilityBoundingSet = "";
        PrivateNetwork = !mesh.enable;
        RestrictAddressFamilies = [
          "AF_UNIX"
        ]
        ++ lib.optionals mesh.enable [
          "AF_INET"
          "AF_INET6"
          "AF_NETLINK"
        ];
        IPAddressDeny = lib.mkIf mesh.enable [
          "localhost"
          "link-local"
          "multicast"
        ];
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        ProtectProc = "invisible";
        ProcSubset = "pid";
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
      };
    };

    # Photos (backend/src/cloud/photos/): a pass every few minutes reads the
    # pictures that are new or changed since the last one, their camera's
    # details and, with losos.cloud.photos.faces, their faces. It is its own
    # process at the lowest CPU and disk priority, writing in short
    # transactions to the database the server reads, so an upload never
    # waits on it; the models are loaded only when there is a picture to read.
    systemd.services.losos-cloud-photos = {
      description = "Read new photos' details and faces in LosOS Cloud";
      after = [ "losos-cloud.service" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = lib.escapeShellArgs (
          [
            "${pkg}/bin/losos-cloud"
            "--root"
            stateDir
            "photos"
          ]
          ++ lib.optionals cfg.photos.faces [
            "--models"
            "${faceModels}"
          ]
        );
        User = "losos-cloud";
        Group = "losos-cloud";
        StateDirectory = "losos-cloud";
        StateDirectoryMode = "0700";
        UMask = "0077";
        Nice = 19;
        CPUSchedulingPolicy = "idle";
        IOSchedulingClass = "idle";
        # One core at most, and a bound on memory: a picture is shrunk to
        # 2048 pixels before faces are looked for, and the models take
        # about 100 MB.
        CPUQuota = "100%";
        MemoryMax = "1G";
        TimeoutStartSec = "6h";
        NoNewPrivileges = true;
        CapabilityBoundingSet = "";
        PrivateNetwork = true;
        RestrictAddressFamilies = [ "AF_UNIX" ];
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        ProtectProc = "invisible";
        ProcSubset = "pid";
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
      };
    };
    systemd.timers.losos-cloud-photos = {
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnBootSec = "3min";
        OnUnitInactiveSec = "2min";
        AccuracySec = "30s";
      };
    };

    # LosOS Calendar's subscriptions (backend/src/cloud/webcal.rs): one
    # fetch per connection, by a process with the network the cloud lacks
    # and none of its files. It answers the cloud's group alone, reads
    # http and https only (webcal:// is https first, then http), and never
    # the box itself: lososd and the other local services listen on
    # loopback, and link-local is where cloud machines keep their secrets.
    # Addresses on the LAN stay open, for a calendar served at home.
    systemd.sockets.losos-webcal = {
      description = "Calendar subscriptions for LosOS Cloud";
      wantedBy = [ "sockets.target" ];
      socketConfig = {
        ListenStream = webcalSocket;
        Accept = true;
        SocketGroup = "losos-cloud";
        SocketMode = "0660";
        MaxConnections = 4;
      };
    };
    systemd.services."losos-webcal@" = {
      description = "Read one subscribed calendar for LosOS Cloud";
      environment.SSL_CERT_FILE = "/etc/ssl/certs/ca-certificates.crt";
      serviceConfig = {
        ExecStart = "${pkg}/bin/losos-cloud webcal-fetch --curl ${pkgs.curl}/bin/curl";
        StandardInput = "socket";
        StandardOutput = "socket";
        StandardError = "journal";
        # curl gives up after 30 s (webcal::MAX_SECS).
        RuntimeMaxSec = "45s";
        LogLevelMax = "notice";
        DynamicUser = true;
        NoNewPrivileges = true;
        CapabilityBoundingSet = "";
        RestrictAddressFamilies = [
          "AF_UNIX"
          "AF_INET"
          "AF_INET6"
        ];
        IPAddressDeny = [
          "localhost"
          "link-local"
          "multicast"
        ];
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        TemporaryFileSystem = "/var/lib:ro";
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        ProtectProc = "invisible";
        ProcSubset = "pid";
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        MemoryMax = "128M";
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
        UMask = "0077";
      };
    };

    services.nginx.virtualHosts."losos-front".locations = {
      # The web app, on real paths: every address under /cloud/ is the one
      # document. Signed out, LosOS Login's page first, which sends the
      # person back to the address they asked for ($losos_app_back).
      "/cloud/" = {
        tryFiles = "$uri /cloud/index.html";
        extraConfig = ''
          auth_request /internal/losos-app-sign-in;
          error_page 401 = @losos-app-sign-in;
          error_page 403 @losos-app-held;
          error_page 500 =503 @losos-app-waiting;
        '';
      };
      # Its API: the person if LosOS Login names one, and the service
      # answers 401 otherwise, never a redirect a script cannot follow.
      "/cloud/api/" = {
        proxyPass = upstream;
        recommendedProxySettings = false;
        # The live editor's y-sync socket is /cloud/api/collab/ws.
        proxyWebsockets = true;
        extraConfig = ''
          ${forwarded}
          ${passSignIn "/internal/losos-app-authz"}
          ${files}
        '';
      };
      # Where Nextcloud answered: the desktop and phone apps set up against
      # it keep their address. The prefix comes off here and the service
      # puts it back on the hrefs it answers with.
      "^~ /nextcloud/" = {
        proxyPass = upstream;
        recommendedProxySettings = false;
        extraConfig = ''
          ${forwarded}
          rewrite ^/nextcloud(/.*)$ $1 break;
          ${passSignIn "/internal/losos-cloud-share"}
          proxy_set_header Authorization $losos_share_authorization;
          proxy_set_header X-Losos-Cloud-Prefix /nextcloud;
          ${files}
        '';
      };
      # A calendar its owner published (LosOS Calendar), for any calendar
      # app holding the address: the token in it is the whole permission,
      # so no LosOS Login, and no person passed on.
      "/cloud/feed/" = {
        proxyPass = upstream;
        recommendedProxySettings = false;
        extraConfig = ''
          ${forwarded}
          limit_except GET {
            deny all;
          }
          proxy_set_header ${signIn.header.user} "";
          proxy_set_header ${signIn.header.name} "";
          proxy_set_header ${signIn.header.email} "";
          proxy_set_header X-Losos-Cloud-Login "";
        '';
      };
      "= /nextcloud".return = "301 /cloud/";
      # Calendar and contacts apps look here first (RFC 6764).
      "= /.well-known/caldav".return = "301 /remote.php/dav/";
      "= /.well-known/carddav".return = "301 /remote.php/dav/";
      # Knowledge bases: one page for every site, read by anyone the site's
      # owner allowed, so neither LosOS Login nor the LAN guard stands in
      # front. The service says which sites exist.
      "/kb/" = {
        tryFiles = "$uri /kb/index.html";
      };
      # Their data, with the person LosOS Login names for a site kept to
      # people on the box. The ask never refuses (modules/identity.nix), so
      # anyone else goes on as nobody, and passSignIn replaces whatever
      # name the request brought along with the empty one.
      "/kb/api/" = {
        proxyPass = upstream;
        recommendedProxySettings = false;
        extraConfig = ''
          ${forwarded}
          ${passSignIn "/internal/losos-app-authz"}
        '';
      };
      # Other boxes. No person and no LAN guard: the handshake signs both
      # ends with the boxes' keys before anything else is read.
      "/mesh/v1/" = lib.mkIf mesh.enable {
        proxyPass = upstream;
        proxyWebsockets = true;
        extraConfig = ''
          proxy_read_timeout 86400s;
          proxy_send_timeout 86400s;
        '';
      };
      # The pre-paint theme script the page loads, outside the LAN guard
      # of `/` for the same reason as /assets/ (modules/containers.nix).
      "= /theme-boot.js" = { };
      # What the Nextcloud apps and file managers call. A LosOS Login name
      # and password on the LAN open it as that person (lososd checks
      # them); anything else, an app's own password above all, goes on to
      # the service as it came. Regex, so it wins over the `/` prefix.
      "~ ^/(?:status\\.php|remote\\.php|public\\.php|index\\.php|ocs/v[12]\\.php)(?:/|$)" = {
        proxyPass = upstream;
        recommendedProxySettings = false;
        extraConfig = ''
          ${forwarded}
          ${passSignIn "/internal/losos-cloud-share"}
          proxy_set_header Authorization $losos_share_authorization;
          ${files}
        '';
      };
    };

    # The share in a file manager's Network view, announced over DNS-SD
    # (modules/mdns.nix), which is how GNOME Files, KDE Dolphin and other
    # Linux file managers list shares on the LAN. Over TLS whenever the box
    # has it, so the password does not cross the LAN in the clear. Windows
    # Explorer and the macOS Finder do not list WebDAV announcements;
    # handbook/docs/manual/files-on-your-computer.md has their way in.
    losos.mdns.services = {
      losos-cloud =
        let
          tls = config.losos.tls.enable;
        in
        ''
          Name=LosOS Cloud on %H
          Type=${if tls then "_webdavs._tcp" else "_webdav._tcp"}
          Port=${if tls then "443" else "80"}
          TxtText=path=/remote.php/webdav/
        '';
    };
  };
}
