# The box's credential manager: every secret it keeps, encrypted by
# systemd-creds and handed to services by systemd.
#
# WHERE THEY LIVE. One file per secret in /etc/credstore.encrypted, the
# directory systemd itself searches when a unit names a credential with
# `LoadCredentialEncrypted=<name>` and no path. /etc is tmpfs here, so
# modules/impermanence.nix binds that one directory back from /persist, and
# the erase deletes it (modules/backup.nix). Each file is AES256-GCM, sealed to
# the TPM2 chip with no PCR binding when the box has one, and to the root-only
# host key /var/lib/systemd/credential.secret (under /var, so persisted) when
# it does not. The name is embedded in the file and checked on every decrypt,
# so a file renamed over another secret's is refused, not used.
#
# HOW A SERVICE GETS ONE. A module declares the secret under
# losos.credentials.entries.<name> and lists the units that read it; this
# module gives each of them `LoadCredentialEncrypted=<name>`. PID 1 opens the
# file as the unit starts and puts the plaintext in the unit's own credential
# directory (/run/credentials/<unit>/<name>, $CREDENTIALS_DIRECTORY, %d in a
# unit file), readable by that unit alone and gone when it stops. The service
# is told the path, never the value: Authelia gets `..._FILE=%d/authelia-jwt`,
# lososd `LOSOS_ADMIN_TOKEN_FILE=%d/admin-token`. Nothing reads the store
# directly but PID 1, this module's boot unit and lososd.
#
# The one exception is a reader that cannot take a credential directory: the
# Nextcloud pod mounts its admin password by host path, and nix/images.nix has
# the path baked in. An entry with `file` set is opened at boot into
# /run/losos-credentials (tmpfs, 0700) and the old path becomes a link to it,
# the arrangement modules/keyring.nix used before this module replaced it.
#
# WHO MAKES THEM. Most are the box's own (`format` set): the boot unit below
# makes a random value the first time and seals it. A box that ran an older
# build has the same secrets as plaintext files (`legacy`); the boot unit
# adopts each value as it is, so nothing that already holds it breaks, then
# overwrites and deletes the plaintext. A secret the owner gives (`format`
# null: the backup bucket's keys) is sealed by lososd when the owner saves the
# pane that asks for it, and the boot unit only retires its plaintext.
#
# lososd shows the list on the Credentials pane and `losos credentials`
# (backend/src/credentials.rs), from /etc/losos/credentials.json rendered
# below, and makes a new value for an entry with `rotate`. No value is ever in
# its API.
#
# WHAT IT COVERS, and what it does not, in the words modules/keyring.nix used:
# a copy of /persist that leaves the box (a backup, a Longhorn replica, a
# support image) and a file-read bug in a service that could reach /var/secrets
# both find ciphertext. Root on the running box can still ask PID 1 or
# systemd-creds to open anything, by design: the box boots with nobody there.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  cfg = config.losos.credentials;

  store = "/etc/credstore.encrypted";
  runtimeDir = "/run/losos-credentials";
  runtimeName = lib.removePrefix "/run/" runtimeDir;

  # From the declared option, never from systemd-creds' `auto`, which follows
  # the hardware: a box set to tpm.enable = false because its chip misbehaves
  # would otherwise seal everything to that chip. "tpm2" rather than
  # "host+tpm2": the host key sits on the same LUKS volume as the files, so it
  # adds nothing against anyone who can read one of them, and it is a second
  # thing to lose. No PCRs (`--tpm2-pcrs=` on every encrypt, whatever
  # systemd's default is): the box takes firmware and kernel updates
  # unattended, and a PCR that moves would lock it out of its own secrets,
  # the same call the installer makes when it enrols the disk. "host" rather
  # than "null" without a chip: null is a fixed empty key, with neither
  # confidentiality nor authenticity.
  #
  # A box declared tpm.enable with no chip at all (a VM, a test) seals to the
  # host key instead, and says so: that box cannot have unlocked its disk with
  # a TPM either. The key used is written to ${runtimeDir}/key for lososd.
  key = if config.losos.tpm.enable then "tpm2" else "host";

  creds = "${config.systemd.package}/bin/systemd-creds --no-ask-password";

  entries = lib.mapAttrsToList (name: e: e // { inherit name; }) cfg.entries;

  # The shapes the box makes. backend/src/credentials.rs makes the same three
  # when an owner asks for a new value; a change here is a change there.
  mint = {
    "hex-32" = "od -An -tx1 -N32 /dev/urandom | tr -d ' \\n'";
    "hex-64" = "od -An -tx1 -N64 /dev/urandom | tr -d ' \\n'";
    "base64-24" = "head -c 24 /dev/urandom | base64 | tr -d '\\n'";
  };

  # Best effort and no more: ext4 journals metadata and may put the new block
  # elsewhere, so shred(1)'s own warning applies. One write, and it helps in
  # the common case.
  retire = f: ''
    if [ -f ${f} ] && [ ! -L ${f} ]; then
      size=$(stat -c %s ${f})
      if [ "$size" -gt 0 ]; then
        dd if=/dev/urandom of=${f} bs="$size" count=1 conv=notrunc status=none || true
        sync ${f} || true
      fi
      rm -f ${f}
      echo "credentials: $name: deleted the plaintext at ${f}" >&2
    fi
  '';

  ensure =
    e:
    let
      q = lib.escapeShellArg;
    in
    ''
      name=${q e.name}
      blob=${store}/$name
      work=${runtimeDir}/.work/$name
      rm -f "$work" "$blob.new"
      remade=

      # A file that will not open: the TPM was cleared or the host key lost,
      # and there is no shell to look into it from. Keep it aside, loudly,
      # and make a new value below, which the pane then says it did.
      if [ -e "$blob" ] && ! ${creds} decrypt --name="$name" "$blob" - > /dev/null; then
        stamp=$(date +%Y%m%d%H%M%S)
        mv -f "$blob" "${store}/.unreadable-$name-$stamp"
        echo "credentials: $name: the sealed file did not open; kept as .unreadable-$name-$stamp" >&2
        remade=1
      fi

      if [ ! -e "$blob" ]; then
        from=
        ${lib.optionalString (e.legacySealed != null) ''
          if [ -z "$from" ] && [ -e ${q e.legacySealed.path} ] \
            && ${creds} decrypt --name=${q e.legacySealed.name} ${q e.legacySealed.path} "$work"; then
            from=${q e.legacySealed.path}
          fi
        ''}
        for f in ${lib.escapeShellArgs e.legacy}; do
          if [ -z "$from" ] && [ -f "$f" ] && [ ! -L "$f" ] && [ -s "$f" ]; then
            cat "$f" > "$work"
            from=$f
          fi
        done
        if [ -n "$from" ]; then
          echo "credentials: $name: adopting the value an older build kept at $from" >&2
          touch ${runtimeDir}/adopted/"$name"
        ${
          if e.format != null then
            ''
              else
                ${mint.${e.format}} > "$work"
                echo "credentials: $name: made a new value" >&2
            ''
          else
            ""
        }
        fi
        if [ -e "$work" ]; then
          ${creds} --with-key="$LOSOS_CREDENTIAL_KEY" --tpm2-pcrs= encrypt --name="$name" "$work" "$blob.new"
          chmod 0600 "$blob.new"
          mv -f "$blob.new" "$blob"
          rm -f "$work"
        fi
      fi
      if [ -n "$remade" ] && [ -e "$blob" ]; then
        touch ${runtimeDir}/remade/"$name"
      fi

      # Only once the sealed copy is in place.
      if [ -e "$blob" ]; then
        :
        ${lib.concatMapStrings retire (map q e.legacy)}
        ${lib.optionalString (e.legacySealed != null) "rm -f ${q e.legacySealed.path}"}
      fi
      ${lib.optionalString (e.file != null) (materialise e)}
    '';

  # The link for a reader that takes a path, not a credential directory.
  # rename(2) over the old path rather than `ln -sf`, which unlinks first and
  # leaves a moment with nothing there under a running kubelet.
  materialise =
    e:
    let
      path = lib.escapeShellArg e.file.path;
      run = "${runtimeDir}/${e.name}";
    in
    ''
      if [ -e "$blob" ]; then
        ${creds} decrypt --name="$name" "$blob" ${run}.new
        chown ${e.file.owner}:${e.file.owner} ${run}.new
        chmod ${e.file.mode} ${run}.new
        mv -f ${run}.new ${run}
        install -d -m 0700 "$(dirname ${path})"
        ln -sfn ${run} ${path}.losos-new
        mv -fT ${path}.losos-new ${path}
      fi
    '';

  # What lososd reads (backend/src/credentials.rs). No value, no path to one.
  registry = {
    inherit store runtimeDir key;
    entries = map (e: {
      inherit (e)
        name
        app
        description
        format
        services
        restart
        rotate
        pane
        ;
    }) entries;
  };

  # Each secret in its own script: `set -e` is ignored inside anything on
  # the left of `||`, and one secret that fails must not leave the next one
  # unsealed.
  ensureScript = e: pkgs.writeShellScript "losos-credential-${e.name}" ''
    set -euo pipefail
    ${ensure e}
  '';

  unit = u: if lib.hasInfix "." u then u else "${u}.service";

  readers = lib.unique (
    map unit (lib.concatMap (e: e.services ++ lib.optionals (e.file != null) e.file.readers) entries)
  );
in
{
  config = lib.mkMerge [
    {
      assertions = map (e: {
        assertion = builtins.match "[a-z0-9][a-z0-9-]*" e.name != null;
        message = "losos.credentials.entries.${e.name}: a credential name is lowercase letters, digits and dashes.";
      }) entries
      ++ map (e: {
        assertion = e.rotate -> e.format != null;
        message = "losos.credentials.entries.${e.name}: only a credential the box makes (`format` set) can be made new.";
      }) entries;

      warnings = lib.optional (config.losos.keyring != { }) ''
        losos.keyring is no longer read: every secret of the box is in the
        credential store now, whatever it says (modules/credentials.nix).
        Remove the losos.keyring line from overrides.nix.
      '';

      environment.etc."losos/credentials.json".text = builtins.toJSON registry;

      systemd.services = lib.mkMerge (
        [
          {
            losos-credentials = {
              description = "Seal the box's secrets, and adopt the plaintext an older build kept";
              wantedBy = [ "multi-user.target" ];
              after = [ "local-fs.target" ];
              before = readers;
              unitConfig.RequiresMountsFor = [
                store
                "/var/lib/systemd"
              ];
              serviceConfig = {
                Type = "oneshot";
                RemainAfterExit = true;
                UMask = "0077";
                RuntimeDirectory = runtimeName;
                RuntimeDirectoryMode = "0700";
                # A restart (every `nixos-rebuild switch` that changes the
                # script) must not delete the links a running pod reads.
                RuntimeDirectoryPreserve = "yes";
                ProtectHome = true;
                NoNewPrivileges = true;
                ProtectKernelTunables = true;
                LockPersonality = true;
              }
              # Nothing here but systemd-creds needs the chip.
              // lib.optionalAttrs config.losos.tpm.enable {
                DevicePolicy = "closed";
                DeviceAllow = [
                  "/dev/tpmrm0 rw"
                  "/dev/tpm0 rw"
                ];
              };
              path = [
                config.systemd.package
                pkgs.coreutils
              ];
              # A secret that could not be sealed is logged and shown as not
              # set on the pane; its readers fail on their own, loudly, and
              # the rest of the box comes up. The unit itself succeeds, or a
              # `nixos-rebuild switch` would report the whole rebuild failed.
              script = ''
                install -d -m 0700 ${store} ${runtimeDir}/.work ${runtimeDir}/adopted ${runtimeDir}/remade
                export LOSOS_CREDENTIAL_KEY=${key}
                if [ "$LOSOS_CREDENTIAL_KEY" = tpm2 ] && [ ! -e /dev/tpmrm0 ]; then
                  echo "credentials: losos.tpm.enable is on but this machine has no TPM; sealing to the host key" >&2
                  LOSOS_CREDENTIAL_KEY=host
                fi
                echo "$LOSOS_CREDENTIAL_KEY" > ${runtimeDir}/key
                ${lib.concatMapStringsSep "\n" (
                  e: "${ensureScript e} || echo 'credentials: ${e.name}: could not be sealed' >&2"
                ) entries}
                rm -rf ${runtimeDir}/.work
              '';
            };
          }
        ]
        ++ map (
          e:
          lib.genAttrs e.services (_: {
            after = [ "losos-credentials.service" ];
            wants = [ "losos-credentials.service" ];
            serviceConfig.LoadCredentialEncrypted = [ e.name ];
          })
        ) entries
      );
    }
  ];
}
