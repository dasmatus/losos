# nixos-test-vms config for the losos installer.
#
# Boots a VM with three empty virtio disks (/dev/vdb, /dev/vdc, /dev/vdd) plus
# its own root on /dev/vda, then runs the `losos-install` installer against
# them and asserts:
#   1. drive detection finds the three target disks and skips the VM root,
#   2. disko merges all three into one LVM volume group `persist-vg`,
#   3. the single `persist` LV is opened as LUKS and mounted as ext4 at
#      /mnt/persist, WITH the `encrypt` feature actually enabled,
#   4. autodetection and both firmware overrides render the expected target.
#   5. the first target drive has a BIOS boot partition and ESP.
#   6. nixos-install installs the configured GRUB bootloader and it boots under
#      SeaBIOS.
#
# The installer is driven through its --disko-script seam (a prebuilt
# `config.system.build.diskoScript`) so the test never needs nix or the full
# appliance closure inside the VM — it exercises the installer's drive
# detection + override generation + the real disko format/mount path, which is
# the "find drives, merge them to an LVM array, run disko" claim. The boot test
# installs the running test system, which imports the production boot module,
# through nixos-install's normal bootloader-install path.
{
  pkgs,
  disko,
}:

pkgs.testers.nixosTest {
  name = "losos-install-lvm";

  nodes.installer =
    {
      config,
      lib,
      pkgs,
      ...
    }:
    let
      # The disks the installer is expected to find and merge. The VM root is
      # /dev/vda (excluded by detection as a mounted disk), so these three
      # empty images are the only candidates detection should return.
      targets = [
        "/dev/vdb"
        "/dev/vdc"
        "/dev/vdd"
      ];
      # The losos-ctl derivation the `losos-install` wrapper is drawn from.
      # Reusing flake/packages.nix keeps the VM test free of `self` (the
      # installer module must not depend on it) without rebuilding the cabal
      # project a second way.
      lososPkgs = import ../flake/packages.nix { inherit pkgs; };
    in
    {
      imports = [
        disko.nixosModules.disko
        ../modules/options.nix
        ../modules/disko.nix
        ../modules/boot.nix
        ../modules/installer.nix
      ];

      # Supply the installer binary; leave autorun false (the test drives
      # losos-install manually through its --emit-target / --disko-script seams).
      losos.installer.package = lososPkgs.losos-ctl;

      # Drive list the disko layout pools into `persist-vg`, plus TPM mode.
      # The test uses the keyfile path (unattended); the keyfile is created
      # by the test script before disko runs (disko's luks passwordFile).
      losos.targetDrives = targets;
      losos.tpm.enable = false;
      losos.bios = true;
      boot.initrd.luks.devices = lib.mkForce { };
      # boot.nix's no-TPM path ships the keyfile as an initrd secret, which a
      # directly booted test VM cannot carry ("values must be unquoted paths").
      # Nothing here unlocks LUKS at boot, so drop both halves.
      boot.initrd.secrets = lib.mkForce { };
      fileSystems."/persist" = lib.mkForce {
        device = "tmpfs";
        fsType = "tmpfs";
        options = [ "noauto" ];
      };
      # qemu-vm.nix points GRUB at the VM's own root disk with mkVMOverride,
      # which would also beat boot.nix's mkDefault and silently install GRUB
      # to /dev/vda instead of the target. Clear that, so the device list is
      # boot.nix's own (the first target drive, /dev/vdb).
      boot.loader.grub.device = lib.mkOverride 5 "";
      # nixos-install installs the bootloader by running the system's own
      # switch-to-configuration in the chroot; test nodes leave it out unless
      # asked.
      system.switch.enable = true;
      boot.loader.grub.extraConfig = ''
        serial --unit=0 --speed=115200
        terminal_input serial
        terminal_output serial
        echo BIOS_GRUB_BOOT_OK
        halt
      '';

      # Don't let disko inject /persist into the test VM's fileSystems — the
      # test boots from /dev/vda and only formats the targets on demand.
      disko.enableConfig = false;

      # Runtime tools for the installer, assertions, and BIOS boot check.
      services.lvm.enable = true;
      environment.systemPackages = with pkgs; [
        lvm2
        cryptsetup
        util-linux
        disko
        grub2
        nixos-install-tools
        qemu
      ];

      # Expose the prebuilt diskoScript at a fixed path so the test can drive
      # the installer's --disko-script seam without nix-string interpolation
      # (and so it's part of the VM closure).
      environment.etc."losos/disko-script".source = config.system.build.diskoScript;

      virtualisation = {
        memorySize = 2048;
        cores = 2;
        useEFIBoot = false;
        # Three empty 2 GiB disks → /dev/vdb, /dev/vdc, /dev/vdd.
        emptyDiskImages = [
          2048
          2048
          2048
        ];
      };
    };

  testScript = ''
    installer.start()
    installer.wait_for_unit("default.target")

    # disko's luks `passwordFile` reads this at format time.
    installer.succeed(
        "install -d -m 700 /etc/keys",
        "head -c 4096 /dev/urandom > /etc/keys/persist-keyfile",
        "chmod 600 /etc/keys/persist-keyfile",
    )

    # DEBUG: what does the VM see?
    print("LSBLK:\n" + installer.succeed("lsblk -bdno NAME,SIZE,RM,TYPE"))
    print("MOUNTED:\n" + installer.succeed("findmnt -rn -o SOURCE"))

    # 1. Drive detection in isolation: --emit-target writes the
    #    install-target Nix file listing detected drives, without touching
    #    the disks.
    installer.succeed("losos-install --emit-target /tmp/autodetected.nix")
    detected = installer.succeed("cat /tmp/autodetected.nix")
    for d in ("vdb", "vdc", "vdd"):
        assert f"/dev/{d}" in detected, f"detector missed /dev/{d}"
    assert "/dev/vda" not in detected, "detector picked up the VM root disk"
    assert "losos.targetDrives" in detected, "no losos.targetDrives assignment emitted"
    assert "losos.bios = true;" in detected, "SeaBIOS autodetection did not select BIOS"

    # Explicit BIOS must override autodetection. UEFI cannot be selected when
    # the installer itself was booted in BIOS mode, because bootctl needs EFI
    # variables to install the firmware boot entry.
    installer.succeed(
        "losos-install --bios --emit-target /tmp/forced-bios.nix",
    )
    assert "losos.bios = true;" in installer.succeed("cat /tmp/forced-bios.nix")
    installer.fail("losos-install --uefi --emit-target /tmp/forced-uefi.nix")
    installer.succeed("test ! -e /tmp/forced-uefi.nix")

    # 2. Full format+mount via the prebuilt diskoScript (the installer's
    #    --disko-script seam): the "merge them to an LVM array and run disko"
    #    step.
    installer.succeed("losos-install --disko-script /etc/losos/disko-script")

    # The three drives are now PVs in one volume group.
    pv_count = installer.succeed("vgs --noheadings -o Pv_count persist-vg").strip()
    assert pv_count == "3", f"expected 3 PVs in persist-vg, got {pv_count!r}"

    # One logical volume `persist` consuming the pool.
    assert installer.succeed("lvs --noheadings -o lv_name persist-vg").strip() == "persist"

    # The LV is opened as LUKS `persist` and mounted as ext4 at /mnt/persist
    # (disko.rootMountPoint defaults to /mnt, so the whole tree mounts there).
    installer.succeed("test -e /dev/mapper/persist")
    assert installer.succeed("findmnt -no FSTYPE /mnt/persist").strip() == "ext4", \
        "expected ext4 at /mnt/persist"
    installer.succeed("mountpoint -q /mnt/persist")

    # ext4 is not a preference here: it is the whole reason the appliance gave
    # up btrfs's compression and data checksums. fscrypt needs a filesystem
    # that implements it, and the feature has to be set at mkfs time — it
    # cannot be turned on later. modules/disko.nix asks for it via
    # `extraArgs = [ "-O" "encrypt" ]`; if that argument were ever silently
    # dropped, the filesystem type assertion above would still pass and the
    # shared domain would sit unprotected. So check the feature, not just the
    # format.
    installer.succeed("tune2fs -l /dev/mapper/persist | grep -qw encrypt")

    # Each target drive carries an LVM PV partition; the VM root does not.
    for d in ("vdb", "vdc", "vdd"):
        installer.succeed(f"lsblk -ln -o FSTYPE /dev/{d} | grep -q LVM2")
    installer.fail("lsblk -ln -o FSTYPE /dev/vda | grep -q LVM2")

    # BIOS GRUB's embedding partition is first on vdb, followed by the ESP.
    assert installer.succeed("lsblk -no PARTTYPE /dev/vdb1").strip().lower() == \
        "21686148-6449-6e6f-744e-656564454649"
    installer.succeed("lsblk -ln -o FSTYPE /dev/vdb | grep -q vfat")
    installer.fail("lsblk -ln -o FSTYPE /dev/vdc | grep -q vfat")

    # Install the current system to the formatted target through nixos-install.
    # Its boot.nix is the production module, so this exercises the generated
    # GRUB installation path instead of invoking grub-install separately.
    installer.succeed(
        "mkdir -p /mnt/persist/nix /mnt/nix /mnt/etc/keys",
        "mount --bind /mnt/persist/nix /mnt/nix",
        "cp /etc/keys/persist-keyfile /mnt/etc/keys/persist-keyfile",
        "chmod 600 /mnt/etc/keys/persist-keyfile",
        # The store path, not the /run/current-system symlink: the profile
        # nixos-install sets is read again inside the /mnt chroot, where that
        # symlink resolves to nothing.
        "nixos-install --root /mnt --system $(readlink -f /run/current-system) "
        "--no-root-passwd --no-channel-copy",
        "umount -R /mnt || true",
        "sync",
    )
    # Boot the target disk alone under SeaBIOS. The injected grub.cfg prints a
    # marker on serial and halts; poll the log for it rather than betting on a
    # fixed timeout, since this is QEMU inside a (possibly TCG) test VM.
    # -snapshot, not readonly=on: an emulated IDE disk cannot be read-only.
    installer.succeed(
        "qemu-system-x86_64 -machine pc -accel tcg -m 128 "
        "-display none -monitor none -serial file:/tmp/bios-boot.log -no-shutdown "
        "-drive file=/dev/vdb,format=raw,if=ide -snapshot -boot order=c "
        "-daemonize -pidfile /tmp/bios-boot.pid"
    )
    try:
        installer.wait_until_succeeds("grep -q BIOS_GRUB_BOOT_OK /tmp/bios-boot.log", timeout=300)
    finally:
        print("BIOS BOOT LOG:\n" + installer.execute("cat /tmp/bios-boot.log")[1])
        installer.execute("kill $(cat /tmp/bios-boot.pid)")

    installer.succeed("umount -R /mnt || true")
  '';
}
