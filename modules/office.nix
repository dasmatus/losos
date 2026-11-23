# Native LosOS Office.
#
# Collabora Online / WOPI integration has been removed. Native Office
# handles `.md`, `.paper`, and `.slides` in the web app; legacy formats
# are imported into native formats. Cloud mode serves office capabilities
# through LosOS Cloud itself. A configured `losos.office.folder` runs the
# standalone folder host.
{
  config,
  lib,
  ...
}:

let
  cfg = config.losos.office;
  pkg = config.losos.backend.package;
  standalone = cfg.enable && cfg.folder != null && pkg != null;
  socket = "/run/losos-office/http.sock";
  stateDir = "/var/lib/losos-office";
  lanOnly = import ./lan-only.nix;
in
{
  config = lib.mkIf standalone {
    users.users.losos-office = {
      isSystemUser = true;
      group = "losos-office";
      home = stateDir;
    };
    users.groups.losos-office = { };
    users.users.nginx.extraGroups = [ "losos-office" ];

    systemd.services.losos-office = {
      description = "Native LosOS Office folder host";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];
      serviceConfig = {
        User = "losos-office";
        Group = "losos-office";
        ExecStart = lib.escapeShellArgs [
          "${pkg}/bin/losos-office"
          "--root"
          cfg.folder
          "--socket"
          socket
        ];
        StateDirectory = "losos-office";
        StateDirectoryMode = "0700";
        RuntimeDirectory = "losos-office";
        RuntimeDirectoryMode = "0750";
        ReadWritePaths = [ cfg.folder ];
        Restart = "on-failure";
        UMask = "0027";
        NoNewPrivileges = true;
        CapabilityBoundingSet = "";
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_UNIX"
        ];
        RestrictNamespaces = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
      };
    };

    systemd.tmpfiles.rules = [ "d ${cfg.folder} 0750 losos-office losos-office -" ];

    services.nginx.virtualHosts."losos-front".locations = {
      "/office/api/" = {
        proxyPass = "http://unix:${socket}";
        recommendedProxySettings = false;
        extraConfig = ''
          ${lanOnly}
          proxy_set_header Host $host;
          proxy_set_header X-Losos-Cloud-User "";
          proxy_set_header X-Losos-Cloud-Name "";
        ''
        + lib.optionalString config.losos.identity.enable ''
          auth_request /internal/losos-office-authz;
          auth_request_set $losos_office_user $upstream_http_remote_user;
          auth_request_set $losos_office_name $upstream_http_remote_name;
          proxy_set_header X-Losos-Cloud-User $losos_office_user;
          proxy_set_header X-Losos-Cloud-Name $losos_office_name;
        '';
      };
    }
    // lib.optionalAttrs config.losos.identity.enable {
      "= /internal/losos-office-authz" = {
        proxyPass = "http://127.0.0.1:${toString config.losos.identity.port}/auth/api/authz/auth-request";
        extraConfig = ''
          internal;
          client_max_body_size 0;
          proxy_pass_request_body off;
          proxy_set_header Content-Length "";
          proxy_set_header X-Original-Method $request_method;
          proxy_set_header X-Original-URL https://$host$request_uri;
          proxy_set_header X-Forwarded-For $remote_addr;
        '';
      };
    };
  };
}
