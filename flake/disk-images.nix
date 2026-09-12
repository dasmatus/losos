# Bootable media, built locally off the `install` system.
#
# Everything below is about `losos-disk-qcow2`, and it all starts from: THIS IS
# NOT THE APPLIANCE. devenv.nix expects a second medium in this file, the
# closure-carrying installer ISO (`losos-disk-iso`) — that one installs the
# real thing, so none of the caveats here apply to it and it needs its own
# comment next to its own derivation. Do not let this header cover it.
#
# `nix build .#losos-disk-qcow2` (or `devenv shell build-media qcow2`) produces
# a QCOW2 that boots in QEMU on any laptop, so LosOS can be shown to somebody
# who has neither a spare mini-PC nor an afternoon. That is the entire reason
# this file exists. It is not an installable product, it is not "LosOS in a
# file", and handing it to somebody as a way to actually run this is handing
# them a box with no disk encryption.
#
# ── What a disk image cannot carry, and therefore does not ──────────────────
#
# The appliance's disk layout is LUKS-on-LVM unlocked by a TPM2-sealed key (or
# a keyfile baked into the initrd), under a tmpfs root that impermanence
# repopulates from /persist every boot. A QCOW2 file can carry none of that:
# the key would have to sit in the image next to the data it protects, and
# there is no TPM to seal it to. So the image gives up, explicitly and in the
# code below, every one of these:
#
#   1. FULL-DISK ENCRYPTION. losos.tpm.enable = false, disko.enableConfig =
#      false, and the root is plain ext4 on a plain GPT partition. Everything
#      docs/security-model.md claims about physical access is void here: the
#      qcow2 can be opened with qemu-nbd on any machine and /home/notshared
#      and /home/shared read straight out of it. There is no /etc/keys, no
#      /crypto_keyfile.bin, nothing to unlock because nothing is locked.
#   2. fscrypt ON THE SHARED DOMAIN. The second layer needs an ext4 formatted
#      with `-O encrypt` (modules/disko.nix does that; make-disk-image's mkfs
#      does not), so losos.shared.fscrypt.enable is off. The property "opaque
#      even to root while sharing is off" does not hold in this image.
#   3. THE LVM POOL. One partition, no volume group, no free extents. There is
#      nothing for `losos-ctl grow` to grow — grow.rs's lvextend step fails on
#      a device that is not an LV. Resize the qcow2 from the host instead.
#   4. STATELESSNESS. No tmpfs root and no impermanence, so state accumulates
#      across reboots the way it does on an ordinary Linux box. This is the
#      deviation with the widest blast radius, and it is the one to remember
#      when this image is used for development: a bug that only appears when
#      the root is wiped — a directory somebody forgot to add to
#      modules/impermanence.nix — CANNOT reproduce here. tests/impermanence.nix
#      is what covers that, not this.
#
# ── What does survive, i.e. what this is actually good for ──────────────────
#
# The whole product surface above the disk: the Nginx front door and its
# LAN-only guard, the admin SPA, lososd and its Bearer-authed API, the
# first-run setup wizard, the self-signed certificate, Nextcloud and Forgejo as
# static pods in the box's own k3s cluster, the hardening baseline, and the
# no-shell-logins model (the image has no root password either — same as the
# appliance; if you are debugging a kernel panic rather than demoing, add a
# `users.users.root.initialHashedPassword` of your own to vmOverlay below).
#
# ── Distribution ────────────────────────────────────────────────────────────
#
# Local target only. The image is a multi-GiB artifact: it cannot be built in
# CI (10 minutes / 8 GB per job, and the RAM quota counts filesystem writes —
# see .forgejo/workflows/ci.yml) and it cannot be published on Codeberg, whose
# whole recommended budget for packages + LFS + attachments is ~1.5 GiB and is
# already spent on one ISO. If it is ever published, it goes somewhere that
# hosts large files, with the four points above repeated at the download link,
# not implied by a filename.
#
# x86_64-linux only, like the rest of this flake: the config being imaged is
# nixosConfigurations.install, which is pinned to that system.
{
  self,
  nixpkgs,
  pkgs,
}:

let
  inherit (pkgs) lib;

  # ext4 labels are capped at 16 bytes, and this one is load-bearing twice
  # over: fileSystems."/" below finds the root by it, and it is the name that
  # shows up in `lsblk` for anyone who later wonders what a stray qcow2 on
  # their disk is.
  label = "losos-demo";

  # The appliance, minus every property a disk image cannot honestly provide.
  #
  # An overlay on nixosConfigurations.install rather than a third nixosSystem
  # with its own module list, on purpose: that list is long, load-bearing and
  # changes often, and a copy of it here would drift silently into a demo that
  # no longer resembles the product. extendModules
  # re-evaluates that exact config with the module below appended, so
  # everything not named here tracks the appliance automatically — and every
  # deviation is one visible mkForce.
  vmOverlay =
    { modulesPath, ... }:
    {
      # virtio block/net/scsi in the initrd plus the qemu guest agent. Without
      # this the kernel comes up and finds no disk.
      imports = [ (modulesPath + "/profiles/qemu-guest.nix") ];

      # ── 1. No TPM, no LUKS, no LVM, no disko ──────────────────────────────
      #
      # tpm.enable = false is the honest value (there is no vTPM in a plain
      # `qemu-system-x86_64` run) but on its own it selects the *keyfile*
      # unlock path, which is worse here, not better: boot.nix would inject
      # /etc/keys/persist-keyfile into the initrd as a secret, and
      # make-disk-image's `switch-to-configuration boot` would die reading a
      # file that exists on no build machine. Both LUKS declarations are
      # therefore forced empty on the next two lines — and forcing
      # boot.initrd.luks.devices is not optional either: with disko off,
      # `.persist` would be a submodule with no `device`, which is an eval
      # error. Read all of it as: the image is not encrypted and does not
      # pretend to be.
      losos.tpm.enable = false;
      boot.initrd.luks.devices = lib.mkForce { };
      boot.initrd.secrets = lib.mkForce { };

      # disko still *evaluates* (modules/disko.nix is in the install module
      # list and its targetDrives assertion still has to pass), it just stops
      # producing config. Same switch flake.nix throws for the ISO, for the
      # same reason: this system must not inherit the target machine's layout.
      disko.enableConfig = false;

      # The layout make-disk-image actually creates with partitionTableType =
      # "efi": vda1 is a FAT ESP labelled ESP, vda2 is the ext4 root. mkForce
      # because this replaces the whole set — including modules/boot.nix's
      # `fileSystems."/persist".neededForBoot`, which has no device to point at
      # once disko is off.
      fileSystems = lib.mkForce {
        "/" = {
          device = "/dev/disk/by-label/${label}";
          fsType = "ext4";
        };
        "/boot" = {
          device = "/dev/disk/by-label/ESP";
          fsType = "vfat";
          options = [ "umask=0077" ];
        };
      };

      # ── 2. No impermanence ────────────────────────────────────────────────
      #
      # There is no /persist to bind-mount from, and with a persistent root
      # there is nothing to bind it back onto. The appliance's statelessness is
      # gone; see point 4 of the header for what that costs a developer.
      environment.persistence = lib.mkForce { };

      # ── 3. No fscrypt ─────────────────────────────────────────────────────
      #
      # modules/fscrypt.nix addresses /persist/home/shared/data on a filesystem
      # made with `mkfs.ext4 -O encrypt`. Neither exists here. Left on, the
      # unlock unit fails on every boot of the demo.
      losos.shared.fscrypt.enable = false;

      # ── 4. EFI variables ──────────────────────────────────────────────────
      #
      # systemd-boot still installs into the ESP, but nothing may write EFI
      # variables: the build VM has no efivarfs, and a QCOW2 has no NVRAM of
      # its own anyway — boot entries live in the OVMF_VARS file the host
      # passes to QEMU. mkForce because modules/boot.nix wants this true, and
      # on real hardware it should stay true.
      boot.loader.efi.canTouchEfiVariables = lib.mkForce false;

      # A serial console, so `qemu -nographic` shows the boot and a CI-less
      # smoke test can read it. tty0 stays first so a graphical run still
      # prints somewhere.
      boot.kernelParams = [
        "console=tty0"
        "console=ttyS0,115200"
      ];

      # ── 5. Nothing that reboots or rebuilds the demo out from under you ───
      #
      # system.autoUpgrade rebuilds from git+file:///etc/nixos#install at
      # 03:00, and /etc/nixos in this image is not a git checkout — on the
      # appliance impermanence carries one over from the installer. And
      # midnight-reboot fires unconditionally at 00:07 with Persistent=true,
      # which on a laptop means the demo VM reboots the moment you start it
      # after midnight. Both are real appliance behaviour and both are wrong
      # for an image somebody is watching.
      #
      # `enable = false` on a unit somebody else defined masks it — the unit
      # file becomes a symlink to /dev/null. The timers.target.wants symlink
      # stays behind and looks alive in a directory listing; systemd will not
      # start a masked unit, so that is not a bug to go fixing.
      system.autoUpgrade.enable = lib.mkForce false;
      systemd.timers.midnight-reboot.enable = false;

      # ── 6. No GPU stack ───────────────────────────────────────────────────
      #
      # losos.gpu.enable pulls mesa and llvm twice over — 2.06 GiB of NAR
      # across 129 store paths on the install closure, 992 MiB of it purely
      # the 32-bit half — to hand /dev/dri to the Nextcloud pod. QEMU's
      # virtual display has no VA-API to offer, so in this image it is dead
      # weight. Measured on this config: turning it off takes the closure from
      # 5.78 GiB over 882 paths to 4.03 GiB over 767. mkForce, because
      # modules/overrides.nix sets it at normal priority.
      losos.gpu.enable = lib.mkForce false;

      # Useful in a VM, harmless anywhere: lets the host ask the guest to
      # freeze filesystems and shut down cleanly.
      services.qemuGuest.enable = true;
    };

  vmSystem = self.nixosConfigurations.install.extendModules { modules = [ vmOverlay ]; };

  # The image itself: $out/losos-demo.qcow2. Bound here rather than referenced
  # back out of `self.packages`, so the runner below cannot turn into a cycle
  # through the attrset this file is merged into.
  image = import "${nixpkgs}/nixos/lib/make-disk-image.nix" {
    inherit pkgs lib;
    inherit (vmSystem) config;

    # The `losos-disk-` prefix is a contract with devenv.nix: `heavy` there
    # skips it in `build-pkgs` and selects it in `build-media`, both by
    # attribute-name prefix. Renaming this attribute without renaming it there
    # makes `build-pkgs` start building a multi-gigabyte image and
    # `build-media` build nothing.
    name = "losos-disk-qcow2";
    baseName = "losos-demo"; # the filename says demo; keep it that way
    format = "qcow2";
    inherit label;

    # GPT with an ESP, because the appliance boots systemd-boot on UEFI and a
    # demo that boots by some other path demonstrates some other system.
    partitionTableType = "efi";
    # 256M (the default) fits two generations of an initrd for
    # linuxPackages_latest and not much else; 512M leaves room for the
    # generations a developer's rebuild loop piles up.
    bootSize = "512M";

    # 20 GiB, sparse — the qcow2 on disk is a fraction of this. Sized for what
    # the box does to itself after first boot rather than for the closure:
    # k3s imports the Nextcloud and Forgejo image tarballs into containerd,
    # which unpacks a second copy of both (~2.7 GiB) beside the ~2.9 GiB store
    # this image ships, and then Nextcloud wants somewhere to put data.
    diskSize = 20480;

    # The bootloader step runs in a throwaway VM; 1024 MiB (the default) is
    # thin for nixos-enter plus systemd-boot's builder over a store this size.
    memSize = 2048;

    # No nixpkgs channel inside the image. It would add a full copy of the
    # nixpkgs tree for the benefit of `nix-env`, which this box does not have a
    # shell to run, and it would make the image's hash move with every input
    # bump for no change in what boots.
    copyChannel = false;
  };

  # One command to actually see the thing: `nix run .#losos-disk-qcow2-run`.
  # The binary inside is `losos-vm-run`; writeShellApplication sets
  # meta.mainProgram, so `nix run` finds it under either spelling.
  #
  # The attribute wears the `losos-disk-` prefix even though the script itself
  # is a few hundred bytes, because BUILDING it builds the image: the qcow2's
  # store path is interpolated into the text below, which makes it an input.
  # Under any other name devenv.nix's `build-pkgs` — "every flake package
  # except the images and media" — would quietly start building a
  # multi-gigabyte disk image.
  #
  # Copy-on-write over the store image, so the demo is disposable (delete the
  # local qcow2 to reset it) and the build output is never written to. Port 80
  # in the guest lands on 8080 on the host — QEMU's user-mode network puts the
  # host at 10.0.2.2, which the front vhost's LAN-only guard admits under
  # 10.0.0.0/8. Loopback would NOT be admitted; that guard denies 127.0.0.1 on
  # purpose (see modules/containers.nix), so `-nic user` is not an incidental
  # choice here.
  #
  # UEFI, because the appliance boots systemd-boot: OVMF_CODE as read-only
  # pflash, and a private writable copy of OVMF_VARS for the boot entry —
  # which is the NVRAM the image itself cannot have.
  runner = pkgs.writeShellApplication {
    name = "losos-vm-run";
    runtimeInputs = [
      pkgs.qemu
      pkgs.coreutils
    ];
    text = ''
      disk="''${LOSOS_VM_DISK:-losos-demo.qcow2}"
      vars="''${LOSOS_VM_VARS:-losos-demo-efivars.fd}"

      if [ ! -e "$disk" ]; then
        echo "creating $disk (copy-on-write over the read-only store image)"
        qemu-img create -f qcow2 -F qcow2 -b ${image}/losos-demo.qcow2 "$disk"
      fi
      if [ ! -e "$vars" ]; then
        cp ${pkgs.OVMF.fd.variables} "$vars"
        chmod 0644 "$vars"
      fi

      echo "admin UI:  http://localhost:8080/   (the LAN-only guard sees 10.0.2.2)"
      echo "over TLS:  https://localhost:8443/"
      echo "quit:      Ctrl-a x"
      exec qemu-system-x86_64 \
        -machine q35,accel=kvm:tcg \
        -cpu max \
        -smp "''${LOSOS_VM_CPUS:-4}" \
        -m "''${LOSOS_VM_MEM:-4096}" \
        -drive if=pflash,format=raw,unit=0,readonly=on,file=${pkgs.OVMF.fd.firmware} \
        -drive if=pflash,format=raw,unit=1,file="$vars" \
        -drive file="$disk",if=virtio,format=qcow2 \
        -nic user,model=virtio-net-pci,hostfwd=tcp::8080-:80,hostfwd=tcp::8443-:443 \
        -nographic "$@"
    '';
  };
  # ── The closure-carrying installer ISO ────────────────────────────────────
  #
  # Unlike the qcow2 above, this one IS the appliance: it is the ordinary
  # installer medium, and what it installs is the real LUKS-on-LVM, TPM-sealed,
  # tmpfs-root system. The only difference is what rides along in the store.
  #
  # The plain ISO carries the installer and one small output, so a fresh box
  # fetches and builds the rest itself. Measured on a dev machine that closure
  # is 5.78 GiB over 882 store paths. Most of it substitutes from
  # cache.nixos.org, but three things cannot: the Nextcloud, Forgejo and pause
  # image tarballs (849 MiB, and no public cache has them because dockerTools
  # builds them from this flake) and `losos-ctl` itself. A repurposed mini-PC
  # compresses multi-gigabyte layer tars and compiles Rust as part of its first
  # install, with nobody watching and no shell to watch from.
  #
  # `isoImage.storeContents` puts the finished `install` toplevel on the medium,
  # and `nix copy` prefers a path it already has over building one. So the
  # expensive half of an install becomes a read off the USB stick.
  #
  # THREE THINGS THIS DOES NOT DO, each of which has bitten a medium like it:
  #
  #   It does not remove the network requirement. `losos-install` clones
  #   LOSOS_FLAKE_URL at run time and `nixos-install` evaluates that clone,
  #   which fetches nixpkgs. Carrying store paths changes what gets BUILT, not
  #   whether the flake gets EVALUATED.
  #
  #   It does not help if the medium and the clone disagree. The installer
  #   installs whatever revision it cloned; an ISO built from a different commit
  #   evaluates to different store paths, the carried ones match nothing, and
  #   the install silently falls back to building — slower than the plain ISO,
  #   because it also read 2.4 GiB it could not use. Build this from the commit
  #   you mean to install.
  #
  #   It does not belong in CI or on a release page. The plain ISO is already
  #   ~1.5 GB, which is Codeberg's entire recommended budget for packages and
  #   attachments; this adds ~2.4 GiB of compressed closure on top. It is a
  #   local artefact. modules/cache.nix is the online half of the same problem
  #   and is cheaper whenever a cache is reachable.
  isoSystem = self.nixosConfigurations.iso.extendModules {
    modules = [
      (
        { lib, ... }:
        {
          # mkForce, because the plain ISO already sets this to the admin UI
          # alone (flake.nix) and a list would otherwise merge into "both" —
          # which is not wrong, just redundant: the UI is inside the toplevel.
          isoImage.storeContents = lib.mkForce [
            self.nixosConfigurations.install.config.system.build.toplevel
          ];

          # A distinct volume label and file name, so a closure-carrying medium
          # cannot be mistaken for the plain one after it has been written to a
          # stick and the shell history is gone. They install identically and
          # differ only in how long it takes, which is exactly the kind of
          # difference nobody remembers about an unlabelled USB stick.
          # `image.fileName`, not `isoImage.isoName`: nixpkgs renamed it and the
          # old spelling still works but warns on every evaluation of this
          # flake, including the ones CI and `devenv test` run.
          image.fileName = lib.mkForce "losos-installer-full.iso";
          isoImage.volumeID = lib.mkForce "LOSOS_FULL";
        }
      )
    ];
  };
in
{
  losos-disk-qcow2 = image;
  losos-disk-qcow2-run = runner;
  losos-disk-iso = isoSystem.config.system.build.isoImage;
}
