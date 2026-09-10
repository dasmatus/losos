# fscrypt on the `shared` data domain.
#
# This is the second encryption layer, and it exists for a threat the first one
# does not cover. LUKS (modules/disko.nix) protects a box that is switched off:
# lift the drive, learn nothing. It protects nothing at all once the box has
# booted and /persist is unlocked — at that point every process with root, and
# every service that can be talked into reading a path, sees the shared user's
# contributed files in the clear.
#
# fscrypt closes that. The shared domain is unlocked only while
# losos.sharingMyStorage is on; with sharing off the key is not in the kernel
# keyring and the directory is opaque **even to root on the running machine**.
# Three states, all of which must hold:
#
#   powered off          LUKS locked      -> key unreachable, data opaque
#   booted, sharing off  key not in keyring -> data opaque even to root
#   booted, sharing on   key in keyring   -> Longhorn and the pods can read it
#
# The key lives at losos.shared.fscrypt.keyFile, i.e. inside the LUKS-protected
# /persist. That is what makes the layering compose rather than merely stack:
# powered off, LUKS keeps the key itself unreachable, so the fscrypt layer
# cannot be attacked offline either.
#
# WHY /persist PATHS AND NOT /home/shared/data. impermanence bind-mounts
# /persist/home/shared/data onto /home/shared/data. fscrypt identifies a policy
# by the *mountpoint* that owns its metadata (it keeps that metadata in
# <mountpoint>/.fscrypt), and a bind mount is a different mount entry pointing
# at the same inodes. Running `fscrypt encrypt` against the bind-mounted view
# makes it look for metadata under /home, which is tmpfs and is thrown away on
# reboot. Every path below is therefore the real one under /persist.
#
# This module is the sole reader of losos.shared.fscrypt.*. Before it existed,
# those options were declared and consumed by nothing, three other files
# referenced it by name, and two ordered units against services it was supposed
# to define — and systemd silently ignores an ordering dependency on a unit that
# does not exist, so the whole thing failed invisibly and the btrfs->ext4 trade
# (which gave up data checksums and zstd compression to make fscrypt possible)
# bought nothing.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  cfg = config.losos.shared.fscrypt;
  sharing = config.losos.sharingMyStorage;
  useTpm = config.losos.tpm.enable;

  fscrypt = "${pkgs.fscrypt-experimental}/bin/fscrypt";

  # The real filesystem, not the bind-mounted view — see the header.
  persistRoot = "/persist";
  sharedRoot = "${persistRoot}/home/shared/data";

  keyFile = toString cfg.keyFile;
  # TPM2 path: the plaintext key never lands on disk. `systemd-creds encrypt
  # --with-key=tpm2` seals it to the TPM, so it is released only to a boot
  # state the TPM agrees with; the sealed blob sits next to where the plain
  # keyfile would have been.
  sealedFile = "${keyFile}.tpm2";
  credName = "losos-shared-fscrypt";

  # Where the plaintext key is materialised for the moment fscrypt needs it.
  # RuntimeDirectory, so it is tmpfs and vanishes when the unit stops.
  runDir = "/run/losos-fscrypt";
  runKey = "${runDir}/key";

  # The protector name fscrypt records in its metadata. Stable, because
  # `fscrypt unlock` finds the protector by it on every later boot.
  protector = "losos-shared";

  # Namespaced per contributing box, so a pooled Longhorn volume stays
  # attributable to the machine that supplied it: <machine-id>/<username>.
  #
  # /etc/machine-id is itself a persisted file (modules/impermanence.nix lists
  # it), and systemd populates it in early boot well before multi-user.target,
  # so it is readable by the time these oneshots run. It is read at RUN time,
  # not eval time — a store path derived from a machine id would be wrong on
  # every other box and would bake a machine identifier into a world-readable
  # /nix/store entry.
  policyDirExpr = ''"${sharedRoot}/$(cat /etc/machine-id)/shared"'';

  # Materialise the plaintext key at $runKey, minting it on first use.
  #
  # Idempotent by construction: the sealed blob (TPM) or the keyfile (no-TPM) is
  # generated only when absent, and every later boot just unseals or copies it.
  # 32 bytes because that is fscrypt's raw_key protector length.
  materialiseKey =
    if useTpm then
      ''
        if [ ! -s ${sealedFile} ]; then
          echo "fscrypt: minting a new protector key and sealing it to the TPM"
          head -c 32 /dev/urandom \
            | systemd-creds encrypt --with-key=tpm2 --name=${credName} - ${sealedFile}
          chmod 0600 ${sealedFile}
        fi
        systemd-creds decrypt --name=${credName} ${sealedFile} ${runKey}
      ''
    else
      ''
        # No TPM. Same shape as the LUKS no-TPM path in modules/disko.nix: a
        # 0600 keyfile on the encrypted volume. Weaker than sealing — anything
        # that can read /var/secrets can unlock the domain — but a design that
        # only supported TPM2 would brick every losos.tpm.enable = false
        # machine, which is a supported configuration.
        if [ ! -s ${keyFile} ]; then
          echo "fscrypt: minting a new protector key (no TPM; keyfile mode)"
          install -d -m 0700 "$(dirname ${keyFile})"
          head -c 32 /dev/urandom > ${keyFile}
          chmod 0600 ${keyFile}
        fi
        install -m 0600 ${keyFile} ${runKey}
      '';
in
{
  config = lib.mkIf cfg.enable {
    assertions = [
      {
        # fscrypt is a filesystem feature, not a userspace overlay: without
        # `-O encrypt` at mkfs time every command below fails at run time, on a
        # box with no shell to read the error from. modules/disko.nix sets it;
        # this catches anyone who changes the format without reading why.
        assertion = cfg.enable -> config.losos.shared.fscrypt.keyFile != "";
        message = ''
          losos.shared.fscrypt.enable requires losos.shared.fscrypt.keyFile.
          The domain cannot be locked or unlocked without a protector key.
        '';
      }
    ];

    environment.systemPackages = [ pkgs.fscrypt-experimental ];

    # ── One-time filesystem preparation ──────────────────────────────────────
    # `fscrypt setup` writes /etc/fscrypt.conf and then per-mountpoint metadata
    # at <mountpoint>/.fscrypt. Both are one-time: the ConditionPathExists makes
    # this a no-op on every later boot, the same shape as the adminpass
    # generator in modules/nextcloud-common.nix.
    #
    # /etc is tmpfs here, so fscrypt.conf is regenerated each boot and only the
    # /persist metadata is durable — which is the half that matters, and the
    # reason the condition tests the persistent path rather than the config.
    systemd.services.losos-fscrypt-setup = {
      description = "Prepare /persist for fscrypt (one-time)";
      wantedBy = [ "multi-user.target" ];
      after = [ "persist.mount" ];
      unitConfig.ConditionPathExists = "!${persistRoot}/.fscrypt";
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      path = [
        pkgs.fscrypt-experimental
        pkgs.coreutils
      ];
      script = ''
        set -eu
        # --force: no TTY on this box to answer the prompt with.
        ${fscrypt} setup --quiet --force || true
        ${fscrypt} setup ${persistRoot} --quiet --force
      '';
    };

    # ── Lock / unlock, driven by the sharing toggle ──────────────────────────
    # This is the unit the whole layer exists for. It runs on every boot and on
    # every `nixos-rebuild switch` (its script text changes with
    # losos.sharingMyStorage, so systemd restarts it), and it drives the domain
    # to whichever state the toggle now says.
    #
    # Ordered before both Kubernetes instances: the local cluster's pods and
    # Longhorn on the mesh both mount this domain, and a pod that starts against
    # a locked directory sees an empty encrypted view rather than an error.
    systemd.services.losos-fscrypt-shared = {
      description = "Unlock or lock the shared data domain (fscrypt)";
      wantedBy = [ "multi-user.target" ];
      after = [
        "losos-fscrypt-setup.service"
        "persist.mount"
      ];
      requires = [ "losos-fscrypt-setup.service" ];
      before = [
        "k3s.service"
        "rke2-agent.service"
      ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        UMask = "0077";
        RuntimeDirectory = "losos-fscrypt";
        RuntimeDirectoryMode = "0700";
      };
      path = [
        pkgs.fscrypt-experimental
        pkgs.coreutils
        pkgs.systemd
      ];
      script = ''
        set -eu
        policy=${policyDirExpr}

        ${materialiseKey}

        install -d -m 0700 "$(dirname "$policy")"

        if [ ! -d "$policy" ]; then
          # First time on this box: create the directory and encrypt it while
          # it is still empty. fscrypt refuses to encrypt a non-empty directory
          # and we never work around that — see the else-branch below.
          install -d -m 0700 "$policy"
          ${fscrypt} encrypt "$policy" \
            --quiet \
            --source=raw_key \
            --key=${runKey} \
            --name=${protector} \
            --no-recovery
          chown shared:shared "$policy"
        elif ! ${fscrypt} status "$policy" >/dev/null 2>&1; then
          # The directory exists but carries no policy. Encrypting it would
          # require it to be empty, and fscrypt does not encrypt in place — so
          # doing anything clever here means destroying data. Refuse, loudly,
          # and leave the box running with the domain unprotected rather than
          # empty. There is no shell to recover from a wrong guess.
          echo "fscrypt: $policy exists but is not encrypted; refusing to touch it." >&2
          echo "fscrypt: the shared domain is NOT protected. Move the data aside" >&2
          echo "fscrypt: and let this unit recreate the directory to fix it." >&2
          exit 0
        fi

        ${
          if sharing then
            ''
              # Sharing is on: put the key in the kernel keyring so Longhorn and
              # the pods can read the domain. Unlocking an already-unlocked
              # policy is not an error worth failing the unit over.
              ${fscrypt} unlock "$policy" --key=${runKey} --quiet || true
            ''
          else
            ''
              # Sharing is off: drop the key. From here the directory is opaque
              # to every process on this machine, root included, until the
              # toggle is turned back on and this unit re-runs.
              #
              # `fscrypt lock` fails if anything still holds a file open in the
              # domain, which after a rebuild is the normal case, so the failure
              # is tolerated and the domain locks on the next boot. It is not
              # tolerated silently: say so in the journal.
              ${fscrypt} lock "$policy" --quiet \
                || echo "fscrypt: could not lock $policy now (files still open); it locks on next boot" >&2
            ''
        }

        # The plaintext key never outlives the unit. RuntimeDirectory would
        # remove it anyway when the unit stops, but RemainAfterExit means that
        # is not until shutdown, and there is no reason to leave it readable
        # for the life of the boot.
        rm -f ${runKey}
      '';
    };
  };
}
