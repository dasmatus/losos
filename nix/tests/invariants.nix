# Evaluation-time invariants of the installed system — the things that are
# cheap to break by editing a default and expensive to discover on a box
# with no shell. No VM: this check evaluates the `install` configuration
# and asserts on the merged option values, so `nix flake check --no-build`
# (CI's eval job) fails the moment one of them drifts, at zero build cost.
#
# Each entry here is a bug that was once true on main for weeks without a
# test turning red. They are asserted
# against the *published* flake — the one with no modules/install-target.nix
# — because that is the shape in which the defaults are load-bearing: it is
# what a `github:` upgrade URI evaluates on a box whose live files could not
# be read. (With --impure, which the nightly upgrade and lososd pass, the
# box's own install-target.nix and overrides.nix win; this check runs pure,
# so it sees neither.) A developer's checkout that carries an
# install-target.nix of its own would make the unlock-mode assertions read
# that file instead; that file is written by the installer onto the box and
# is not meant to be in a working tree. The keyfile half of the unlock story
# (the secret is baked, the chip is not asked) is nix/tests/install.nix's; the
# TPM half end to end (format, enrol, reboot, unlock from swtpm) is
# nix/tests/tpm.nix's.
{
  pkgs,
  lib,
  config,
  # The install configuration as a box that joined the mesh: the tunnel on,
  # a name of its own, losos.cluster.enable on.
  mesh,
}:

let
  must = cond: msg: if cond then true else throw "nix/tests/invariants.nix: ${msg}";

  frontRoutes = c: builtins.attrNames c.services.nginx.virtualHosts."losos-front".locations;

  gc = config.nix.gc;
  loader = config.boot.loader;
in
assert must gc.automatic
  "nix.gc.automatic is off: the self-upgrading box never frees a generation and /persist (which is /nix) fills until the 03:00 switch fails";
assert must (lib.hasInfix "--delete-older-than" gc.options)
  "nix.gc.options does not bound generation age (${gc.options}); a collector that keeps every generation collects nothing";
assert must
  (loader.systemd-boot.configurationLimit != null && loader.systemd-boot.configurationLimit <= 10)
  "boot.loader.systemd-boot.configurationLimit is unbounded: with linuxPackages_latest every nightly kernel lands in a 500 MiB ESP until the bootloader install fails";
assert must config.losos.tpm.enable
  "losos.tpm.enable is off in the published flake: the installer seals the disk key to the TPM2 wherever a chip exists, so a github: upgrade of such a box would bake a keyfile into the initrd it does not have and fail the bootloader install";
assert must (config.boot.initrd.secrets == { })
  "the TPM path declares an initrd secret (${toString (builtins.attrNames config.boot.initrd.secrets)}): the recovery keyfile exists inside /persist on that path too, and an entry here would put it on the unencrypted ESP";
assert must config.boot.initrd.systemd.tpm2.enable
  "boot.initrd.systemd.tpm2.enable is off: the initrd cannot read the sealed LUKS2 token and every boot stops at a passphrase prompt";
assert must (lib.elem "tpm2-device=auto" config.boot.initrd.luks.devices.persist.crypttabExtraOpts)
  "crypttab for `persist` has no tpm2-device=auto (${toString config.boot.initrd.luks.devices.persist.crypttabExtraOpts}): the enrolled token is never consulted";
assert must (lib.elem "--impure" config.system.autoUpgrade.flags)
  "system.autoUpgrade does not pass --impure (${toString config.system.autoUpgrade.flags}): a remote upgradeFlakeUri then evaluates the published tree without this box's install-target.nix and overrides.nix, flips a keyfile box to the TPM shape and resets every setting";
assert must
  (
    let
      release = lib.removePrefix "v" (import ../version.nix);
      stable = !(lib.hasInfix "-" release) && lib.versionAtLeast release "1.0.0";
    in
    config.losos.updates.channel == (if stable then "weekly" else "daily")
  )
  "losos.updates.channel defaults to ${config.losos.updates.channel}: boxes follow the daily channel before LosOS 1.0.0 and the weekly one from then on";
assert must (config.losos.configRepo.upstream.branch == "channel/${config.losos.updates.channel}")
  "losos.configRepo.upstream.branch is ${config.losos.configRepo.upstream.branch}, not the release channel: the box would follow a branch .github/workflows/channels.yml does not move";
assert must config.losos.configRepo.upstream.enable
  "losos.configRepo.upstream.enable is off: a box never takes a new version";
assert must (lib.hasSuffix "#install" config.losos.upgradeFlakeUri)
  "losos.upgradeFlakeUri has no #install fragment (${config.losos.upgradeFlakeUri}); nixos-rebuild would look for nixosConfigurations.<hostname>, which this flake does not export";
assert must
  (lib.any (lib.hasSuffix "/bin/losos-ctl config --update") (
    lib.toList (config.systemd.services.nixos-upgrade.serviceConfig.ExecStartPre or [ ])
  ))
  "nixos-upgrade.service no longer asks lososd to follow the published tree first (losos-ctl config --update): /etc/nixos then never changes and the nightly update rebuilds the installed version forever";
assert must (!config.losos.bios && config.boot.loader.systemd-boot.enable)
  "the published flake is not on the UEFI/systemd-boot path; the README tells owners to boot the installer in UEFI mode";
assert must
  (lib.elem "lososPersistentSources" config.system.activationScripts.createPersistentStorageDirs.deps)
  "impermanence's createPersistentStorageDirs no longer waits for lososPersistentSources: inside nixos-install's chroot there is no passwd file yet (userborn), so impermanence's chown-by-name fails on every persisted directory";
assert must
  (
    lib.hasInfix "ensure /persist/home/notshared 1000 1000 700" config.system.activationScripts.lososPersistentSources.text
    && lib.hasInfix "ensure /persist/etc/rancher 0 0 0755" config.system.activationScripts.lososPersistentSources.text
  )
  "lososPersistentSources does not pre-create the persisted directories by numeric id (a home as its owner with its homeMode, a plain directory as root 0755)";
assert must
  (
    lib.head config.nix.settings.substituters
    == "http://127.0.0.1:${toString config.losos.cache.local.port}"
  )
  "the box does not substitute through its local copy first (${toString config.nix.settings.substituters}): every nightly rebuild downloads again what the 04:30 clean-up deleted";
assert must
  (config.systemd.services ? losos-nix-cache && config.systemd.services ? losos-ghcr-cache)
  "the local copy (losos-nix-cache) or the GHCR helper behind it (losos-ghcr-cache) is not on the box";
assert must
  (lib.all (u: lib.elem u config.losos.cache.local.upstreams) [
    "https://cache.nixos.org"
    "http://127.0.0.1:8787"
    "https://losos.dasmat.us/proxy"
  ])
  "the local copy does not ask cache.nixos.org, the GHCR helper and the Pages copy (${toString config.losos.cache.local.upstreams}): the LosOS packages and images would be built on the box";
assert must
  (lib.elem "/var" (map (d: d.directory) config.environment.persistence."/persist".directories))
  "/var is not persisted whole: the local copy in /var/cache/losos-nix-cache would vanish at the 00:07 restart";
assert must (lib.any (lib.hasPrefix "losos-1:") config.nix.settings.extra-trusted-public-keys)
  "no losos-1 key in nix.settings.extra-trusted-public-keys: nix declines the cache's signatures and silently builds from source, the slow install the cache exists to avoid";
assert must (lib.elem config.system.build.nixos-rebuild config.systemd.services.lososd.path)
  "nixos-rebuild is not on lososd's unit path: systemd-run resolves the rebuild command against the caller's PATH, so every Apply, storage-mode change and factory reset fails before the rebuild unit exists";
assert must (!(config.systemd.services.lososd.serviceConfig.MemoryDenyWriteExecute or false))
  "lososd runs with MemoryDenyWriteExecute: Wasmtime cannot make its compiled plugins executable, lososd exits at start and restarts forever, and every /api/ request on the installed box answers 502";
assert must ((config.systemd.services.lososd.serviceConfig.ProcSubset or "all") != "pid")
  "lososd runs with ProcSubset=pid: that hides /proc/devices and /proc/mounts, lvm2's vgs exits 4 without them, and every `losos-ctl grow` (the Storage pane's Use reserve) fails at its first step";
assert must (lib.elem "/git/" (frontRoutes config) && config.systemd.services ? losos-git)
  "LosOS Git is not on the box: no /git/ route on the front vhost or no losos-git.service, so the configuration repository has nowhere to go and the nightly update stops following main";
assert must (lib.elem "r /var/lib/losos/k3s-static-pods/forgejo.yaml" config.systemd.tmpfiles.rules)
  "the Forgejo static pod of an older generation is not removed: it keeps port 3000, where LosOS Git listens for lososd";
assert must (mesh.boot.kernel.sysctl."vm.overcommit_memory" == 1)
  "vm.overcommit_memory is not settled on a box that joins the mesh: rke2 and redis both define it, and a clash means a box with Join the mesh on does not build";
assert must (lib.elem "node.longhorn.io/create-default-disk=true" mesh.services.rke2.nodeLabel)
  "a box that joins the mesh is not labelled node.longhorn.io/create-default-disk=true: the edge's Longhorn creates disks only on labelled nodes, so the mesh has no storage and every storage order waits for a volume that never comes";
assert must
  (config.services.journald.settings.Journal.SystemMaxUse == config.losos.logs.journalMaxUse)
  "the journal is not capped at losos.logs.journalMaxUse: journald's own default lets it take up to 4 GiB of /persist, which is also /nix";
assert must
  (lib.all (rule: config.services.logrotate.settings.${rule}.enable or false) [
    "losos-audit"
    "losos-moderation"
    "losos-rebuild"
  ])
  "lososd's audit.log, moderation.log or rebuild.log has no logrotate rule: each is appended to for the life of the box (modules/logs.nix)";
assert must
  (
    config.services.k3s.extraKubeletConfig.containerLogMaxSize or null == "10Mi"
    && mesh.services.rke2.extraKubeletConfig.containerLogMaxSize or null == "10Mi"
    && mesh.services.rke2.extraKubeletConfig.port or null == mesh.losos.cluster.kubeletPort
  )
  "the pod log cap is missing from a cluster, or merging it dropped the mesh kubelet's own port (modules/logs.nix, modules/cluster.nix)";
# GRUB's limit is only set under `losos.bios`, which the published flake has
# off; asserting it here would read the module's default. nix/tests/install.nix
# boots the BIOS path and is where that half is exercised.

pkgs.runCommand "losos-invariants" { } ''
  echo "install configuration invariants hold" > $out
''
