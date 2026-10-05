# Evaluation-time invariants of the installed system — the things that are
# cheap to break by editing a default and expensive to discover on a box
# with no shell. No VM: this check evaluates the `install` configuration
# and asserts on the merged option values, so `nix flake check --no-build`
# (CI's eval job) fails the moment one of them drifts, at zero build cost.
#
# Each entry here is a finding from the 2026-10-05 critical review that was
# true on main for weeks without a test turning red. They are asserted
# against the *published* flake — the one with no modules/install-target.nix,
# which is what a `github:` upgrade URI evaluates — because that is the shape
# in which the defaults are load-bearing.
{
  pkgs,
  lib,
  config,
}:

let
  must = cond: msg: if cond then true else throw "tests/invariants.nix: ${msg}";

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
# GRUB's limit is only set under `losos.bios`, which the published flake has
# off; asserting it here would read the module's default. tests/install.nix
# boots the BIOS path and is where that half is exercised.

pkgs.runCommand "losos-invariants" { } ''
  echo "install configuration invariants hold" > $out
''
