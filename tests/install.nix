# nixos-test-vms config for the losos installer.
#
# Boots a VM with three empty virtio disks (/dev/vdb, /dev/vdc, /dev/vdd) plus
# its own root on /dev/vda, then runs the `losos-install` installer against
# them and asserts:
#   1. drive detection finds the three target disks and skips the VM root,
#   2. disko merges all three into one LVM volume group `persist-vg`,
#   3. the single `persist` LV is opened as LUKS and mounted as btrfs at
#      /mnt/persist,
#   4. the ESP lands on the first target drive only.
#
# The installer is driven through its --disko-script seam (a prebuilt
# `config.system.build.diskoScript`) so the test never needs nix or the full
# appliance closure inside the VM — it exercises the installer's drive
# detection + override generation + the real disko format/mount path, which is
# the "find drives, merge them to an LVM array, run disko" claim. The
# subsequent `nixos-install` step is a thin, standard wrapper and isn't run
# here (it would require building the whole appliance closure in the VM).
{
  pkgs,
  disko,
}:

pkgs.testers.nixosTest {
  name = "losos-install-lvm";

  nodes.installer =
    { config, pkgs, ... }:
    let
      # The disks the installer is expected to find and merge. The VM root is
      # /dev/vda (excluded by detection as a mounted disk), so these three
      # empty images are the only candidates detection should return.
      targets = [
        "/dev/vdb"
        "/dev/vdc"
        "/dev/vdd"
      ];
    in
    {
      imports = [
        disko.nixosModules.disko
        ../modules/options.nix
        ../modules/disko.nix
        ../modules/installer.nix
      ];

      # Drive list the disko layout pools into `persist-vg`, plus TPM mode.
      # The test uses the keyfile path (unattended); the keyfile is created
      # by the test script before disko runs (disko's luks passwordFile).
      losos.targetDrives = targets;
      losos.tpm.enable = false;

      # Don't let disko inject /persist into the test VM's fileSystems — the
      # test boots from /dev/vda and only formats the targets on demand.
      disko.enableConfig = false;

      # lvm2 at runtime (mirrors production) + the tools the assertions use.
      services.lvm.enable = true;
      environment.systemPackages = with pkgs; [
        lvm2
        cryptsetup
        util-linux
        disko
      ];

      # Expose the prebuilt diskoScript at a fixed path so the test can drive
      # the installer's --disko-script seam without nix-string interpolation
      # (and so it's part of the VM closure).
      environment.etc."losos/disko-script".source = config.system.build.diskoScript;

      virtualisation = {
        memorySize = 2048;
        cores = 2;
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
    installer.succeed("losos-install --emit-target /tmp/detected.nix")
    detected = installer.succeed("cat /tmp/detected.nix")
    for d in ("vdb", "vdc", "vdd"):
        assert f"/dev/{d}" in detected, f"detector missed /dev/{d}"
    assert "/dev/vda" not in detected, "detector picked up the VM root disk"
    assert "losos.targetDrives" in detected, "no losos.targetDrives assignment emitted"

    # 2. Full format+mount via the prebuilt diskoScript (the installer's
    #    --disko-script seam): the "merge them to an LVM array and run disko"
    #    step.
    installer.succeed("losos-install --disko-script /etc/losos/disko-script")

    # The three drives are now PVs in one volume group.
    pv_count = installer.succeed("vgs --noheadings -o Pv_count persist-vg").strip()
    assert pv_count == "3", f"expected 3 PVs in persist-vg, got {pv_count!r}"

    # One logical volume `persist` consuming the pool.
    assert installer.succeed("lvs --noheadings -o lv_name persist-vg").strip() == "persist"

    # The LV is opened as LUKS `persist` and mounted as btrfs at /mnt/persist
    # (disko.rootMountPoint defaults to /mnt, so the whole tree mounts there).
    installer.succeed("test -e /dev/mapper/persist")
    assert installer.succeed("findmnt -no FSTYPE /mnt/persist").strip() == "btrfs", \
        "expected btrfs at /mnt/persist"
    installer.succeed("mountpoint -q /mnt/persist")

    # Each target drive carries an LVM PV partition; the VM root does not.
    for d in ("vdb", "vdc", "vdd"):
        installer.succeed(f"lsblk -ln -o FSTYPE /dev/{d} | grep -q LVM2")
    installer.fail("lsblk -ln -o FSTYPE /dev/vda | grep -q LVM2")

    # The ESP lands on the first target drive only (vdb): its first partition
    # is vfat. The other targets have no vfat partition.
    installer.succeed("lsblk -ln -o FSTYPE /dev/vdb | grep -q vfat")
    installer.fail("lsblk -ln -o FSTYPE /dev/vdc | grep -q vfat")

    installer.succeed("umount -R /mnt || true")
  '';
}