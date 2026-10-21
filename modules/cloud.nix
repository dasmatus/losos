# LosOS Cloud: people's files, in the browser at /cloud/ and to the
# Nextcloud desktop and phone apps and file managers over the addresses
# they already know (backend/src/cloud/, the losos-cloud binary).
#
#   /cloud/         the web app (admin-ui/app/cloud/), behind LosOS Login
#   /cloud/api/     its JSON API
#   /status.php, /remote.php/, /ocs/, /index.php/, /public.php/
#                   what the Nextcloud apps call: discovery, sign-in (Login
#                   Flow v2, which opens /cloud/connect/<token>), WebDAV with
#                   chunked uploads, trash and versions
#
# nginx is the only way in. The service listens on a Unix socket only
# nginx's group may open, which is what makes the person nginx names in
# X-Losos-Cloud-User believable: LosOS Login's (modules/identity.nix) for
# the page and its API, lososd's check of a LosOS Login password on the LAN
# for WebDAV (backend/src/cloud_share.rs). The apps sign in with a password
# of their own per device, which the service checks itself, from the LAN
# and through the tunnel alike.
#
# Files live under /var/lib/losos-cloud/<login>/files, persisted with /var.
# The owner is `notshared`, the account that holds their files in the LosOS
# cloud pod too (modules/app-sign-in.nix), so `losos-cloud import` can move
# them over unchanged. nix/tests/cloud.nix boots it.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.cloud;
  enabled = config.lososInternal.appSignIn.cloud;
  pkg = config.losos.backend.package;
  signIn = import ./app-sign-in.nix { inherit lib; };

  stateDir = "/var/lib/losos-cloud";
  runDir = "/run/losos-cloud";
  socket = "${runDir}/http.sock";
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

  # Uploads of any size, streamed, and sync runs that take all day.
  files = ''
    client_max_body_size 0;
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

    systemd.services.losos-cloud = {
      description = "LosOS Cloud";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];
      before = [ "nginx.service" ];
      serviceConfig = {
        ExecStart = lib.concatStringsSep " " ([
          "${pkg}/bin/losos-cloud"
          "--root ${stateDir}"
          "serve"
          "--socket ${socket}"
          "--trash-days ${toString cfg.trashDays}"
        ]
        # Files the Windows share writes, into the table as they land
        # (modules/windows.nix).
        ++ lib.optional cfg.windows.enable "--watch");
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
        extraConfig = ''
          ${forwarded}
          ${passSignIn "/internal/losos-app-authz"}
          ${files}
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

    # The share in a file manager's Network view, while the LosOS cloud pod
    # does not announce its own (modules/containers.nix).
    losos.mdns.services = lib.mkIf (config.losos.nextcloud.mode != "container") {
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
