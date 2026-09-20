# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`losos` is a **stateless NixOS appliance** flake: a tmpfs root rebuilt every
boot, with all durable state bind-mounted back from an encrypted `/persist`
via `impermanence`. It runs Nextcloud (for the `notshared` user) and a
contributed mesh-storage domain (for the `shared` user) on repurposed mini-PCs,
with **no SSH and no shell logins** — a set-and-forget box reached only through
service web UIs and a dedicated admin endpoint. See `README.md` for the
install/mesh/auto-upgrade walkthrough; this file covers what the README doesn't: build/dev commands and
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

**There are two CI lanes and both must be kept in step by hand**, with each
other and with `devenv.nix`: `.forgejo/workflows/` (Codeberg) and
`.github/workflows/ci.yml` (GitHub). Same flake, same ordinary gates, plus one
GitHub-only tag-release job that builds the heavy media, uploads them to
Codeberg's generic package registry and updates both release pages. A change to
one is a change to both — the GitHub file's header lists the deliberate
platform differences. `.github/actions/` now carries `setup-nix`,
`cachix-push`, `github-release` and `forgejo-generic-package`; the Forgejo
release API helper lives under `.forgejo/actions/forgejo-release` and is used
from GitHub's tag-release job as well.

CI (Codeberg Actions, `.forgejo/workflows/ci.yml`) runs on hosted runners
capped at **10 minutes / 8 GB per job** (and the RAM quota counts filesystem
writes), so each job is scoped to fit. GitHub's runners cap at 6 hours, so the
GitHub lane carries explicit `timeout-minutes` instead — but it keeps the
cap-driven structure anyway, because the reasons outlive the cap: clippy and
test stay separate jobs (as one they compile the local crates twice), and the
`iso` job still imports `losos-ctl` as an artifact rather than rebuilding it. The **install toplevel** remains a
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

`admin-ui/app` is the shipped SPA and **is** built by nix
(`nix build .#losos-admin-ui` runs `npm ci` then `tsc --noEmit && vite build`,
so the typecheck is part of the package). For an iteration loop it also has
`npm run dev` (with `LOSOS_API_ORIGIN` pointed at a real box), `npm run
typecheck`, and `npm run test:browser` — the last is the playwright check that
`.#checks.x86_64-linux.losos-admin-ui` wraps.

`admin-ui/design-system/react` has its own `npm run typecheck` / `npm test`
(esbuild + tsc, no nix, no CI) — dev-machine-only, run manually from
`admin-ui/design-system/react/` when touching the React wrapper. It is not
what the appliance serves.

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
`boot.nix`): the root is tmpfs; `/persist` is LUKS-encrypted ext4 (with the
`encrypt` feature, because fscrypt needs it and btrfs cannot provide it). Only the
dirs in `environment.persistence."/persist".directories` survive a reboot
(`/nix`, `/var`, `/etc/ssh`, `/etc/keys`, `/etc/nixos`, `/etc/rancher`, the two
data homes, `machine-id`). **Anything new that must persist across reboot must be added
to that list** or it silently vanishes on the next boot. `/persist` is
`neededForBoot` so impermanence bind-mounts resolve before the sysroot is
populated. Unlock is TPM2 (`losos.tpm.enable = true`, default) or a keyfile
at `/etc/keys/persist-keyfile` (no-TPM path, injected into the initrd as
`/crypto_keyfile.bin`).

**Two isolated data domains, no shell** (`configuration.nix`): `notshared`
(uid 1000) owns Nextcloud, `shared` (uid 1001) owns the contributed mesh
storage domain; both homes are
mode `700`, each with its own primary group, so neither can read the other —
`isNormalUser` without an explicit `group` puts both in `users`, and a `750`
home then grants that group r-x, which silently defeated the isolation until
`tests/impermanence.nix` caught it. Neither has a password, and
`services.openssh.enable = false`. The only config change reachable from the
running box is via the standalone admin UI: the **Local ↔ Mesh** storage
toggle, **join the compute mesh**, and **share my compute when I sleep**.

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
no bus needed). The admin UI (`admin-ui/app/`, packaged as `losos-admin-ui`
with `buildNpmPackage`) is a React 19 + Vite + Tailwind v4 SPA on real paths
(`/`, `/apps`, `/storage`, `/mesh`, `/settings/<pane>`), whose built `dist/`
**is** the front Nginx vhost's document root — served with
`try_files $uri $uri/ /index.html`, without which every deep link and every
reload 404s. The vhost proxies `/api/*` to lososd's loopback API. It replaced
a `dashboard/` + `settings/` pair of plain-JS pages; if you find a reference
to those, or to `/ds/` or `/common.js`, it is stale. The command layer is written against a `Losos` effect
**trait** with a real (`io_backend`) and an in-memory (`fake`) implementation,
so the state machine is unit-tested with no filesystem; the installer repeats
the pattern with `Install` + `plan_install`, whose plan is data, so the
ordering of destructive steps is asserted without formatting anything. Wire
contract: `backend/schema.json`. Set `losos.backend.package = null` to run
without it.

**Workloads and the single front door** (`modules/containers.nix` +
`modules/workloads.nix` + `modules/cluster.nix` + `modules/nextcloud-common.nix`):
rootless Podman is gone, and so is systemd-nspawn. Nextcloud and Forgejo run as
**Kubernetes workloads in the box's own local k3s cluster** when their
`losos.<svc>.mode == "container"`; the shared Nextcloud truth still lives in
`nextcloud-common.nix` so host-native and workload mode can't drift. Nginx is
the only thing holding public ports: path-based routing on the appliance's mDNS
name (`<hostName>.local:80/nextcloud`, `:80/forgejo`) and the admin SPA on the
same `:80` vhost. `admin-ui/design-system/` (`tokens.css` + `losos.css`, plus
a `react/` wrapper package for claude.ai/design) is **dev-machine-only and no
longer served at all** — the shipped SPA carries its own Tailwind v4 token
layer in `admin-ui/app/src/styles/index.css`. It stays out of the Nix closure
because `losos-admin-ui`'s source *root* is `admin-ui/app`; that is a stronger
guarantee than the `design-system` name filter it replaced, which would have
stopped matching on a rename. The workload pods are
`hostNetwork` (the local cluster runs no CNI), so they answer on loopback — see
the gotcha about what that costs the `lanOnly` guard.

**The `losos.*` option namespace** (`options.nix`): all project-specific
knobs (`targetDrive`, `tpm.enable`, `sharingMyStorage`, `forgejo.enable`,
`nextcloud.*`, `cluster.*` (mesh join, compute window), `shared.fscrypt.*`,
`upgradeFlakeUri`, `backend.package`,
`admin.{enable,port,apiPort,tokenFile,ui}`, `proxy.*` (appliance side of the
master proxy), `edge.*` (VPS side, consumed via the `nixosModules.edge` flake
output)) live
under `options.losos`, with defaults applied in `defaults.nix`. **Modules
never read bare `config.X`; they read `config.losos.X`.** This is the only
sanctioned place to add new options — earlier code wrongly declared options
inside `config` blocks.

**Hardening** (`hardening.nix`, `install` only): `losos.hardening.*` is built from
KSPP primitives rather than wrapping an upstream profile, because there is none
left to wrap — `profiles/hardened.nix` was removed in 26.05 and
`linux_hardened` is now a `throw`. `hardening.enable` (default true) is the
layer that costs nothing: kernel params, sysctls, a module blacklist that also
blocks explicit `modprobe` (plain `boot.blacklistedKernelModules` does **not** —
it only stops alias autoloading), `boot.tmp.useTmpfs`, dbus-broker, and systemd
sandboxing on `nginx`/`avahi-daemon`/`lososd`. Four opt-in flags — `apparmor`,
`malloc`, `nosmt`, `usbguard` — carry what can break something. Not imported by
`iso`: the live medium needs squashfs. `tests/hardening.nix` asserts both halves,
including that nothing blocks the kernel surface k3s, rke2, containerd and
Longhorn need.

**Online growth** (`grow.rs` + `losos.storage.fillPercent`): the LV deliberately
stops short of the whole volume group, so `/persist` — which *is* `/nix`, via
impermanence — can be grown without opening the box. `losos-ctl grow` runs
lvextend, then `cryptsetup resize`, then `resize2fs`, in that order, online.
The order is the whole correctness argument and the failure is silent, so it is
asserted against the *plan* in `grow.rs` and end-to-end in `tests/resize.nix`.

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

- **Three hardening settings are deliberately *not* applied**, and all three are
  on every checklist you will be tempted to copy from. `rp_filter` is `2`
  (loose), not `1` — strict drops the multicast replies that make an SSH-less
  box reachable, *and* Calico does not work under it. `user.max_user_namespaces`
  stays non-zero — zeroing it stops both kubelets and containerd.
  `/tmp` is not `noexec` — nix builds execute there and the nightly unattended
  rebuild is the only self-repair path. `tests/hardening.nix` asserts all three
  absent, so "fixing" one turns a test red instead of bricking a box.
- **`fileSystems."/tmp"` does not mount anything.** NixOS masks `tmp.mount`
  unless `boot.tmp.useTmpfs` is set, so the declaration silently produces no
  mount and `/tmp` inherits the root filesystem's options. Cost one VM-test
  round trip to find.
- **The allocator preload file is `/etc/ld-nix.so.preload`**, read by NixOS'
  patched loader — not the glibc-standard `/etc/ld.so.preload`, which never
  exists here. Asserting the standard path passes when the allocator is off and
  fails when it is on.
- **`losos-ctl grow` runs three commands and only the order is load-bearing.**
  `resize2fs` asks the LUKS *mapping* for its size, so running it before
  `cryptsetup resize` reads the pre-grow size, prints "Nothing to do!" and
  exits 0. And `lvextend -l 77` without the `+` is an absolute extent count,
  which *shrinks* a larger volume — under mounted ext4 that destroys it.
  `lososd` needs lvm2/cryptsetup/e2fsprogs on its unit `path` or the first
  step fails with "No such file or directory". The same applies to `curl`,
  which `GET /api/apps/search` shells out to: `path` **replaces** PATH, so a
  binary left off that list is not on it by accident, and the failure surfaces
  to the owner as a feature that quietly never works.
- **The LUKS key file must not live under `/root` or `/home`.** `lososd` runs
  with `ProtectHome=true` — on purpose, so a compromised request handler cannot
  read either data domain — and that hides `/root` too. `cryptsetup resize`
  then fails with "Failed to open key file", *after* `lvextend` has already
  grown the volume. `/etc/keys/persist-keyfile` is the path everything agrees
  on: `disko.nix` writes it, `daemon.nix` passes it as `LOSOS_LUKS_KEYFILE`
  (no-TPM path only), and `tests/resize.nix` uses it rather than a fixture of
  its own, so the three cannot drift apart unnoticed.

- **`lososd` is restarted mid-rebuild.** `nixos-rebuild switch` restarts the
  changed `lososd.service`, killing the watcher thread that was tracking the
  rebuild. `Daemon.startSupervisor` re-attaches on daemon startup: if
  `state.json` says `Building`, it spawns a fresh watcher for
  `losos-rebuild-<job>`. Without this the rebuild records `building`
  forever. Don't drop the re-attach.
- **The admin routes deny loopback on purpose** (`lanOnly` in
  `containers.nix`): master-proxy tunnel traffic reaches the front vhost *from
  127.0.0.1* (rathole's `local_addr`), so the LAN-only guard on `/`,
  `/assets/`, `/setup/` and `/api/` must not allow loopback — a
  `curl localhost/` on the box (or in a VM test) gets 403 by design. "Fixing"
  it with `allow 127.0.0.1` exposes the whole admin surface to the internet
  whenever `losos.proxy.enable` is on. Test against lososd's :8082 directly,
  or curl with a LAN source address.
- **The admin token is created by lososd, not by NixOS.** `losos.admin.tokenFile`
  (default `/var/secrets/losos-admin-token`, persisted via `/var`) is written
  with a 64-hex-char random value (mode 0600) by lososd on first start if
  absent. NixOS doesn't manage it — don't try to declare it as a store path.
- **`/etc/rancher` must stay persisted** (`impermanence.nix`). `/var` covers
  the bulk of both Kubernetes instances' state, but the agent writes
  `/etc/rancher/node/password` on its first join and the server stores a hash of
  it keyed by node name. On a tmpfs root that file is regenerated every boot and
  the server then refuses the rejoin ("Node password rejected"). Drop the line
  and the box silently falls out of the mesh on the first reboot after enrolling.
- **`--disable`, `--flannel-backend` and `--disable-network-policy` are
  server-only flags.** `k3s agent` hard-errors on an unknown flag, so passing any
  of them to an agent crash-loops the unit forever — on a box with no shell.
  nixpkgs' own `nixos/tests/rancher/multi-node.nix` gives its server nodes
  `disable` and its agent node neither; rke2's `role` description says the same.
  Gate every server-only flag on the role.
- **The two Kubernetes instances are different clusters on purpose.**
  `services.k3s` (role `server`) runs *this box's* Nextcloud and Forgejo;
  `services.rke2` (role `agent`) joins the edge's mesh. An agent's kubelet cannot
  start while its server is unreachable, and `midnight-reboot.timer` fires
  unconditionally at 00:07 — so putting the box's own services in the edge's
  cluster would take them down for any outage spanning midnight. Don't "simplify"
  this into one cluster. rke2 rather than a second k3s because nixpkgs builds both
  from one name-parameterized generator, so their state dirs
  (`/var/lib/rancher/{k3s,rke2}`) and unit names don't collide — and there is no
  `dataDir` option to make a second k3s work.
- **`hostNetwork` pods have no distinguishable source address.** The local
  cluster runs `--flannel-backend=none`, so its pods share the host's netns and
  nginx sees them as `127.0.0.1` or the LAN IP. The old nspawn design relied on
  containers having their own subnet, and the `lanOnly` guard denied it first.
  That depth is gone: never write a `deny <podCidr>` rule, because it can't match.
- **The compute window is enforced on the edge, in the appliance's zone.** The
  NoSchedule taint can only be written by the edge (NodeRestriction lets no one
  else), so the comparison happens on a machine that is not the owner's — an
  edge VPS running UTC against an appliance shipping `Europe/Berlin`. The zone
  therefore travels with the two bounds (`--window-tz`, `ComputeWindow.tz`) and
  the edge evaluates each node with `TZ="$tz" date`. Before that it read its own
  clock, and a 23:00–07:00 window entered in Berlin was enforced 00:00–08:00 in
  winter and 01:00–09:00 in summer, sliding an hour at each DST change — which
  handed strangers' pods the first hours of the owner's working day, the exact
  thing the feature exists to prevent. Don't hoist `now` back out of the
  per-node loop in `modules/edge.nix`: it is per-node because the zone is.
- **`system.stateVersion = "26.11"` is set-once** — matches the nixos-unstable
  this flake tracks; don't change it.
- **The `result` symlink is a `nix build` artifact** (pointing into
  `/nix/store`), gitignored, never committed.