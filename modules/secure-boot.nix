# Secure Boot for the installer medium: one signable UEFI loader.
#
# Imported by the `iso` system only. The installed system keeps its own
# loader (systemd-boot, modules/boot.nix); signing *that* means a key on the
# box for every nightly rebuild, which is a different design (lanzaboote) and
# not this file's.
#
# What a UEFI firmware verifies under Secure Boot is the PE binary it is asked
# to start, against the certificates in its `db`, and nothing after that
# unless the started binary verifies the next one itself. The stock NixOS ISO
# starts GRUB, which then reads the kernel and initrd out of the ISO9660
# filesystem unverified — and GRUB built without shim refuses to boot a kernel
# at all once Secure Boot is on. So the medium does not boot through GRUB
# under UEFI any more. Its `EFI/BOOT/BOOTX64.EFI` is a unified kernel image
# (UKI): systemd-stub with the kernel, the initrd and the kernel command line
# as PE sections, built by `ukify`. The firmware's one signature check then
# covers everything that runs before the squashfs is mounted, and a changed
# byte anywhere in that set — a different kernel, an edited `init=`, a
# tampered initrd — fails it. The BIOS path (syslinux) is unchanged, because
# Secure Boot is a UEFI feature and SeaBIOS verifies nothing.
#
# The UKI leaves `nix build` UNSIGNED, on purpose. A Nix build is pure and
# its outputs are world-readable store paths that CI pushes to a public
# cache, so the private db key cannot be a build input. `losos-sign-iso`
# (flake/packages.nix) signs the finished ISO in place instead: it finds the
# EFI system partition image inside the ISO through the El Torito catalog,
# pulls the UKI out of that FAT image with mtools, `sbsign`s it and writes it
# back over the original. Nothing else in the ISO moves, so the El Torito
# entry, the hybrid GPT and the squashfs stay valid. CI does that with the
# `SECURE_BOOT_DB_KEY` secret on main and on tags (never on a fork's pull
# request, which has no key and ships unsigned), and tests/secure-boot.nix
# does it with a throwaway key it generates in the build sandbox. The FAT
# image is sized with 8 MiB of slack so the signed copy, a few KiB larger
# than the unsigned one, always fits.
#
# The firmware's check stops at the initrd; the system image the medium
# carries (`nix-store.squashfs`, the 1.5 GB that is actually LosOS) is read
# by that initrd, unverified by anything above it. So the UKI's command line
# carries the squashfs's SHA-256 (`losos.medium.sha256=`), and a stage-1 unit
# below hashes the file on the medium before the squashfs is mounted and
# refuses to go on when they differ. The hash rides inside the signed image,
# so changing the squashfs means changing the command line, which breaks the
# signature. Only the UEFI path carries it: under BIOS nothing above the
# kernel is verified either, and a hash check against an unverified hash
# proves nothing, so the unit is conditioned on the parameter being present.
# (The squashfs is built here rather than inside make-iso9660-image so the
# hash is known when the UKI is built; the ISO builder takes the same file.)
#
# The public certificate (`losos.secureBoot.certFile`, keys/secure-boot-db.pem)
# rides on the ESP as `EFI/losos/losos-secure-boot.{cer,pem}` when the
# committed file carries one, so a firmware's "enroll key from file" dialog
# can take it straight from the stick: a machine with only the stock
# Microsoft keys refuses the medium until its owner enrols this certificate
# in `db` (or turns Secure Boot off), and that refusal is the feature.
#
# `system.build.isoImage` is forced here because iso-image.nix offers no seam
# for the EFI image: its `efiImg` is a `let` binding, and the only way to
# ship a different one is to turn `isoImage.makeEfiBootable` off (so the
# module adds neither its image nor GRUB's files to the ISO) and call
# make-iso9660-image ourselves with the same arguments plus ours. The
# unsigned UKI is exported as `system.build.uki` for the test and for
# `losos-sign-iso --show`.
{
  config,
  lib,
  pkgs,
  utils,
  modulesPath,
  ...
}:

let
  cfg = config.losos.secureBoot;

  kernelFile = "${config.boot.kernelPackages.kernel}/${config.system.boot.loader.kernelFile}";
  initrdFile = "${config.system.build.initialRamdisk}/${config.system.boot.loader.initrdFile}";

  # The medium's system image, built once here so its hash can go into the
  # UKI; make-iso9660-image gets it as a plain file at the path it would have
  # put its own.
  squashfs = pkgs.callPackage "${modulesPath}/../lib/make-squashfs.nix" {
    fileName = "nix-store";
    storeContents = config.isoImage.storeContents;
    comp = config.isoImage.squashfsCompression;
  };

  # The same line GRUB and syslinux boot the medium with (iso-image.nix's
  # menu builders): the toplevel's init plus every kernel parameter, which
  # on the ISO includes `root=LABEL=<volumeID>` for stage 1 to find the
  # squashfs. Plus the squashfs's hash, filled in by the UKI build.
  cmdline = "init=${config.system.build.toplevel}/init ${toString config.boot.kernelParams}";

  mediumHashParam = "losos.medium.sha256";
  roStoreMount = "${utils.escapeSystemdPath "/sysroot/nix/.ro-store"}.mount";

  # What `bootctl status` and a UKI-aware boot menu show for the medium.
  # IMAGE_ID is also the byte string tests/secure-boot.nix flips to make its
  # tampered copy: it occurs in the UKI's .osrel section and nowhere else in
  # the ISO outside the (compressed) squashfs.
  osRelease = pkgs.writeText "losos-installer-os-release" ''
    NAME="LosOS"
    ID=losos
    IMAGE_ID=losos-installer
    IMAGE_VERSION="${config.system.nixos.label}"
    PRETTY_NAME="LosOS installer ${config.system.nixos.label}"
  '';

  uki =
    pkgs.runCommand "losos-installer-uki"
      {
        nativeBuildInputs = [ pkgs.systemdUkify ];
        # Shown by `losos-sign-iso --show` and the test's log.
        passthru = {
          inherit cmdline osRelease;
        };
      }
      ''
        hash=$(sha256sum ${squashfs} | cut -d' ' -f1)
        ukify build \
          --stub=${pkgs.systemd}/lib/systemd/boot/efi/linuxx64.efi.stub \
          --linux=${kernelFile} \
          --initrd=${initrdFile} \
          --cmdline=${lib.escapeShellArg cmdline}" ${mediumHashParam}=$hash" \
          --os-release=@${osRelease} \
          --uname=${config.boot.kernelPackages.kernel.modDirVersion} \
          --output=$out
      '';

  # The committed certificate file is comment-only until the key ceremony
  # has run; then the medium ships no certificate rather than an empty file.
  certLines = lib.splitString "\n" (builtins.readFile cfg.certFile);
  haveCert = lib.any (l: l == "-----BEGIN CERTIFICATE-----") certLines;

  # The EFI system partition image: the UKI as the removable-media loader,
  # and the public certificate beside it. Built the way iso-image.nix builds
  # its own (fixed dates, fixed volume id, sorted mcopy) so the ISO stays
  # reproducible — plus the slack the in-place signing needs.
  efiImg =
    pkgs.runCommand "losos-efi-image_eltorito"
      {
        nativeBuildInputs = [
          pkgs.buildPackages.mtools
          pkgs.buildPackages.libfaketime
          pkgs.buildPackages.dosfstools
        ]
        ++ lib.optional haveCert pkgs.buildPackages.openssl;
        strictDeps = true;
      }
      ''
        mkdir ./contents && cd ./contents
        mkdir -p ./EFI/BOOT ./EFI/losos
        cp ${uki} ./EFI/BOOT/BOOTX64.EFI
        ${lib.optionalString haveCert ''
          # Firmware "enroll from file" dialogs want DER; people want PEM.
          openssl x509 -in ${cfg.certFile} -outform DER -out ./EFI/losos/losos-secure-boot.cer
          openssl x509 -in ${cfg.certFile} -out ./EFI/losos/losos-secure-boot.pem
        ''}
        find . -exec touch --date=2000-01-01 {} +

        usage_size=$(( $(du -s --block-size=1M --apparent-size . | tr -cd '[:digit:]') * 1024 * 1024 ))
        # FAT overhead, plus 8 MiB so losos-sign-iso can write the signed
        # UKI (a few KiB larger) over the unsigned one without running out
        # of clusters.
        image_size=$(( ($usage_size * 110) / 100 + 8 * 1024 * 1024 ))
        block_size=$((1024*1024))
        image_size=$(( ($image_size / $block_size + 1) * $block_size ))
        echo "Usage size: $usage_size"
        echo "Image size: $image_size"
        truncate --size=$image_size "$out"
        mkfs.vfat --invariant -i 4c6f536f -n LOSOSEFI "$out"

        for d in $(find EFI -type d | sort); do
          faketime "2000-01-01 00:00:00" mmd -i "$out" "::/$d"
        done
        for f in $(find EFI -type f | sort); do
          mcopy -pvm -i "$out" "$f" "::/$f"
        done
        fsck.vfat -vn "$out"
      '';
in
{
  config = lib.mkIf cfg.enable {
    # Keep iso-image.nix from adding its GRUB image and files; the medium
    # gets the one below instead. BIOS and USB stay as the base profile set
    # them.
    isoImage.makeEfiBootable = lib.mkForce false;

    system.build.uki = uki;
    system.build.efiImage = efiImg;

    # systemd-stub announces loader features the way systemd-boot does, which
    # turns on the unit that writes a fresh random seed to the ESP at boot.
    # The medium's ESP is a read-only FAT image on a stick, so the unit fails
    # and paints one red line on tty1 for nothing: the seed is for systemd-boot
    # on an installed disk, and the installed system keeps it.
    systemd.suppressedSystemUnits = [ "systemd-boot-random-seed.service" ];

    # Stage 1: hash the system image on the medium against the value the
    # signed command line carries, before it is mounted. Ordered like the
    # ISO module's own copytoram unit, which also runs between the medium
    # and the store mount.
    boot.initrd.systemd.services.losos-verify-medium = {
      description = "Verify the medium's system image against the signed hash";
      requiredBy = [ roStoreMount ];
      before = [
        roStoreMount
        "initrd-switch-root.target"
      ];
      unitConfig = {
        RequiresMountsFor = "/sysroot/iso";
        ConditionKernelCommandLine = mediumHashParam;
      };
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        StandardOutput = "journal";
        StandardError = "journal";
      };
      path = [ pkgs.coreutils ];
      script = ''
        # To the journal, and straight to /dev/console: the screen, or
        # whatever the last console= on the command line names. Not
        # StandardOutput=journal+console, whose console half is journald's
        # forwarding and lands wherever journald's TTYPath points (a serial
        # port under the test instrumentation), leaving the screen blank
        # at the one moment the owner must read it.
        say() {
          echo "$1"
          echo "$1" > /dev/console 2>/dev/null || true
        }
        # No sed in the systemd initrd; bash does the parsing.
        expected=
        for word in $(cat /proc/cmdline); do
          case "$word" in
            ${mediumHashParam}=*) expected=''${word#*=} ;;
          esac
        done
        say "LosOS: verifying the medium's system image against the signed hash (a few seconds)"
        actual=$(sha256sum /sysroot/iso/nix-store.squashfs | cut -d' ' -f1)
        if [ "$actual" != "$expected" ]; then
          say "LosOS: THE MEDIUM HAS BEEN ALTERED: its system image does not match the hash"
          say "LosOS: in the signed loader (expected $expected, found $actual). Refusing to start it."
          sleep 5
          systemctl --no-block poweroff
          exit 1
        fi
        say "LosOS: the medium's system image matches the signed hash"
      '';
    };

    system.build.isoImage = lib.mkForce (
      pkgs.callPackage "${modulesPath}/../lib/make-iso9660-image.nix" (
        {
          inherit (config.isoImage) compressImage volumeID;
          contents = config.isoImage.contents ++ [
            {
              source = efiImg;
              target = "/boot/efi.img";
            }
            {
              source = squashfs;
              target = "/nix-store.squashfs";
            }
          ];
          isoName = "${config.image.baseName}.iso";
          bootable = config.isoImage.makeBiosBootable;
          bootImage = "/isolinux/isolinux.bin";
          syslinux = if config.isoImage.makeBiosBootable then pkgs.syslinux else null;
          # Already built above and added to contents; an empty list here
          # keeps the builder from making a second one.
          squashfsContents = [ ];
          efiBootable = true;
          efiBootImage = "boot/efi.img";
        }
        // lib.optionalAttrs (config.isoImage.makeUsbBootable && config.isoImage.makeBiosBootable) {
          usbBootable = true;
          isohybridMbrImage = "${pkgs.syslinux}/share/syslinux/isohdpfx.bin";
        }
      )
    );

    assertions = [
      {
        assertion = pkgs.stdenv.hostPlatform.isx86_64;
        message = "losos.secureBoot: the medium's UKI is built for x86_64 (BOOTX64.EFI) only.";
      }
    ];
  };
}
