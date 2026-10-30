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
# the fallback administrator. Both use one secret, `mail-master` in the box's
# credential store (modules/credentials.nix), which systemd opens into the
# credential directory of Stalwart and of lososd alone. lososd runs with
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

  # Where an older build kept the secret as plaintext; the credential store
  # adopts it from here.
  legacySecret = "/var/lib/losos-mail/master-secret";
  # The account the shared side runs as (lososInternal.sideUser): `shared`
  # in a mesh, `kindashared` with the local gateways only.
  user = if config.lososInternal.sideUser == null then "shared" else config.lososInternal.sideUser;
  # The real path is /persist/home/<user>/data, bind-mounted here by
  # modules/impermanence.nix; the unit sees the bind-mounted view.
  dataDir = "/home/${user}/data/mail";
  # The box's Web Key Directory: public keys lososd publishes, nginx serves.
  wkdDir = "/var/lib/losos-wkd";
  credential = "%{file:/run/credentials/losos-mail.service/mail-master}%";

  fscrypt = config.losos.${user}.fscrypt.enable;
  unlock = "losos-fscrypt-${user}.service";

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
    resolver.public-suffix = [
      "file://${pkgs.publicsuffix-list}/share/publicsuffix/public_suffix_list.dat"
    ];
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
  # The credential store every secret here lives in.
  imports = [ ./credentials.nix ];

  config = lib.mkIf live {
    # Not rotatable from the pane: lososd holds its copy from the moment it
    # starts, so a new value would mean restarting the admin API under the
    # page that asked for it. The mailboxes do not depend on it either way.
    losos.credentials.entries.mail-master = {
      app = "mail";
      description = "The secret LosOS Mail and the box share, to act for each person in their own mailbox.";
      format = "hex-32";
      services = [
        "losos-mail"
        "lososd"
      ];
      legacy = [ legacySecret ];
    };

    systemd.services.losos-mail-dir = {
      description = "LosOS Mail: the mailboxes' directory in the ${user} domain";
      after = [ "local-fs.target" ] ++ lib.optional fscrypt unlock;
      requires = lib.optional fscrypt unlock;
      before = [ "losos-mail.service" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${pkgs.coreutils}/bin/install -d -o ${user} -g ${user} -m 0700 ${dataDir}";
      };
    };

    systemd.services.losos-mail = {
      description = "LosOS Mail (Stalwart), as the ${user} user";
      wantedBy = [ "multi-user.target" ];
      after = [
        "local-fs.target"
        "network.target"
        "losos-mail-dir.service"
      ]
      ++ lib.optional fscrypt unlock;
      requires = [
        "losos-mail-dir.service"
      ]
      ++ lib.optional fscrypt unlock;
      # A new configuration (a new port or domain) is a restart.
      restartTriggers = [ configFile ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${lib.getExe package} --config=${configFile}";
        User = user;
        Group = user;
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
        # The same listener for the mail plugin's manifest
        # (backend/plugins/mail), which names origins.
        LOSOS_MAIL_URL = "http://127.0.0.1:${toString port}";
        LOSOS_MAIL_SECRET_FILE = "%d/mail-master";
        LOSOS_MAIL_DOMAIN = domain;
        # OpenPGP (backend/src/mail_pgp.rs): people's keys, the private
        # half locked with their passphrase and sealed again with
        # systemd-creds, under lososd's own 0700 state directory; the
        # public halves in the Web Key Directory nginx serves.
        LOSOS_MAIL_KEYS_DIR = "/var/lib/losos/mail-keys";
        LOSOS_MAIL_WKD_DIR = wkdDir;
        LOSOS_MAIL_KEYSERVER = lib.optionalString (
          config.losos.mail.keyserver != null
        ) config.losos.mail.keyserver;
      };
    };

    # ── The box's Web Key Directory ────────────────────────────────────────
    # Public keys only, written by lososd when a person sets their key and
    # served as plain files, so a mail program anywhere finds the key of
    # eva@<domain> the way WKD says (draft-koch-openpgp-webkey-service):
    # the direct method's /.well-known/openpgpkey/hu/<hash> and the
    # advanced method's /.well-known/openpgpkey/<domain>/hu/<hash>, which
    # lososd writes under both paths. Not behind the LAN-only guard: a key
    # directory is for the people who do not have the key yet, which is
    # everyone outside. Nothing here is secret, and nginx serves nothing
    # but these files under that prefix.
    systemd.tmpfiles.rules = [ "d ${wkdDir} 0755 root root -" ];

    services.nginx.virtualHosts."losos-front".locations."/.well-known/openpgpkey/" = {
      alias = "${wkdDir}/";
      extraConfig = ''
        default_type application/octet-stream;
        autoindex off;
        # Every mail program asks from anywhere; WKD asks servers to say so.
        add_header Access-Control-Allow-Origin "*" always;
        # An add_header here drops every one the server sets, so the rest
        # are repeated (as chat.nix's noStore does).
        add_header Content-Security-Policy "default-src 'none'; frame-ancestors 'none'" always;
        add_header X-Content-Type-Options "nosniff" always;
        add_header X-Frame-Options "DENY" always;
        add_header Referrer-Policy "no-referrer" always;
        add_header Cache-Control "public, max-age=3600" always;
        limit_except GET HEAD { deny all; }
      '';
    };
  };
}
