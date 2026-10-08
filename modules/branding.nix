# What a LosOS system calls itself: LosOS and its release tag.
#
# Imported by both systems; modules/live-branding.nix adds what only the
# installer medium shows. The tag comes from flake/version.nix and is the
# system label, so the boot menu entries, the toplevel's store name and the
# console banners carry it. os-release keeps NixOS's VERSION and VERSION_ID
# and adds the tag as IMAGE_VERSION.
#
# On the installed box the os-release ID stays `nixos`: switch-to-configuration
# and other tools compare it, and nothing an owner sees depends on it.
# Internal names stay too: the `nixos` user, /etc/nixos, the kernel's
# version string and the boot units nixpkgs names after itself ("NixOS
# Activation").
{
  config,
  lib,
  pkgs,
  ...
}:

let
  inherit (config.system.nixos) release;
  version = import ../flake/version.nix;
  art = import ./brand-art.nix { inherit pkgs version; };
  inherit (art.palette) ground accent;

  # "#rrggbb" as the "r;g;b" a terminal escape wants.
  rgb =
    hex:
    lib.concatMapStringsSep ";" (i: toString (lib.fromHexString (builtins.substring i 2 hex))) [
      1
      3
      5
    ];
in
{
  # The boot menu entries (systemd-boot, GRUB, syslinux) and os-release NAME.
  system.nixos.distroName = "LosOS";
  system.nixos.label = version;
  system.image.version = version;
  system.nixos.extraOSReleaseArgs = {
    # systemd's "Welcome to …" line, in stage 1 and stage 2.
    PRETTY_NAME = lib.mkDefault "LosOS ${version}, built on NixOS ${release}";
    # The name in that line, in the accent colour.
    ANSI_COLOR = "1;38;2;${rgb accent}";
    HOME_URL = "https://github.com/dasmatus/losos";
  };

  # The banner agetty prints above the login prompt (tty2 to tty6 on the
  # box; tty1 is modules/console.nix). Above nixpkgs' mkDefault, below a
  # plain definition such as the installer's.
  services.getty.greetingLine = lib.mkOverride 900 ''<<< LosOS ${version}, built on NixOS ${release} (\m) - \l >>>'';

  # The console's sixteen colours, on the same palette: the dark ground and
  # light text, and the accent wash where the stock palette has blue, which
  # is the panel behind the tty1 banner (modules/console.nix). Normal, then
  # bright: black red green yellow blue magenta cyan white.
  console.colors = [
    "0a0e12"
    "d1756c"
    "4fa583"
    "c79a3e"
    "0d2a2f"
    "a58bc4"
    "3a909b"
    "e2e8ee"
    "6b7986"
    "e8958d"
    "6fc29f"
    "dbb35e"
    "6cc6d2"
    "c3aee0"
    "48b3c0"
    "ffffff"
  ];

  # GRUB is the box's loader on a BIOS machine (modules/boot.nix). systemd-boot,
  # the UEFI one, shows text only and takes its entry titles from distroName.
  boot.loader.grub = {
    theme = "${art}/grub";
    splashImage = "${art}/grub/background.png";
    backgroundColor = ground;
  };

  system.build.lososBootArt = art;
}
