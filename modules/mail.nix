# LosOS Mail: Stalwart, run as the shared user, for the people LosOS Login
# knows.
#
# ── On the shared side, and only while sharing ───────────────────────────────
# Mail belongs to the shared user: the server runs as `shared` (uid 1001) and
# keeps its mailboxes in /home/shared/data/mail, inside the fscrypt domain
# that is unlocked only while losos.sharingMyStorage is on
# (modules/fscrypt.nix). Mail runs only while the storage counts as shared:
# sharing on and the box in a mesh (lososInternal.sharedSide). Otherwise
# nothing here exists: no server, no
# /mail/ page (modules/containers.nix), no Mail entry in the admin pages
# (lososd lists "mail" in /api/me's `shared` only while it is configured),
# and /api/mail/* answers 404. Chat follows the same rule. The one switch is
# lososInternal.mail.live in modules/options.nix.
#
# ── A plain unit, not nixpkgs' services.stalwart ─────────────────────────────
# The nixpkgs module adds a systemd-tmpfiles `d` line for its data
# directory. Under the shared domain that line runs at boot, before
# losos-fscrypt-shared.service has put the key in the keyring, and creating
# or chowning an entry in a locked fscrypt directory fails (ENOKEY): a
# failed systemd-tmpfiles-setup on every boot. So a oneshot makes the
# directory, as root, after the unlock, and the server's unit otherwise
# keeps the module's hardening. It is a unit of its own, not an
# ExecStartPre: the server's BindPaths= is set up before any of its
# processes run, '+' ones included, and fails while the directory is
# missing.
#
# ── Who reaches it ───────────────────────────────────────────────────────────
# Stalwart listens on loopback HTTP only (JMAP and its management API); no
# SMTP, IMAP or POP listener, so nothing on the LAN talks to it and no
# password of its own exists. lososd is its one client (backend/src/mail.rs):
# it signs in with Stalwart's master user as `<person>%losos`, for the person
# LosOS Login signed in, and makes their account on their first call with
# the fallback administrator. Both use one secret, made here, readable by root
# alone and handed to Stalwart by LoadCredential. lososd runs with
# ProtectHome=true and never reads the mailboxes itself.
#
# Mail between the people on this box is delivered locally. Mail to and from
# the internet needs an edge with a public address and port 25 (a home
# connection usually has neither), so lososd refuses other domains for now.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  live = config.lososInternal.mail.live;
  domain = config.lososInternal.mail.domain;
  port = config.losos.mail.port;
  package = pkgs.stalwart;

  # Root-only. /var is persisted, so the secret survives reboots; a new
  # one would only mean lososd restarting with it, the mailboxes do not
  # depend on it.
  secretDir = "/var/lib/losos-mail";
  secretFile = "${secretDir}/master-secret";
  # The real path is /persist/home/shared/data, bind-mounted here by
  # modules/impermanence.nix; the unit sees the bind-mounted view.
  dataDir = "/home/shared/data/mail";
  credential = "%{file:/run/credentials/losos-mail.service/master}%";

  fscrypt = config.losos.shared.fscrypt.enable;

  settings = {
    server = {
      hostname = domain;
      listener.http = {
        bind = [ "127.0.0.1:${toString port}" ];
        protocol = "http";
      };
    };
    # Everything in one RocksDB under the shared domain, as nixpkgs'
    # module does from state version 24.11 on.
    store.db = {
      type = "rocksdb";
      path = "${dataDir}/db";
      compression = "lz4";
    };
    storage = {
      data = "db";
      fts = "db";
      lookup = "db";
      blob = "db";
      directory = "internal";
    };
    directory.internal = {
      type = "internal";
      store = "db";
    };
    tracer.journal = {
      type = "journal";
      level = "info";
      enable = true;
    };
    resolver.type = "system";
    resolver.public-suffix = [ "file://${pkgs.publicsuffix-list}/share/publicsuffix/public_suffix_list.dat" ];
    spam-filter.resource = "file://${package.spam-filter}/spam-filter.toml";
    # Shipped with the package, so Stalwart never fetches it from GitHub.
    # Nobody opens it: the listener is loopback and lososd uses the API.
    webadmin = {
      path = "/var/cache/losos-mail";
      resource = "file://${package.webadmin}/webadmin.zip";
      auto-update = false;
    };
    authentication = {
      # lososd makes the domain and each person's account with it.
      fallback-admin = {
        user = "losos-admin";
        secret = credential;
      };
      # lososd acts as each person with it (backend/src/mail.rs, MASTER_USER).
      master = {
        user = "losos";
        secret = credential;
      };
    };
    # Stalwart's own list of keys read from this file rather than its
    # database, plus the master user, which would otherwise be a database
    # key with a warning at every start.
    config.local-keys = [
      "store.*"
      "directory.*"
      "tracer.*"
      "!server.blocked-ip.*"
      "!server.allowed-ip.*"
      "server.*"
      "certificate.*"
      "config.local-keys.*"
      "authentication.fallback-admin.*"
      "authentication.master.*"
      "cluster.*"
      "storage.data"
      "storage.blob"
      "storage.lookup"
      "storage.fts"
      "storage.directory"
      "resolver.*"
      "spam-filter.resource"
      "webadmin.*"
    ];
  };

  configFile = (pkgs.formats.toml { }).generate "losos-mail.toml" settings;
in
{
  config = lib.mkIf live {
    systemd.services.losos-mail-secret = {
      description = "LosOS Mail: make the secret lososd and Stalwart share";
      wantedBy = [ "multi-user.target" ];
      before = [
        "losos-mail.service"
        "lososd.service"
      ];
      unitConfig.ConditionPathExists = "!${secretFile}";
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        UMask = "0077";
      };
      path = [ pkgs.coreutils ];
      script = ''
        set -eu
        install -d -m 0700 ${secretDir}
        head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' > ${secretFile}.tmp
        mv ${secretFile}.tmp ${secretFile}
      '';
    };

    systemd.services.losos-mail-dir = {
      description = "LosOS Mail: the mailboxes' directory in the shared domain";
      after = [ "local-fs.target" ] ++ lib.optional fscrypt "losos-fscrypt-shared.service";
      requires = lib.optional fscrypt "losos-fscrypt-shared.service";
      before = [ "losos-mail.service" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${pkgs.coreutils}/bin/install -d -o shared -g shared -m 0700 ${dataDir}";
      };
    };

    systemd.services.losos-mail = {
      description = "LosOS Mail (Stalwart), as the shared user";
      wantedBy = [ "multi-user.target" ];
      after = [
        "local-fs.target"
        "network.target"
        "losos-mail-secret.service"
        "losos-mail-dir.service"
      ]
      ++ lib.optional fscrypt "losos-fscrypt-shared.service";
      requires = [
        "losos-mail-secret.service"
        "losos-mail-dir.service"
      ]
      ++ lib.optional fscrypt "losos-fscrypt-shared.service";
      # A new configuration (a new port or domain) is a restart.
      restartTriggers = [ configFile ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${lib.getExe package} --config=${configFile}";
        User = "shared";
        Group = "shared";
        LoadCredential = "master:${secretFile}";
        CacheDirectory = "losos-mail";
        LimitNOFILE = 65536;
        KillMode = "process";
        KillSignal = "SIGINT";
        Restart = "on-failure";
        RestartSec = 5;
        SyslogIdentifier = "losos-mail";

        # /home is an empty tmpfs with only the mailboxes bound in: the
        # rest of the shared domain and the owner's home stay out of reach.
        ProtectHome = "tmpfs";
        BindPaths = [ dataDir ];
        ProtectSystem = "strict";
        PrivateTmp = true;
        PrivateDevices = true;
        DeviceAllow = [ "" ];
        CapabilityBoundingSet = [ "" ];
        AmbientCapabilities = [ "" ];
        NoNewPrivileges = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        ProcSubset = "pid";
        ProtectClock = true;
        ProtectControlGroups = true;
        ProtectHostname = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectProc = "invisible";
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
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
        LOSOS_MAIL_ADDR = "127.0.0.1:${toString port}";
        LOSOS_MAIL_SECRET_FILE = secretFile;
        LOSOS_MAIL_DOMAIN = domain;
      };
      after = [ "losos-mail-secret.service" ];
      wants = [ "losos-mail-secret.service" ];
    };
  };
}
