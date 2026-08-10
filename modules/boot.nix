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

  # systemd stage 1 is a hard prerequisite for TPM2-based LUKS unlock, and
  # also gives us clean crypttab handling for the keyfile path.
  boot.initrd.systemd.enable = true;
  boot.initrd.systemd.tpm2.enable = useTpm;

  # tpm_tis must be in the initrd for the TPM2 token to be readable early.
  boot.initrd.availableKernelModules =
    if useTpm then
      [ "tpm_tis" "xhci_pci" "ahci" "nvme" "usb_storage" "sd_mod" ]
    else
      [ "xhci_pci" "ahci" "nvme" "usb_storage" "sd_mod" ];

  # Keyfile path: inject the keyfile from the host into the initrd. The TPM
  # path needs no secret — the key lives in the LUKS2 header, sealed by TPM2.
  boot.initrd.secrets =
    if useTpm then { } else { "/crypto_keyfile.bin" = "/etc/keys/persist-keyfile"; };

  # impermanence bind-mounts from /persist, so /persist must be mounted before
  # the sysroot is populated.
  fileSystems."/persist".neededForBoot = true;
}