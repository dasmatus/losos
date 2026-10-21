# Text inside the owner's files, for the admin pages' palette
# (backend/src/textsearch/, the losos-search binary).
#
#   losos-search-index   a timer-driven pass over the owner's LosOS Cloud
#                        files that reads only what changed since the last
#                        one into an SQLite full-text index, as `losos-cloud`
#   losos-search-cloud   one search of that index per connection
#   losos-search-git     one `git grep` over the repositories' default
#                        branches per connection, as `forgejo`
#
# The two searches are socket-activated with Accept=yes: systemd holds the
# socket, starts a process for each search and nothing stays in memory in
# between. The sockets are 0600 root, so lososd is the only client; it relays
# GET /api/cloud/text and /api/git/text to them.
#
# Each unit sees one directory of the apps' state and nothing else under
# /var/lib: the indexer the owner's files (read-only) and its index, the Git
# search the repositories (read-only). No network, no capabilities.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.search;
  pkg = config.losos.backend.package;
  search = "${pkg}/bin/losos-search";

  stateDir = "/var/lib/losos-search";
  db = "${stateDir}/cloud.sqlite";
  # The owner's files as LosOS Cloud keeps them (modules/cloud.nix).
  files = "/var/lib/losos-cloud/notshared/files";
  withCloud = config.losos.cloud.enable;
  repos = "/var/lib/forgejo/repositories";
  runDir = "/run/losos-search";
  cloudSocket = "${runDir}/cloud.sock";
  gitSocket = "${runDir}/git.sock";
  withGit = config.losos.forgejo.enable;

  sandbox = {
    NoNewPrivileges = true;
    CapabilityBoundingSet = "";
    PrivateNetwork = true;
    RestrictAddressFamilies = [ "AF_UNIX" ];
    PrivateTmp = true;
    PrivateDevices = true;
    ProtectSystem = "strict";
    ProtectHome = true;
    # Only the binds below exist under /var/lib.
    TemporaryFileSystem = "/var/lib:ro";
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
    SystemCallArchitectures = "native";
    SystemCallFilter = [
      "@system-service"
      "~@privileged"
    ];
    UMask = "0077";
    Nice = 10;
    IOSchedulingClass = "idle";
  };

  # One search per connection: the socket is stdin and stdout.
  perConnection = {
    StandardInput = "socket";
    StandardOutput = "socket";
    StandardError = "journal";
    RuntimeMaxSec = "15s";
    # systemd logs a start and a stop for every search; keep those out of
    # the journal and the warnings in.
    LogLevelMax = "notice";
  };
in
{
  config = lib.mkIf (cfg.enable && pkg != null) (
    lib.mkMerge [
      (lib.mkIf withCloud {
        systemd.services.losos-search-index = {
          description = "Read new and changed files in LosOS Cloud into the search index";
          serviceConfig = sandbox // {
            Type = "oneshot";
            User = "losos-cloud";
            Group = "losos-cloud";
            ExecStart = lib.escapeShellArgs [
              search
              "index"
              "--root"
              files
              "--db"
              db
              "--pdftotext"
              "${pkgs.poppler-utils}/bin/pdftotext"
            ];
            StateDirectory = "losos-search";
            StateDirectoryMode = "0700";
            BindPaths = [ stateDir ];
            # "-": the folder appears on the owner's first sign-in.
            BindReadOnlyPaths = [ "-${files}" ];
            MemoryMax = "256M";
            CPUQuota = "50%";
            TimeoutStartSec = "6h";
          };
        };
        systemd.timers.losos-search-index = {
          wantedBy = [ "timers.target" ];
          timerConfig = {
            OnBootSec = "3min";
            OnUnitInactiveSec = "2min";
            AccuracySec = "30s";
          };
        };

        systemd.sockets.losos-search-cloud = {
          description = "Searches of the text in LosOS Cloud files";
          wantedBy = [ "sockets.target" ];
          socketConfig = {
            ListenStream = cloudSocket;
            Accept = true;
            SocketMode = "0600";
            MaxConnections = 8;
          };
        };
        systemd.services."losos-search-cloud@" = {
          description = "One search of the text in LosOS Cloud files";
          serviceConfig =
            sandbox
            // perConnection
            // {
              User = "losos-cloud";
              Group = "losos-cloud";
              ExecStart = "${search} cloud --db ${db}";
              # Read-write: SQLite keeps its reader locks next to the file.
              BindPaths = [ "-${stateDir}" ];
              MemoryMax = "64M";
            };
        };

        systemd.services.lososd.environment.LOSOS_SEARCH_CLOUD_SOCKET = cloudSocket;
      })

      (lib.mkIf withGit {
        systemd.sockets.losos-search-git = {
          description = "Searches of the text in LosOS Git repositories";
          wantedBy = [ "sockets.target" ];
          socketConfig = {
            ListenStream = gitSocket;
            Accept = true;
            SocketMode = "0600";
            MaxConnections = 8;
          };
        };
        systemd.services."losos-search-git@" = {
          description = "One search of the text in LosOS Git repositories";
          serviceConfig =
            sandbox
            // perConnection
            // {
              User = "forgejo";
              Group = "forgejo";
              ExecStart = lib.escapeShellArgs [
                search
                "git"
                "--repos"
                repos
                "--git"
                "${pkgs.gitMinimal}/bin/git"
              ];
              BindReadOnlyPaths = [ "-${repos}" ];
              MemoryMax = "256M";
            };
        };

        systemd.services.lososd.environment.LOSOS_SEARCH_GIT_SOCKET = gitSocket;
      })
    ]
  );
}
