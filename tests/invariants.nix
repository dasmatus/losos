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
# (the secret is baked, the chip is not asked) is tests/install.nix's; the
# TPM half end to end (format, enrol, reboot, unlock from swtpm) is
# tests/tpm.nix's.
{
  pkgs,
  lib,
  config,
  # on -> the install configuration with losos.sharingMyStorage forced to `on`.
  sharing,
}:

let
  must = cond: msg: if cond then true else throw "tests/invariants.nix: ${msg}";

  # Federation runs only while the box shares its disk (modules/options.nix,
  # lososInternal.federation). Asserted on both sides, with the per-service
  # switches at their defaults (on), because each half fails quietly: a gate
  # that never opens looks like a server nobody federates with, and one that
  # never closes publishes the box to every server that asks.
  shared = sharing true;
  unshared = sharing false;
  frontRoutes = c: builtins.attrNames c.services.nginx.virtualHosts."losos-front".locations;
  ncRefused = c: builtins.filter (lib.hasInfix "ocm-provider") (frontRoutes c);

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
assert must (lib.hasSuffix "#install" config.losos.upgradeFlakeUri)
  "losos.upgradeFlakeUri has no #install fragment (${config.losos.upgradeFlakeUri}); nixos-rebuild would look for nixosConfigurations.<hostname>, which this flake does not export";
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
assert must (lib.elem "https://proxy.losos.dasmat.us" config.nix.settings.extra-substituters)
  "the LosOS cache proxy is not among nix.settings.extra-substituters (${toString config.nix.settings.extra-substituters}): a fresh install then builds the 2.3 GiB Nextcloud image on the box instead of downloading it";
assert must (lib.elem "https://losos.dasmat.us/proxy" config.nix.settings.extra-substituters)
  "the GitHub Pages copy of the cache is not among nix.settings.extra-substituters (${toString config.nix.settings.extra-substituters}): with the proxy down, the box then builds LosOS's own packages itself";
assert must (lib.any (lib.hasPrefix "losos-1:") config.nix.settings.extra-trusted-public-keys)
  "no losos-1 key in nix.settings.extra-trusted-public-keys: nix declines the cache's signatures and silently builds from source, the slow install the cache exists to avoid";
assert must (lib.elem config.system.build.nixos-rebuild config.systemd.services.lososd.path)
  "nixos-rebuild is not on lososd's unit path: systemd-run resolves the rebuild command against the caller's PATH, so every Apply, storage-mode change and factory reset fails before the rebuild unit exists";
assert must ((config.systemd.services.lososd.serviceConfig.ProcSubset or "all") != "pid")
  "lososd runs with ProcSubset=pid: that hides /proc/devices and /proc/mounts, lvm2's vgs exits 4 without them, and every `losos-ctl grow` (the Storage pane's Use reserve) fails at its first step";
assert must (shared.lososInternal.federation.forgejo && shared.lososInternal.federation.nextcloud)
  "with losos.sharingMyStorage on and both federation switches at their defaults, lososInternal.federation is not on for both services";
assert must (lib.elem "= /.well-known/nodeinfo" (frontRoutes shared) && ncRefused shared == [ ])
  "with disk sharing on, the front vhost lacks LosOS Git's /.well-known/nodeinfo route or still refuses LosOS cloud's federation addresses";
assert must
  (!unshared.lososInternal.federation.forgejo && !unshared.lososInternal.federation.nextcloud)
  "with losos.sharingMyStorage off, lososInternal.federation still says a service federates: the shared data pool is locked and nothing may talk to other servers";
assert must
  (!(lib.elem "= /.well-known/nodeinfo" (frontRoutes unshared)) && ncRefused unshared != [ ])
  "with disk sharing off, the front vhost still routes /.well-known/nodeinfo to LosOS Git or no longer refuses LosOS cloud's federation addresses";
# GRUB's limit is only set under `losos.bios`, which the published flake has
# off; asserting it here would read the module's default. tests/install.nix
# boots the BIOS path and is where that half is exercised.

pkgs.runCommand "losos-invariants" { } ''
  echo "install configuration invariants hold" > $out
''
