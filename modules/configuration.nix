# Base system configuration: networking, time/locale, packages, users.
# Boot, disk, and services live in their own modules.
{
  pkgs,
  lib,
  config,
  ...
}:

{
  imports = [ ./mdns.nix ];

  # No nixpkgs.overlays. The one entry this file used to carry rebuilt
  # tahoe-lafs against Python 3.12 — nixos-unstable moved to 3.14 and
  # txi2p-tahoe does not build under it — and it left with Tahoe-LAFS itself
  # (see modules/services.nix). Think hard before adding another: an overlay
  # takes every dependent path out of the binary cache, and this box rebuilds
  # itself unattended at 03:00 with no shell to watch a compile from.

  # Declarative users rebuilt via `services.userborn`, so user changes apply
  # without a reboot.
  services.userborn.enable = true;

  # Hostname + mDNS: the appliance is reachable on the LAN as `<hostName>.local`
  # through systemd-resolved's mDNS (modules/mdns.nix). (`networking.hostName`
  # is the bare label, routed through the losos.hostName option so the admin
  # app can change it — see modules/overrides.nix; resolved publishes the
  # `<hostname>.local` form. Keep it ≤15 chars for NetBIOS/mDNS safety.)
  networking.hostName = config.losos.hostName;
  networking.networkmanager.enable = true;

  # Publish `mattbox.local` and resolve other `.local` names on the LAN, so
  # the appliance is reachable by name without a local DNS server, on every
  # physical interface (modules/mdns.nix says why there is no interface list).
  losos.mdns = {
    enable = true;
    # `_losos._tcp` on the front vhost's port says what this machine is, so
    # a computer on the LAN can list the boxes by browsing one service type
    # (LosOS Desktop does) instead of asking every workstation for
    # /setup/state.json. The release tag is there for a client to show; the
    # name is the box's own (%H, the host name), which is also its `.local`
    # name.
    services.losos = ''
      Name=%H
      Type=_losos._tcp
      Port=80
      TxtText=txtvers=1 version=${import ../nix/version.nix}
    '';
    # What Avahi announced for every box before `_losos._tcp` existed, kept
    # for whatever lists the machines on a LAN by it (a file manager's
    # Network view, `avahi-browse` on someone's laptop). Port 9, discard, as
    # Avahi had it: there is no service behind it.
    services.workstation = ''
      Name=%H
      Type=_workstation._tcp
      Port=9
    '';
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
  # (the cluster tooling, LosOS Cloud) are pulled in by their modules.
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
  # `users` is the wrong owner for either data domain. nix/tests/impermanence.nix
  # asserts both halves in a booted VM.
  users.groups.notshared = {
    gid = 1000;
  };
  users.groups.shared = {
    gid = 1001;
  };
  users.groups.kindashared = {
    gid = 1004;
  };

  # `notshared` — the owner's account: their files in LosOS Cloud carry this
  # name, and their apps run as it. No Linux login (no password, no SSH).
  users.users."notshared" = {
    uid = 1000;
    description = "Internal (default) — the owner's private data on this box";
    isNormalUser = true;
    group = "notshared";
    homeMode = "700"; # owner only; shared cannot enter, let alone read
  };

  # `shared` — contributes this box's storage to the mesh. Its home holds the
  # fscrypt-protected data domain (modules/fscrypt.nix); no Linux login, and
  # no web UI of its own any more now that the Tahoe WUI is gone.
  #
  # The uid, the gid and the 700 home are the isolation model and are asserted
  # by nix/tests/impermanence.nix in a booted VM — the account outlives the service
  # that used to own it, so do not renumber or repurpose it.
  users.users."shared" = {
    uid = 1001;
    description = "Shared — mesh storage contributor";
    isNormalUser = true;
    group = "shared";
    homeMode = "700"; # owner only; notshared cannot enter, let alone read
  };

  # `kindashared` — the shared account's counterpart for the local edge
  # gateways of this box's own network, and nothing further: its home holds a
  # second fscrypt data domain, unlocked only while losos.sharingLocally is on
  # (modules/fscrypt.nix), which lososd allows only while a local gateway
  # answers (backend/src/edge.rs). Same shape as `shared` and asserted by the
  # same test: its own group, a 700 home, no Linux login. Not 1002, which
  # stays retired (below).
  users.users."kindashared" = {
    uid = 1004;
    description = "Kindashared — storage shared with local edge gateways only";
    isNormalUser = true;
    group = "kindashared";
    homeMode = "700"; # owner only; neither other account can enter
  };

  # ── Retired workload uids ─────────────────────────────────────────────────
  # The box's own apps no longer run as pods. 1002 was Nextcloud's and stays
  # unused: the files a box's Nextcloud left behind carry it until LosOS Cloud
  # takes them in (modules/cloud.nix). 1003 was Forgejo's; LosOS Git's account
  # has it now (modules/git.nix). 1004 is `kindashared`, above.

  # ── GPU / graphics ────────────────────────────────────────────────────────
  # Enables /dev/dri + Mesa and the VA-API drivers for hardware video decode
  # and encode. intel-media-driver covers Intel iGPUs (typical mini-PC), mesa-va-drivers
  # covers AMD/Radeon — both coexist; drop the one that doesn't match the box.
  hardware.graphics = lib.mkIf config.losos.gpu.enable {
    enable = true;
    # NOT enable32Bit, and it used to be. That option installs a second, 32-bit
    # copy of Mesa and the VA-API drivers into /run/opengl-driver-32, and the
    # only things that can load them are 32-bit userspace binaries: Steam, Wine,
    # old proprietary games. This appliance has no desktop and no 32-bit
    # anything.
    #
    # So the 32-bit half was unreachable by construction, and it is not small:
    # a closure attribution of the install system put the graphics stack at
    # 2.06 GiB over 129 store paths, of which 61 paths and roughly 992 MiB were
    # the 32-bit copy alone. That is a sixth of everything a fresh box has to
    # fetch or build, for a code path nothing on it can enter.
    enable32Bit = false;
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

  # Set on first install; do not change afterwards. Matches the nixos-unstable
  # release this flake tracks.
  system.stateVersion = "26.11";
}
