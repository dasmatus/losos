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
  mesh = cfg.mesh;
  enabled = config.lososInternal.appSignIn.cloud;
  pkg = config.losos.backend.package;
  # The same gate as the shared side's chat, mail and calls.
  knowledgeBases = config.losos.sharingMyStorage && config.losos.cluster.enable;
  # Office edits the cloud's files unless it was given a folder of its own.
  office = config.losos.office.enable && config.losos.office.folder == null;
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
    proxy_set_header X-Losos-Cloud-Login $losos_user;
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
        ExecStart = lib.concatStringsSep " " [
          "${pkg}/bin/losos-cloud"
          "--root ${stateDir}"
          "serve"
          "--socket ${socket}"
          "--trash-days ${toString cfg.trashDays}"
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
        );
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

    # The share in a file manager's Network view, while the LosOS Cloud pod
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
