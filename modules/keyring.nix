# A keyring for the secrets this box mints for itself.
#
# Not GNOME keyring, and not anything shaped like it. There is no session bus
# here, no desktop, no interactive user and no one to type a passphrase, so an
# agent holding an unlocked store in RAM has nothing to be unlocked *by*. What
# is left that can hand a durable secret to a service on an unattended machine
# is systemd-creds, which this repo already depends on (modules/fscrypt.nix
# seals the fscrypt protector key the same way) and which needs no daemon, no
# extra package and no key-management story of its own.
#
# WHAT THIS COVERS, precisely, because the reassuring version of the claim is
# wrong. Before this module the Nextcloud admin password sat in plaintext at
# /var/secrets/nextcloud-admin-pass, mode 0600, chowned to the Nextcloud uid so
# the pod could read it. Nextcloud needs it exactly once, at
# `occ maintenance:install`, and then keeps it readable forever by the one
# application it is the admin password *for*. This module ends that:
#
#   a copy of /persist that leaves the box   backup, Longhorn replica, support
#                                            image: carries a sealed blob, not
#                                            a password
#   a file-read bug in a service that can    reads ciphertext; the key is in
#   reach /var/secrets                       the TPM or in a root-only file
#                                            under /var/lib/systemd
#   Nextcloud PHP reading its own admin      the plaintext is on tmpfs, and on
#   password off the disk                    a fresh box it never touched the
#                                            disk at all
#
# And what it does NOT cover, so the docs say something true:
#
#   root on the running box                  root can unseal, by design: the
#                                            whole point is that boot needs no
#                                            human
#   a thief who steals the box and presses   also not covered by LUKS. The
#   the power button                         enrolment in backend/src/installer.rs
#                                            binds PCR 0+7 (firmware, Secure
#                                            Boot state) and that thief
#                                            reproduces both
#
# So this is defence in depth, the same posture docs/security-model.md takes
# on the admin headers and the k3s mesh design takes on fscrypt. LUKS covers a
# drive lifted out of a switched-off box. This covers a switched-on one.
#
# HOW IT FITS THE EXISTING WIRING, which was the hard part. Three consumers
# name the plaintext path and none of them can be changed from here:
# services.nextcloud loads it as a systemd credential, modules/workloads.nix
# hostPath-mounts it into the pod with `type: File`, and flake/images.nix
# hardcodes the literal /var/secrets/nextcloud-admin-pass inside the image.
# Moving the path is therefore not an option. Instead the path becomes a
# symlink into a tmpfs RuntimeDirectory, and the durable file next to it is the
# sealed blob. Every consumer follows the symlink without knowing it is there:
# systemd's credential loader opens it, the kubelet's `type: File` check is an
# os.Stat and follows it, and containerd resolves it as root when it binds the
# file into the pod.
#
# The generator in modules/nextcloud-common.nix is left alone and becomes a
# no-op: it is guarded by ConditionPathExists = "!<path>", and by the time it
# runs the path exists. That is deliberate rather than incidental. The
# generator stays the whole story on a box with losos.keyring.enable = false,
# so the two paths cannot silently diverge into "the keyring is on but nothing
# mints the secret".
#
# WHY THERE IS A RESTORE UNIT. Turning the keyring off after it has sealed
# would otherwise leave a symlink on /persist pointing into a /run directory
# nothing creates any more. The generator would then try to mint through a
# dangling link, fail, and take k3s down with it (modules/workloads.nix makes
# the kubelet Require that unit). On a box with no shell that is a brick.
# losos-keyring-restore.service exists so the option is reversible.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  cfg = config.losos.keyring;

  # Derived from the declared option, never from `auto`. `auto` picks the TPM
  # whenever a chip is present, which makes the behaviour follow the hardware
  # instead of the configuration: a box set to tpm.enable = false *because* its
  # TPM is unreliable would then seal its secrets to that unreliable TPM.
  #
  # "tpm2" rather than "host+tpm2". Both the blobs and the host key live on the
  # same LUKS volume, so adding the host key resists no attacker who can
  # already read one of them, and adds a second thing to lose. modules/fscrypt.nix
  # makes the same choice for the same reason.
  #
  # "host" rather than "null" on the no-TPM path. Same shape as the no-TPM LUKS
  # keyfile in modules/disko.nix: a root-only key sitting beside the ciphertext.
  # It still buys two real things. Reading /var/secrets alone yields nothing,
  # and AES256-GCM is authenticated, so an edited blob fails to decrypt loudly
  # instead of quietly installing an attacker-chosen admin password. "null" is a
  # fixed zero-length key and provides neither confidentiality nor authenticity,
  # which the man page says in those words.
  withKey = if config.losos.tpm.enable then "tpm2" else "host";

  # No --tpm2-pcrs, so the blob binds to no PCRs. That is the right call for a
  # box that rebuilds itself unattended at 03:00: binding to PCR 0 or 7 would
  # mean a firmware or bootloader update locks the appliance out of its own
  # secrets, with no shell to recover from. (systemd-creds(1): "this is also the
  # default if this option is not used".)
  systemdCreds = "${config.systemd.package}/bin/systemd-creds";

  inherit (cfg) runtimeDir;
  runtimeName = lib.removePrefix "/run/" runtimeDir;

  # The uid the pod runs as, from the same binding modules/workloads.nix uses.
  # It matters because the pod reads the file through a bind mount and DAC still
  # applies inside the container. In native mode PID 1 loads the credential as
  # root, so root ownership is right there.
  #
  # This has to be set here rather than left to modules/workloads.nix, which
  # fixes the ownership with a tmpfiles `z` line — and tmpfiles.d(5) says `z`
  # "does not follow symlinks", so from the second boot onwards that line would
  # silently do nothing.
  nextcloudUid =
    let
      declared = config.users.users.nextcloud.uid or null;
    in
    if declared == null then 1002 else declared;

  # ── The tenants ───────────────────────────────────────────────────────────
  # One entry per secret the box generates for itself. Adding a tenant is a
  # matter of adding an entry, provided the secret is one this box *mints*:
  # a secret provisioned out of band (losos.proxy.tokenFile,
  # losos.proxy.bootstrapTokenFile) arrives as plaintext from elsewhere and
  # sealing it here would seal a copy while the original still exists.
  #
  # /var/secrets/losos-admin-token is the obvious second candidate and is
  # deliberately absent: lososd mints it itself in Rust
  # (backend/, losos.admin.tokenFile) on first start, so sealing it belongs in
  # the daemon rather than in a oneshot racing it.
  #
  # /etc/keys/persist-keyfile cannot be a tenant at all. It is consumed by the
  # initrd, which runs before /var exists, and a host-key blob is unreadable
  # there by construction.
  tenants = [
    {
      name = "nextcloud-admin-pass";
      path = toString config.losos.nextcloud.adminpassFile;
      # The same 24 random bytes, base64, that modules/nextcloud-common.nix
      # mints. Duplicated on purpose: this module has to produce the secret
      # before the generator's ConditionPathExists is evaluated, and a shared
      # store-path script between the two files would couple two modules that
      # are otherwise independent. If one changes, change both.
      mint = "head -c 24 /dev/urandom | base64";
      owner = if config.losos.nextcloud.mode == "container" then toString nextcloudUid else "0";
      mode = "0600";
    }
  ];

  blobOf = t: "${cfg.store}/${t.name}.cred";
  runOf = t: "${runtimeDir}/${t.name}";
  # Embedded in the blob and checked on decrypt, so a blob cannot be renamed
  # over another secret's file and quietly accepted.
  credOf = t: "losos-${t.name}";

  openTenant = t: ''
    blob=${blobOf t}
    run=${runOf t}
    path=${t.path}

    rm -f "$run.new"

    if [ -e "$blob" ]; then
      if ${systemdCreds} --no-ask-password decrypt --name=${credOf t} "$blob" "$run.new"; then
        mv -f "$run.new" "$run"
      else
        # Loud, and then self-healing. A blob that will not decrypt means the
        # TPM was cleared or the host key was lost, and there is no shell to
        # diagnose it from. Re-minting is safe for this particular secret:
        # Nextcloud reads it only at first install, and after that
        # `losos-ctl set-password` drives `occ user:resetpassword`. The old
        # blob is kept rather than deleted, because "we threw away your
        # ciphertext" is not a decision a boot script gets to make.
        stamp=$(date +%Y%m%d%H%M%S)
        echo "keyring: ${t.name}: the sealed blob did not decrypt." >&2
        echo "keyring: ${t.name}: keeping it at $blob.unreadable.$stamp and minting a new secret." >&2
        rm -f "$run.new"
        mv -f "$blob" "$blob.unreadable.$stamp"
      fi
    fi

    # Migration. A box installed before this module existed has a real
    # plaintext file at $path and no blob. Adopt its value rather than minting
    # a new one: on that box Nextcloud is already installed with this password.
    if [ ! -s "$run" ] && [ ! -L "$path" ] && [ -f "$path" ] && [ -s "$path" ]; then
      echo "keyring: ${t.name}: adopting the plaintext left by an earlier boot" >&2
      cat "$path" > "$run"
    fi

    if [ ! -s "$run" ]; then
      echo "keyring: ${t.name}: nothing sealed and nothing to adopt; minting" >&2
      ( umask 077; ${t.mint} > "$run" )
    fi

    if [ ! -e "$blob" ]; then
      ( umask 077; ${systemdCreds} --no-ask-password --with-key=${withKey} \
          encrypt --name=${credOf t} "$run" "$blob.new" )
      chmod 0600 "$blob.new"
      mv -f "$blob.new" "$blob"
    fi

    chown ${t.owner}:${t.owner} "$run"
    chmod ${t.mode} "$run"

    # Retire the durable plaintext, if there still is one. The overwrite before
    # the unlink is best effort and nothing more: ext4 journals metadata and
    # may write the new block elsewhere, so shred(1)'s own warning applies. It
    # costs one write and helps in the common case, which is the whole claim.
    if [ -f "$path" ] && [ ! -L "$path" ]; then
      size=$(stat -c %s "$path")
      if [ "$size" -gt 0 ]; then
        dd if=/dev/urandom of="$path" bs="$size" count=1 conv=notrunc status=none || true
        sync "$path" || true
      fi
      rm -f "$path"
    fi

    # rename(2) rather than `ln -sf`, which unlinks and re-creates and so leaves
    # a window with nothing at $path. Consumers are ordered after this unit, but
    # a `nixos-rebuild switch` restarts it under a running kubelet.
    ln -sfn "$run" "$path.keyring-new"
    mv -fT "$path.keyring-new" "$path"
  '';

  closeTenant = t: ''
    blob=${blobOf t}
    run=${runOf t}
    path=${t.path}

    if [ -L "$path" ]; then
      target=$(readlink "$path")
      case "$target" in
        ${runtimeDir}/*)
          rm -f "$path.keyring-new"
          if [ -e "$blob" ]; then
            ( umask 077; ${systemdCreds} --no-ask-password decrypt \
                --name=${credOf t} "$blob" "$path.keyring-new" )
            chown ${t.owner}:${t.owner} "$path.keyring-new"
            chmod ${t.mode} "$path.keyring-new"
            mv -fT "$path.keyring-new" "$path"
            echo "keyring: ${t.name}: keyring disabled; restored the plaintext at $path" >&2
          elif [ -s "$run" ]; then
            ( umask 077; cat "$run" > "$path.keyring-new" )
            chown ${t.owner}:${t.owner} "$path.keyring-new"
            chmod ${t.mode} "$path.keyring-new"
            mv -fT "$path.keyring-new" "$path"
          else
            # Nothing to restore. Drop the link so the generator in
            # modules/nextcloud-common.nix sees an absent path and mints,
            # instead of writing into a directory that no longer exists.
            rm -f "$path"
            echo "keyring: ${t.name}: keyring disabled and nothing sealed; removed the dangling link at $path" >&2
          fi
          ;;
      esac
    fi
  '';

  # Everything that reads a tenant's plaintext. systemd silently ignores an
  # ordering dependency on a unit that does not exist, so one list covers the
  # native path (nextcloud-setup, phpfpm-nextcloud), the workload path (k3s's
  # kubelet mounts the file into the static pod) and the generator itself.
  consumers = [
    "losos-nextcloud-adminpass.service"
    "nextcloud-setup.service"
    "phpfpm-nextcloud.service"
    "k3s.service"
  ];
in
{
  config = lib.mkMerge [
    {
      assertions = [
        {
          assertion = cfg.enable -> lib.hasPrefix "/run/" cfg.runtimeDir;
          message = ''
            losos.keyring.runtimeDir must be under /run. It is a systemd
            RuntimeDirectory, which is tmpfs, and that is the entire point: a
            decrypted secret anywhere else is a decrypted secret on /persist.
          '';
        }
        {
          assertion = cfg.enable -> !(lib.hasPrefix "/run/" cfg.store);
          message = ''
            losos.keyring.store must survive a reboot; ${cfg.store} does not.
            The sealed blobs are the only durable copy of these secrets.
          '';
        }
      ];
    }

    (lib.mkIf cfg.enable {
      # ── Unseal, or mint and seal ────────────────────────────────────────────
      # Not guarded by ConditionPathExists. Unlike the certificate in
      # modules/tls.nix, the work here is needed on *every* boot: /run is tmpfs,
      # so the plaintext has to be put back before anything asks for it. The
      # one-time half is inside the script, keyed on whether the blob exists.
      systemd.services.losos-keyring = {
        description = "Unseal the appliance's own secrets into tmpfs";
        wantedBy = [ "multi-user.target" ];
        after = [
          "local-fs.target"
          "persist.mount"
        ];
        before = consumers;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          UMask = "0077";
          RuntimeDirectory = runtimeName;
          RuntimeDirectoryMode = "0700";
          # Without this, restarting the unit (which `nixos-rebuild switch`
          # does whenever the script text changes) deletes the directory and
          # every materialised secret with it, leaving consumers looking at a
          # dangling link until the script gets to the decrypt.
          RuntimeDirectoryPreserve = "yes";

          # Not a sandbox: this needs root to chown the pod's secret and to
          # write under /persist. It is damage limitation, same idea as lososd's
          # unit in modules/daemon.nix.
          ProtectHome = true;
          NoNewPrivileges = true;
          ProtectKernelTunables = true;
          LockPersonality = true;
        }
        # Keep the TPM out of reach of everything that has no business
        # unsealing. DevicePolicy=closed still grants /dev/null, /dev/zero,
        # /dev/full, /dev/random and /dev/urandom, which is all the mint needs.
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
        script = ''
          set -eu
          install -d -m 0700 ${cfg.store}
          ${lib.concatMapStringsSep "\n" openTenant tenants}
        '';
      };

      # The kubelet mounts the plaintext with `type: File`, so a link this unit
      # has not resolved yet is a pod that never starts. modules/workloads.nix
      # already makes k3s Require the generator for the same reason; this is the
      # same dependency for the unit that now actually produces the file.
      systemd.services.k3s = lib.mkIf (
        config.losos.nextcloud.mode == "container" && config.services.k3s.enable
      ) { requires = [ "losos-keyring.service" ]; };
    })

    (lib.mkIf (!cfg.enable) {
      # ── The way back ────────────────────────────────────────────────────────
      # Only reachable on a box that had the keyring on and now does not. On
      # every other box the ConditionPathIsSymbolicLink makes it a no-op, which
      # is why it is cheap enough to ship unconditionally on the disabled path.
      systemd.services.losos-keyring-restore = {
        description = "Restore keyring secrets to plaintext (keyring disabled)";
        wantedBy = [ "multi-user.target" ];
        after = [
          "local-fs.target"
          "persist.mount"
        ];
        before = consumers;
        unitConfig.ConditionPathIsSymbolicLink = map (t: t.path) tenants;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          UMask = "0077";
          ProtectHome = true;
          NoNewPrivileges = true;
        };
        path = [
          config.systemd.package
          pkgs.coreutils
        ];
        script = ''
          set -eu
          ${lib.concatMapStringsSep "\n" closeTenant tenants}
        '';
      };
    })
  ];
}
