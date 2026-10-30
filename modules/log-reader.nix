# Settings › Logs: the journal of the box's services in the admin pages,
# so nobody has to sign in to the box to see why something failed.
#
# The reader (`losos-logs`, backend/src/logs/) runs apart from lososd, as
# its own user in the systemd-journal group, on a Unix socket only root and
# nginx (its group) may open. nginx sends it /api/logs (the script page,
# after LosOS Login like the rest of /api/) and /basic/settings/logs (the
# page without JavaScript), so the page keeps answering while lososd is
# down or restarting, which is when the owner needs it. That page draws
# its own spare-key sign-in for the same reason: lososd draws the others.
#
# Who may read is the reader's call: the spare key (the credential lososd
# reads, `admin-token`), or a person LosOS Login puts in losos-admins.
# Every line is redacted in the reader before it is shown, searched,
# downloaded or handed to lososd's problem reports, which ask for a few
# lines at /internal/logs/excerpt over the socket; nginx never forwards
# that route (the reader refuses any request that carries X-Losos-Front).
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.logReader;
  pkg = config.losos.backend.package;
  ui = config.losos.admin.ui;
  enabled = cfg.enable && pkg != null && config.losos.admin.enable && ui != null;
  identity = config.losos.identity.enable;

  runDir = "/run/losos-logs";
  socket = "${runDir}/http.sock";
  upstream = "http://unix:${socket}";
  lanOnly = import ./lan-only.nix;

  # The Service list, in its order, with only the units this box runs.
  # A glob is a unit made at run time (a rebuild, an app, a backup).
  sources = [
    {
      id = "lososd";
      units = [ "lososd.service" ];
    }
    {
      id = "rebuilds";
      units = [
        "losos-rebuild-*.service"
        "nixos-upgrade.service"
      ];
    }
    {
      id = "login";
      units = [
        "authelia-losos.service"
        "losos-identity-setup.service"
      ];
    }
    {
      id = "web";
      units = [ "nginx.service" ];
    }
    {
      id = "cloud";
      units = [
        "losos-cloud.service"
        "losos-cloud-photos.service"
        "losos-cloud-from-nextcloud.service"
        "losos-search-index.service"
      ];
    }
    {
      id = "git";
      units = [
        "losos-git.service"
        "losos-git-forgejo-import.service"
        "losos-git-boards-import.service"
      ];
    }
    {
      id = "office";
      units = [
        "losos-office.service"
        "losos-office-discovery.service"
        "coolwsd.service"
      ];
    }
    {
      id = "mail";
      units = [
        "losos-mail.service"
        "losos-mail-dir.service"
      ];
    }
    {
      id = "chat";
      units = [
        "losos-chat.service"
        "losos-chat-setup.service"
      ];
    }
    {
      id = "calls";
      units = [ "losos-calls.service" ];
    }
    {
      id = "apps";
      units = [
        "losos-traefik.service"
        "losos-podman-api.service"
        "losos-podman-apps.service"
        "losos-app-routes.service"
        "losos-app-*.service"
        "losos-podman-app@*.service"
      ];
    }
    {
      id = "ci";
      units = [
        "losos-ci.service"
        "losos-ci-runner.service"
      ];
    }
    {
      id = "edge";
      units = [
        "losos-tunnel-client.service"
        "losos-tunnel-uplink.service"
        "losos-registrar-announce.service"
      ];
    }
    {
      id = "mesh";
      units = [
        "k3s.service"
        "rke2-agent.service"
        "losos-mesh-join.service"
      ];
    }
    {
      id = "backup";
      units = [ "losos-backup-*.service" ];
    }
    {
      id = "boot";
      units = [
        "losos-credentials.service"
        "losos-console.service"
        "losos-fscrypt-setup.service"
        "losos-factory-wipe.service"
        "losos-tls-cert.service"
      ];
    }
    {
      id = "logs";
      units = [ "losos-logs.service" ];
    }
    {
      id = "kernel";
      kernel = true;
    }
  ];
  present =
    unit: lib.hasInfix "*" unit || config.systemd.services ? ${lib.removeSuffix ".service" unit};
  catalogue = {
    sources = lib.filter (s: s.kernel or false || s.units != [ ]) (
      map (s: s // { units = lib.filter present (s.units or [ ]); }) sources
    );
  };
  unitsFile = pkgs.writeText "log-units.json" (builtins.toJSON catalogue);

  # Who LosOS Login says is asking, never a refusal: a visitor it does not
  # know, and a LosOS Login that is down, go on as nobody, and the reader
  # shows its own sign-in. A Bearer header (the spare key, or the cookie
  # the page without JavaScript keeps it in) skips the question.
  askLogin = ''
    auth_request /internal/losos-logs-authz;
    auth_request_set $losos_user $upstream_http_remote_user;
    auth_request_set $losos_groups $upstream_http_remote_groups;
    proxy_set_header Remote-User $losos_user;
    proxy_set_header Remote-Groups $losos_groups;
  '';
  # The question the rest of /api/ asks (modules/identity.nix): no answer
  # from LosOS Login is a 401, which the script page turns into its sign-in.
  askAuthelia = ''
    auth_request /internal/losos-authz;
    auth_request_set $losos_user $upstream_http_remote_user;
    auth_request_set $losos_groups $upstream_http_remote_groups;
    proxy_set_header Remote-User $losos_user;
    proxy_set_header Remote-Groups $losos_groups;
  '';
  noPerson = ''
    proxy_set_header Remote-User "";
    proxy_set_header Remote-Groups "";
  '';
  toReader = ''
    ${lanOnly}
    proxy_set_header X-Losos-Front 1;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Losos-As "";
    proxy_set_header X-Losos-Via-Edge "";
    proxy_set_header X-Losos-Edge "";
    # A week of the whole journal searched for a word takes a while.
    proxy_read_timeout 120s;
  '';
in
{
  config = lib.mkIf enabled {
    users.users.losos-logs = {
      isSystemUser = true;
      group = "losos-logs";
      # Every service's journal, and the user journals.
      extraGroups = [ "systemd-journal" ];
    };
    users.groups.losos-logs = { };
    users.users.nginx.extraGroups = [ "losos-logs" ];

    # The spare key, to check it and to hide it.
    losos.credentials.entries.admin-token.services = [ "losos-logs" ];

    environment.etc."losos/log-units.json".source = unitsFile;

    systemd.services.losos-logs = {
      description = "The box's journal for the admin pages (Settings › Logs)";
      wantedBy = [ "multi-user.target" ];
      # Up early and on its own: nothing it needs is another LosOS unit.
      after = [ "systemd-journald.service" ];
      before = [ "nginx.service" ];
      restartTriggers = [ unitsFile ];
      serviceConfig = {
        User = "losos-logs";
        Group = "losos-logs";
        ExecStart = lib.escapeShellArgs (
          [
            "${pkg}/bin/losos-logs"
            "--socket"
            socket
            "--units"
            "/etc/losos/log-units.json"
            "--token-file"
            "%d/admin-token"
            "--lososd"
            "127.0.0.1:${toString config.losos.admin.apiPort}"
            "--ui"
            "${ui}"
            "--journalctl"
            "${config.systemd.package}/bin/journalctl"
          ]
          ++ lib.optional identity "--identity"
          ++ lib.optional (config.losos.office.enable && config.losos.office.folder != null) "--office"
        );
        RuntimeDirectory = "losos-logs";
        RuntimeDirectoryMode = "0750";
        Restart = "always";
        RestartSec = 2;
        UMask = "0007";
        NoNewPrivileges = true;
        CapabilityBoundingSet = "";
        AmbientCapabilities = "";
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
        RestrictAddressFamilies = [
          "AF_UNIX"
          "AF_INET"
        ];
        # lososd's loopback port, nothing else.
        IPAddressDeny = "any";
        IPAddressAllow = "localhost";
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
        MemoryMax = "256M";
      };
    };

    services.nginx.virtualHosts."losos-front".locations =
      # The script page's lines and download. After LosOS Login, like the
      # rest of /api/ (modules/identity.nix, `askAuthelia`), which still
      # answers while lososd does not.
      lib.genAttrs [ "= /api/logs" "= /api/logs/download" ] (_: {
        proxyPass = upstream;
        extraConfig = ''
          ${toReader}
          ${if identity then askAuthelia else noPerson}
        '';
      })
      // {
        # The page without JavaScript, its download and its spare-key
        # sign-in. https, as every page under /basic/ (LosOS Login's cookie
        # and the spare key's are Secure).
        "= /basic/settings/logs" = {
          proxyPass = upstream;
          extraConfig = ''
            if ($losos_proto = http) {
              return 301 https://$host$request_uri;
            }
            ${toReader}
            ${if identity then askLogin else noPerson}
            proxy_set_header Authorization $losos_basic_bearer;
            proxy_set_header X-Losos-Basic-Prefs $cookie_losos_basic;
          '';
        };
        "^~ /basic/settings/logs/" = {
          proxyPass = upstream;
          extraConfig = ''
            if ($losos_proto = http) {
              return 301 https://$host$request_uri;
            }
            ${toReader}
            ${if identity then askLogin else noPerson}
            proxy_set_header Authorization $losos_basic_bearer;
            proxy_set_header X-Losos-Basic-Prefs $cookie_losos_basic;
            client_max_body_size 4k;
          '';
        };
        # Nothing the reader serves under /internal/ is for nginx to pass on.
        "^~ /internal/logs/".return = "404";
      }
      // lib.optionalAttrs identity {
        "= /internal/losos-logs-authz" = {
          proxyPass = "http://127.0.0.1:${toString config.losos.identity.port}/auth/api/authz/auth-request";
          extraConfig = ''
            internal;
            if ($http_authorization ~* "^bearer ") {
              return 200;
            }
            if ($cookie_losos_spare) {
              return 200;
            }
            proxy_intercept_errors on;
            error_page 401 403 500 502 503 504 = @losos-logs-nobody;
            proxy_pass_request_body off;
            proxy_set_header Content-Length "";
            proxy_set_header X-Original-Method $request_method;
            proxy_set_header X-Original-URL https://$host$request_uri;
            proxy_set_header X-Forwarded-For $remote_addr;
          '';
        };
        "@losos-logs-nobody".extraConfig = "return 200;";
      };
  };
}
