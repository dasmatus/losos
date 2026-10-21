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
# ciphertext. The SFU only decides who is in which call: a person proves who
# they are with a Matrix OpenID token, which it checks with Dendrite on
# loopback, and joins only a call their room was invited to.
#
# It runs as `shared`, which may open Dendrite's port (modules/chat.nix's
# owner rule), keeps nothing on disk, and answers HTTP on a unix socket only
# nginx can open. Media is UDP on losos.calls.udpPort, on the box's own
# addresses.
{
  config,
  lib,
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
in
{
  config = lib.mkIf live {
    users.groups.losos-calls = { };
    # nginx opens the socket; nobody else is in the group.
    users.users.nginx.extraGroups = [ "losos-calls" ];

    systemd.services.losos-calls = {
      description = "LosOS calls (the SFU behind chat's calls), as the shared user";
      wantedBy = [ "multi-user.target" ];
      after = [
        "network-online.target"
        "dendrite.service"
      ];
      wants = [ "network-online.target" ];
      serviceConfig = {
        Type = "simple";
        User = "shared";
        Group = "losos-calls";
        ExecStart = lib.escapeShellArgs (
          [
            "${pkg}/bin/losos-calls"
            "--listen"
            "unix:${socket}"
            "--udp-port"
            (toString cfg.udpPort)
            "--server-name"
            chat.serverName
            "--homeserver"
            "127.0.0.1:${toString chat.port}"
          ]
          ++ lib.concatMap (a: [
            "--address"
            a
          ]) config.lososInternal.callsAddresses
          ++ lib.optionals (tap != null) [
            "--tap"
            tap
          ]
        );
        RuntimeDirectory = "losos-calls";
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

    systemd.tmpfiles.rules = lib.optional (tap != null) "d ${tap} 0700 shared losos-calls - -";

    networking.firewall.allowedUDPPorts = [ cfg.udpPort ];

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
          proxy_set_header Host $host;
          proxy_set_header Remote-User "";
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
          proxy_pass_request_body off;
          proxy_set_header Content-Length "";
          proxy_set_header X-Original-Method $request_method;
          proxy_set_header X-Original-URL https://$host$request_uri;
          proxy_set_header X-Forwarded-For $remote_addr;
        '';
      };
      "@losos-calls-authz-down".extraConfig = "return 401;";
    };
  };
}
