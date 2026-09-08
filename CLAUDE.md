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
# Build the flake outputs (Rust daemon+facade, static admin UI) and the target system closure
nix build .#losos-ctl .#losos-admin-ui
nix build .#nixosConfigurations.install.config.system.build.toplevel

# Build the installer ISO (write to USB, boot on the target machine)
nix build .#nixosConfigurations.iso.config.system.build.isoImage

# Dev environment: devenv (devenv.nix + devenv.yaml), NOT the flake devShell.
# Every gate is a named script there — fmt, lint, test-rust, check-flake,
# check-eval, check-pins, build-pkgs, build-iso, vm-tests. CI does NOT invoke
# them — it spells the same commands out directly, because routing jobs
# through `devenv shell` blew Codeberg's 10-minute cap (9m31s and 10m05s
# against 1m32s for the one non-devenv job). Keep the two in step by hand.
# Switches to fish on interactive entry; the `case $- in *i*)` guard keeps
# `devenv shell <script>` and CI running under bash.
devenv shell        # or: direnv allow
devenv test         # everything except the VM tests and the ISO
```

`nix develop` still resolves, but it is a **toolchain-only** shell for anyone
without the devenv CLI — deliberately no scripts and no hooks, so it cannot
drift from devenv.nix.

devenv runs standalone rather than through the flake: `devenv.lib.mkShell`
cannot evaluate purely (it needs an absolute project root for `.devenv/`), and
the documented workaround needs `--impure`, which would spread to CI. The cost
is two lock files — `flake.lock` pins the nixpkgs that *builds* the appliance,
`devenv.lock` the one that *lints and tests* it. **Bump them together**;
`check-pins` fails the build if they disagree.

Both Rust crates set `doCheck = false`, so `nix build` compiles the shipping
binaries and does not run the suites. The suites run via `cargo test` — in
`devenv test`/`test-rust` locally and in CI's lint job. This is a concession
to Codeberg's 10-minute cap: doCheck recompiles each crate in test
configuration and links a binary per test target, which put
`nix build .#losos-ctl` between 7.7 and 10.3 minutes across observed runs and
cancelled real runs on unmodified main. **A bare `nix build` is therefore not
a test gate** — run `devenv test` before trusting a change. There is no PHP
suite anymore — the old Nextcloud plugin is retired (see architecture) — and no
Haskell: the backend was a cabal project until it was ported to Rust.

Both crates must stay clippy-clean (`-D warnings`) and rustfmt-clean. That is
enforced in three places now: the devenv pre-commit hooks, the `lint` script,
and CI. Until those existed nothing ran the linters at all — back when
`doCheck` was on it ran the test suite, never clippy — and backend-registrar
had drifted to 17 rustfmt hunks plus a clippy error without CI noticing.

```sh
# `nix develop -c` does not change directory, hence --manifest-path
cargo test --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```

CI (Codeberg Actions, `.forgejo/workflows/ci.yml`) runs on hosted runners
capped at **10 minutes / 8 GB per job** (and the RAM quota counts filesystem
writes), so each job is scoped to fit. The **install toplevel** remains a
local-only gate. The **installer ISO is now built in CI**: its closure is 769
paths, of which 766 are stock nixpkgs served by cache.nixos.org and only
`losos-ctl` is expensive, so the `iso` job imports that one from the
`losos-ctl` job as a 12 MiB artifact instead of recompiling it. Measured:
~4.7 GiB written against a 10 GiB quota, 50 s for squashfs + xorriso.

The VM tests are the real acceptance gate for the control plane and are *not*
run by CI, so run them locally when touching either:

```sh
nix build .#checks.x86_64-linux.losos-admin-daemon   # lososd: D-Bus + HTTP + token
nix build .#checks.x86_64-linux.losos-install        # installer: detect + disko + LVM
```

Inside the dev shell, `bcd` → `cd backend`, `rcd` → `cd backend-registrar`.
`admin-ui/design-system/react` has its own `npm run typecheck` / `npm test`
(esbuild + tsc, no nix, no CI) — dev-machine-only, run manually from
`admin-ui/design-system/react/` when touching the React wrapper.

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
mode `700`, each with its own primary group, so neither can read the other —
`isNormalUser` without an explicit `group` puts both in `users`, and a `750`
home then grants that group r-x, which silently defeated the isolation until
`tests/impermanence.nix` caught it. Neither has a password, and
`services.openssh.enable = false`. The only config change reachable from the
running box is the **Local ↔ Mesh toggle** in the standalone admin UI.

**The lososd daemon, losos-ctl facade, and admin endpoint**
(`modules/daemon.nix` + `backend/` + `admin-ui/`): the privileged logic lives
in a root systemd daemon `lososd` (Rust, `backend/`, one crate with two
binaries; zbus for D-Bus, actix-web for HTTP, clap for the CLI). It runs the
bus listener and the rebuild supervisor on a tokio runtime and the HTTP server
on its own thread under an actix `System` — the two runtimes are deliberately
not shared. It owns `/var/lib/losos/state.json` (sole writer,
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
**trait** with a real (`io_backend`) and an in-memory (`fake`) implementation,
so the state machine is unit-tested with no filesystem; the installer repeats
the pattern with `Install` + `plan_install`, whose plan is data, so the
ordering of destructive steps is asserted without formatting anything. Wire
contract: `backend/schema.json`. Set `losos.backend.package = null` to run
without it.

**Containers and the single front door** (`modules/containers.nix` +
`modules/services.nix` + `modules/nextcloud-common.nix`): rootless Podman is
gone. Nextcloud and Forgejo run as declarative **NixOS Containers**
(systemd-nspawn) on private subnets (`10.231.1.2` / `10.231.2.2`) when their
`losos.<svc>.mode == "container"`; the shared native Nextcloud stack lives in
`nextcloud-common.nix` (`lososInternal.nextcloudStack`) so host-native and
container mode can't drift. Nginx is the only thing holding public ports:
path-based routing on the appliance's mDNS name (`<hostName>.local:80/nextcloud`,
`:80/forgejo`), the admin SPA on the same `:80` vhost, the Tahoe web UI proxied on `:3456`
(port-based — Tahoe generates absolute links). Shared stylesheets live in
`admin-ui/design-system/` (`tokens.css` + `losos.css`, served at `/ds/`);
`admin-ui/design-system/react` is a dev-machine-only React wrapper package for
claude.ai/design — never part of the Nix closure (the `losos-admin-ui` package
filters `design-system/` out of its `admin-ui/` copy and ships only the two
stylesheets under `/ds/`). Each container
backend answers only on its private IP or loopback.

**The `losos.*` option namespace** (`options.nix`): all project-specific
knobs (`targetDrive`, `tpm.enable`, `sharingMyStorage`, `forgejo.enable`,
`nextcloud.*`, `tahoe.introducerFurl`, `upgradeFlakeUri`, `backend.package`,
`admin.{enable,port,apiPort,tokenFile,ui}`, `proxy.*` (appliance side of the
master proxy), `edge.*` (VPS side, consumed via the `nixosModules.edge` flake
output)) live
under `options.losos`, with defaults applied in `defaults.nix`. **Modules
never read bare `config.X`; they read `config.losos.X`.** This is the only
sanctioned place to add new options — earlier code wrongly declared options
inside `config` blocks.

**Auto-upgrade + nightly reboot** (`updates.nix`): `system.autoUpgrade`
rebuilds from `losos.upgradeFlakeUri` at 03:00. The default,
`git+file:///etc/nixos#install`, only advances the system consistently and
does not pull new nixpkgs; set a `github:` URI to actually upgrade.

**The `#install` fragment is load-bearing.** Without it `nixos-rebuild`
resolves `nixosConfigurations.$(hostname)`, which this flake does not export
— it exports `iso` and `install` — so every nightly run died with "flake does
not provide attribute", silently, on a box with no shell to notice it from.
The daemon had it right all along (`io_backend.rs`, `/etc/nixos#install`);
only the NixOS-side default was wrong.

A separate `midnight-reboot.timer` reboots unconditionally at 00:07 with
`Persistent=true` to catch up if the box was off.

## Gotchas that bite silently

- **`lososd` is restarted mid-rebuild.** `nixos-rebuild switch` restarts the
  changed `lososd.service`, killing the watcher thread that was tracking the
  rebuild. `Daemon.startSupervisor` re-attaches on daemon startup: if
  `state.json` says `Building`, it spawns a fresh watcher for
  `losos-rebuild-<job>`. Without this the rebuild records `building`
  forever. Don't drop the re-attach.
- **The admin routes deny loopback on purpose** (`lanOnly` in
  `containers.nix`): master-proxy tunnel traffic reaches the front vhost *from
  127.0.0.1* (rathole's `local_addr`), so the LAN-only guard on `/`,
  `/settings`, `/ds`, `/common.js` and `/api` must not allow loopback — a
  `curl localhost/` on the box (or in a VM test) gets 403 by design. "Fixing"
  it with `allow 127.0.0.1` exposes the whole admin surface to the internet
  whenever `losos.proxy.enable` is on. Test against lososd's :8082 directly,
  or curl with a LAN source address.
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