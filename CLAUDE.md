# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`losos` is a **stateless NixOS appliance** flake: a tmpfs root rebuilt every
boot, with all durable state bind-mounted back from an encrypted `/persist`
via `impermanence`. It runs Nextcloud (for the `notshared` user) and Tahoe-LAFS
(for the `shared` user) on repurposed mini-PCs, with **no SSH and no shell
logins** — a set-and-forget box reached only through service web UIs. See
`README.md` for the install/Tahoe/auto-upgrade walkthrough; this file covers
what the README doesn't: build/dev commands and the cross-file architecture.

## Build & develop

```sh
# Build the two flake outputs (backend + app) and the target system closure
nix build .#losos-ctl .#losos-app
nix build .#nixosConfigurations.install.config.system.build.toplevel

# Build the installer ISO (write to USB, boot on the target machine)
nix build .#nixosConfigurations.iso.config.system.build.isoImage

# Dev shell: full Haskell stdlib (ghcWithPackages) + PHP 8.3 + occ.
# Switches to fish on interactive entry; honors `nix develop -c <cmd>` for bash.
nix develop .#
```

There is **no test runner that runs from the host without the dev shell**. The
backend's HUnit spec runs automatically inside `nix build .#losos-ctl` via
`callCabal2nix`'s `doCheck = true`. The PHP suite needs `composer install`
(which fetches PHPUnit), so it only runs from inside `nix develop`:

```sh
nix develop .# -c bash -c 'cd nextcloud-app && composer install --quiet && composer test'   # PHP unit suite
nix develop .# -c bash -c 'cd nextcloud-app && composer lint'                               # php-cs-fixer dry-run
cd backend && cabal test                                                                     # Haskell suite (or `nix build .#losos-ctl`)
```

Inside the dev shell, `ncd` → `cd nextcloud-app`, `bcd` → `cd backend`,
`losos-test` / `losos-lint` run the PHP suite/lint. `losos-occ` runs `occ`
against a Nextcloud install pointed at by `$NEXTCLOUD_ROOT`.

## Architecture (cross-file big picture)

**Two NixOS systems share one module set** (`flake.nix`):
- `iso` — a minimal live ISO that carries the `disko` layout so it can format
  the target disk and enroll encryption keys. Imports only `options.nix` +
  `disko.nix`.
- `install` — the target system installed on disk. Imports everything:
  `options`, `configuration`, `impermanence`, `disko`, `boot`, `services`,
  `updates`, `defaults`. The flake passes `specialArgs.self = self` so
  `services.nix` can reach `self.packages.${system}.losos-app` to install the
  in-tree Nextcloud plugin.

**Stateless-by-impermanence model** (`impermanence.nix` + `disko.nix` +
`boot.nix`): the root is tmpfs; `/persist` is LUKS-encrypted btrfs. Only the
dirs in `environment.persistence."/persist".directories` survive a reboot
(`/nix`, `/var`, `/etc/ssh`, `/etc/keys`, `/etc/nixos`, the two data homes,
`machine-id`). **Anything new that must persist across reboot must be added
to that list** or it silently vanishes on the next boot. `/persist` is
`neededForBoot` so impermanence bind-mounts resolve before the sysroot is
populated. Unlock is TPM2 (`losos.tpm.enable = true`, default) or a keyfile
at `/etc/keys/persist-keyfile` (no-TPM path, injected into the initrd as
`/crypto_keyfile.bin`).

**Two isolated data domains, no shell** (`configuration.nix`): `notshared`
(uid 1000) owns Nextcloud, `shared` (uid 1001) owns Tahoe-LAFS; both homes are
mode `750` so neither can read the other, neither has a password, and
`services.openssh.enable = false`. The only config change reachable from the
running box is the **Local ↔ Mesh toggle** in the Nextcloud admin panel.

**The losos web-UI ↔ privileged-backend bridge** (`services.nix` +
`flake/packages.nix`): the `losos` Nextcloud app (PHP, `nextcloud-app/`,
packaged as `losos-app`) is symlinked into Nextcloud via
`services.nextcloud.extraApps`. Its controller talks over a **sudoers rule**
to `losos-ctl` (Haskell, `backend/`, packaged via `callCabal2nix` as
`losos-ctl`): user `nextcloud` → root, `NOPASSWD`, command pinned to exactly
the `losos-ctl` binary. So PHP-FPM triggers a `nixos-rebuild` without ever
holding root or a shell. `losos-ctl` rewrites the `losos.sharingMyStorage`
line in the persisted flake, spawns a detached rebuild (`setsid -f`), and
writes progress to a state file the panel polls. Its logic is written against
a `Losos` effect type class with an `IO` and a pure `TestM` interpreter, so
the state machine is unit-tested with no filesystem. Wire contract:
`backend/schema.json`. Set `losos.backend.package = null` to run without it
(the panel then reports "backend not installed").

**The `losos.*` option namespace** (`options.nix`): all project-specific
knobs (`targetDrive`, `tpm.enable`, `sharingMyStorage`, `forgejo.enable`,
`nextcloud.*`, `tahoe.introducerFurl`, `upgradeFlakeUri`,
`backend.package`) live under `options.losos`, with defaults applied in
`defaults.nix`. **Modules never read bare `config.X`; they read
`config.losos.X`.** This is the only sanctioned place to add new options —
earlier code wrongly declared options inside `config` blocks.

**Auto-upgrade + nightly reboot** (`updates.nix`): `system.autoUpgrade`
rebuilds from `losos.upgradeFlakeUri` at 03:00 (default `git+file:///etc/nixos`
— only advances the system consistently, doesn't pull new nixpkgs; set a
`github:` URI to actually upgrade). A separate `midnight-reboot.timer` reboots
unconditionally at 00:07 with `Persistent=true` to catch up if the box was off.

## Gotchas that bite silently

- **`losos-ctl` sudoers rules MUST use attrsets with `options = ["NOPASSWD"]`**,
  never a bare path string. The sudo module coerces a bare string to
  `{options=[];}`, which requires a password — and `nextcloud` is a
  passwordless system user invoked via `sudo -n`, so the whole bridge fails
  silently. See the comment block in `services.nix`.
- **`tahoe-lafs` is overlaid onto Python 3.12** (`configuration.nix`). nixos-unstable
  defaults to Python 3.14, under which `txi2p-tahoe` fails to build. The tahoe
  module picks the package up via `services.tahoe.*.package = pkgs.tahoe-lafs`.
  Don't drop this overlay. (Set module `package = pkgs.tahoe-lafs` explicitly —
  nixpkgs renamed `tahoelafs` → `tahoe-lafs` and the module default still points
  at the old attr.)
- **Tahoe user/group assertion fix** (`configuration.nix`): the tahoe module
  creates `tahoe.<node>` / `tahoe.introducer-<name>` users without a group,
  tripping nixpkgs' "user without group" assertion. Explicit
  `users.groups."tahoe-*"` + `users.users."tahoe.*".group` are required.
- **`system.stateVersion = "26.11"` is set-once** — matches the nixos-unstable
  this flake tracks; don't change it.
- **The `result` symlink is a `nix build` artifact** (pointing into
  `/nix/store`), gitignored, never committed.