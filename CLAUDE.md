# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`losos` is a **stateless NixOS appliance** flake: a tmpfs root rebuilt every
boot, with all durable state bind-mounted back from an encrypted `/persist`
via `impermanence`. It runs Nextcloud (for the `notshared` user) and Tahoe-LAFS
(for the `shared` user) on repurposed mini-PCs, with **no SSH and no shell
logins** — a set-and-forget box reached only through service web UIs and a
dedicated admin endpoint. See `README.md` for the install/Tahoe/auto-upgrade
walkthrough; this file covers what the README doesn't: build/dev commands and
the cross-file architecture.

## Build & develop

```sh
# Build the flake outputs (Haskell daemon+facade, static admin UI) and the target system closure
nix build .#losos-ctl .#losos-admin-ui
nix build .#nixosConfigurations.install.config.system.build.toplevel

# Build the installer ISO (write to USB, boot on the target machine)
nix build .#nixosConfigurations.iso.config.system.build.isoImage

# Dev shell: full Haskell stdlib (ghcWithPackages). Switches to fish on
# interactive entry; honors `nix develop -c <cmd>` for bash.
nix develop .#
```

The backend's HUnit spec runs automatically inside `nix build .#losos-ctl`
via `callCabal2nix`'s `doCheck = true`. There is no PHP suite anymore — the
old Nextcloud plugin is retired (see architecture).

```sh
cd backend && cabal test        # Haskell suite (or `nix build .#losos-ctl`)
```

Inside the dev shell, `bcd` → `cd backend`.

## Architecture (cross-file big picture)

**Two NixOS systems share one module set** (`flake.nix`):
- `iso` — a minimal live ISO that carries the `disko` layout so it can format
  the target disk and enroll encryption keys. Imports only `options.nix` +
  `disko.nix` + `installer.nix`.
- `install` — the target system installed on disk. Imports everything:
  `options`, `configuration`, `impermanence`, `disko`, `boot`, `services`,
  `nextcloud-common`, `containers`, `daemon`, `overrides`, `updates`,
  `defaults`. The flake passes `specialArgs.self = self` so `defaults.nix`
  can reach `self.packages.${system}.{losos-ctl,losos-admin-ui}` to wire the
  control plane and the admin UI.

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
running box is the **Local ↔ Mesh toggle** in the standalone admin UI.

**The lososd daemon, losos-ctl facade, and admin endpoint**
(`modules/daemon.nix` + `backend/` + `admin-ui/`): the privileged logic lives
in a root systemd daemon `lososd` (Haskell, `backend/`, one cabal package
with two executables). It owns `/var/lib/losos/state.json` (sole writer,
atomic temp+rename), exports a method per subcommand on the **system D-Bus**
(bus `org.losos1`, path `/org/losos1`, interface `org.losos.Control1`), and
serves a **Bearer-authed loopback JSON HTTP API** on `127.0.0.1:${losos.admin.apiPort}`
(default 8082). Rebuilds run as `systemd-run` transient units
(`losos-rebuild-<job>`); a daemon watcher thread polls the unit and records
done/failed, and re-attaches after a daemon restart (which happens because
`nixos-rebuild switch` restarts lososd itself mid-rebuild). `losos-ctl` is a
thin **facade** CLI that keeps the old subcommand/JSON surface
(`state`, `change --mode`, `status`, `settings`, `apply` (stdin),
`factory-reset`; `--json` no-op) and relays each call to lososd over D-Bus.
`losos-ctl install` stays a local subcommand (the ISO installer runs as root,
no bus needed). The standalone admin UI (`admin-ui/`, packaged as
`losos-admin-ui`) is a dependency-free static SPA — `dashboard/` + `settings/`
plain-JS pages — served by the front Nginx vhost, which proxies `/api/*` to
lososd's loopback API. The command layer is written against a `Losos` effect
type class with an `IO` and a pure `TestM` interpreter, so the state machine
is unit-tested with no filesystem. Wire contract: `backend/schema.json`.
Set `losos.backend.package = null` to run without it.

**Containers and the single front door** (`modules/containers.nix` +
`modules/services.nix` + `modules/nextcloud-common.nix`): rootless Podman is
gone. Nextcloud and Forgejo run as declarative **NixOS Containers**
(systemd-nspawn) on private subnets (`10.231.1.2` / `10.231.2.2`) when their
`losos.<svc>.mode == "container"`; the shared native Nextcloud stack lives in
`nextcloud-common.nix` (`lososInternal.nextcloudStack`) so host-native and
container mode can't drift. Nginx is the only thing holding public ports:
path-based routing on the appliance's mDNS name (`<hostName>.local:80/nextcloud`,
`:80/forgejo`), the admin SPA on `:8081`, the Tahoe web UI proxied on `:3456`
(port-based — Tahoe generates absolute links). Each container backend answers
only on its private IP or loopback.

**The `losos.*` option namespace** (`options.nix`): all project-specific
knobs (`targetDrive`, `tpm.enable`, `sharingMyStorage`, `forgejo.enable`,
`nextcloud.*`, `tahoe.introducerFurl`, `upgradeFlakeUri`, `backend.package`,
`admin.{enable,port,apiPort,tokenFile,ui}`, `cfd.{enable,tunnels}`) live
under `options.losos`, with defaults applied in `defaults.nix`. **Modules
never read bare `config.X`; they read `config.losos.X`.** This is the only
sanctioned place to add new options — earlier code wrongly declared options
inside `config` blocks.

**Auto-upgrade + nightly reboot** (`updates.nix`): `system.autoUpgrade`
rebuilds from `losos.upgradeFlakeUri` at 03:00 (default `git+file:///etc/nixos`
— only advances the system consistently, doesn't pull new nixpkgs; set a
`github:` URI to actually upgrade). A separate `midnight-reboot.timer` reboots
unconditionally at 00:07 with `Persistent=true` to catch up if the box was off.

## Gotchas that bite silently

- **`lososd` is restarted mid-rebuild.** `nixos-rebuild switch` restarts the
  changed `lososd.service`, killing the watcher thread that was tracking the
  rebuild. `Daemon.startSupervisor` re-attaches on daemon startup: if
  `state.json` says `Building`, it spawns a fresh watcher for
  `losos-rebuild-<job>`. Without this the rebuild records `building`
  forever. Don't drop the re-attach.
- **The admin token is created by lososd, not by NixOS.** `losos.admin.tokenFile`
  (default `/var/secrets/losos-admin-token`, persisted via `/var`) is written
  with a 64-hex-char random value (mode 0600) by lososd on first start if
  absent. NixOS doesn't manage it — don't try to declare it as a store path.
- **tahoe-lafs is overlaid onto Python 3.12** (`configuration.nix`). nixos-unstable
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