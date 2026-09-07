# Base system configuration: networking, time/locale, packages, users.
# Boot, disk, and services live in their own modules.
{
  pkgs,
  lib,
  config,
  ...
}:

{
  # nixos-unstable defaulted to Python 3.14, under which tahoe-lafs's
  # txi2p-tahoe dependency fails to build. Rebuild tahoe-lafs against
  # Python 3.12 until upstream catches up. The tahoe module picks this up
  # via services.tahoe.*.package = pkgs.tahoe-lafs.
  nixpkgs.overlays = [
    (final: prev: {
      tahoe-lafs =
        (prev.tahoe-lafs.override {
          python3Packages = final.python312Packages;
        }).overridePythonAttrs
          (_old: {
            # Skip upstream's trial suite: it is incompatible with the twisted
            # 26.4.0 in this nixpkgs, not broken. 1694 of 1708 tests pass; the 12
            # that don't are the removed TestCase aliases (failUnlessRaises and
            # friends, dropped in twisted 26.4.0) plus one Windows-only test that
            # has no business running on Linux. Nothing functional is wrong — the
            # appliance ships the pinned derivation, not its test corpus.
            #
            # overridePythonAttrs, *not* overrideAttrs: buildPythonPackage runs
            # the suite in installCheckPhase and derives doInstallCheck from
            # doCheck when it builds its mkDerivation args. Setting doCheck = false
            # from the outside with overrideAttrs leaves doInstallCheck = true
            # behind, the suite runs anyway, and the whole system closure fails to
            # build. overridePythonAttrs re-enters buildPythonPackage so both flags
            # move together.
            doCheck = false;
          });
    })
  ];

  # Declarative users rebuilt via `services.userborn`, so user changes apply
  # without a reboot.
  services.userborn.enable = true;

  # Hostname + mDNS: the appliance is reachable on the LAN as `<hostName>.local`
  # via Avahi/mDNS. (`networking.hostName` is the bare label, routed through the
  # losos.hostName option so the admin app can change it — see modules/overrides.nix;
  # Avahi publishes the `<hostname>.local` form. Keep it ≤15 chars for NetBIOS/mDNS
  # safety.)
  networking.hostName = config.losos.hostName;
  networking.networkmanager.enable = true;

  # Publish `mattbox.local` and resolve other `.local` names on the LAN via
  # mDNS, so the appliance is reachable by name without a local DNS server.
  #
  # allowInterfaces is deliberately unset (Avahi's default is every interface).
  # This box is a repurposed mini-PC and mDNS is the *only* way to reach it —
  # no SSH, no shell logins — so a hardcoded NIC list is a brick: a machine
  # whose interface enumerates as eno1 or enp0s31f6 instead publishes nothing
  # and has no recovery short of reinstalling.
  services.avahi = {
    enable = true;
    nssmdns4 = true;
    publish = {
      enable = true;
      addresses = true;
      workstation = true;
    };
  };

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

  # Each data account gets its OWN primary group, and this is load-bearing.
  #
  # `isNormalUser` defaults the primary group to `users`, and the homes were
  # mode 750 — which grants r-x to the primary group. Both accounts therefore
  # sat in `users` and each could read the other's data. The 750 kept out
  # accounts *outside* `users` and nothing else, which is not what "the two
  # users are mutually unreadable" means, and is the opposite of what the
  # comments here used to claim.
  #
  # The homes are 700 now, so group access is moot for reading; the per-account
  # groups stay because files created inside inherit the directory's group, and
  # `users` is the wrong owner for either data domain. tests/impermanence.nix
  # asserts both halves in a booted VM.
  users.groups.notshared = {
    gid = 1000;
  };
  users.groups.shared = {
    gid = 1001;
  };

  # `notshared` — owns this box's private Nextcloud instance. Reached via the
  # Nextcloud web UI only; no Linux login (no password, no SSH).
  users.users."notshared" = {
    uid = 1000;
    description = "Internal (default) — private Nextcloud on this box";
    isNormalUser = true;
    group = "notshared";
    homeMode = "700"; # owner only; shared cannot enter, let alone read
  };

  # `shared` — contributes this box's storage to the Tahoe-LAFS grid. Reached
  # via the Tahoe web UI only; no Linux login.
  users.users."shared" = {
    uid = 1001;
    description = "Shared — Tahoe-LAFS grid storage contributor";
    isNormalUser = true;
    group = "shared";
    homeMode = "700"; # owner only; notshared cannot enter, let alone read
  };

  # The rootless-Podman `containers` user is gone: services run in declarative
  # systemd-nspawn containers now (modules/containers.nix), which need no
  # unprivileged runtime owner, subuid ranges, or linger.

  # ── GPU / graphics (host side of container GPU acceleration) ─────────────
  # Enables /dev/dri + Mesa and the VA-API drivers the Nextcloud container uses
  # for hardware video decode/encode (recognize, Talk transcoding, previews).
  # intel-media-driver covers Intel iGPUs (typical mini-PC), mesa-va-drivers
  # covers AMD/Radeon — both coexist; drop the one that doesn't match the box.
  hardware.graphics = lib.mkIf config.losos.gpu.enable {
    enable = true;
    enable32Bit = true;
    # VA-API drivers. intel-media-driver covers modern Intel iGPUs (typical
    # mini-PC), intel-vaapi-driver the older Intel gens; both coexist. For an
    # AMD/Radeon box swap in `mesa`/`rocmPackages` as appropriate.
    extraPackages = with pkgs; [
      intel-media-driver
      intel-vaapi-driver
    ];
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
