# Calls in LosOS chat: losos-calls, the box's own SFU (backend/src/calls/),
# and the one nginx location the chat page reaches it on.
#
# Calls belong to the shared side, like chat itself: they exist only while
# chat runs (lososInternal.chat: the box shares its storage in a mesh it has
# joined, behind LosOS Login) and losos.calls.enable is on. Otherwise there
# is no unit, no open port and no /calls/api/, and the chat page, which asks
# /calls/api/health once, draws no call button.
#
# The SFU forwards frames it cannot read. Every frame is encrypted in the
# sender's browser (admin-ui/app/src/calls/e2ee.ts) with keys the members
# hand each other over Olm, so the SFU, the box and the network see only
# ciphertext. The SFU only decides who is in which call: nginx asks LosOS
# Login before every request and hands it the person as Remote-User, whose
# Matrix id is @<name>:<server name>, and that person joins only a call
# their room was invited to. The SFU admits only people of this box, so a
# Matrix OpenID token would only repeat what LosOS Login already said.
#
# It runs as `shared`, keeps nothing on disk, and answers HTTP on a unix
# socket only nginx can open, so a Remote-User it reads is nginx's.
#
# Where the media goes depends on the edge. A box that tunnels through one
# (losos.proxy.enable) opens no port for calls either: losos-calls keeps one
# QUIC connection to the edge the tunnel uses, the edge gives it a UDP port
# there, and the browsers are told the edge's addresses
# (edge/registrar/src/calls_relay.rs). The edge's certificate is pinned by
# the hash `GET /calls/relay` gives over the registrar URL the tunnel and
# the announce already trust, and the box proves itself with the same token.
# The unit follows the tunnel: lososd's restarts of losos-tunnel-client
# when it picks another edge reach it through PartOf, and it does not run
# while lososd finds no edge. Without the proxy, media is UDP on
# losos.calls.udpPort, on the box's own addresses.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.calls;
  live = cfg.enable && config.lososInternal.chat;
  pkg = config.losos.backend.package;
  chat = config.losos.chat;
  identityPort = config.losos.identity.port;
  lanOnly = import ./lan-only.nix;
  runDir = "/run/losos-calls";
  socket = "${runDir}/http.sock";
  tap = config.lososInternal.callsTap;
  # The account the shared side runs as (lososInternal.sideUser).
  user = if config.lososInternal.sideUser == null then "shared" else config.lososInternal.sideUser;
  proxy = config.losos.proxy;
  throughEdge = proxy.enable;
  edgePathEnv = "/run/losos/edge-path.env";
  edgeNone = "/run/losos/edge-none";

  common = [
    "--listen"
    "unix:${socket}"
    "--server-name"
    chat.serverName
    # Podcast mode: a member's recording of the call, to an RTMP address.
    "--ffmpeg"
    "${pkgs.ffmpeg-headless}/bin/ffmpeg"
  ]
  ++ lib.optionals (tap != null) [
    "--tap"
    tap
  ];

  # Ask the edge lososd chose for its relay, then become losos-calls.
  throughEdgeStart = pkgs.writeShellScript "losos-calls-through-edge" ''
    set -eu
    registrar="''${LOSOS_EDGE_PATH_URL:-${proxy.registrarUrl}}"
    info="$(${pkgs.curl}/bin/curl -fsS --max-time 20 "$registrar/calls/relay")"
    port="$(printf '%s' "$info" | ${pkgs.jq}/bin/jq -er .port)"
    cert="$(printf '%s' "$info" | ${pkgs.jq}/bin/jq -er .certSha256)"
    host="''${registrar#*://}"
    host="''${host%%/*}"
    case "$host" in
      \[*) host="''${host%%]*}]" ;;
      *) host="''${host%:*}" ;;
    esac
    exec ${pkg}/bin/losos-calls ${lib.escapeShellArgs common} \
      --relay "$host:$port" --relay-cert "$cert" \
      --relay-id ${lib.escapeShellArg proxy.applianceId} \
      --relay-token-file "$CREDENTIALS_DIRECTORY/edge-token" \
      --registrar "$registrar" --curl ${pkgs.curl}/bin/curl
  '';
in
{
  config = lib.mkIf live {
    users.groups.losos-calls = { };
    # nginx opens the socket; nobody else is in the group.
    users.users.nginx.extraGroups = [ "losos-calls" ];

    systemd.services.losos-calls = {
      description = "LosOS calls (the SFU behind chat's calls), as the ${user} user";
      wantedBy = [ "multi-user.target" ] ++ lib.optional throughEdge "losos-tunnel-client.service";
      partOf = lib.optional throughEdge "losos-tunnel-client.service";
      after = [
        "network-online.target"
        "losos-chat.service"
      ]
      ++ lib.optional throughEdge "losos-tunnel-client.service";
      wants = [ "network-online.target" ];
      unitConfig = lib.optionalAttrs throughEdge {
        ConditionPathExists = "!${edgeNone}";
      };
      serviceConfig = {
        Type = "simple";
        User = user;
        Group = "losos-calls";
        ExecStart =
          if throughEdge then
            throughEdgeStart
          else
            lib.escapeShellArgs (
              [
                "${pkg}/bin/losos-calls"
                "--udp-port"
                (toString cfg.udpPort)
              ]
              ++ common
              ++ lib.concatMap (a: [
                "--address"
                a
              ]) config.lososInternal.callsAddresses
            );
        RuntimeDirectory = "losos-calls";
      }
      // lib.optionalAttrs throughEdge {
        EnvironmentFile = "-${edgePathEnv}";
        LoadCredential = "edge-token:${toString proxy.tokenFile}";
      }
      // {
        RuntimeDirectoryMode = "0750";
        Restart = "on-failure";
        RestartSec = "5s";
        MemoryMax = "512M";
        ProtectHome = true;
        ProtectSystem = "strict";
        ReadWritePaths = lib.optional (tap != null) tap;
        PrivateTmp = true;
        PrivateDevices = true;
        NoNewPrivileges = true;
        CapabilityBoundingSet = "";
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
        # AF_NETLINK: the box's addresses, for the ICE candidates.
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
          "AF_NETLINK"
        ];
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
        UMask = "0007";
      };
    };

    systemd.tmpfiles.rules = lib.optional (tap != null) "d ${tap} 0700 ${user} losos-calls - -";

    networking.firewall.allowedUDPPorts = lib.optional (!throughEdge) cfg.udpPort;

    services.nginx.virtualHosts."losos-front".locations = {
      # Creating a call and its signalling socket. LosOS Login first, with
      # no spare-key bypass: a call is a person's, never the box's.
      "/calls/api/" = {
        proxyPass = "http://unix:${socket}";
        proxyWebsockets = true;
        recommendedProxySettings = false;
        extraConfig = ''
          ${lanOnly}
          auth_request /internal/losos-calls-authz;
          # The person LosOS Login signed in, the only one the SFU takes
          # (backend/src/calls/person.rs). Set on every request, so a
          # client's own Remote-User never reaches it.
          auth_request_set $losos_calls_user $upstream_http_remote_user;
          proxy_set_header Host $host;
          proxy_set_header Remote-User $losos_calls_user;
          proxy_set_header Remote-Groups "";
          proxy_set_header X-Losos-As "";
          client_max_body_size 64k;
          # A call's socket is quiet between renegotiations.
          proxy_read_timeout 1h;
          proxy_send_timeout 1h;
        '';
      };
      "= /internal/losos-calls-authz" = {
        proxyPass = "http://127.0.0.1:${toString identityPort}/auth/api/authz/auth-request";
        extraConfig = ''
          internal;
          proxy_intercept_errors on;
          error_page 500 502 503 504 = @losos-calls-authz-down;
          # The request's body never comes here, but its length is held to
          # this location's limit first (modules/identity.nix, askAboutRequest).
          client_max_body_size 0;
          proxy_pass_request_body off;
          proxy_set_header Content-Length "";
          proxy_set_header X-Original-Method $request_method;
          proxy_set_header X-Original-URL https://$host$request_uri;
          proxy_set_header X-Forwarded-For $remote_addr;
        '';
      };
      "@losos-calls-authz-down".extraConfig = "return 401;";
    }
    // lib.optionalAttrs (throughEdge && config.losos.admin.ui != null) {
      # A guest's page and socket, the only part of calls that answers
      # without LosOS Login and off the LAN: the edge carries the guest
      # link's host to the box through the tunnel. The page is static, the
      # socket admits a guest only with an invite for that one call
      # (backend/src/calls/guests.rs), and nothing here reaches chat.
      "/calls/guest/".tryFiles = "$uri /calls/guest/guest.html";
      # The segmenter for the black background, from the chat page's copy.
      "/calls/guest/chat/calls-ml/".alias = "${config.losos.admin.ui}/chat/calls-ml/";
      "= /calls/guest/ws" = {
        proxyPass = "http://unix:${socket}";
        proxyWebsockets = true;
        recommendedProxySettings = false;
        extraConfig = ''
          proxy_set_header Host $host;
          proxy_set_header Remote-User "";
          proxy_set_header Remote-Groups "";
          proxy_set_header X-Losos-As "";
          client_max_body_size 64k;
          proxy_read_timeout 1h;
          proxy_send_timeout 1h;
        '';
      };
    };
  };
}
