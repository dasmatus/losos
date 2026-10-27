# The box CI's discovery jobs (.github/workflows/ci.yml) boot on Linux, macOS
# and Windows runners and check with nix/tests/discovery/check.py: the same
# modules as nix/tests/windows.nix's box, as kernel + initrd with the whole
# store inside, so a plain QEMU on any of the three can boot it with no disk
# and no Nix. Built only for those jobs, never shipped: it has a root shell
# on its serial port, which the jobs use for the one thing a client cannot
# do, switching the share off.
#
# Its LAN is 192.168.77.0/24: the box is .2, the computer under test is on
# the same link (a tap on Linux, vmnet on macOS, a TAP adapter on Windows).
{
  config,
  pkgs,
  lib,
  modulesPath,
  ...
}:

let
  lososPkgs = import ../../packages.nix { inherit pkgs; };
in
{
  imports = [
    "${modulesPath}/installer/netboot/netboot.nix"
    "${modulesPath}/profiles/qemu-guest.nix"
    ../../../modules/options.nix
    ../../../modules/mdns.nix
    ../../../modules/containers.nix
    ../../../modules/tls.nix
    ../../../modules/daemon.nix
    ../../../modules/identity.nix
    ../../../modules/cloud.nix
    ../../../modules/windows.nix
    ../../../modules/config-repo.nix
  ];

  system.stateVersion = "26.11";
  networking.hostName = "mattbox";
  losos = {
    hostName = "mattbox";
    backend.package = lososPkgs.losos-ctl;
    admin.ui = lososPkgs.losos-admin-ui;
    cloud.enable = true;
    git.enable = false;
    configRepo.enable = false;
    mdns.enable = true;
  };
  # The same box with the share off, for the WebDAV entry.
  specialisation.off.configuration.losos.cloud.windows.enable = lib.mkForce false;

  networking.usePredictableInterfaceNames = false;
  networking.useDHCP = false;
  networking.interfaces.eth0.ipv4.addresses = [
    {
      address = "192.168.77.2";
      prefixLength = 24;
    }
  ];

  # Test media only: root on the serial console, nothing else.
  boot.kernelParams = [ "console=ttyS0,115200" ];
  services.getty.autologinUser = "root";
  environment.systemPackages = [ pkgs.jq ];

  # Fast to pack, the runners unpack it under emulation.
  netboot.squashfsCompression = "zstd -Xcompression-level 6";

  system.build.discoveryBox = pkgs.linkFarm "losos-disk-discovery-box" [
    {
      name = "bzImage";
      path = "${config.system.build.kernel}/${config.system.boot.loader.kernelFile}";
    }
    {
      name = "initrd";
      path = "${config.system.build.netbootRamdisk}/initrd";
    }
    {
      name = "cmdline";
      path = pkgs.writeText "cmdline" "init=${config.system.build.toplevel}/init ${toString config.boot.kernelParams}";
    }
  ];
}
