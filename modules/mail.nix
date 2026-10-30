# LosOS Mail: Stalwart, run as the shared user, for the people LosOS Login
# knows.
#
# ── On the shared side, and only while sharing ───────────────────────────────
# Mail belongs to the shared user: the server runs as `shared` (uid 1001) and
# keeps its mailboxes in /home/shared/data/mail, on the encrypted /persist
# beside the shared folder modules/fscrypt.nix locks (that folder is
# /home/shared/data/<machine-id>/shared). Mail runs only while the storage
# counts as shared: sharing on and the box in a mesh
# (lososInternal.sharedSide). Otherwise
# nothing here exists: no server, no
# /mail/ page (modules/containers.nix), no Mail entry in the admin pages
# (lososd lists "mail" in /api/me's `shared` only while it is configured),
# and /api/mail/* answers 404. Chat follows the same rule. The one switch is
# lososInternal.mail.live in modules/options.nix.
#
# ── Stalwart 1.0, set up by a plan ───────────────────────────────────────────
# Stalwart 1.0 (nix/stalwart.nix) keeps one file on disk, config.json, which
# names its data store; the rest of its settings live in its own registry,
# managed over JMAP. This module writes those settings as a plan, a JSON
# file in the store, and `losos-mail setup` (backend/src/mail_setup.rs)
# applies it before the server starts: Stalwart in recovery mode, a single
# HTTP listener on a free loopback port, the plan, then a clean stop. It
# runs again only when the plan changes.
#
# The plan is also what keeps the server from making its first-start
# defaults: SMTP, IMAP, POP3 and HTTPS on every address, a log under
# /var/log, a web interface, address lists and spam rules fetched from
# GitHub. It makes one loopback HTTP listener, logs to the journal, turns
# the web interface and the address lists off and sets no rules address.
# The units may reach localhost and nothing else, whatever the registry
# says.
#
# ── Memory ───────────────────────────────────────────────────────────────────
# Stalwart's defaults size its caches for a mail host: about 2.7 GiB of
# limits on a 4 GiB box. The plan and config.json cut them to what a few
# people's mail needs, and systemd holds the rest: MemoryHigh slows the
# server down before MemoryMax stops it.
#
# ── The upgrade from Stalwart 0.15 ────────────────────────────────────────────
# 1.0 cannot open a 0.15 store (mailboxes/db). On a box that has one,
# `losos-mail-migrate` starts the old server from nixpkgs on a free loopback
# port beside the new one, makes each person's account in 1.0 and has
# Vandelay (nixpkgs' stalwart-vandelay) copy their folders, messages,
# identities and filters over JMAP. When everyone is across it renames the
# old store to db-0.15 and keeps it. A copy cut short starts again on the next boot and
# carries on where it stopped.
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
# it is the server's administrator, named in STALWART_RECOVERY_ADMIN, makes
# each person's account on their first call and acts as them by signing in
# as `<address>%losos`. The secret is `mail-master` in the box's credential
# store (modules/credentials.nix), which systemd opens into the credential
# directory of these units and of lososd alone. lososd runs with
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
  package = pkgs.callPackage ../nix/stalwart.nix { };
  # The 0.15 server and the copier, for the upgrade's copy only.
  legacyPackage = pkgs.stalwart_0_15;
  vandelay = pkgs.stalwart-vandelay;
  lososMail = "${config.losos.backend.package}/bin/losos-mail";

  # Where an older build kept the secret as plaintext; the credential store
  # adopts it from here.
  legacySecret = "/var/lib/losos-mail/master-secret";
  # The account the shared side runs as (lososInternal.sideUser): `shared`
  # in a mesh, `kindashared` with the local gateways only.
  user = if config.lososInternal.sideUser == null then "shared" else config.lososInternal.sideUser;
  # The real path is /persist/home/<user>/data, bind-mounted here by
  # modules/impermanence.nix; the unit sees the bind-mounted view.
  dataDir = "/home/shared/data/mail";
  # Stalwart 1.0's store, and 0.15's, which only the upgrade's copy opens.
  storeDir = "${dataDir}/store";
  legacyStore = "${dataDir}/db";
  # The hash of the last plan `losos-mail setup` applied.
  planMarker = "${dataDir}/plan.sha256";
  # The box's Web Key Directory: public keys lososd publishes, nginx serves.
  wkdDir = "/var/lib/losos-wkd";

  fscrypt = config.losos.${user}.fscrypt.enable;
  unlock = "losos-fscrypt-${user}.service";

  mib = n: n * 1024 * 1024;

  # The one file Stalwart reads: its store, RocksDB, with small caches.
  configFile = pkgs.writeText "losos-mail-config.json" (
    builtins.toJSON {
      "@type" = "RocksDb";
      path = storeDir;
      cacheSize = mib 64;
      bufferSize = mib 32;
      poolWorkers = 2;
    }
  );

  # Everything else, applied by `losos-mail setup` (backend/src/mail_setup.rs).
  plan = pkgs.writeText "losos-mail-plan.json" (
    builtins.toJSON {
      entries = [
        {
          # One HTTP listener, on loopback, for lososd. Any other, such as
          # the defaults an older start may have made, is removed.
          type = "x:NetworkListener";
          key = "name";
          only = true;
          items = [
            {
              name = "http";
              bind = {
                "127.0.0.1:${toString port}" = true;
              };
              protocol = "http";
              useTls = false;
              implicitTls = false;
            }
          ];
        }
        {
          # The journal, not a file under /var/log.
          type = "x:Tracer";
          key = "@type";
          only = true;
          items = [
            {
              "@type" = "Journal";
              enable = true;
              level = "info";
            }
          ];
        }
        {
          # No address lists fetched from GitHub.
          type = "x:Asn";
          singleton = true;
          value = {
            "@type" = "Disabled";
          };
        }
        {
          # No rules fetched either: mail stays between the people on this
          # box, who are not spammers to each other.
          type = "x:SpamSettings";
          singleton = true;
          value = {
            spamFilterRulesUrl = null;
          };
        }
        {
          # Stalwart's own web interface, which it would download; lososd
          # and the Mail page are the interface.
          type = "x:Application";
          all = {
            enable = false;
          };
        }
        {
          type = "x:Domain";
          key = "name";
          items = [
            {
              name = domain;
              reportAddressUri = null;
            }
          ];
        }
        {
          type = "x:SystemSettings";
          singleton = true;
          value = {
            defaultHostname = domain;
            defaultDomainId = {
              "$ref" = [
                "x:Domain"
                "name"
                domain
              ];
            };
            maxBufferedMemory = mib 64;
          };
        }
        {
          # Sized for a few people on a 4 GiB box, not a mail host. The
          # caches for very large accounts are off: none is that large here.
          type = "x:Cache";
          singleton = true;
          value = {
            messages = mib 32;
            messagesLarge = 0;
            groupwareLarge = 0;
            events = mib 4;
            contacts = mib 4;
            files = mib 4;
            scheduling = mib 1;
            accounts = mib 4;
          };
        }
        {
          # Mail only: calendars, contacts and files are LosOS Cloud's.
          # One indexing task at a time, on a two-core box.
          type = "x:Search";
          singleton = true;
          value = {
            maxConcurrentIndexTasks = 1;
            indexCalendar = false;
            indexContacts = false;
            indexFiles = false;
          };
        }
      ];
    }
  );

  # The 0.15 configuration the upgrade's copy starts the old server with:
  # what this module wrote before 1.0, on a loopback address the copy picks.
  legacySecretPath = "/run/credentials/losos-mail-migrate.service/mail-master";
  legacyConfig = (pkgs.formats.toml { }).generate "losos-mail-0.15.toml" {
    server = {
      hostname = domain;
      listener.http = {
        bind = [ "%{env:LOSOS_MAIL_LEGACY_BIND}%" ];
        protocol = "http";
      };
    };
    store.db = {
      type = "rocksdb";
      path = legacyStore;
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
    # The session's URLs name the address the copy dialled, not the
    # hostname, which nothing on the box answers.
    http.url = "protocol + '://' + local_ip + ':' + local_port";
    tracer.journal = {
      type = "journal";
      level = "info";
      enable = true;
    };
    resolver.type = "system";
    resolver.public-suffix = [
      "file://${pkgs.publicsuffix-list}/share/publicsuffix/public_suffix_list.dat"
    ];
    spam-filter.resource = "file://${legacyPackage.spam-filter}/spam-filter.toml";
    webadmin = {
      path = "/var/cache/losos-mail";
      resource = "file://${legacyPackage.webadmin}/webadmin.zip";
      auto-update = false;
    };
    authentication = {
      fallback-admin = {
        user = "losos-admin";
        secret = "%{file:${legacySecretPath}}%";
      };
      master = {
        user = "losos";
        secret = "%{file:${legacySecretPath}}%";
      };
    };
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
      "http.url"
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

  # Stalwart with its administrator: the name lososd uses
  # (backend/src/mail.rs, MASTER_USER) and the secret from the unit's
  # credential directory, in the environment of this process alone.
  start = pkgs.writeShellScript "losos-mail-start" ''
    secret=$(< "$CREDENTIALS_DIRECTORY/mail-master")
    export STALWART_RECOVERY_ADMIN="losos:$secret"
    unset secret
    exec ${lib.getExe package} --config=${configFile}
  '';

  # The sandbox the server and both of its jobs run in.
  sandbox = {
    User = "shared";
    Group = "shared";
    CacheDirectory = "losos-mail";
    LimitNOFILE = 65536;
    # Loopback, and nothing else, until mail reaches the internet through
    # an edge.
    IPAddressDeny = "any";
    IPAddressAllow = "localhost";

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
        "losos-mail-migrate"
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
        ExecStart = [
          "${pkgs.coreutils}/bin/install -d -o shared -g shared -m 0700 ${dataDir}"
          "${pkgs.coreutils}/bin/install -d -o shared -g shared -m 0700 ${storeDir}"
        ];
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
      ++ lib.optional fscrypt "losos-fscrypt-shared.service";
      # A new plan or a new server is a restart, and the setup then
      # applies the plan before the server starts.
      restartTriggers = [
        configFile
        plan
        package
      ];
      environment = {
        STALWART_HOSTNAME = domain;
        # The address the JMAP session hands its clients (Vandelay follows
        # it); without it Stalwart names https://<hostname>, which nothing
        # on the box answers.
        STALWART_PUBLIC_URL = "http://127.0.0.1:${toString port}";
      };
      serviceConfig = sandbox // {
        Type = "simple";
        ExecStartPre = "${lososMail} setup --stalwart ${lib.getExe package} --config ${configFile} --plan ${plan} --secret-file %d/mail-master --marker ${planMarker}";
        ExecStart = start;
        # The setup's recovery start is a server start of its own.
        TimeoutStartSec = 300;
        KillMode = "process";
        KillSignal = "SIGINT";
        Restart = "on-failure";
        RestartSec = 5;
        SyslogIdentifier = "losos-mail";
        MemoryHigh = "512M";
        MemoryMax = "1G";
        CPUWeight = 50;
      };
    };

    # The upgrade from Stalwart 0.15: only where its store is still there.
    systemd.services.losos-mail-migrate = {
      description = "LosOS Mail: copy the mail of Stalwart 0.15 into 1.0";
      wantedBy = [ "multi-user.target" ];
      after = [ "losos-mail.service" ];
      requires = [ "losos-mail.service" ];
      unitConfig.ConditionPathExists = legacyStore;
      serviceConfig = sandbox // {
        Type = "oneshot";
        ExecStart = "${lososMail} migrate --legacy-stalwart ${lib.getExe legacyPackage} --legacy-config ${legacyConfig} --legacy-store ${legacyStore} --addr 127.0.0.1:${toString port} --domain ${domain} --vandelay ${lib.getExe vandelay} --work ${dataDir}/migrate --secret-file %d/mail-master";
        # A large mailbox takes as long as it takes.
        TimeoutStartSec = "infinity";
        SyslogIdentifier = "losos-mail-migrate";
        MemoryHigh = "512M";
        MemoryMax = "1G";
        CPUWeight = 20;
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
