# The binary caches the appliance substitutes from, and the local copy of
# them the box keeps on /persist.
#
# Imported by BOTH systems. A substituter the target only learns about once it
# is installed does nothing for the install itself, which is where the time
# goes: the install closure carries the LosOS packages and the workload images,
# which cache.nixos.org cannot serve, and with nothing serving them a fresh
# install *builds* them on a repurposed mini-PC.
#
# WHAT FILLS THE LOSOS CACHE. `.github/workflows/ci.yml` publishes signed GHCR
# artifacts through .github/actions/proxy-push after each build job. Those are
# exactly the paths cache.nixos.org cannot serve, because this flake builds
# them. `losos-ghcr-cache` (tools/losos-ghcr-cache.py, 127.0.0.1:8787) speaks
# Nix's binary-cache protocol and pulls one artifact with oras when Nix asks
# for it. The GitHub Pages copy is a remote fallback signed with the same key.
#
# THE LOCAL COPY (losos.cache.local, on the box only). /nix is on /persist, so
# a path the box has is never fetched again while it stays in the store. But
# the 04:30 clean-up (modules/updates.nix) deletes everything no generation
# refers to, and that includes the compilers, archivers and other build-time
# inputs the 03:00 rebuild fetched to build this box's own derivations. The
# next night fetches them again, from the internet, every night. So every Nix
# download on the box goes through `losos-nix-cache`, an nginx of its own on
# 127.0.0.1:8790 that asks the peers and upstreams in order and keeps each
# answer under /var/cache/losos-nix-cache. /var is persisted whole
# (modules/impermanence.nix), so the copy outlives the 00:07 restart and the
# GC both, and nginx's cache manager holds it to `maxSize` and `minFree`,
# least recently used first.
#
# Trust does not move. nginx stores and returns upstream bytes unchanged; the
# narinfo signatures cover the store path, NAR hash, size and references, and
# Nix verifies them against nix.settings.trusted-public-keys exactly as it did
# when it talked to cache.nixos.org itself. A wrong or missing signature is
# refused on the box, whoever served it (nix/tests/nix-cache.nix proves both).
#
# Its own nginx and not a server in the front one (modules/containers.nix):
# a typo in a cache location must not take the admin UI down with it, and an
# unprivileged user with nothing but a cache directory is all this needs.
{
  lib,
  pkgs,
  config,
  ...
}:

let
  cfg = config.losos.cache;
  inherit (cfg) local;
  enabled = cfg.substituters != [ ];
  front = "http://127.0.0.1:${toString local.port}";
  dir = "/var/cache/losos-nix-cache";

  # A source is a binary cache the proxy asks: scheme, host[:port], and an
  # optional path prefix (the Pages copy lives under /proxy).
  parse =
    option: url:
    let
      m = builtins.match "(https?)://([^/]+)(/.*)?" url;
      path = lib.removeSuffix "/" (if m == null || lib.elemAt m 2 == null then "" else lib.elemAt m 2);
    in
    if m == null then
      throw "losos.cache.local.${option}: `${url}` is not an http:// or https:// address"
    else
      {
        scheme = lib.elemAt m 0;
        host = lib.elemAt m 1;
        inherit path;
      };

  sources =
    map (u: parse "peers" u // { peer = true; }) local.peers
    ++ map (u: parse "upstreams" u // { peer = false; }) (lib.remove front local.upstreams);

  # One named location per source. A miss (or an error: the peer is off, the
  # internet is down) hands the request to the next one, and the last one to
  # @miss, a plain 404 that Nix reads as "not here" rather than as a broken
  # substituter. Each location looks in the cache before it proxies, under
  # the same key, so a path cached from any source is a hit at the first.
  #
  # The host goes through a variable on purpose: nginx then resolves it per
  # request, so a box that boots with no network still starts the proxy, and
  # a box that moves networks follows the new DNS.
  sourceLocation =
    i: s:
    let
      next = if i + 1 < lib.length sources then "@source${toString (i + 1)}" else "@miss";
    in
    ''
      location @source${toString i} {
        ${lib.optionalString s.peer ''
          # A request that already came from a peer is not passed on to ours:
          # two boxes that name each other would hand it back and forth.
          if ($http_x_losos_nix_cache_hop) { return 404; }
          proxy_set_header X-Losos-Nix-Cache-Hop 1;
        ''}
        set $losos_source_host "${s.host}";
        proxy_pass ${s.scheme}://$losos_source_host${s.path}$uri;
        error_page 400 401 403 404 405 408 410 429 500 502 503 504 = ${next};
      }
    '';

  # Who may ask. Loopback always (this port is not one the tunnel forwards, so
  # unlike the front vhost's lanOnly guard, 127.0.0.1 here really is the box);
  # private LAN addresses only with shareOnLan, minus the mesh's pod network
  # like lanOnly (modules/lan-only.nix).
  access =
    if local.shareOnLan then
      ''
        allow 127.0.0.0/8;
        allow ::1;
        deny 10.42.0.0/16;
        allow 10.0.0.0/8;
        allow 172.16.0.0/12;
        allow 192.168.0.0/16;
        deny all;
      ''
    else
      ''
        allow 127.0.0.0/8;
        deny all;
      '';

  cacheInfo = pkgs.writeText "nix-cache-info" ''
    StoreDir: /nix/store
    WantMassQuery: 1
    Priority: 10
  '';

  nginxConf = pkgs.writeText "losos-nix-cache.conf" ''
    daemon off;
    worker_processes 2;
    pid /run/losos-nix-cache/nginx.pid;
    error_log stderr warn;

    events {
      worker_connections 512;
    }

    http {
      default_type application/octet-stream;
      sendfile on;
      tcp_nopush on;
      server_tokens off;

      # One line per request in the journal, with whether it was a hit, so
      # `journalctl -t losos_nix_cache` says what came from where.
      log_format losos '$remote_addr "$request" $status $body_bytes_sent '
                       'cache=$upstream_cache_status from=$upstream_addr '
                       'time=$request_time';
      access_log syslog:server=unix:/run/systemd/journal/dev-log,nohostname,tag=losos_nix_cache losos;

      client_body_temp_path ${dir}/tmp/client;
      proxy_temp_path ${dir}/tmp/proxy;
      fastcgi_temp_path ${dir}/tmp/fastcgi;
      uwsgi_temp_path ${dir}/tmp/uwsgi;
      scgi_temp_path ${dir}/tmp/scgi;

      # systemd-resolved's stub: it answers .local names over mDNS and the
      # test VMs' /etc/hosts, which nginx's own resolver would not read.
      resolver 127.0.0.53 valid=300s;
      resolver_timeout 5s;

      # A 32 MiB key zone holds about 250 000 entries. inactive drops a path
      # nobody asked for in 30 days, so last month's kernel goes even below
      # max_size; min_free keeps the owner's files ahead of the copy.
      proxy_cache_path ${dir}/store levels=1:2 keys_zone=losos_nix:32m
        max_size=${local.maxSize} min_free=${local.minFree} inactive=30d
        use_temp_path=off;

      server {
        listen ${if local.shareOnLan then "" else "127.0.0.1:"}${toString local.port};
        ${access}

        proxy_cache losos_nix;
        proxy_cache_key $uri;
        proxy_cache_valid 200 365d;
        # Upstreams say how long to keep things; the copy keeps them until
        # inactive or the size cap says otherwise. Store paths never change.
        proxy_ignore_headers Cache-Control Expires Set-Cookie Vary X-Accel-Expires;
        proxy_hide_header Set-Cookie;
        # One download per path, however many requests wait for it; a big NAR
        # on a slow line takes minutes.
        proxy_cache_lock on;
        proxy_cache_lock_age 600s;
        proxy_cache_lock_timeout 600s;
        proxy_cache_use_stale error timeout updating;
        proxy_intercept_errors on;
        recursive_error_pages on;

        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_ssl_server_name on;
        proxy_ssl_verify on;
        proxy_ssl_verify_depth 4;
        proxy_ssl_trusted_certificate /etc/ssl/certs/ca-certificates.crt;
        # Short to connect, so a peer that is off costs three seconds once;
        # long to read, because losos-ghcr-cache pulls a whole artifact
        # before it answers the narinfo.
        proxy_connect_timeout 3s;
        proxy_send_timeout 60s;
        proxy_read_timeout 300s;

        add_header X-Losos-Nix-Cache $upstream_cache_status always;

        # A file rather than `return`: return runs before allow/deny, and
        # nothing here should answer a stranger. Priority 10 puts this ahead
        # of cache.nixos.org (40), which stays in nix.settings.substituters
        # behind it as a fallback.
        location = /nix-cache-info {
          default_type text/x-nix-cache-info;
          alias ${cacheInfo};
        }

        # What Nix asks a binary cache for, and nothing else.
        location ~ "^/(?:[0-9a-df-np-sv-z]{32}\.narinfo|nar/[0-9a-z][0-9a-z./_-]*)$" {
          root ${pkgs.emptyDirectory};
          try_files /.never @source0;
        }

        location / {
          return 404;
        }

        ${lib.concatStrings (lib.imap0 sourceLocation sources)}

        location @miss {
          return 404 "not in any cache\n";
        }
      }
    }
  '';
in
{
  assertions = [
    # A substituter with no key is worse than no substituter: nix declines the
    # signature, silently falls back to building from source, and the
    # operator sees "the install is still slow" rather than an error.
    {
      assertion = enabled -> cfg.trustedPublicKeys != [ ];
      message =
        "losos.cache.substituters is set but losos.cache.trustedPublicKeys is empty. "
        + "Nix would refuse the signatures and build from source anyway, which is "
        + "the slow install the cache exists to avoid.";
    }
    {
      assertion = local.enable -> sources != [ ];
      message = "losos.cache.local.enable is on with no peers and no upstreams: the local cache would answer every path with a 404.";
    }
    {
      assertion = local.enable -> config.services.resolved.enable;
      message = "losos.cache.local resolves its upstreams through systemd-resolved's stub (127.0.0.53), and services.resolved is off.";
    }
  ];

  nix.settings = lib.mkMerge [
    (lib.mkIf (enabled || local.enable) {
      extra-trusted-public-keys = cfg.trustedPublicKeys;
      connect-timeout = 5;
      fallback = true;
    })
    # The installer medium, and a box with the local copy off, ask the LosOS
    # caches themselves. extra-* rather than replacing the defaults:
    # cache.nixos.org still serves upstream NixOS paths.
    (lib.mkIf (enabled && !local.enable) {
      extra-substituters = cfg.substituters;
    })
    # The box asks only its local copy, which asks everyone else. nixpkgs
    # appends cache.nixos.org to this list (mkAfter), and that stays as the
    # direct fallback for a night the proxy is down. The LosOS caches are not
    # listed again: every path the proxy misses would be asked twice.
    (lib.mkIf local.enable {
      substituters = [ front ];
    })
  ];

  # Nix speaks its ordinary binary-cache HTTP protocol to localhost. On a
  # cache miss this service uses oras to pull the matching per-store-path OCI
  # artifact from GHCR, rewrites only the unsigned URL field in narinfo, and
  # serves the NAR locally. Nix still verifies the original narinfo signature.
  systemd.services.losos-ghcr-cache = lib.mkIf enabled {
    description = "Local GHCR-backed Nix binary cache";
    wantedBy = [ "multi-user.target" ];
    after = [ "network.target" ];
    path = [ pkgs.oras ];
    environment = {
      LOSOS_GHCR_CACHE = "ghcr.io/dasmatus/losos/nix-cache";
      LOSOS_GHCR_CACHE_DIR =
        if config.losos.installer.autorun then
          "/mnt/persist/var/cache/losos-ghcr-cache"
        else
          "/var/cache/losos-ghcr-cache";
    }
    # Behind the local copy this is only a staging area between the narinfo
    # and the NAR request: losos-nix-cache keeps the lasting copy, under its
    # size cap, so an artifact here is dropped after an hour.
    // lib.optionalAttrs local.enable {
      LOSOS_GHCR_CACHE_KEEP_SECONDS = "3600";
    };
    serviceConfig = {
      ExecStart = "${pkgs.python3}/bin/python3 ${../tools/losos-ghcr-cache.py}";
      Restart = "on-failure";
      RestartSec = 3;
    };
  };

  users.users.losos-nix-cache = lib.mkIf local.enable {
    isSystemUser = true;
    group = "losos-nix-cache";
    description = "LosOS local Nix cache";
  };
  users.groups.losos-nix-cache = lib.mkIf local.enable { };

  networking.firewall.allowedTCPPorts = lib.mkIf (local.enable && local.shareOnLan) [ local.port ];

  systemd.services.losos-nix-cache = lib.mkIf local.enable {
    description = "Local copy of the Nix binary caches";
    wantedBy = [ "multi-user.target" ];
    after = [
      "network.target"
      "nss-lookup.target"
    ];
    wants = lib.optional enabled "losos-ghcr-cache.service";
    # /var/cache must be the persisted one, not the tmpfs root's.
    unitConfig.RequiresMountsFor = [ dir ];
    serviceConfig = {
      ExecStart = "${pkgs.nginx}/bin/nginx -c ${nginxConf} -e stderr";
      ExecReload = "${pkgs.coreutils}/bin/kill -HUP $MAINPID";
      Restart = "on-failure";
      RestartSec = 3;
      User = "losos-nix-cache";
      Group = "losos-nix-cache";
      CacheDirectory = [
        "losos-nix-cache"
        "losos-nix-cache/store"
        "losos-nix-cache/tmp"
      ];
      CacheDirectoryMode = "0700";
      RuntimeDirectory = "losos-nix-cache";
      UMask = "0077";
      NoNewPrivileges = true;
      ProtectSystem = "strict";
      ProtectHome = true;
      PrivateTmp = true;
      PrivateDevices = true;
      ProtectKernelTunables = true;
      ProtectKernelModules = true;
      ProtectKernelLogs = true;
      ProtectControlGroups = true;
      ProtectClock = true;
      ProtectHostname = true;
      ProtectProc = "invisible";
      RestrictNamespaces = true;
      RestrictRealtime = true;
      RestrictSUIDSGID = true;
      LockPersonality = true;
      MemoryDenyWriteExecute = true;
      CapabilityBoundingSet = "";
      RestrictAddressFamilies = [
        "AF_UNIX"
        "AF_INET"
        "AF_INET6"
      ];
      SystemCallArchitectures = "native";
      SystemCallFilter = [
        "@system-service"
        "~@privileged"
      ];
    };
  };
}
