# Base system configuration: networking, time/locale, packages, users.
# Boot, disk, and services live in their own modules.
{
  pkgs,
  lib,
  ...
}:

{
  # nixos-unstable defaulted to Python 3.14, under which tahoe-lafs's
  # txi2p-tahoe dependency fails to build. Rebuild tahoe-lafs against
  # Python 3.12 until upstream catches up. The tahoe module picks this up
  # via services.tahoe.*.package = pkgs.tahoe-lafs.
  nixpkgs.overlays = [
    (final: prev: {
      tahoe-lafs = prev.tahoe-lafs.override {
        python3Packages = final.python312Packages;
      };
    })
  ];

  # Declarative users rebuilt via `services.userborn`, so user changes apply
  # without a reboot.
  services.userborn.enable = true;

  networking.hostName = "losos";
  networking.networkmanager.enable = true;

  time.timeZone = "Europe/Berlin";

  i18n.defaultLocale = "en_US.UTF-8";
  i18n.extraLocaleSettings = {
    LC_ADDRESS = "de_DE.UTF-8";
    LC_IDENTIFICATION = "de_DE.UTF-8";
    LC_MEASUREMENT = "de_DE.UTF-8";
    LC_MONETARY = "de_DE.UTF-8";
    LC_NAME = "de_DE.UTF-8";
    LC_NUMERIC = "de_DE.UTF-8";
    LC_PAPER = "de_DE.UTF-8";
    LC_TELEPHONE = "de_DE.UTF-8";
    LC_TIME = "de_DE.UTF-8";
  };

  # A small, sensible base package set. Service-specific packages
  # (nextcloud, tahoe-lafs) are pulled in by their modules.
  environment.systemPackages = with pkgs; [
    curl
    git
    vim
    htop
    cryptsetup
  ];

  # Set-and-forget appliance: no remote shell access. SSH is off (and nothing
  # in the target config enables it), so the only way to reach a shell would be
  # the physical console — and neither data account has a password, so NixOS
  # refuses to log them in. The two data domains are therefore isolated.
  services.openssh.enable = false;

  # `notshared` — owns this box's private Nextcloud instance. Reached via the
  # Nextcloud web UI only; no Linux login (no password, no SSH).
  users.users."notshared" = {
    uid = 1000;
    description = "Internal (default) — private Nextcloud on this box";
    isNormalUser = true;
    homeMode = "750"; # private home: shared can't read notshared's data
  };

  # `shared` — contributes this box's storage to the Tahoe-LAFS grid. Reached
  # via the Tahoe web UI only; no Linux login.
  users.users."shared" = {
    uid = 1001;
    description = "Shared — Tahoe-LAFS grid storage contributor";
    isNormalUser = true;
    homeMode = "750"; # private home: notshared can't read shared's data
  };

  # System account reserved for future restricted deploy triggers.
  # system.autoUpgrade itself runs as a root systemd unit, so this account is
  # intentionally login-less (no password, no usable shell).
  users.users."update" = {
    uid = 990;
    description = "Update user";
    isSystemUser = true;
    group = "nogroup";
  };

  # The Tahoe-LAFS module creates system users `tahoe.<node>` and
  # `tahoe.introducer-<name>` without setting a group, which trips nixpkgs'
  # "user without group" assertion. Give them explicit groups.
  users.groups."tahoe-shared" = { };
  users.groups."tahoe-introducer-local" = { };
  users.users."tahoe.shared".group = "tahoe-shared";
  users.users."tahoe.introducer-local".group = "tahoe-introducer-local";

  # Set on first install; do not change afterwards. Matches the nixos-unstable
  # release this flake tracks.
  system.stateVersion = "26.11";
}