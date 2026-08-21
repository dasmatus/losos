# Bootloader and initrd. The LUKS unlock method for /persist is decided in
# disko.nix; this module provides the initrd infrastructure both paths need
# (systemd stage 1 + TPM2 support) and the keyfile secret for the keyfile path.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  useTpm = config.losos.tpm.enable;
in
{
  boot.loader.systemd-boot.enable = true;
  boot.loader.efi.canTouchEfiVariables = true;

  # Latest upstream kernel. The losos appliance runs on repurposed mini-PCs
  # whose NVMe/Wi-Fi/sleep hardware quirks are fixed fastest in mainline, so
  # track linuxPackages_latest rather than the nixpkgs default LTS. The
  # auto-upgrade flake (updates.nix) advances the system — and nixpkgs with
  # it — so this stays current without manual bumps. Scoped to the appliance
  # (boot.nix is only imported by the `install` system); the installer ISO
  # and the edge VPS keep the nixpkgs default kernel.
  boot.kernelPackages = pkgs.linuxPackages_latest;

  # systemd stage 1 is a hard prerequisite for TPM2-based LUKS unlock, and
  # also gives us clean crypttab handling for the keyfile path.
  boot.initrd.systemd.enable = true;
  boot.initrd.systemd.tpm2.enable = useTpm;

  # /persist sits on a LUKS volume that sits on an LVM logical volume
  # (disko.nix pools losos.targetDrives into `persist-vg`). With systemd stage
  # 1 the VG must be activated *before* cryptsetup looks for
  # /dev/persist-vg/persist. `services.lvm.enable` turns on lvm2 at runtime
  # and — because we use systemd initrd — `boot.initrd.services.lvm.enable`
  # defaults true, which adds lvm2 to the initrd and activates VGs early.
  # disko's lvm_vg type only adds kernel modules, not this service, so this
  # line is load-bearing.
  services.lvm.enable = true;

  # dm_mod is the device-mapper base (LVM + LUKS both sit on it); dm-snapshot
  # is pulled in by disko's lvm_vg config but we list it explicitly to be safe.
  # tpm_tis must be in the initrd for the TPM2 token to be readable early.
  boot.initrd.availableKernelModules =
    lib.optional useTpm "tpm_tis"
    ++ [
      "dm_mod"
      "dm-snapshot"
      "xhci_pci"
      "ahci"
      "nvme"
      "usb_storage"
      "sd_mod"
    ];

  # Keyfile path: inject the keyfile from the host into the initrd. The TPM
  # path needs no secret — the key lives in the LUKS2 header, sealed by TPM2.
  boot.initrd.secrets =
    if useTpm then { } else { "/crypto_keyfile.bin" = "/etc/keys/persist-keyfile"; };

  # Point the initrd unlock of `persist` (whose device entry disko generates
  # from the layout) at that baked-in secret. Deliberately NOT expressed as
  # disko's settings.keyFile: disko reuses that value at *format* time with
  # precedence over passwordFile, and /crypto_keyfile.bin only ever exists
  # inside the initrd — formatting on the installer would fail.
  boot.initrd.luks.devices.persist.keyFile = lib.mkIf (!useTpm) "/crypto_keyfile.bin";

  # impermanence bind-mounts from /persist, so /persist must be mounted before
  # the sysroot is populated.
  fileSystems."/persist".neededForBoot = true;
}