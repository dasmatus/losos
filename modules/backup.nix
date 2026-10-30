# Backups to an S3 bucket, restoring them, and the boot-time wipe of an erase.
#
# lososd decides (backend/src/backup.rs, backend/src/erase.rs); this module
# provides what it starts. Three pieces:
#
#   losos-backup     a script lososd runs as a `systemd-run` transient unit
#                    (one unit per job, `losos-backup-<job>`). It dumps the
#                    databases, opens the fscrypt domain if sharing left it
#                    locked, and hands everything to restic, which encrypts it
#                    on the box before it goes to the bucket.
#   losos-restore    the same shape the other way: restic pulls the latest
#                    snapshot into a staging directory, the apps are stopped,
#                    the files and databases are put back, the apps start.
#   losos-factory-wipe.service
#                    runs early in the boot after an erase, when lososd has
#                    left a marker on /persist, and deletes the owner's data
#                    before any service that owns it has started.
#
# WHY TRANSIENT UNITS AND NOT lososd ITSELF. lososd runs with ProtectHome=true
# (modules/daemon.nix) so a compromised request handler cannot read either
# data domain. A transient unit is started by PID 1 outside that sandbox, and
# the copy needs to read both. It also survives the lososd restart a rebuild
# causes, the same reason rebuilds run this way (backend/src/supervisor.rs).
#
# SECRETS. The bucket's keys are the credential `backup-bucket`, which lososd
# seals into the credential store when the owner saves the bucket
# (modules/credentials.nix); each job gets it as LoadCredentialEncrypted= and
# reads it from its own credential directory. The repository password is
# RESTIC_PASSWORD_FILE
# pointing at the recovery code (or, for a restore, at the code the owner
# typed, on /run). None of them is ever an argument, so none is in `ps` or the
# journal.
#
# WHY THE WIPE IS AT BOOT. On a running box LosOS Cloud, LosOS Git, both
# Kubernetes instances and lososd itself hold files open in exactly the
# directories that have to go. Deleting them underneath live services leaves
# half-written state behind for whatever restarts first. Early in the next
# boot nothing has started yet: the unit is ordered before sysinit.target and
# before tmpfiles, so every service then finds an empty directory and sets
# itself up as on the first boot.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  enabled = config.losos.backend.package != null;
  fs = config.lososInternal.fscrypt;
  fscryptOn = config.losos.shared.fscrypt.enable;
  k3s = config.services.k3s.enable;
  # pg_dump and pg_restore for a box that still runs PostgreSQL. Nothing
  # declares it any more, and its package option then has no value.
  pg =
    if config.services.postgresql.enable then
      config.services.postgresql.package
    else
      pkgs.postgresql_18;

  # The scripts' working directory: the job log, the last report, and the
  # staging directories. The wipe deletes it with the rest, since a staging
  # copy of the databases is the owner's data.
  workDir = "/var/lib/losos-backup";
  # What an erase gave up outside the box, as counts. The one directory the
  # wipe keeps, so the next owner (or the same one) can see it happened.
  eraseDir = "/var/lib/losos-erase";
  marker = "/persist/.losos-factory-wipe";

  # The local cluster's containerd (modules/cluster.nix, `localCriSocket`),
  # not rke2's: the box's own pods are the ones holding the app data.
  localCri = "unix:///run/containerd/containerd.sock";

  # Where everything lives. Absolute, because restic stores absolute paths
  # and a restore puts each one back where it came from.
  appDirs = [
    "/var/lib/losos-cloud"
    "/var/lib/losos-git"
  ];
  # LosOS Cloud's database (modules/cloud.nix). SQLite in WAL mode: the live
  # file is copied with SQLite's own backup into the staging folder, and the
  # three files restic would read mid-write are left out of the copy.
  cloudDb = "/var/lib/losos-cloud/cloud.db";
  # LosOS Git's records (modules/git.nix), copied whole the same way.
  gitDb = "/var/lib/losos-git/git.db";
  proxyToken = toString config.losos.proxy.tokenFile;
  # LosOS Mail's store (modules/mail.nix), when mail runs here. It sits
  # beside the shared folder, not inside it, so the folder's copy misses
  # it; RocksDB files copied under a running server need not agree with
  # each other, so the server stops for the moment a local copy takes.
  mail = config.lososInternal.mail.live;
  mailStore = "/home/shared/data/mail/store";
  mailPlanMarker = "/home/shared/data/mail/plan.sha256";

  # Opening the fscrypt domain for the length of a copy, and closing it again
  # only if it was closed before. Linux gives no ciphertext for a locked
  # fscrypt file (open() is ENOKEY), so there is no way to copy the domain
  # without its key; what keeps it encrypted is restic, which encrypts what
  # it read before it leaves the box. modules/fscrypt.nix owns the key; this
  # reuses its own snippet for putting it somewhere.
  domainFns =
    if fscryptOn then
      ''
        policy=""
        relock=0
        keydir=""
        open_domain() {
          local candidate
          candidate=${fs.policyDirExpr}
          if [ ! -d "$candidate" ]; then
            return 0
          fi
          policy=$candidate
          if fscrypt status "$policy" 2>/dev/null | grep -c '^Unlocked: *No' >/dev/null; then
            keydir=$(mktemp -d /run/losos-backup-key.XXXXXX)
            local key="$keydir/key"
            ${fs.materialiseKeyTo ''"$key"''}
            fscrypt unlock "$policy" --key="$key" --quiet
            rm -f "$key"
            relock=1
            echo "${"$"}{0##*/}: opened the shared folder for the copy"
          fi
        }
        close_domain() {
          if [ "$relock" = 1 ]; then
            fscrypt lock "$policy" --quiet \
              || echo "${"$"}{0##*/}: the shared folder could not be locked now (files open); it locks on the next boot" >&2
            relock=0
          fi
          if [ -n "$keydir" ]; then
            rm -rf "$keydir"
            keydir=""
          fi
        }
      ''
    else
      ''
        policy=""
        open_domain() {
          local candidate
          candidate="/persist/home/shared/data/$(cat /etc/machine-id)/shared"
          if [ -d "$candidate" ]; then
            policy=$candidate
          fi
        }
        close_domain() { :; }
      '';

  # restic's S3 options from the two variables lososd writes beside the keys
  # (backup.rs, `Target::env_file`). Only the data packs take a Glacier
  # class; restic keeps its own metadata in Standard, so opening the
  # repository and checking a code stay instant. GLACIER and DEEP_ARCHIVE
  # packs are not readable until AWS thaws them: a restore turns on restic's
  # `s3-restore` feature, which asks for the thaw at AWS's Standard tier
  # (3 to 5 hours for Flexible Retrieval, up to 12 for Deep Archive) and
  # waits up to 48 hours, and tidying old backups never repacks,
  # which would need a thaw as well.
  #
  # The keys first, out of the job's credential directory: the lines
  # `Target::env_file` writes, each name checked here as well, so the file
  # can only ever set these six.
  #
  # Storage bought on the mesh (backup.rs, `BoughtStorage`) is a restic REST
  # repository behind the edge's backup door, and its file holds no secret,
  # only the order (LOSOS_DOOR_ORDER). The door's password is made here
  # from the code file the job is given, as backup.rs's `door_secret` does:
  # the box's own code for a backup, the code the owner typed for a
  # restore, so a new box opens the old one's door with the old code.
  s3Fns = ''
    bucket="''${CREDENTIALS_DIRECTORY:?no credentials}/backup-bucket"
    while IFS='=' read -r k v; do
      case "$k" in
        RESTIC_REPOSITORY | AWS_ACCESS_KEY_ID | AWS_SECRET_ACCESS_KEY | AWS_DEFAULT_REGION | LOSOS_S3_STORAGE_CLASS | LOSOS_S3_THAW | LOSOS_DOOR_ORDER)
          export "$k=$v"
          ;;
      esac
    done < "$bucket"
    if [ -n "''${LOSOS_DOOR_ORDER:-}" ]; then
      code=$(tr -d '[:space:]' < "''${RESTIC_PASSWORD_FILE:?}")
      export RESTIC_REST_USERNAME="$LOSOS_DOOR_ORDER"
      RESTIC_REST_PASSWORD=$(printf 'losos-backup-door\n%s\n%s' "$LOSOS_DOOR_ORDER" "$code" | sha256sum | cut -c1-64)
      export RESTIC_REST_PASSWORD
      unset code
    fi
    s3opts=()
    if [ -n "''${LOSOS_S3_STORAGE_CLASS:-}" ]; then
      s3opts+=(-o "s3.storage-class=$LOSOS_S3_STORAGE_CLASS")
    fi
    thaw=''${LOSOS_S3_THAW:-0}
  '';

  common = [
    pkgs.restic
    pkgs.coreutils
    pkgs.findutils
    pkgs.gnugrep
    pkgs.jq
    pkgs.util-linux
    pkgs.systemd
    pkgs.rsync
    pkgs.sqlite
    pg
  ]
  ++ lib.optional fscryptOn fs.package
  ++ lib.optional k3s pkgs.cri-tools;

  backupScript = pkgs.writeShellApplication {
    name = "losos-backup";
    runtimeInputs = common;
    text = ''
      dir=''${LOSOS_BACKUP_DIR:?}
      stage="$dir/stage"
      umask 077
      # A transient unit has no HOME, and restic wants a cache directory.
      export RESTIC_CACHE_DIR="$dir/cache"
      ${s3Fns}
      ${domainFns}
      # The staging copy holds database dumps: it never outlives the run.
      trap 'close_domain; rm -rf "$stage"' EXIT

      echo "backup: checking the bucket"
      if ! probe=$(restic cat config 2>&1 >/dev/null); then
        case "$probe" in
          *"Is there a repository"* | *"unable to open config file"* | *"config file does not exist"*)
            echo "backup: first backup to this bucket, creating the repository"
            restic "''${s3opts[@]}" init
            ;;
          *)
            echo "$probe" >&2
            exit 1
            ;;
        esac
      fi

      rm -rf "$stage"
      mkdir -p "$stage/databases" "$stage/settings" "$stage/look" "$stage/identity" "$stage/git"

      if [ -f ${cloudDb} ]; then
        echo "backup: copying LosOS Cloud's database"
        sqlite3 ${cloudDb} ".backup '$stage/databases/cloud.db'"
      fi

      if [ -f ${gitDb} ]; then
        echo "backup: copying LosOS Git's records"
        sqlite3 ${gitDb} ".backup '$stage/git/git.db'"
      fi

      if [ -f /etc/nixos/modules/overrides.nix ]; then
        cp /etc/nixos/modules/overrides.nix "$stage/settings/overrides.nix"
      fi
      for f in look.json background.img; do
        if [ -f "/var/lib/losos/$f" ]; then
          cp "/var/lib/losos/$f" "$stage/look/$f"
        fi
      done
      # The people who sign in (modules/identity.nix): names, groups and
      # password hashes.
      if [ -f /var/lib/losos-identity/users.yml ]; then
        cp /var/lib/losos-identity/users.yml "$stage/identity/users.yml"
      fi
      if [ -s ${lib.escapeShellArg proxyToken} ]; then
        cp ${lib.escapeShellArg proxyToken} "$stage/identity/proxy-token"
      fi

      ${lib.optionalString mail ''
        if [ -d ${mailStore} ]; then
          echo "backup: copying LosOS Mail, which pauses for the copy"
          mail_was_up=0
          if systemctl is-active --quiet losos-mail.service; then
            mail_was_up=1
            systemctl stop losos-mail.service
          fi
          cp -a --reflink=auto ${mailStore} "$stage/mail" || copied=no
          if [ "$mail_was_up" = 1 ]; then
            systemctl start losos-mail.service || true
          fi
          if [ "''${copied:-}" = no ]; then
            echo "backup: LosOS Mail could not be copied" >&2
            exit 1
          fi
        fi
      ''}
      paths=("$stage")
      for p in ${lib.escapeShellArgs appDirs}; do
        if [ -d "$p" ]; then
          paths+=("$p")
        fi
      done
      open_domain
      if [ -n "$policy" ]; then
        paths+=("$policy")
      fi

      echo "backup: copying ''${#paths[@]} folders"
      summary=$(restic "''${s3opts[@]}" backup --json --quiet --tag losos \
        --exclude '${cloudDb}' --exclude '${cloudDb}-wal' --exclude '${cloudDb}-shm' \
        --exclude '/var/lib/losos-cloud/.previews' --exclude '/var/lib/losos-cloud/*/uploads' \
        "''${paths[@]}" | jq -c 'select(.message_type == "summary")')
      close_domain
      rm -rf "$stage"

      jq -n --argjson s "$summary" --arg now "$(date +%s)" \
        '{time: ($now | tonumber), snapshot: $s.snapshot_id,
          files: $s.total_files_processed, bytes: $s.total_bytes_processed}' \
        > "$dir/last.json.tmp"
      mv "$dir/last.json.tmp" "$dir/last.json"
      echo "backup: done, snapshot $(jq -r .snapshot_id <<<"$summary" | cut -c1-8)"

      # Keep the last seven. A failure here leaves the new snapshot safe.
      # In a class that needs a thaw, only packs nothing uses are deleted.
      tidy=()
      if [ "$thaw" = 1 ]; then
        tidy+=(--max-repack-size 0)
      fi
      restic "''${s3opts[@]}" forget --quiet --tag losos --keep-last 7 --prune "''${tidy[@]}" \
        || echo "backup: older backups could not be tidied away this time" >&2
    '';
  };

  restoreScript = pkgs.writeShellApplication {
    name = "losos-restore";
    runtimeInputs = common;
    text = ''
      dir=''${LOSOS_BACKUP_DIR:?}
      out="$dir/restore"
      umask 077
      export RESTIC_CACHE_DIR="$dir/cache"
      ${s3Fns}
      ${domainFns}

      stopped=()
      start_apps() {
        if [ "''${#stopped[@]}" -gt 0 ]; then
          echo "restore: starting the apps"
          systemctl start "''${stopped[@]}" || true
          stopped=()
        fi
      }
      cleanup() {
        close_domain
        start_apps
        rm -rf "$out"
      }
      trap cleanup EXIT

      rm -rf "$out"
      mkdir -p "$out"
      if [ "$thaw" = 1 ]; then
        export RESTIC_FEATURES=s3-restore
        s3opts+=(-o s3.enable-restore=true -o s3.restore-timeout=48h)
        echo "restore: asking AWS to thaw the backup; this takes hours"
      fi
      echo "restore: fetching the latest backup"
      restic "''${s3opts[@]}" restore latest --tag losos --target "$out"
      staged="$out$dir/stage"

      echo "restore: stopping the apps"
      for unit in losos-cloud.service losos-git.service ${lib.optionalString mail "losos-mail.service"} k3s.service; do
        if systemctl is-active --quiet "$unit"; then
          systemctl stop "$unit" || true
          stopped+=("$unit")
        fi
      done
      ${lib.optionalString k3s ''
        # Stopping k3s leaves its pods running; stop them through the
        # runtime so nothing writes while the files are swapped.
        crictl --runtime-endpoint ${localCri} pods --quiet \
          | xargs -r crictl --runtime-endpoint ${localCri} stopp || true
      ''}

      for p in ${lib.escapeShellArgs appDirs}; do
        if [ -d "$out$p" ]; then
          echo "restore: putting back $p"
          mkdir -p "$p"
          rsync -aHAX --numeric-ids --delete "$out$p/" "$p/"
        fi
      done
      if [ -f "$staged/databases/cloud.db" ]; then
        echo "restore: putting back LosOS Cloud's database"
        rm -f ${cloudDb}-wal ${cloudDb}-shm
        install -m 0600 "$staged/databases/cloud.db" ${cloudDb}
        chown --reference=/var/lib/losos-cloud ${cloudDb}
      fi

      if [ -s "$staged/git/git.db" ]; then
        echo "restore: putting back LosOS Git's records"
        rm -f ${gitDb}-wal ${gitDb}-shm
        # losos-git's uid and gid (modules/git.nix).
        install -m 0600 -o 1003 -g 1003 "$staged/git/git.db" ${gitDb}
      fi

      # A backup from before LosOS Git holds Forgejo's folder and database
      # and no git.db. Both go back as Forgejo left them, LosOS Git's own
      # records go, and LosOS Git's import (modules/git.nix) carries them
      # over again, as it did on the upgrade.
      legacy=0
      if [ ! -s "$staged/git/git.db" ] && [ -d "$out/var/lib/forgejo" ]; then
        legacy=1
        echo "restore: this backup is from before LosOS Git; putting back Forgejo's data to import"
        mkdir -p /var/lib/forgejo
        rsync -aHAX --numeric-ids --delete "$out/var/lib/forgejo/" /var/lib/forgejo/
        rm -rf /var/lib/losos-git/repositories /var/lib/losos-git/forgejo
        rm -f ${gitDb} ${gitDb}-wal ${gitDb}-shm
        if [ -s "$staged/databases/forgejo.pgdump" ]; then
          if systemctl is-active --quiet postgresql.service; then
            echo "restore: loading Forgejo's database for the import"
            runuser -u postgres -- dropdb --if-exists forgejo
            runuser -u postgres -- createdb forgejo
            runuser -u postgres -- pg_restore --no-owner --dbname=forgejo < "$staged/databases/forgejo.pgdump"
          else
            echo "restore: Forgejo's database needs Postgres, which this box no longer runs; only its repositories come back" >&2
          fi
        fi
      fi

      if systemctl is-active --quiet postgresql.service; then
        for dump in "$staged"/databases/*.pgdump; do
          [ -e "$dump" ] || continue
          db=$(basename "$dump" .pgdump)
          [ "$db" = forgejo ] && continue
          if runuser -u postgres -- psql -tAc "select 1 from pg_database where datname = '$db'" | grep -cx 1 >/dev/null; then
            echo "restore: loading the $db database"
            runuser -u postgres -- pg_restore --clean --if-exists --dbname="$db" < "$dump"
          else
            echo "restore: this box has no $db database; skipped" >&2
          fi
        done
      fi

      shopt -s nullglob
      sources=("$out"/persist/home/shared/data/*/shared)
      shopt -u nullglob
      if [ "''${#sources[@]}" -gt 0 ]; then
        open_domain
        if [ -n "$policy" ]; then
          echo "restore: putting back the shared folder"
          rsync -aHAX --numeric-ids --delete "''${sources[0]}/" "$policy/"
        else
          echo "restore: the shared folder is not set up on this box yet; skipped" >&2
        fi
        close_domain
      fi

      ${lib.optionalString mail ''
        if [ -d "$staged/mail" ]; then
          echo "restore: putting back LosOS Mail"
          mkdir -p ${mailStore}
          rsync -aHAX --numeric-ids --delete "$staged/mail/" ${mailStore}/
          chown -R shared:shared ${mailStore}
          # The restored settings may be older than this build's plan:
          # the next start applies it again.
          rm -f ${mailPlanMarker}
        fi
      ''}
      for f in look.json background.img; do
        if [ -f "$staged/look/$f" ]; then
          install -m 0600 "$staged/look/$f" "/var/lib/losos/$f"
        fi
      done
      if [ -s "$staged/identity/users.yml" ]; then
        install -D -m 0640 "$staged/identity/users.yml" /var/lib/losos-identity/users.yml
        chgrp authelia-losos /var/lib/losos-identity/users.yml 2>/dev/null || true
      fi
      if [ -s "$staged/identity/proxy-token" ]; then
        install -D -m 0600 "$staged/identity/proxy-token" ${lib.escapeShellArg proxyToken}
      fi
      # The code that opened this backup becomes the box's code, so the box
      # keeps the identity the backup had and its next backup goes to the
      # same repository.
      install -D -m 0600 "''${RESTIC_PASSWORD_FILE:?}" /var/secrets/losos-recovery-code
      if [ -f "$staged/settings/overrides.nix" ]; then
        install -m 0600 "$staged/settings/overrides.nix" "$dir/restored-overrides.nix"
      fi
      if [ "$legacy" = 1 ]; then
        echo "restore: carrying Forgejo's records into LosOS Git"
        systemctl start losos-git-forgejo-import.service \
          || echo "restore: the import did not finish; LosOS Git tries it again when it next starts" >&2
      fi
      echo "restore: done"
    '';
  };

  # The owner's data, as /persist paths: what impermanence bind-mounts back
  # (modules/impermanence.nix). Everything the apps, the clusters and lososd
  # made, and the secrets they minted; not /nix, not /etc/nixos (the flake
  # the box builds from), not /etc/keys (what unlocks the disk).
  wipeDirs = [
    "/persist/var/lib/losos-cloud"
    # What a box's Nextcloud left behind once LosOS Cloud took its files in.
    "/persist/var/lib/nextcloud"
    "/persist/var/lib/losos-git"
    # What LosOS Git's import leaves of Forgejo on an upgraded box
    # (modules/git.nix keeps it). Without it the old owner's database and
    # attachments outlive the reset, and the import runs again next boot.
    "/persist/var/lib/forgejo"
    "/persist/var/lib/postgresql"
    "/persist/var/lib/redis-nextcloud"
    "/persist/var/lib/losos"
    # The text of the owner's cloud files, for the palette (modules/search.nix).
    "/persist/var/lib/losos-search"
    "/persist/var/lib/losos-backup"
    "/persist/var/lib/losos-public-names"
    # Who can sign in, and Authelia's sessions (modules/identity.nix).
    "/persist/var/lib/losos-identity"
    "/persist/var/lib/authelia-losos"
    "/persist/var/lib/redis-losos-login"
    "/persist/var/lib/rancher"
    "/persist/var/lib/kubelet"
    "/persist/var/lib/longhorn"
    "/persist/var/secrets"
    # Every credential the box sealed (modules/credentials.nix).
    "/persist/etc/credstore.encrypted"
    "/persist/var/log/journal"
    "/persist/etc/rancher"
    # fscrypt's protector metadata: the old protector's key is in
    # /var/secrets and goes with it, so losos-fscrypt-setup makes new ones.
    "/persist/.fscrypt"
  ];
  # The two data domains are emptied, not removed: impermanence expects the
  # directories, and their modes are the isolation model.
  emptyDirs = [
    "/persist/home/notshared/data"
    "/persist/home/shared/data"
  ];

  wipeScript = pkgs.writeShellApplication {
    name = "losos-factory-wipe";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.findutils
    ];
    text = ''
      echo "erase: deleting the owner's data"
      for d in ${lib.escapeShellArgs wipeDirs}; do
        rm -rf --one-file-system "$d"
      done
      for d in ${lib.escapeShellArgs emptyDirs}; do
        if [ -d "$d" ]; then
          find "$d" -mindepth 1 -maxdepth 1 -exec rm -rf --one-file-system {} +
        fi
      done
      # Last, so an interrupted wipe runs again on the next boot.
      sync
      rm -f ${marker}
      sync
      echo "erase: done; the box starts as on its first boot"
    '';
  };
in
{
  # The credential store every secret here lives in.
  imports = [ ./credentials.nix ];

  config = lib.mkIf enabled {
    systemd.services.lososd.environment = {
      LOSOS_BACKUP_SCRIPT = lib.getExe backupScript;
      LOSOS_RESTORE_SCRIPT = lib.getExe restoreScript;
      LOSOS_BACKUP_DIR = workDir;
      LOSOS_ERASE_GRACE_SECS = toString (config.losos.reset.graceMinutes * 60);
      LOSOS_ERASE_REPORT = "${eraseDir}/report.json";
      LOSOS_WIPE_MARKER = marker;
    };

    # The bucket's keys, set by the owner on the Backups pane and sealed by
    # lososd. An older build kept them as plaintext beside the bucket's JSON.
    losos.credentials.entries.backup-bucket = {
      app = "backup";
      description = "The keys of the bucket backups go to, as the owner typed them.";
      pane = "/settings/backup";
      legacy = [ "/var/secrets/losos-backup.env" ];
    };

    systemd.tmpfiles.rules = [
      "d ${workDir} 0700 root root -"
      "d ${eraseDir} 0700 root root -"
    ];

    systemd.services.losos-factory-wipe = {
      description = "Delete the owner's data after an erase";
      wantedBy = [ "sysinit.target" ];
      before = [
        "sysinit.target"
        "systemd-tmpfiles-setup.service"
        "systemd-journal-flush.service"
      ];
      after = [ "local-fs.target" ];
      unitConfig = {
        DefaultDependencies = false;
        ConditionPathExists = marker;
        RequiresMountsFor = [ "/persist" ];
      };
      serviceConfig = {
        Type = "oneshot";
        ExecStart = lib.getExe wipeScript;
      };
    };
  };
}
