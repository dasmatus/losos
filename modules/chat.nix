# LosOS Chat: a Matrix homeserver (Dendrite) on the box, the chat page at
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
# client API is LAN-only, like the admin pages. With
# losos.chat.federation.enable (on by default) the federation and key APIs
# answer other servers too: from the LAN alone while the server name is a
# .local one, so boxes on one network talk to each other and nothing else
# reaches them; from anywhere for a name other servers can reach.
#
# LosOS Git rooms (backend/src/git_rooms.rs): lososd takes LosOS Git's
# events on a socket in /run/losos-git-chat, a directory only LosOS Git's
# group may enter, and posts those of a repository with a room into it.
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
  inherit (pkgs) dendrite;
  inherit (cfg) port;
  sharedUid = 1001;

  stateDir = "/home/shared/data/chat";
  secretsDir = "/var/secrets/losos-chat";
  runDir = "/run/losos-chat";
  threadsSocket = "${runDir}/threads.sock";
  asTokenFile = "${secretsDir}/as_token";
  inherit (cfg) serverName;
  federation = cfg.federation.enable;
  # A .local name is this network's: federate with the boxes on it alone.
  lanName = lib.hasSuffix ".local" serverName;
  gitEvents = config.losos.git.enable;
  gitEventsDir = "/run/losos-git-chat";
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
      # when this box federates beyond its LAN: the boxes of a LAN hand out
      # their own keys.
      key_perspectives = lib.optionals (federation && !lanName) [
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
      # Never fetch from this box itself on another server's say. A box
      # with a public name keeps out of the LAN too: a remote server must
      # not make Dendrite call it. A box with a .local name federates with
      # the LAN and nothing else (nginx answers the LAN alone).
      deny_networks = [
        "127.0.0.1/8"
        "169.254.0.0/16"
        "::1/128"
        "fe80::/64"
      ]
      ++ lib.optionals (!lanName) [
        "10.0.0.0/8"
        "172.16.0.0/12"
        "192.168.0.0/16"
        "100.64.0.0/10"
        "fc00::/7"
      ];
      allow_networks = [ "0.0.0.0/0" ] ++ lib.optional lanName "::/0";
      # The other boxes of a LAN answer with certificates they made
      # themselves; who each is rests on its server key either way.
      disable_tls_validation = config.losos.cloud.mesh.selfSigned;
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
            description = "LosOS Chat (Dendrite Matrix homeserver), as the shared user";
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
            description = "Thread lists of LosOS Chat";
            wantedBy = [ "sockets.target" ];
            socketConfig = {
              ListenStream = threadsSocket;
              Accept = true;
              SocketMode = "0600";
              MaxConnections = 16;
            };
          };
          systemd.services."losos-chat-threads@" = {
            description = "One thread list of LosOS Chat";
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
            "= /.well-known/matrix/server".extraConfig = ''
              ${lib.optionalString lanName lanOnly}
              ${json { "m.server" = "${serverName}:443"; }}
            '';
            "/_matrix/federation/" = {
              proxyPass = "http://127.0.0.1:${toString port}";
              recommendedProxySettings = false;
              extraConfig = ''
                ${lib.optionalString lanName lanOnly}
                ${toDendrite}
                client_max_body_size ${toString cfg.maxUploadMiB}m;
              '';
            };
            "/_matrix/key/" = {
              proxyPass = "http://127.0.0.1:${toString port}";
              recommendedProxySettings = false;
              extraConfig = ''
                ${lib.optionalString lanName lanOnly}
                ${toDendrite}
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
                  ${toDendrite}
                  client_max_body_size ${toString cfg.maxUploadMiB}m;
                '';
              };
              "/_matrix/key/" = {
                proxyPass = "http://127.0.0.1:${toString port}";
                recommendedProxySettings = false;
                extraConfig = ''
                  ${lanOnly}
                  ${toDendrite}
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
