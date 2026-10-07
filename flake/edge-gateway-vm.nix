# The disk-image half of the edge gateway: what makes modules/edge-gateway.nix
# bootable as a QCOW2 (`nix build .#losos-disk-edge-qcow2`). Hardware and
# boot only; the gateway's behaviour is the module.
#
# The same shape as the demo image's overlay in flake/disk-images.nix, with
# none of its caveats: the gateway has no encrypted layout to give up, no
# impermanence and no owner data. A plain ext4 root found by label, systemd-boot
# on an ESP, a serial console beside tty1, the QEMU guest profile so virtio
# disks and NICs are in the initrd, and the guest agent so Proxmox and libvirt
# can shut it down cleanly. The label is load-bearing: fileSystems."/" finds
# the root by it, and it is what `lsblk` shows for a stray image on somebody's
# disk.
{ modulesPath, ... }:

{
  imports = [ (modulesPath + "/profiles/qemu-guest.nix") ];

  losos.edge.gateway.enable = true;
  networking.hostName = "losos-edge";
  system.stateVersion = "26.11";

  fileSystems = {
    "/" = {
      device = "/dev/disk/by-label/losos-edge";
      fsType = "ext4";
      autoResize = true;
    };
    "/boot" = {
      device = "/dev/disk/by-label/ESP";
      fsType = "vfat";
      options = [ "umask=0077" ];
    };
  };

  boot.loader.systemd-boot.enable = true;
  # The build VM has no efivarfs and a QCOW2 has no NVRAM; the boot entry
  # lives in the OVMF_VARS file the hypervisor keeps.
  boot.loader.efi.canTouchEfiVariables = false;
  boot.growPartition = true;
  boot.kernelParams = [
    "console=tty0"
    "console=ttyS0,115200"
  ];
  services.qemuGuest.enable = true;

  # A gateway is a LAN appliance: it takes an address by DHCP on whatever
  # NIC it has, and nothing here names an interface.
  networking.useDHCP = true;
}
