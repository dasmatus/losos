# Office documents in the browser: Collabora Online (coolwsd) on loopback,
# behind the front vhost, reading and saving through a WOPI host
# (backend/src/office/).
#
#   browser ── /browser/…  /cool/…ws ──► nginx ──► coolwsd 127.0.0.1:9980
#                                                     │ WOPI, server side
#                                                     ▼
#                                    WOPI host 127.0.0.1:8093 (/wopi/files/)
#                                    LosOS Cloud, or losos-office over a folder
#
# What is load-bearing here:
#
#   * coolwsd and the WOPI host listen on 127.0.0.1 only, and nginx routes
#     neither /wopi/ nor /hosting/discovery. The browser hands its access
#     token to coolwsd; only coolwsd calls the host. alias_groups names the
#     one WOPI host coolwsd may fetch from, so a WOPISrc pointing anywhere
#     else (another port on the box, a LAN address) is refused.
#   * /browser/ and /cool/ are public wherever LosOS Cloud is (the LAN, the
#     tunnel, a share opened from another box): the token is the
#     authentication, and nginx refuses the three coolwsd endpoints that are
#     not about a document (the admin console, metrics, convert-to).
#   * coolwsd sends its own Content-Security-Policy with `frame-ancestors`,
#     so the admin pages' policy and X-Frame-Options: DENY must not ride on
#     these two prefixes (modules/containers.nix, the header map).
#   * TLS ends at nginx (`ssl.termination`): coolwsd speaks http on loopback
#     and writes https addresses into its pages, so the editor works on the
#     box's https address only (the one LosOS Login needs anyway).
#   * Nothing phones home: no update check, no Zotero, no remote fonts or
#     configuration, no LanguageTool or DeepL.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.office;
  pkg = config.losos.backend.package;
  standalone = cfg.folder != null && pkg != null;
  # Without a folder of its own, Office edits LosOS Cloud's files: the cloud
  # is the WOPI host (backend/src/cloud/office.rs).
  inCloud = cfg.folder == null && pkg != null && config.lososInternal.appSignIn.cloud;

  coolwsd = "http://127.0.0.1:${toString cfg.port}";
  runDir = "/run/losos-office";
  socket = "${runDir}/http.sock";
  stateDir = "/var/lib/losos-office";

  lanOnly = import ./lan-only.nix;
  themes = import ../admin-ui/themes { inherit pkgs; };

  toCoolwsd = ''
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Proto $scheme;
  '';
in
{
  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        services.collabora-online = {
          enable = true;
          inherit (cfg) port;
          settings = {
            # TLS ends at nginx. With the box's https on, coolwsd writes https
            # and wss addresses into its page, and Documents moves itself to
            # https before it opens the editor.
            ssl = {
              enable = false;
              termination = config.losos.tls.enable;
            };
            net = {
              proto = "IPv4";
              listen = "loopback";
              # Only nginx calls coolwsd, and it does so from loopback.
              post_allow.host = [ "127\\.0\\.0\\.1" ];
              lok_allow.host = [ "127\\.0\\.0\\.1" ];
              # The page that frames the editor is on the box's own origin.
              content_security_policy = "frame-ancestors 'self'";
            };
            storage.wopi = {
              "@allow" = true;
              alias_groups = {
                "@mode" = "groups";
                group = [
                  {
                    host = {
                      "@allow" = true;
                      "+content" = "http://127.0.0.1:${toString cfg.wopiPort}";
                    };
                  }
                ];
              };
            };
            admin_console.enable = false;
            # Macros in a .docm, .xlsm or .pptm never run here, whoever
            # wrote them. coolwsd's default, pinned.
            security.enable_macros_execution = false;
            # One document process kept warm, not four: the first person to
            # open a document waits a second less, and every one kept costs
            # memory on a small box.
            num_prespawn_children = 1;
            fetch_update_check = 0;
            allow_update_popup = false;
            zotero.enable = false;
            welcome.enable = false;
            # The languages the handbook is written in, for spelling.
            allowed_languages = "en_GB en_US de_DE sk_SK";
            logging.level = "warning";
          };
        };

        systemd.services.coolwsd.serviceConfig = {
          # Documents are unloaded when it reaches this; the box's own
          # services come first.
          MemoryHigh = "1500M";
          MemoryMax = "2G";
        };

        services.nginx.virtualHosts."losos-front".locations = {
          # The editor page, its scripts, styles and pictures.
          "^~ /browser/" = {
            proxyPass = coolwsd;
            recommendedProxySettings = false;
            # Every editor page links /browser/<version>/branding.css, which
            # coolwsd does not ship: the box answers it with the LosOS look
            # (admin-ui/themes/office/branding.css).
            extraConfig = ''
              ${toCoolwsd}
              location ~ ^/browser/[^/]+/branding\.css$ {
                alias ${themes.office.branding};
                default_type text/css;
              }
            '';
          };
          # The admin console's pages, which are off anyway.
          "^~ /browser/dist/admin/".return = "404";
          # The document's own connection, and downloads, uploads and the
          # clipboard of an open document.
          "^~ /cool/" = {
            proxyPass = coolwsd;
            proxyWebsockets = true;
            recommendedProxySettings = false;
            extraConfig = ''
              ${toCoolwsd}
              client_max_body_size 1g;
              proxy_read_timeout 36000s;
              proxy_send_timeout 36000s;
            '';
          };
          # Not about any one document: refused, whoever asks.
          "^~ /cool/adminws".return = "404";
          "^~ /cool/getMetrics".return = "404";
          "^~ /cool/convert-to".return = "404";
          "= /hosting/capabilities" = {
            proxyPass = coolwsd;
            recommendedProxySettings = false;
            extraConfig = toCoolwsd;
          };
        };
      }

      (lib.mkIf inCloud {
        # LosOS Cloud runs with no network (modules/cloud.nix), so it can
        # neither ask coolwsd which types it opens nor take coolwsd's calls
        # itself. Both go through files and sockets instead:
        #
        #   * coolwsd's discovery document is saved here once it is up, and
        #     again whenever it restarts; the cloud reads the copy.
        #   * nginx listens on the WOPI port, loopback only, and relays
        #     /wopi/files/ and nothing else to the cloud's socket, with no
        #     person named. The access token is the whole authentication on
        #     that path, as it is on losos-office's own listener.
        systemd.services.losos-office-discovery = {
          description = "Collabora's discovery document, for LosOS Cloud";
          wantedBy = [ "coolwsd.service" ];
          after = [ "coolwsd.service" ];
          partOf = [ "coolwsd.service" ];
          path = [ pkgs.curl ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            DynamicUser = true;
            RuntimeDirectory = "losos-office-discovery";
            RuntimeDirectoryMode = "0755";
            UMask = "0022";
          };
          script = ''
            out=/run/losos-office-discovery/discovery.xml
            for _ in $(seq 1 150); do
              if curl -sf -o "$out.new" ${coolwsd}/hosting/discovery; then
                mv "$out.new" "$out"
                exit 0
              fi
              sleep 2
            done
            echo "coolwsd did not answer its discovery address" >&2
            exit 1
          '';
        };

        services.nginx.virtualHosts."losos-office-wopi" = {
          listen = [
            {
              addr = "127.0.0.1";
              port = cfg.wopiPort;
            }
          ];
          locations."/".return = "404";
          locations."/wopi/files/" = {
            proxyPass = "http://unix:/run/losos-cloud/http.sock";
            recommendedProxySettings = false;
            extraConfig = ''
              proxy_set_header Host 127.0.0.1;
              proxy_set_header X-Losos-Cloud-User "";
              proxy_set_header X-Losos-Cloud-Name "";
              proxy_set_header X-Losos-Cloud-Email "";
              client_max_body_size 1g;
              proxy_request_buffering off;
              proxy_read_timeout 600s;
            '';
          };
        };
      })

      (lib.mkIf standalone {
        users.users.losos-office = {
          isSystemUser = true;
          group = "losos-office";
          home = stateDir;
        };
        users.groups.losos-office = { };
        # nginx opens the editor-session socket, which is 0660 losos-office.
        users.users.nginx.extraGroups = [ "losos-office" ];

        systemd.services.losos-office = {
          description = "Office documents in a folder, for Collabora Online";
          wantedBy = [ "multi-user.target" ];
          after = [ "network.target" ];
          serviceConfig = {
            User = "losos-office";
            Group = "losos-office";
            ExecStart = lib.escapeShellArgs [
              "${pkg}/bin/losos-office"
              "--root"
              cfg.folder
              "--wopi"
              "127.0.0.1:${toString cfg.wopiPort}"
              "--socket"
              socket
              "--coolwsd"
              coolwsd
              "--key-file"
              "${stateDir}/token.key"
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

        # Documents' two calls (admin-ui/app/src/screens/Office.tsx, which the
        # admin app serves at /office/, LAN only like the rest of it), for
        # the signed-in person. LosOS Login names them; without it nobody is
        # named and the host answers 401.
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
        };
      })
    ]
  );
}
