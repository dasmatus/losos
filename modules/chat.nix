# LosOS chat: a Matrix homeserver (Dendrite) on the box, the chat page at
# /chat/, and lososd between the two (backend/src/chat.rs).
#
# Chat belongs to the `shared` user. Dendrite runs as it, and every room,
# message, upload and key it keeps is in the shared data domain
# (/home/shared/data/chat), which is unlocked only while the box shares its
# storage (modules/fscrypt.nix). So all of this exists only while
# losos.chat.enable is on and the box shares its storage in a mesh it has
# joined (lososInternal.sharedSide: losos.sharingMyStorage and
# losos.cluster.enable; lososInternal.chat): otherwise there is no homeserver, no nginx
# location for /chat/ or /_matrix/, and lososd is told nothing, so
# /api/me names no chat and the page draws no entry for it.
#
# Nobody signs in to chat with a Matrix password; there are none. lososd is
# an application service whose namespace is every user on this server, and
# the only way an account is made or a device signed in
# (POST /api/chat/session, behind LosOS Login like every /api/ route). Then
# every request to the client and media APIs is asked about first
# (auth_request to lososd's /internal/chat/authz), which wants a live LosOS
# Login session, a person still on the box and not held, and a token of
# that same person. Signing out of LosOS Login ends chat at the next
# request; removing a person also signs their devices out of Dendrite.
#
# Dendrite answers on loopback only, and only root, nginx and `shared` may
# open that port (the same owner rule identity.nix puts on the apps). The
# client API is LAN-only, like the admin pages. Federation is off unless
# losos.chat.federation.enable, which needs a server name other servers can
# reach; then only the federation and key APIs answer from anywhere.
#
# Dendrite has no thread list. nginx sends
# /_matrix/client/v1/rooms/{room}/threads to lososd, which reads Dendrite's
# relations table through `losos-chat-threads` (socket-activated, as
# `shared`, a 0600 root socket: lososd has ProtectHome and cannot read the
# domain itself).
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
  dendrite = pkgs.dendrite;
  inherit (cfg) port;
  sharedUid = 1001;

  stateDir = "/home/shared/data/chat";
  secretsDir = "/var/secrets/losos-chat";
  runDir = "/run/losos-chat";
  threadsSocket = "${runDir}/threads.sock";
  asTokenFile = "${secretsDir}/as_token";
  serverName = cfg.serverName;
  federation = cfg.federation.enable;
  identityPort = config.losos.identity.port;
  lanOnly = import ./lan-only.nix;

  db = name: { connection_string = "file:${stateDir}/${name}.db"; };
  settings = {
    version = 2;
    global = {
      server_name = serverName;
      private_key = "$CREDENTIALS_DIRECTORY/matrix_key";
      key_validity_period = "168h0m0s";
      cache = {
        max_size_estimated = "64mb";
        max_age = "1h";
      };
      # No identity servers: there is no mail or phone number to look up.
      trusted_third_party_id_servers = [ ];
      disable_federation = !federation;
      presence = {
        enable_inbound = federation;
        enable_outbound = true;
      };
      report_stats.enabled = false;
      jetstream = {
        storage_path = "${stateDir}/jetstream";
        topic_prefix = "Dendrite";
      };
      dns_cache.enabled = false;
    };
    app_service_api = {
      config_files = [ "$CREDENTIALS_DIRECTORY/appservice" ];
      legacy_auth = false;
      legacy_paths = false;
    };
    client_api = {
      registration_disabled = true;
      guests_disabled = true;
      enable_registration_captcha = false;
      rate_limiting = {
        enabled = true;
        threshold = 20;
        cooloff_ms = 500;
      };
    };
    federation_api = {
      database = db "federationapi";
      send_max_retries = 16;
      # Only matrix.org's keys are trusted for other servers' keys, and only
      # when this box federates at all.
      key_perspectives = lib.optionals federation [
        {
          server_name = "matrix.org";
          keys = [
            {
              key_id = "ed25519:auto";
              public_key = "Noi6WqcDj0QmPxCNQqgezwTlBKrfqehY1u2FyWP9uYw";
            }
            {
              key_id = "ed25519:a_RXGa";
              public_key = "l8Hft5qXKn1vfHrg3p4+W8gELQVo8N13JkluMfmn2sQ";
            }
          ];
        }
      ];
      # Never fetch from this box's own networks on another server's say:
      # a remote server must not make Dendrite call the LAN or loopback.
      deny_networks = [
        "127.0.0.1/8"
        "10.0.0.0/8"
        "172.16.0.0/12"
        "192.168.0.0/16"
        "100.64.0.0/10"
        "169.254.0.0/16"
        "::1/128"
        "fe80::/64"
        "fc00::/7"
      ];
      allow_networks = [ "0.0.0.0/0" ];
    };
    key_server.database = db "keyserver";
    media_api = {
      database = db "mediaapi";
      base_path = "${stateDir}/media";
      max_file_size_bytes = cfg.maxUploadMiB * 1024 * 1024;
      dynamic_thumbnails = false;
      max_thumbnail_generators = 2;
      thumbnail_sizes = [
        {
          width = 32;
          height = 32;
          method = "crop";
        }
        {
          width = 96;
          height = 96;
          method = "crop";
        }
        {
          width = 640;
          height = 480;
          method = "scale";
        }
      ];
    };
    mscs = {
      mscs = [ ];
      database = db "mscs";
    };
    relay_api.database = db "relayapi";
    room_server.database = db "roomserver";
    sync_api = {
      database = db "syncserver";
      # Server-side search of unencrypted rooms (Matrix's /search).
      search = {
        enabled = true;
        index_path = "${stateDir}/searchindex";
        language = "en";
      };
    };
    user_api = {
      account_database = db "userapi_accounts";
      bcrypt_cost = 10;
      auto_join_rooms = [ ];
    };
    tracing.enabled = false;
    # The journal takes stderr.
    logging = [
      {
        type = "std";
        level = "warn";
      }
    ];
  };
  configFile = (pkgs.formats.yaml { }).generate "dendrite.yaml" settings;

  # The application service lososd is (backend/src/chat.rs): every user on
  # this server is in its namespace, and it has no URL, so Dendrite pushes it
  # nothing; it only signs people in.
  serverRegex = lib.escapeRegex serverName;
  setup = pkgs.writeShellApplication {
    name = "losos-chat-setup";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.openssl
      dendrite
    ];
    text = ''
      install -d -m 0700 ${secretsDir}
      if [ ! -s ${secretsDir}/matrix_key.pem ]; then
        generate-keys --private-key ${secretsDir}/matrix_key.pem.tmp
        mv ${secretsDir}/matrix_key.pem.tmp ${secretsDir}/matrix_key.pem
      fi
      for s in as_token hs_token; do
        if [ ! -s ${secretsDir}/$s ]; then
          openssl rand -hex 32 > ${secretsDir}/$s.tmp
          mv ${secretsDir}/$s.tmp ${secretsDir}/$s
        fi
      done
      umask 0077
      cat > ${secretsDir}/appservice.yaml.tmp <<EOF
      id: losos
      url: ""
      as_token: "$(cat ${secretsDir}/as_token)"
      hs_token: "$(cat ${secretsDir}/hs_token)"
      sender_localpart: losos
      rate_limited: false
      namespaces:
        users:
          - exclusive: true
            regex: '@.*:${serverRegex}'
        aliases: []
        rooms: []
      EOF
      mv ${secretsDir}/appservice.yaml.tmp ${secretsDir}/appservice.yaml
      # The domain is unlocked (this unit runs after the unlock); the folder
      # is the shared user's, inside it.
      install -d -m 0700 -o shared -g shared ${stateDir}
    '';
  };

  # Only these may open Dendrite's port on loopback: root (lososd), nginx and
  # `shared` (Dendrite itself). modules/daemon.nix says why an owner match.
  loopbackRule = lib.concatStringsSep " " (
    [
      "OUTPUT -o lo -p tcp -d 127.0.0.1 --dport ${toString port}"
      "-m owner ! --uid-owner 0"
      "-m owner ! --uid-owner ${toString sharedUid}"
    ]
    ++ lib.optional config.services.nginx.enable "-m owner ! --uid-owner ${config.services.nginx.user}"
    ++ [ "-j REJECT --reject-with tcp-reset" ]
  );

  toDendrite = ''
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
    ${toDendrite}
  '';
  # A JSON answer nginx writes itself. add_header here drops the server's
  # own headers, so all of them are repeated.
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
            {
              assertion = federation -> !lib.hasSuffix ".local" serverName;
              message = ''
                losos.chat.federation.enable needs a losos.chat.serverName other
                servers can reach; ${serverName} is a name of this LAN only.
              '';
            }
          ];

          systemd.services.losos-chat-setup = {
            description = "Prepare LosOS chat: the homeserver's keys and lososd's application service";
            wantedBy = [ "multi-user.target" ];
            before = [ "dendrite.service" ];
            after = [ "losos-fscrypt-shared.service" ];
            wants = [ "losos-fscrypt-shared.service" ];
            serviceConfig = {
              Type = "oneshot";
              RemainAfterExit = true;
              ExecStart = lib.getExe setup;
            };
          };

          systemd.services.dendrite = {
            description = "LosOS chat (Dendrite Matrix homeserver), as the shared user";
            wantedBy = [ "multi-user.target" ];
            after = [
              "network.target"
              "losos-chat-setup.service"
              "losos-fscrypt-shared.service"
            ];
            requires = [ "losos-chat-setup.service" ];
            wants = [ "losos-fscrypt-shared.service" ];
            serviceConfig = {
              Type = "simple";
              User = "shared";
              Group = "shared";
              WorkingDirectory = stateDir;
              RuntimeDirectory = "dendrite";
              RuntimeDirectoryMode = "0700";
              LoadCredential = [
                "matrix_key:${secretsDir}/matrix_key.pem"
                "appservice:${secretsDir}/appservice.yaml"
              ];
              ExecStartPre = "${pkgs.envsubst}/bin/envsubst -i ${configFile} -o /run/dendrite/dendrite.yaml";
              ExecStart = lib.escapeShellArgs [
                (lib.getExe dendrite)
                "--config"
                "/run/dendrite/dendrite.yaml"
                "--http-bind-address"
                "127.0.0.1:${toString port}"
              ];
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

          # The thread list, one question per connection (see the header).
          systemd.sockets.losos-chat-threads = {
            description = "Thread lists of LosOS chat";
            wantedBy = [ "sockets.target" ];
            socketConfig = {
              ListenStream = threadsSocket;
              Accept = true;
              SocketMode = "0600";
              MaxConnections = 16;
            };
          };
          systemd.services."losos-chat-threads@" = {
            description = "One thread list of LosOS chat";
            serviceConfig = {
              User = "shared";
              Group = "shared";
              ExecStart = "${pkg}/bin/losos-chat-threads --db ${stateDir}/syncserver.db";
              StandardInput = "socket";
              StandardOutput = "socket";
              StandardError = "journal";
              RuntimeMaxSec = "15s";
              LogLevelMax = "notice";
              # Read-write: SQLite keeps its reader locks beside the file.
              ProtectHome = "tmpfs";
              BindPaths = [ "-${stateDir}" ];
              PrivateNetwork = true;
              RestrictAddressFamilies = [ "AF_UNIX" ];
              ProtectSystem = "strict";
              PrivateTmp = true;
              PrivateDevices = true;
              NoNewPrivileges = true;
              CapabilityBoundingSet = "";
              ProtectKernelTunables = true;
              ProtectKernelModules = true;
              ProtectControlGroups = true;
              ProtectProc = "invisible";
              ProcSubset = "pid";
              LockPersonality = true;
              MemoryDenyWriteExecute = true;
              SystemCallArchitectures = "native";
              SystemCallFilter = [
                "@system-service"
                "~@privileged"
              ];
              MemoryMax = "64M";
            };
          };

          systemd.services.lososd = {
            environment = {
              LOSOS_CHAT_HOMESERVER = "127.0.0.1:${toString port}";
              LOSOS_CHAT_SERVER_NAME = serverName;
              LOSOS_CHAT_AS_TOKEN_FILE = asTokenFile;
              LOSOS_CHAT_THREADS_SOCKET = threadsSocket;
              LOSOS_CHAT_LOGIN = "127.0.0.1:${toString identityPort}";
              LOSOS_CHAT_FEDERATION = if federation then "1" else "0";
            };
            after = [ "losos-chat-setup.service" ];
            wants = [ "losos-chat-setup.service" ];
          };

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
            # paths under /chat/ as the Lab is under /lab/, under its own CSP
            # (modules/containers.nix, chatCsp).
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
            # The thread list Dendrite lacks, answered by lososd.
            "~ ^/_matrix/client/v1/rooms/([^/]+)/threads$" = {
              recommendedProxySettings = false;
              extraConfig = ''
                ${lanOnly}
                auth_request /internal/losos-chat-authz;
                error_page 401 = @losos-chat-sign-in;
                rewrite ^/_matrix/client/v1/rooms/([^/]+)/threads$ /internal/chat/threads/$1 break;
                proxy_pass http://127.0.0.1:${toString config.losos.admin.apiPort};
                proxy_set_header Remote-User "";
                proxy_set_header Remote-Groups "";
                proxy_set_header X-Losos-As "";
                proxy_set_header X-Losos-Via-Edge "";
                proxy_set_header X-Losos-Edge "";
              '';
            };
            "= /internal/losos-chat-authz" = {
              proxyPass = "http://127.0.0.1:${toString config.losos.admin.apiPort}/internal/chat/authz";
              recommendedProxySettings = false;
              extraConfig = ''
                internal;
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
            "= /.well-known/matrix/server".extraConfig = json { "m.server" = "${serverName}:443"; };
            "/_matrix/federation/" = {
              proxyPass = "http://127.0.0.1:${toString port}";
              recommendedProxySettings = false;
              extraConfig = ''
                ${toDendrite}
                client_max_body_size ${toString cfg.maxUploadMiB}m;
              '';
            };
            "/_matrix/key/" = {
              proxyPass = "http://127.0.0.1:${toString port}";
              recommendedProxySettings = false;
              extraConfig = toDendrite;
            };
          };
        }
      ]
    ))
  ];
}
