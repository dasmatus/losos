# LosOS Chat: a Matrix homeserver (Synapse) on the box, the chat page at
# /chat/, and lososd between the two (backend/src/chat.rs).
#
# Chat belongs to the `shared` user. Synapse runs as it, and every room,
# message, upload and key it keeps is in the shared data domain
# (/home/shared/data/chat/synapse), which is unlocked only while the box
# shares its storage (modules/fscrypt.nix). So all of this exists only while
# losos.chat.enable is on and the box shares its storage in a mesh it has
# joined (lososInternal.sharedSide: losos.sharingMyStorage and
# losos.cluster.enable; lososInternal.chat): otherwise there is no homeserver, no nginx
# location for /chat/ or /_matrix/, and lososd is told nothing, so
# /api/me names no chat and the page draws no entry for it.
#
# Synapse because calls need two things only it has: sticky events
# (MSC4354, a call membership that ends with its sender's connection) and
# delayed events (MSC4140, the leave a crashed tab still sends). The box ran
# Dendrite before; its files stay in /home/shared/data/chat beside the new
# folder, unread.
#
# Nobody signs in to chat with a Matrix password; there are none. lososd is
# an application service whose namespace is every user on this server, and
# the only way an account is made or a device signed in
# (POST /api/chat/session, behind LosOS Login like every /api/ route). Then
# every request to the client and media APIs is asked about first
# (auth_request to lososd's /internal/chat/authz), which wants a live LosOS
# Login session, a person still on the box and not held, and a token of
# that same person. Signing out of LosOS Login ends chat at the next
# request; removing a person also signs their devices out of Synapse.
#
# Synapse answers on loopback only, and only root, nginx and `shared` may
# open that port (the same owner rule identity.nix puts on the apps). The
# client API is LAN-only, like the admin pages. With
# losos.chat.federation.enable (on by default) the federation and key APIs
# answer other servers too: from the LAN alone while the server name is a
# .local one, so boxes on one network talk to each other and nothing else
# reaches them; from anywhere for a name other servers can reach.
#
# LosOS Git rooms (backend/src/git_rooms.rs): lososd takes LosOS Git's
# events on a socket in /run/losos-git-chat, a directory only LosOS Git's
# group may enter, and posts those of a repository with a room into it.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.chat;
  live = config.lososInternal.chat;
  pkg = config.losos.backend.package;
  synapse = pkgs.matrix-synapse;
  inherit (cfg) port;
  sharedUid = 1001;

  # Dendrite's files are the rest of /home/shared/data/chat.
  chatDir = "/home/shared/data/chat";
  stateDir = "${chatDir}/synapse";
  secretsDir = "/var/secrets/losos-chat";
  credentials = "/run/credentials/losos-chat.service";
  asTokenFile = "${secretsDir}/as_token";
  inherit (cfg) serverName;
  federation = cfg.federation.enable;
  # A .local name is this network's: federate with the boxes on it alone.
  lanName = lib.hasSuffix ".local" serverName;
  gitEvents = config.losos.git.enable;
  gitEventsDir = "/run/losos-git-chat";
  identityPort = config.losos.identity.port;
  lanOnly = import ./lan-only.nix;
  privateRanges = [
    "10.0.0.0/8"
    "172.16.0.0/12"
    "192.168.0.0/16"
    "100.64.0.0/10"
    "fc00::/7"
  ];

  # The journal takes stderr.
  logConfig = (pkgs.formats.yaml { }).generate "losos-chat-log.yaml" {
    version = 1;
    formatters.plain.format = "%(name)s %(levelname)s %(message)s";
    handlers.stderr = {
      class = "logging.StreamHandler";
      formatter = "plain";
    };
    root = {
      level = "WARNING";
      handlers = [ "stderr" ];
    };
    disable_existing_loggers = false;
  };

  settings = {
    server_name = serverName;
    public_baseurl = "https://${serverName}/";
    listeners = [
      {
        inherit port;
        bind_addresses = [ "127.0.0.1" ];
        type = "http";
        tls = false;
        # nginx in front: the client's address is in X-Forwarded-For.
        x_forwarded = true;
        resources = [
          {
            # openid alone still answers lososd's own userinfo question.
            names = [ "client" ] ++ (if federation then [ "federation" ] else [ "openid" ]);
            compress = false;
          }
        ];
      }
    ];
    database = {
      name = "sqlite3";
      args.database = "${stateDir}/homeserver.db";
    };
    media_store_path = "${stateDir}/media";
    max_upload_size = "${toString cfg.maxUploadMiB}M";
    dynamic_thumbnails = false;
    url_preview_enabled = false;
    signing_key_path = "${credentials}/signing_key";
    macaroon_secret_key_path = "${credentials}/macaroon";
    app_service_config_files = [ "${credentials}/appservice" ];
    log_config = logConfig;
    report_stats = false;
    enable_metrics = false;
    enable_registration = false;
    # Accounts and devices come from lososd alone.
    password_config.enabled = false;
    allow_guest_access = false;
    # No identity servers: there is no mail or phone number to look up.
    enable_3pid_lookup = false;
    # Everyone on the box is someone to start a chat with.
    user_directory.search_all_users = true;
    # LosOS Chat shows no presence, and it is the most work Synapse does
    # for each sync; on a two-core box it slowed every call's membership.
    presence.enabled = false;
    # Calls (src/chat/rtc.ts): sticky memberships and the delayed leave.
    experimental_features = {
      msc4354_enabled = true;
      msc4222_enabled = true;
    };
    max_event_delay_duration = "24h";
    # A box has a handful of people, each with a few devices. The defaults
    # are for a public server and stall a busy call or a quick chat.
    rc_message = {
      per_second = 10;
      burst_count = 50;
    };
    rc_delayed_event_mgmt = {
      per_second = 2;
      burst_count = 20;
    };
    rc_joins.local = {
      per_second = 1;
      burst_count = 20;
    };
    caches = {
      global_factor = 0.5;
      # A page keeps its device (src/chat/session.ts), and Synapse keeps a
      # finished sync for two minutes by default and hands it to the same
      # device's next identical request: a reload within that time started
      # from the old first sync, without the rooms joined since.
      sync_response_cache_duration = 0;
    };
    # Only matrix.org's keys are trusted for other servers' keys, and only
    # when this box federates beyond its LAN: the boxes of a LAN hand out
    # their own keys.
    trusted_key_servers = lib.optionals (federation && !lanName) [
      { server_name = "matrix.org"; }
    ];
    suppress_key_server_warning = true;
    # Never fetch from this box itself on another server's say (Synapse's
    # own list covers loopback, link-local and the private ranges). A box
    # with a .local name federates with the LAN and nothing else (nginx
    # answers the LAN alone), so its LAN is allowed back in.
    ip_range_whitelist = lib.optionals lanName privateRanges;
    # The other boxes of a LAN answer with certificates they made
    # themselves; who each is rests on its server key either way.
    federation_verify_certificates = !config.losos.cloud.mesh.selfSigned;
    allow_public_rooms_over_federation = false;
  }
  // lib.optionalAttrs (!federation) {
    # No other server, either way.
    federation_domain_whitelist = [ ];
  };
  configFile = (pkgs.formats.yaml { }).generate "losos-chat.yaml" settings;

  # The application service lososd is (backend/src/chat.rs): every user on
  # this server is in its namespace, and it has no URL, so Synapse pushes it
  # nothing; it only signs people in. Not exclusive: Synapse leaves an
  # exclusive namespace's users out of its user directory, and nobody else
  # can register here anyway.
  serverRegex = lib.escapeRegex serverName;
  setup = pkgs.writeShellApplication {
    name = "losos-chat-setup";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.openssl
      synapse
    ];
    text = ''
      install -d -m 0700 ${secretsDir}
      if [ ! -s ${secretsDir}/signing.key ]; then
        generate_signing_key -o ${secretsDir}/signing.key.tmp
        mv ${secretsDir}/signing.key.tmp ${secretsDir}/signing.key
      fi
      for s in as_token hs_token macaroon; do
        if [ ! -s ${secretsDir}/$s ]; then
          openssl rand -hex 32 > ${secretsDir}/$s.tmp
          mv ${secretsDir}/$s.tmp ${secretsDir}/$s
        fi
      done
      umask 0077
      cat > ${secretsDir}/appservice.yaml.tmp <<EOF
      id: losos
      url: null
      as_token: "$(cat ${secretsDir}/as_token)"
      hs_token: "$(cat ${secretsDir}/hs_token)"
      sender_localpart: losos
      rate_limited: false
      namespaces:
        users:
          - exclusive: false
            regex: '@.*:${serverRegex}'
        aliases: []
        rooms: []
      EOF
      mv ${secretsDir}/appservice.yaml.tmp ${secretsDir}/appservice.yaml
      # The domain is unlocked (this unit runs after the unlock); the folders
      # are the shared user's, inside it.
      install -d -m 0700 -o shared -g shared ${chatDir} ${stateDir}
    '';
  };

  # Only these may open Synapse's port on loopback: root (lososd), nginx and
  # `shared` (Synapse itself). modules/daemon.nix says why an owner match.
  loopbackRule = lib.concatStringsSep " " (
    [
      "OUTPUT -o lo -p tcp -d 127.0.0.1 --dport ${toString port}"
      "-m owner ! --uid-owner 0"
      "-m owner ! --uid-owner ${toString sharedUid}"
    ]
    ++ lib.optional config.services.nginx.enable "-m owner ! --uid-owner ${config.services.nginx.user}"
    ++ [ "-j REJECT --reject-with tcp-reset" ]
  );

  toSynapse = ''
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Proto $scheme;
    # /sync waits up to 30 s for something to happen.
    proxy_read_timeout 120s;
    proxy_buffering off;
  '';
  # Every request of the client and media APIs, asked about first.
  gated = ''
    ${lanOnly}
    auth_request /internal/losos-chat-authz;
    error_page 401 = @losos-chat-sign-in;
    ${toSynapse}
  '';
  # An add_header at location level drops every one the server sets, so the
  # few answers nginx writes itself repeat them all (identity.nix's
  # pageHeaders, without a style to allow).
  noStore = ''
    add_header Content-Security-Policy "default-src 'none'; frame-ancestors 'none'" always;
    add_header Cache-Control "no-store" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;
    add_header Referrer-Policy "no-referrer" always;
  '';
  json = body: ''
    default_type application/json;
    ${noStore}
    return 200 '${builtins.toJSON body}';
  '';
in
{
  config = lib.mkMerge [
    # Off: the page is still in the admin UI's bundle, so nginx sends
    # /chat/ home rather than serve a page with nothing behind it, and
    # answers the homeserver's paths with a plain 404.
    (lib.mkIf (!live && config.losos.admin.enable && config.losos.admin.ui != null) {
      services.nginx.virtualHosts."losos-front".locations = {
        "/chat/".return = "302 /";
        # Not the admin UI's index.html under a homeserver's paths.
        "/_matrix/".return = "404";
      };
    })
    (lib.mkIf live (
      lib.mkMerge [
        {
          assertions = [
            {
              assertion = pkg != null;
              message = "losos.chat needs lososd (losos.backend.package): it signs everyone in.";
            }
          ];

          systemd.services.losos-chat-setup = {
            description = "Prepare LosOS Chat: the homeserver's keys and lososd's application service";
            wantedBy = [ "multi-user.target" ];
            before = [ "losos-chat.service" ];
            after = [ "losos-fscrypt-shared.service" ];
            wants = [ "losos-fscrypt-shared.service" ];
            serviceConfig = {
              Type = "oneshot";
              RemainAfterExit = true;
              ExecStart = lib.getExe setup;
            };
          };

          systemd.services.losos-chat = {
            description = "LosOS Chat (Synapse Matrix homeserver), as the shared user";
            wantedBy = [ "multi-user.target" ];
            after = [
              "network.target"
              "losos-chat-setup.service"
              "losos-fscrypt-shared.service"
            ];
            requires = [ "losos-chat-setup.service" ];
            wants = [ "losos-fscrypt-shared.service" ];
            serviceConfig = {
              Type = "notify";
              NotifyAccess = "main";
              User = "shared";
              Group = "shared";
              WorkingDirectory = stateDir;
              LoadCredential = [
                "signing_key:${secretsDir}/signing.key"
                "macaroon:${secretsDir}/macaroon"
                "appservice:${secretsDir}/appservice.yaml"
              ];
              ExecStart = lib.escapeShellArgs [
                "${synapse}/bin/synapse_homeserver"
                "--config-path"
                configFile
              ];
              ExecReload = "${pkgs.util-linux}/bin/kill -HUP $MAINPID";
              Restart = "on-failure";
              RestartSec = "5s";
              LimitNOFILE = 65535;
              MemoryMax = "1G";
              # Its own folder of the shared domain, and nothing else of /home.
              ProtectHome = "tmpfs";
              BindPaths = [ stateDir ];
              ProtectSystem = "strict";
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
              RestrictNamespaces = true;
              RestrictRealtime = true;
              RestrictSUIDSGID = true;
              LockPersonality = true;
              RestrictAddressFamilies = [
                "AF_INET"
                "AF_INET6"
                "AF_UNIX"
              ];
              SystemCallArchitectures = "native";
              SystemCallFilter = [
                "@system-service"
                "~@privileged"
              ];
              UMask = "0077";
            };
          };

          systemd.services.lososd = {
            environment = {
              LOSOS_CHAT_HOMESERVER = "127.0.0.1:${toString port}";
              LOSOS_CHAT_SERVER_NAME = serverName;
              LOSOS_CHAT_AS_TOKEN_FILE = asTokenFile;
              LOSOS_CHAT_LOGIN = "127.0.0.1:${toString identityPort}";
              LOSOS_CHAT_FEDERATION = if federation then "1" else "0";
            }
            // lib.optionalAttrs gitEvents {
              LOSOS_GIT_CHAT_EVENTS_SOCKET = "${gitEventsDir}/events.sock";
            };
            after = [ "losos-chat-setup.service" ];
            wants = [ "losos-chat-setup.service" ];
          };

          # LosOS Git's events reach lososd's socket in here, and nobody
          # else's do (backend/src/git_rooms.rs).
          systemd.tmpfiles.rules = lib.mkIf gitEvents [
            "d ${gitEventsDir} 0750 root losos-git -"
          ];

          networking.firewall = lib.mkIf config.networking.firewall.enable {
            extraCommands = ''
              iptables -w -D ${loopbackRule} 2>/dev/null || true
              iptables -w -I ${loopbackRule}
            '';
            extraStopCommands = ''
              iptables -w -D ${loopbackRule} 2>/dev/null || true
            '';
          };

          services.nginx.virtualHosts."losos-front".locations = {
            # The chat page: chat/index.html in the admin UI's bundle, on real
            # paths under /chat/, under its own CSP (modules/containers.nix,
            # chatCsp).
            "/chat/" = {
              tryFiles = "$uri /chat/index.html";
              extraConfig = lanOnly;
            };
            "/_matrix/client/" = {
              proxyPass = "http://127.0.0.1:${toString port}";
              recommendedProxySettings = false;
              extraConfig = gated;
            };
            "/_matrix/media/" = {
              proxyPass = "http://127.0.0.1:${toString port}";
              recommendedProxySettings = false;
              extraConfig = ''
                ${gated}
                client_max_body_size ${toString cfg.maxUploadMiB}m;
                proxy_request_buffering off;
              '';
            };
            "= /internal/losos-chat-authz" = {
              proxyPass = "http://127.0.0.1:${toString config.losos.admin.apiPort}/internal/chat/authz";
              recommendedProxySettings = false;
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
                proxy_set_header Remote-User "";
                proxy_set_header Remote-Groups "";
                proxy_set_header X-Losos-As "";
                proxy_set_header X-Losos-Via-Edge "";
                proxy_set_header X-Losos-Edge "";
              '';
            };
            # Signed out of LosOS Login: a Matrix error a client understands
            # (the token is fine, the sign-in is not), and where to sign in.
            "@losos-chat-sign-in".extraConfig = ''
              default_type application/json;
              ${noStore}
              return 401 '${
                builtins.toJSON {
                  errcode = "M_UNKNOWN_TOKEN";
                  error = "Sign in with LosOS Login to use chat.";
                  soft_logout = true;
                  losos_sign_in = "/auth/";
                }
              }';
            '';
            "= /.well-known/matrix/client".extraConfig = ''
              add_header Access-Control-Allow-Origin "*" always;
              ${json { "m.homeserver".base_url = "https://${serverName}"; }}
            '';
          }
          // lib.optionalAttrs federation {
            "= /.well-known/matrix/server".extraConfig = ''
              ${lib.optionalString lanName lanOnly}
              ${json { "m.server" = "${serverName}:443"; }}
            '';
            "/_matrix/federation/" = {
              proxyPass = "http://127.0.0.1:${toString port}";
              recommendedProxySettings = false;
              extraConfig = ''
                ${lib.optionalString lanName lanOnly}
                ${toSynapse}
                client_max_body_size ${toString cfg.maxUploadMiB}m;
              '';
            };
            "/_matrix/key/" = {
              proxyPass = "http://127.0.0.1:${toString port}";
              recommendedProxySettings = false;
              extraConfig = ''
                ${lib.optionalString lanName lanOnly}
                ${toSynapse}
              '';
            };
          };
        }
        # Port 8448 for a .local name. Another Matrix server reads the
        # well-known above with a client that verifies certificates, which a
        # box's own one never passes, and then falls back to <name>:8448, so a
        # LAN of LosOS boxes federates through this listener: federation and
        # keys only, LAN only, with the box's certificate, which the other
        # side's federation client does not verify for a .local name.
        (lib.mkIf (federation && lanName && config.losos.tls.enable) {
          services.nginx.virtualHosts."losos-chat-federation" = {
            inherit serverName;
            onlySSL = true;
            sslCertificate = "/var/lib/losos-tls/cert.pem";
            sslCertificateKey = "/var/lib/losos-tls/key.pem";
            listen = [
              {
                addr = "0.0.0.0";
                port = 8448;
                ssl = true;
              }
            ];
            locations = {
              "/".return = "404";
              "/_matrix/federation/" = {
                proxyPass = "http://127.0.0.1:${toString port}";
                recommendedProxySettings = false;
                extraConfig = ''
                  ${lanOnly}
                  ${toSynapse}
                  client_max_body_size ${toString cfg.maxUploadMiB}m;
                '';
              };
              "/_matrix/key/" = {
                proxyPass = "http://127.0.0.1:${toString port}";
                recommendedProxySettings = false;
                extraConfig = ''
                  ${lanOnly}
                  ${toSynapse}
                '';
              };
            };
          };
          networking.firewall.allowedTCPPorts = [ 8448 ];
        })
      ]
    ))
  ];
}
