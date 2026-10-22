# LosOS Git: the box's own Git server (`losos-git`, backend/src/forge/, a
# binary of lososd's crate).
#
# One service, on the host, as its own user:
#
#   /run/losos-git/http.sock   nginx's way in (modules/containers.nix,
#                              `/git/`). The only listener that believes the
#                              person LosOS Login names in the
#                              X-Losos-Cloud-* headers (modules/app-sign-in.nix),
#                              so it is 0660 and nginx is the only other
#                              member of the group.
#   127.0.0.1:3000             tokens only, never the headers. lososd keeps
#                              the configuration repository and makes
#                              computers' tokens through it with the admin
#                              token LosOS Git writes on its first start
#                              (/var/lib/losos-git/admin-token, read by
#                              lososd as root).
#
# Repositories are bare, at /var/lib/losos-git/repositories/<owner>/<name>.git,
# the layout Forgejo used, and the accounts, tokens and repository records
# are one SQLite file beside them. A box that ran Forgejo has its
# repositories, accounts, tokens, issues and pull requests carried over on
# the first start (`importForgejo` below), and the server takes in every
# repository it finds without a record.
#
# /var/lib is on /persist through impermanence's whole-/var entry, so
# nothing else is listed for it.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.git;
  chat = config.losos.chat;
  # People of other boxes sign in with their chat address: LosOS Git asks
  # their box who a token belongs to (backend/src/forge/matrix.rs). The
  # boxes of this LAN, or any server once this box's chat has a public name.
  chatOutside = chat.federation.enable && !lib.hasSuffix ".local" chat.serverName;
  pkg = config.losos.backend.package;
  on = cfg.enable && pkg != null;

  stateDir = "/var/lib/losos-git";
  # Literals, repeated in modules/containers.nix (the socket) and
  # modules/config-repo.nix (the port, the token file): nothing off the box
  # ever sees either, so they are not options.
  socket = "/run/losos-git/http.sock";
  port = 3000;
  # The internal socket's directory: losos-git and the losos-git-peers
  # group (Mesh CI's user) may enter it, nobody else.
  peersDir = "/run/losos-git-peers";
  # uid and gid Forgejo had, so its repositories keep their owner.
  id = 1003;

  git = pkgs.gitMinimal;

  # Forgejo's records, carried over once (backend/src/forge/import.rs). Its
  # repositories move into place; its database is exported table by table
  # as JSON for `losos-git import-forgejo`, and dumped whole beside that
  # (forgejo.sql or forgejo.db) so what LosOS Git has no place for is still
  # on the box; its attachments are copied. Nothing of Forgejo's is
  # deleted: /var/lib/forgejo and the Postgres data stay as they are.
  #
  # A container-mode box kept the database in the host's Postgres
  # (database `forgejo`), a native one in SQLite. When Postgres is running
  # it is asked over its socket; when it is not (it went with the apps that
  # needed it), a copy of its data directory is started for the export on
  # a private socket and thrown away after. The copy runs the same
  # Postgres the box ran, checked against the data directory's PG_VERSION.
  #
  # `<export>/import.json` says what came over and what did not, row by
  # row; once it says complete this does nothing. A failure leaves
  # everything where it was, fails this unit, and the next start tries
  # again; LosOS Git serves the repositories meanwhile.
  exportDir = "${stateDir}/forgejo";
  tables = lib.concatStringsSep " " [
    "user"
    "org_user"
    "repository"
    "collaboration"
    "access_token"
    "label"
    "issue"
    "pull_request"
    "comment"
    "issue_label"
    "issue_assignees"
    "attachment"
    "milestone"
    "release"
  ];
  # Repositories an older start or the import left owned by someone else.
  ownRepositories = pkgs.writeShellScript "losos-git-own" ''
    if [ -d ${stateDir}/repositories ]; then
      chown -R ${toString id}:${toString id} -- ${stateDir}/repositories
    fi
  '';
  # The Postgres the box ran. Once the apps that needed it are gone it is
  # off, and the package its stateVersion picked (26.11: 18) is named here.
  pg =
    if config.services.postgresql.enable then
      config.services.postgresql.package
    else
      pkgs.postgresql_18;
  importForgejo = pkgs.writeShellApplication {
    name = "losos-git-forgejo-import";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.findutils
      pkgs.gnugrep
      pkgs.util-linux
      pkgs.sqlite
      pg
    ];
    text = ''
      old=/var/lib/forgejo
      out=${exportDir}
      [ -d "$old" ] || exit 0
      if grep -qs '"complete": true' "$out/import.json"; then
        exit 0
      fi
      install -d -m 0700 -o losos-git -g losos-git ${stateDir}
      rm -rf "$out"
      install -d -m 0700 "$out" "$out/tables"
      : > "$out/export-notes.txt"
      note() { printf '%s\n' "$*" >> "$out/export-notes.txt"; echo "losos-git: $*" >&2; }

      if [ -d "$old/repositories" ] && [ ! -e ${stateDir}/repositories ]; then
        echo "losos-git: moving Forgejo's repositories into LosOS Git" >&2
        mv -- "$old/repositories" ${stateDir}/repositories
      fi

      export_json() { # table, command that prints a JSON array
        local t=$1; shift
        if ! "$@" > "$out/tables/$t.json"; then
          return 1
        fi
        [ -s "$out/tables/$t.json" ] || echo '[]' > "$out/tables/$t.json"
      }

      pgdir=""
      for d in /var/lib/postgresql/*/; do
        if [ -f "$d/PG_VERSION" ]; then pgdir=''${d%/}; fi
      done

      if [ -f "$old/data/forgejo.db" ]; then
        echo "losos-git: exporting Forgejo's SQLite database" >&2
        sqlite3 "$old/data/forgejo.db" ".backup '$out/forgejo.db'"
        for t in ${tables}; do
          if [ -n "$(sqlite3 "$out/forgejo.db" "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '$t'")" ]; then
            export_json "$t" sqlite3 -json "$out/forgejo.db" "SELECT * FROM \"$t\""
          else
            note "Forgejo's database had no table $t."
          fi
        done
      elif [ -n "$pgdir" ]; then
        tmp=""
        stop() {
          if [ -n "$tmp" ]; then
            runuser -u losos-git -- pg_ctl -D "$tmp/data" -m fast -w stop >/dev/null 2>&1 || true
            rm -rf "$tmp"
          fi
        }
        trap stop EXIT
        if [ -S /run/postgresql/.s.PGSQL.5432 ]; then
          as=(runuser -u postgres --)
          conn=(-h /run/postgresql -U postgres)
        else
          want=$(cut -d. -f1 <<< "${pg.version}")
          have=$(cat "$pgdir/PG_VERSION")
          if [ "$have" != "$want" ]; then
            echo "losos-git: Postgres data in $pgdir is version $have, this box has $want" >&2
            exit 1
          fi
          echo "losos-git: starting a copy of Postgres's data to export Forgejo's database" >&2
          tmp=$(mktemp -d ${stateDir}/.postgres.XXXXXX)
          cp -a --reflink=auto "$pgdir" "$tmp/data"
          rm -f "$tmp/data/postmaster.pid"
          printf 'local all all trust\n' > "$tmp/hba.conf"
          : > "$tmp/postgresql.conf"
          chown -R losos-git:losos-git "$tmp"
          chmod 0700 "$tmp/data"
          runuser -u losos-git -- pg_ctl -D "$tmp/data" -w -t 300 -l "$tmp/log" \
            -o "-c config_file=$tmp/postgresql.conf -c hba_file=$tmp/hba.conf -c listen_addresses= -c unix_socket_directories=$tmp" \
            start
          as=(runuser -u losos-git --)
          conn=(-h "$tmp" -U postgres)
        fi
        if [ "$("''${as[@]}" psql "''${conn[@]}" -d postgres -Atc "SELECT 1 FROM pg_database WHERE datname = 'forgejo'")" = 1 ]; then
          echo "losos-git: exporting Forgejo's Postgres database" >&2
          "''${as[@]}" pg_dump "''${conn[@]}" -d forgejo > "$out/forgejo.sql"
          for t in ${tables}; do
            if [ "$("''${as[@]}" psql "''${conn[@]}" -d forgejo -Atc "SELECT to_regclass('public.\"$t\"') IS NOT NULL")" = t ]; then
              export_json "$t" "''${as[@]}" psql "''${conn[@]}" -d forgejo -Atc \
                "SELECT coalesce(json_agg(r), '[]'::json) FROM \"$t\" r"
            else
              note "Forgejo's database had no table $t."
            fi
          done
        else
          note "Forgejo's database was not in Postgres; only its repositories were carried over."
        fi
        stop
        trap - EXIT
      else
        note "Forgejo's database was not found; only its repositories were carried over."
      fi

      if [ -d "$old/data/attachments" ]; then
        cp -a --reflink=auto "$old/data/attachments" "$out/attachments"
      fi
      chown -R losos-git:losos-git ${stateDir}
      runuser -u losos-git -- ${pkg}/bin/losos-git import-forgejo \
        --state-dir ${stateDir} --from "$out"
    '';
  };
in
{
  config = lib.mkIf on {
    users.groups.losos-git.gid = id;
    users.groups.losos-git-peers = { };
    systemd.tmpfiles.rules = [ "d ${peersDir} 0750 losos-git losos-git-peers - -" ];
    users.users.losos-git = {
      uid = id;
      group = "losos-git";
      isSystemUser = true;
      description = "LosOS Git";
    };
    # The socket's group: nginx and nobody else.
    users.users.${config.services.nginx.user} = lib.mkIf config.services.nginx.enable {
      extraGroups = [ "losos-git" ];
    };

    systemd.services.losos-git-forgejo-import = {
      description = "Carry Forgejo's records into LosOS Git";
      wantedBy = [ "multi-user.target" ];
      before = [ "losos-git.service" ];
      after = [ "postgresql.service" ];
      unitConfig.ConditionPathExists = "/var/lib/forgejo";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = lib.getExe importForgejo;
        UMask = "0077";
      };
    };

    systemd.services.losos-git = {
      description = "LosOS Git";
      wantedBy = [ "multi-user.target" ];
      # An import that fails does not hold the repositories back.
      wants = [ "losos-git-forgejo-import.service" ];
      after = [
        "network.target"
        "losos-git-forgejo-import.service"
      ];
      # nginx opens the socket; it waits for nothing of ours to start.
      before = lib.optional config.services.nginx.enable "nginx.service";
      # curl asks the box's code model for LosOS Reviewer
      # (backend/src/forge/reviewer/).
      path = [
        git
        pkgs.curl
      ];
      # LosOS Reviewer asks the model Build with AI runs (modules/builder.nix),
      # with the same context size so Ollama never loads it twice. Without
      # Build with AI the reviewer is off and its setting says why.
      environment = {
        RUST_LOG = "info";
      }
      // lib.optionalAttrs config.losos.builder.enable {
        LOSOS_REVIEWER_URL = "http://127.0.0.1:11434";
        LOSOS_REVIEWER_MODEL = config.losos.builder.model;
        LOSOS_REVIEWER_CONTEXT = toString config.losos.builder.contextTokens;
      };
      # Keep trying: on the first switch from a Forgejo generation the old
      # pod can hold port 3000 for a moment.
      startLimitIntervalSec = 0;
      serviceConfig = {
        User = "losos-git";
        Group = "losos-git";
        ExecStartPre = [ "+${ownRepositories}" ];
        ExecStart = lib.escapeShellArgs (
          [
            "${pkg}/bin/losos-git"
            "serve"
            "--state-dir"
            stateDir
            "--socket"
            socket
            "--listen"
            "127.0.0.1:${toString port}"
            "--git"
            "${git}/bin/git"
            # Services on the box (Mesh CI) read commits and write checks
            # here; the directory, made below, is the gate.
            "--internal-socket"
            "${peersDir}/internal.sock"
            # Pushes and pull requests, for Mesh CI (modules/ci.nix). On a
            # box without it the socket is missing and nothing is sent.
            "--events-socket"
            "/run/losos-ci-events/events.sock"
            "--matrix-server-name"
            chat.serverName
          ]
          # Repository rooms in LosOS Chat (modules/chat.nix).
          ++ lib.optionals config.lososInternal.chat [
            "--events-socket"
            "/run/losos-git-chat/events.sock"
          ]
          ++ lib.optional config.losos.cloud.mesh.selfSigned "--matrix-self-signed"
          ++ lib.optional cfg.site.requireSignIn "--require-sign-in"
          ++ lib.optional (cfg.site.defaultPrivate == "public") "--public-by-default"
          ++ lib.optional cfg.site.pushCreate "--push-create"
        );
        Restart = "on-failure";
        RestartSec = "2s";
        StateDirectory = "losos-git";
        StateDirectoryMode = "0700";
        RuntimeDirectory = "losos-git";
        # nginx (in the group) has to reach the socket inside.
        RuntimeDirectoryMode = "0750";
        ReadWritePaths = [ peersDir ];
        UMask = "0077";

        NoNewPrivileges = true;
        CapabilityBoundingSet = "";
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
          "AF_INET6"
        ];
        # People reach it through nginx's socket, lososd on 127.0.0.1, and
        # LosOS Reviewer asks the model on 127.0.0.1. The only other address
        # it dials is a chat server's, to ask who signs in with a chat
        # address: a GET of a fixed path, never with code in it
        # (backend/src/forge/matrix.rs), and never this box itself.
        IPAddressDeny = lib.mkIf (!chatOutside) "any";
        IPAddressAllow = [
          "localhost"
          "10.0.0.0/8"
          "172.16.0.0/12"
          "192.168.0.0/16"
          "fc00::/7"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
      };
    };
  };
}
