# Base system configuration: networking, time/locale, packages, users.
# Boot, disk, and services live in their own modules.
{
  pkgs,
  lib,
  config,
  ...
}:

{
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
  # (nextcloud, the cluster tooling) are pulled in by their modules.
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

  # `shared` — contributes this box's storage to the mesh. Its home holds the
  # fscrypt-protected data domain (modules/fscrypt.nix); no Linux login, and
  # no web UI of its own any more now that the Tahoe WUI is gone.
  #
  # The uid, the gid and the 700 home are the isolation model and are asserted
  # by tests/impermanence.nix in a booted VM — the account outlives the service
  # that used to own it, so do not renumber or repurpose it.
  users.users."shared" = {
    uid = 1001;
    description = "Shared — mesh storage contributor";
    isNormalUser = true;
    group = "shared";
    homeMode = "700"; # owner only; notshared cannot enter, let alone read
  };

  # ── Service accounts for the workload pods ────────────────────────────────
  # Rootless Podman is gone, and so is systemd-nspawn. The services run as pods
  # in this box's own local k3s cluster (modules/workloads.nix), and because
  # that cluster runs no CNI the pods are `hostNetwork` — they share the host's
  # network *and* user namespace. So `runAsUser` in a pod spec is a real uid on
  # this machine, not a container-local fiction, and these two accounts have to
  # exist for reasons that are not cosmetic:
  #
  #   * PostgreSQL peer auth over /run/postgresql compares the *kernel's* uid of
  #     the connecting process and maps it to a role through getpwuid. With no
  #     passwd entry for 1002 the lookup fails, so the Nextcloud pod cannot
  #     authenticate even once the database exists — and it fails as a pod
  #     crash-loop on a box with no shell, not as a rebuild error.
  #   * The tmpfiles rules in modules/workloads.nix chown the state directories
  #     to these ids. Without accounts the directories end up owned by bare
  #     numbers, which works but makes the box unreadable to anyone debugging it.
  #
  # modules/workloads.nix hardcodes the same two numbers and says so in its own
  # comment; keep the pair in step. They are pinned rather than allocated
  # because a dynamically assigned uid would differ per box and could not be
  # written into a pod spec at eval time.
  users.groups.nextcloud.gid = 1002;
  users.users.nextcloud = {
    uid = 1002;
    group = "nextcloud";
    isSystemUser = true;
    description = "Nextcloud workload";
    # Redis' socket is 0660 root:redis-nextcloud. A pod does not inherit the
    # host user's supplementary groups — only what its spec asks for via
    # supplementalGroups — so this membership is what makes the *native* path
    # work; the pod path gets there through the fixed gid below.
    extraGroups = [ "redis-nextcloud" ];
  };

  users.groups.forgejo.gid = 1003;
  users.users.forgejo = {
    uid = 1003;
    group = "forgejo";
    isSystemUser = true;
    description = "Forgejo workload";
  };

  # The Redis group needs a *fixed* gid, and this is the only place it can get
  # one. modules/workloads.nix reads it back out of
  # `config.users.groups."redis-nextcloud".gid` to build the pod's
  # supplementalGroups; left dynamic that read yields null, the group is
  # dropped from the spec, and Nextcloud fails to reach the cache with EACCES on
  # /run/redis-nextcloud/redis.sock. The alternative — widening unixSocketPerm —
  # would open the cache to every uid on the box, which is worse.
  users.groups."redis-nextcloud".gid = 1004;

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

  # Set on first install; do not change afterwards. Matches the nixos-unstable
  # release this flake tracks.
  system.stateVersion = "26.11";
}
