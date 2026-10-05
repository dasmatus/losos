# nixos-test-vms for the TPM2 unlock path, end to end: the installer formats
# the target with the production disko layout, seals the disk key to the VM's
# TPM (swtpm, via virtualisation.tpm), the VM reboots into a configuration
# whose initrd is the production boot.nix + disko.nix output for that layout,
# and /persist comes up with nobody typing anything.
#
# This is the proof behind `losos.tpm.enable = true` being the default: a
# default that needs a passphrase at format time or a `systemd-cryptenroll`
# after the first boot describes no box the ISO produces, and the previous
# TPM path needed both. Asserted here:
#   1. the installer autodetects the chip (`--emit-target` says tpm.enable =
#      true) and `--no-tpm` turns it off,
#   2. `losos-install --disko-script` formats, mounts AND enrols: the LUKS2
#      header carries a systemd-tpm2 token afterwards,
#   3. the keyfile the volume was formatted with is kept inside /persist and
#      still opens the volume (the recovery slot `losos-ctl grow` uses),
#   4. a reboot into the unlock configuration mounts /persist from
#      /dev/mapper/persist unattended, with no fallback to a passphrase in
#      the cryptsetup unit's journal, and with no initrd secret declared.
#
# Mechanics borrowed from nixpkgs' systemd-initrd-luks-tpm2 test: the VM boots
# through a real systemd-boot (virtualisation.useBootLoader) so a
# specialisation can carry the LUKS+/persist initrd config that the first boot
# — on a still-empty disk — cannot have; `bootctl set-default` picks it, and a
# hard reset keeps both the EFI variables and the swtpm state, which the test
# driver stores in its own temporary directory.
{
  pkgs,
  disko,
}:

pkgs.testers.nixosTest {
  name = "losos-tpm-unlock";

  nodes.machine =
    {
      config,
      lib,
      pkgs,
      ...
    }:
    let
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

      losos.installer.package = lososPkgs.losos-ctl;
      losos.targetDrives = [ "/dev/vdb" ];
      # losos.tpm.enable is left at its default on purpose: this test is what
      # proves the default boots. losos.bios likewise (UEFI, systemd-boot).

      # disko's generated NixOS config is not used directly: the test VM
      # module defines `fileSystems` and `boot.initrd.luks.devices` wholesale
      # at mkVMOverride priority, which discards every normal-priority
      # definition of those options before they are merged (disko's and
      # boot.nix's alike). The specialisation below therefore feeds disko's
      # *generated* config — the same `_config` the module would have
      # applied — in through the VM's own `virtualisation.fileSystems` and
      # at the VM's priority, so what boots is still what modules/disko.nix
      # declares, not a copy of it. The first boot, on a still-empty
      # /dev/vdb, carries neither.
      disko.enableConfig = false;

      specialisation.unlock.configuration =
        { config, lib, ... }:
        let
          generated = config.disko.devices._config;
          persistFs = map (c: c."/persist") (
            builtins.filter (c: c ? "/persist") generated.fileSystems.contents
          );
        in
        {
          # Only /persist: disko's /boot is the target's ESP, and the VM boots
          # from its own. neededForBoot is boot.nix's line, discarded with the
          # rest of its fileSystems definition by the override described above.
          virtualisation.fileSystems."/persist" = lib.mkMerge (persistFs ++ [ { neededForBoot = true; } ]);
          # mkMerge outside, mkVMOverride inside: the module system pushes an
          # override into a plain attrset but not into a merge marker.
          boot.initrd.luks.devices = lib.mkMerge (
            map (
              c: lib.mkVMOverride (lib.attrByPath [ "initrd" "luks" "devices" ] { } c)
            ) generated.boot.contents
          );
        };

      # The VM's OVMF has no EFI variable store at image-build time, where
      # the test framework installs the bootloader; the production value
      # (true) is for a real firmware.
      boot.loader.efi.canTouchEfiVariables = lib.mkForce false;
      # nixos-install is not run here; the framework's own image build
      # installs systemd-boot. Keep switch-to-configuration for bootctl.
      system.switch.enable = true;

      environment.etc."losos/disko-script".source = config.system.build.diskoScript;

      services.lvm.enable = true;
      environment.systemPackages = with pkgs; [
        lvm2
        cryptsetup
        util-linux
        disko
      ];

      virtualisation = {
        memorySize = 2048;
        cores = 2;
        useBootLoader = true;
        useEFIBoot = true;
        tpm.enable = true;
        # One empty 4 GiB disk → /dev/vdb.
        emptyDiskImages = [ 4096 ];
      };
    };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the installer sees the chip and defaults to it"):
        machine.succeed("test -c /dev/tpmrm0")
        machine.succeed("losos-install --emit-target /tmp/auto.nix")
        auto = machine.succeed("cat /tmp/auto.nix")
        assert "losos.tpm.enable = true;" in auto, auto
        machine.succeed("losos-install --no-tpm --emit-target /tmp/notpm.nix")
        assert "losos.tpm.enable = false;" in machine.succeed("cat /tmp/notpm.nix")
        machine.succeed("losos-install --tpm --emit-target /tmp/forced.nix")
        assert "losos.tpm.enable = true;" in machine.succeed("cat /tmp/forced.nix")

    with subtest("format, mount and enrol, with nothing typed"):
        # The real installer path for this seam: generate the keyfile, run
        # the prebuilt disko script, seal the token. stdin is /dev/null so a
        # prompt anywhere would fail the step instead of hanging the test.
        machine.succeed("losos-install --disko-script /etc/losos/disko-script < /dev/null")
        machine.succeed("test -e /dev/mapper/persist")
        assert machine.succeed("findmnt -no FSTYPE /mnt/persist").strip() == "ext4"
        dump = machine.succeed("cryptsetup luksDump /dev/persist-vg/persist")
        assert "systemd-tpm2" in dump, dump

    with subtest("the keyfile stays inside /persist as the recovery slot"):
        # What install_tail does on the real path (the --disko-script seam
        # stops after the format): the /persist copy, and no /mnt/etc copy.
        machine.succeed(
            "install -d -m 700 /mnt/persist/etc/keys",
            "install -m 600 /etc/keys/persist-keyfile /mnt/persist/etc/keys/persist-keyfile",
        )
        machine.succeed(
            "cryptsetup open --test-passphrase --key-file /mnt/persist/etc/keys/persist-keyfile /dev/persist-vg/persist"
        )
        machine.succeed("sync", "umount -R /mnt", "cryptsetup close persist")

    with subtest("the next boot unlocks from the chip"):
        # The framework's boot image names its entries by hash, not
        # nixos-generation-N, and the specialisation's `init=` is its own
        # toplevel rather than a .../specialisation/unlock path, so find the
        # entry by its title. Both the EFI variable (bootctl) and
        # loader.conf's default are set: systemd-boot honours the variable
        # first and falls back to the file, and a lost variable would
        # silently boot the wrong system.
        entry = machine.succeed("grep -l '^title NixOS (unlock)$' /boot/loader/entries/*.conf").strip()
        assert entry.count("\n") == 0, entry
        entry_id = entry.rsplit("/", 1)[1]
        machine.succeed(f"bootctl set-default {entry_id}")
        machine.succeed(f"sed -i 's|^default .*|default {entry_id}|' /boot/loader/loader.conf")
        machine.succeed("sync")
        machine.crash()
        machine.wait_for_unit("multi-user.target")
        assert machine.succeed("findmnt -no SOURCE /persist").strip() == "/dev/mapper/persist"
        assert machine.succeed("findmnt -no FSTYPE /persist").strip() == "ext4"
        machine.succeed("test -f /persist/etc/keys/persist-keyfile")
        # The unit that opened it, from the initrd journal (flushed into this
        # boot's). A chip that failed would have fallen back to a prompt and
        # the boot would not have reached multi-user.target unattended, but
        # say it in words anyway.
        unlock_log = machine.succeed("journalctl -b -u systemd-cryptsetup@persist.service")
        assert "falling back" not in unlock_log, unlock_log
        assert "Failed" not in unlock_log, unlock_log
        # No secret rode along in the initrd: the key on the ESP is the
        # thing the TPM path exists to avoid.
        machine.fail("test -e /crypto_keyfile.bin")
        machine.succeed("cryptsetup luksDump /dev/persist-vg/persist | grep -q systemd-tpm2")
  '';
}
