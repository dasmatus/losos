# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`losos` is a **stateless NixOS appliance** flake: a tmpfs root rebuilt every
boot, with all durable state bind-mounted back from an encrypted `/persist`
via `impermanence`. It runs Nextcloud (for the `notshared` user) and a
contributed mesh-storage domain (for the `shared` user) on repurposed mini-PCs,
with **no SSH and no shell logins** — a set-and-forget box reached only through
service web UIs and a dedicated admin endpoint. User documentation lives in
`wiki/`, which `.github/workflows/wiki.yml` publishes to the GitHub wiki on
push to `main` (edit `wiki/`, not the web wiki — it is overwritten). The
README is a short overview that links there. This file covers build/dev
commands and the cross-file architecture.

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
# through `devenv shell` blew the old Codeberg CI's 10-minute cap (9m31s and
# 10m05s against 1m32s for the one non-devenv job). Keep the two in step by hand.
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
`devenv.lock` (via the rev in `devenv.yaml`) the one that *lints and tests*
it. **Bump them together**; `check-pins` and CI's `pins` job fail if the three
disagree. They did disagree for two weeks after dependabot bumped `flake.lock`
alone, with nothing in CI to notice.

Both Rust crates set `doCheck = false`, so `nix build` compiles the shipping
binaries and does not run the suites. The suites run via `cargo test` — in
`devenv test`/`test-rust` locally and in CI's lint job. This was a concession
to the 10-minute cap of Codeberg, the project's former CI host: doCheck
recompiles each crate in test configuration and links a binary per test
target, which put
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

**Every Rust binary links with mold and compiles through ccache (C) and
sccache (rustc)** — `flake/fast-build.nix`, one stdenv shared by both flake
devShells, devenv and `flake/packages.nix`, so `cargo` in a shell and
`nix build` link the same way. In the shells it is on with no setup (caches
under `~/.cache`; CI persists them with `actions/cache` in the clippy and
test jobs). Inside `nix build` the caches are gated on the host exposing
`/var/cache/ccache` and `/var/cache/sccache` to the sandbox
(`extra-sandbox-paths`, directories `0770 root:nixbld`) and are otherwise
never invoked, so a box's 03:00 rebuild is the plain build it always was;
mold is never gated. ccache cannot cache rustc — the minutes saved are
sccache's — and edge-vercel on Vercel's builders gets neither. Two things
in that file look odd and are load-bearing: the stdenv is composed mold
first and ccache around it (the other way round, `useMoldLinker` reads
ccache's version as the compiler's and silently drops `-fuse-ld=mold`), and
`RUSTC_WRAPPER` is a script named anything but `sccache` (the `cc` crate
fronts the C compiler with sccache too when it sees that name, with
`CCACHE_DISABLE` set, and ccache then caches nothing).

**GitHub is the only forge.** The project left Codeberg on 2026-09-30, and
the `.forgejo/` lane went with it; references to Codeberg that remain in
comments are history explaining a choice its CI forced. CI is
`.github/workflows/ci.yml`, kept in step by hand with `devenv.nix`, plus a
tag-release job that builds the heavy media, attaches the installer ISO to the
GitHub release and pushes both media to GHCR (the demo QCOW2 is over GitHub's
2 GiB per-asset limit). `.github/actions/` carries `setup-nix`, `proxy-push`
and `github-release`.

GitHub's runners cap at 6 hours, so jobs carry explicit `timeout-minutes`. The
structure Codeberg's 10-minute / 8 GB cap forced is kept because the reasons
outlive the cap: clippy and test stay separate jobs (as one they compile the
local crates twice), and the `iso` job imports `losos-ctl` as an artifact
rather than rebuilding it. The **install toplevel** remains a
local-only gate. The **installer ISO is now built in CI**: its closure is 769
paths, of which 766 are stock nixpkgs served by cache.nixos.org and only
`losos-ctl` is expensive, so the `iso` job imports that one from the
`losos-ctl` job as a 12 MiB artifact instead of recompiling it. Measured:
~4.7 GiB written against a 10 GiB quota, 50 s for squashfs + xorriso.
The same job then boots that ISO under OVMF and SeaBIOS with
`tests/iso-boot.py`, which passes when the installer's DNS query for
github.com shows up in a pcap of the guest NIC. Don't add `restrict=on` to its
netdev: QEMU then drops the DNS server from its DHCP offer and no query is
ever sent. Secure Boot is deliberately not a leg: nothing here is signed, and
the README tells owners to switch it off.

One flake check is not a VM: `losos-invariants` (`tests/invariants.nix`)
evaluates the published `install` configuration and asserts the option values
the appliance cannot afford to lose by a default drifting (garbage collection,
the boot-menu cap, the unlock mode, the `#install` fragment). `nix flake check
--no-build` runs it, so CI's eval job fails on it at zero build cost.

The VM tests are the real acceptance gate for the control plane and are *not*
run by CI, so run them locally when touching either:

```sh
nix build .#checks.x86_64-linux.losos-admin-daemon   # lososd: D-Bus + HTTP + token
nix build .#checks.x86_64-linux.losos-install        # installer: detect + disko + LVM (keyfile path, BIOS)
nix build .#checks.x86_64-linux.losos-tpm-unlock     # installer: format + enrol + reboot + unlock from swtpm
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

## Pull requests

`.github/pull_request_template.md` is the shape of every PR description:
Before / After prose, a **screenshots table**, How, Tested, notes for the
reviewer. Fill every section. The screenshots are **required for any change
a person can see** — the admin UI, the wizard, the installer and tty1
screens, the two app themes, the wiki and docs pages — one Before and one
After per screen, taken at the same window size and in the same state so
the only difference is the change. A change with nothing visible says
"No visible change." under that heading and why, instead of the table.
Sessions working from a project thread keep their screenshots in the
project folder under `/mnt/project-files/demo/<topic>/` and name that folder
in the table (the project folder is not reachable from GitHub, so the owner
drops the images into the body if they should live on the PR); the
attribution block the project requires goes above the template, as the
first two lines. Under Tested, say what did not run and why — a session
has no KVM, so the VM tests are usually "not run" rather than silently
missing.

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
populated. The volume is always formatted, unattended, from a random keyfile
the installer generates at `/etc/keys/persist-keyfile`. Unlock is TPM2 by
default (`losos.tpm.enable = true`): the installer runs `systemd-cryptenroll
--tpm2-device=auto --tpm2-pcrs= --unlock-key-file=…` right after `disko`, so
the initrd carries no secret and the keyfile stays only inside `/persist` as
the recovery slot (`losos-ctl grow` authenticates with it). No PCR binding, on
purpose: the box updates itself unattended with no shell to recover a lockout
from (same call as `keyring.nix`). Where the installer medium sees no
`/dev/tpmrm0`, or with `losos-ctl install --no-tpm`, the box is in keyfile
mode: `tpm.enable = false` in `install-target.nix` and the keyfile injected
into the initrd as `/crypto_keyfile.bin`, on the unencrypted ESP. The two
crypttab shapes are exclusive: systemd-cryptsetup given a key file *and*
`tpm2-device=` reads the file as a sealed blob. `tests/invariants.nix` pins the
default and that the TPM path declares no initrd secret; `tests/tpm.nix` boots
it end to end under swtpm. A remote `github:` upgrade URI evaluates a tree
without `install-target.nix`, so `flake.nix` prefers the live
`/etc/nixos/modules/{install-target,overrides}.nix` whenever it can read them,
and both rebuild paths (`updates.nix`, `supervisor.rs`) pass `--impure` for
that; see the auto-upgrade section.

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
to those, or to `/ds/` or `/common.js`, it is stale. The SPA's primitives
(`admin-ui/app/src/components/ui/`) are **shadcn/ui components on the box's
palette** (`components.json` points the CLI at `src/styles/index.css`, which
aliases shadcn's colour names onto `tokens.css`; `accent` and `muted` are
deliberately not aliased because the app already uses both names for
something else). Which flavour each one comes from is decided by the admin
page's `style-src 'self'`, which (verified in Chromium) allows React `style`
props and every other CSSOM write but refuses `setAttribute("style")` and any
`<style>` element a library creates at runtime: sonner renders unstyled unless
its shipped `styles.css` is imported into the bundle, and Radix's scroll lock
(react-remove-scroll) silently does nothing, so nothing modal is Radix. The
Sidebar and its sheet, tooltip and folds are shadcn's *base* registry on
Base UI, whose scroll lock is CSSOM; the Switch is shadcn's React Aria
flavour (`react-aria-components`), laid into settings rows by `SwitchRow` as
shadcn's "Switch with a description" Field; only the confirmation Toast is
Radix, which needs no scroll lock. Dialog stays on the native `<dialog>`.
`tests/app.browser.mjs` serves the real CSP header and fails on any new
violation, so a library swap that injects styles turns a check red. The command layer is written against a `Losos` effect
**trait** with a real (`io_backend`) and an in-memory (`fake`) implementation,
so the state machine is unit-tested with no filesystem; the installer repeats
the pattern with `Install` + `plan_install`, whose plan is data, so the
ordering of destructive steps is asserted without formatting anything. Wire
contract: `backend/schema.json`. Set `losos.backend.package = null` to run
without it.

**Every option on one pane, and the configuration in LosOS Git**
(`flake/options-doc.nix` + `modules/config-repo.nix` + `backend/src/options.rs`
+ `backend/src/config_repo.rs` + `admin-ui/app/src/screens/settings/pane-advanced.tsx`
+ `pane-history.tsx`): the `losos.*` declarations are walked at build time
into `/etc/losos/options.json` (name, editor kind, default, description, the
running value, danger and read-only flags); lososd joins the current
`overrides.nix` into it at `GET /api/options`, and the Advanced pane draws a
typed editor per row. `classify` in the Nix file is the gate: a declaration
with a type it does not know is a `throw`, so the eval job goes red rather
than an empty row landing on the pane, and `tests/advanced.browser.mjs`
renders the real document (`tests/admin-ui.nix` passes the
`losos-options-doc` check) and fails on any row the pane cannot draw.
`POST /api/apply` is now checked line by line against the document
(`check_body`): undeclared, read-only (installer-fixed, packages) and
ill-typed values, and any `${`, are refused with the key named. Every
apply, mode change and reset is a commit in `/etc/nixos` (the repository
the installer made), and a reconciler thread in lososd polls every 30 s:
it creates the admin's Forgejo account (`notshared`, the owner password,
kept in step on sign-in and set-password) and a private `losos-config`
repository through a bot account the Forgejo start-up script mints a token
for (`flake/forgejo-bootstrap.nix`, token at `/var/lib/forgejo/.losos-token`),
pushes when behind, and when a clone has pushed ahead fast-forwards, gates
the new `overrides.nix` the same way, and rebuilds; a diverged history is
reported, never touched. There is deliberately **no Forgejo Actions
runner** behind this. The sixteen settings the other panes own are linked
from the Advanced pane, not edited twice; `OWNED` in
`admin-ui/app/src/lib/option-value.ts` and `OWNED_NAMES` in `lib/api.ts`
are that list, and `buildOverridesNix` filters extras against it so the
file never carries a key twice. Two parser facts matter here:
`overrides.rs`'s `parse_line` cuts a value at the first `#` *outside*
quotes (it used to cut inside them, which mangled the default
`upgradeFlakeUri`), and a list is one `[ "a" "b" ]` line, strings only.

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
longer served at all** — the shipped SPA carries its own palette in
`admin-ui/app/src/styles/tokens.css`, which `index.css` republishes as
Tailwind v4 theme keys. It stays out of the Nix closure
because `losos-admin-ui`'s source *root* is `admin-ui/app`; that is a stronger
guarantee than the `design-system` name filter it replaced, which would have
stopped matching on a rename.

**One look across the homepage, Nextcloud and Forgejo** (`admin-ui/themes/`):
`tokens.css` is plain CSS on purpose, because both apps' themes ship it byte
for byte (as `losos-tokens.css`) and map their own variables onto it — change
a colour there and all three move together. Nextcloud gets a *theme folder*
(`themes/losos/` copied into the package by `nextcloud-stack.nix`, plus
`theme = "losos"` in config), because it resolves themes against its real
server root and a symlink beside the package is never found. Forgejo gets
`theme-losos-{auto,light,dark}.css` in `$FORGEJO_CUSTOM/public/assets/css/`
(tmpfiles links natively, copied by the image's entrypoint) with
`DEFAULT_THEME = losos-auto`, which Forgejo stamps on accounts at creation —
an existing account keeps its old theme until its owner switches.
Owners see the two apps as **LosOS cloud** and **LosOS Git**: the theme
folder's `defaults.php` names Nextcloud, Forgejo's `APP_NAME` names it, and
both get logos drawn from `admin-ui/themes/brand/` (`marks.py` turns
`fish.png` into the SVGs; every PNG and the `.ico` is rendered from those at
build time). The rest of the upstream marketing goes by config (no skeleton
files, no help or sign-up links, no "Powered by") and two Forgejo template
overrides. Two non-obvious bits: Nextcloud's header filter inverts any logo
it thinks is its own white one, so `server.css` sets
`--image-logoheader-custom`; and the theming app reads a few `core/img/`
files by absolute path, so `nextcloud-stack.nix` overwrites those in the
package, which is why the image sets `integrity.check.disabled` as
`services.nextcloud` already does.
`admin-ui/themes/default.nix` is plain data imported by all three callers, for
the same reason `nextcloud-stack.nix` is. The workload pods are
`hostNetwork` (the local cluster runs no CNI), so they answer on loopback — see
the gotcha about what that costs the `lanOnly` guard.

**The Vercel demo host** (`edge-vercel/`): a third Rust crate, outside the
flake, that runs the registrar's router as one Vercel Function with the
registry as one `jsonb` row in a Neon Postgres (tokio-postgres over rustls;
`DATABASE_URL` from Vercel's marketplace integration, loaded before and
upserted after every request) and two demo-only routes (`/status`,
`/status/traefik`) plus a static page. It exists for presentations and is
meant to be deleted afterwards: the only changes it needed in
`backend-registrar` are `server::build` (the router and reconciler without
the listener and the timer; `serve` calls it) and `Registry::{export,import}`
(a snapshot with ages, so the registry can travel between processes;
`import` applies the heartbeat TTL because no reconciler ran in between).
The tunnel, Traefik, the mesh and the Stripe gate cannot run there, so on
that host `/cluster/join` and `/market/*` answer 503 and `/noise-public-key`
404. `devenv.nix`'s `crates` list and CI lint/test it like the other two;
`nix build` never sees it. Its `tests/postgres.rs` is gated on
`LOSOS_TEST_DATABASE_URL` (CI's test job provides a Postgres service); the
rest of its suite runs on the in-memory store. Its second static page,
`public/find.html`, is the "find my box" flow: Chrome's Local Network Access
permission lets it read a box's `/setup/state.json` from the public origin,
which `modules/setup.nix` allows for exactly the origins in
`losos.setup.finderOrigins` (an nginx map, asserted by `tests/setup.nix`),
never `*` — the document is LAN inventory.

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

**BIOS and the tty1 banner** (`losos.bios`, `modules/console.nix`): the
installer ISO's firmware menu writes `losos.bios` into `install-target.nix`:
BIOS and UEFI can be chosen explicitly, or autodetect uses the absence of
`/sys/firmware/efi` (also available noninteractively with `--bios`/`--uefi`).
The menu exists only on the installer ISO, not the installed system, and
takes autodetect after 30 s unanswered: the medium is meant to install
unattended, and `tests/iso-boot.py` never types. BIOS
switches `boot.nix` to GRUB and adds an EF02 partition to the first drive in
`disko.nix`; the ESP stays `/boot` either way. `console.nix` replaces getty on
tty1 with a banner service showing the LAN IPv4 and `<hostName>.local`.

**Auto-upgrade + nightly reboot** (`updates.nix`): `system.autoUpgrade`
rebuilds from `losos.upgradeFlakeUri` at 03:00. The default,
`git+file:///etc/nixos#install`, only advances the system consistently and
does not pull new nixpkgs; set a `github:` URI to actually upgrade. **Both
rebuild paths are `--impure` on purpose** (`system.autoUpgrade.flags` and
lososd's `rebuild_command`): the published tree carries neither this box's
`modules/install-target.nix` nor its `modules/overrides.nix`, so `flake.nix`
imports the live copies from `/etc/nixos` when it can read them, and only an
impure evaluation can. Drop the flag and a `github:` upgrade flips a keyfile
box to the TPM shape, forgets its drives and firmware mode, and resets every
setting. `builtins.pathExists` on an absolute path is `false` under pure
evaluation (not an error), so CI, `nix flake check` and the installer stay
pure and see the in-tree file or the defaults. `tests/invariants.nix` pins
the flag.
`nix.gc` runs at 04:30 with `--delete-older-than 14d`, and `boot.nix` caps
both loaders at five generations: `/nix` *is* `/persist`, the ESP is 500 MiB,
and `linuxPackages_latest` lands a new kernel there most nights, so without
both the box fills itself up and the 03:00 switch fails on the bootloader
step. `tests/invariants.nix` asserts both at eval time.

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
  to the owner as a feature that quietly never works. `nixos-rebuild` is on
  that list for the same reason: systemd-run resolves a bare command against
  the *caller's* PATH, so without it every Apply failed before the rebuild
  unit existed. And lososd must not get `ProcSubset=pid`: it hides
  `/proc/devices`, and vgs exits 4 without it. `tests/invariants.nix` pins
  both.
- **The LUKS keyfile is hex text, and must stay NUL-free.** disko hands
  `passwordFile` to `luksFormat` as `<(echo -n "$(cat FILE)")`, a command
  substitution that drops NUL bytes and a trailing newline, while the initrd
  (`/crypto_keyfile.bin`) and `systemd-cryptenroll --unlock-key-file` read
  the file raw. The installer used to write 4096 random bytes, so the volume
  was formatted with one key and unlocked with another on practically every
  install; nothing reached that failure until `tests/tpm.nix` enrolled the
  chip. `ensure_keyfile` now writes 2048 random bytes as 4096 hex characters.
  Don't "harden" it back to raw bytes, and don't put a newline in it.
- **The LUKS key file must not live under `/root` or `/home`.** `lososd` runs
  with `ProtectHome=true` — on purpose, so a compromised request handler cannot
  read either data domain — and that hides `/root` too. `cryptsetup resize`
  then fails with "Failed to open key file", *after* `lvextend` has already
  grown the volume. `/etc/keys/persist-keyfile` is the path everything agrees
  on: `disko.nix` formats from it, the installer keeps it inside `/persist`
  on both unlock paths, `daemon.nix` passes it as `LOSOS_LUKS_KEYFILE`, and
  `tests/resize.nix` uses it rather than a fixture of its own, so they cannot
  drift apart unnoticed.

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
- **The owner's password unlocks the admin pages, and Nextcloud is the judge.**
  `POST /api/sign-in` (`backend/src/signin.rs`) sends the password to
  Nextcloud over loopback (`$LOSOS_NEXTCLOUD_LOGIN_URL`, set per mode in
  `daemon.nix`) in a Basic header and answers with the admin token on a 200.
  lososd keeps no hash of the password on purpose: a second copy would drift
  the moment the owner changed it inside Nextcloud. The cost is that the
  route answers 503 while Nextcloud is down, which is what the printed spare
  key is for — so don't remove the key path from the unlock dialog, and don't
  turn the sign-in probe into a `curl -u` (the password would be in an argv).
  A new password wants 12+ characters, both cases, a digit and a symbol
  (`setup::validate_password`); a sign-in checks only the candidate's shape
  (`signin::validate_candidate`), or a password set under an older rule set
  would lock its owner out.
- **Nextcloud trusts the box's own IP address through one nginx header.**
  The `/nextcloud` location sets `X-Losos-Server-Addr $server_addr`, and the
  pod's `losos.config.php` appends that value to `trusted_domains` per
  request, IP literals only. That is how a libvirt VM, which gets no mDNS
  name, is reached at `http://192.168.122.x/nextcloud` without "Untrusted
  domain". Don't replace it with a `192.168.*` wildcard: Nextcloud's `*`
  matches `[-.a-zA-Z0-9]*`, so `192.168.attacker.example` would be trusted
  too. `localhost` and `127.0.0.1` need nothing — Nextcloud trusts them
  unconditionally.
- **The admin UI's app tiles link through `index.php`.** Nextcloud answers
  `/nextcloud/index.php/apps/<id>/` on every configuration, while the short
  `/nextcloud/apps/<id>/` form only works when the pod's Apache rewrites
  pretty URLs, and without that it was an Apache 404 straight from the
  admin home. `admin-ui/app/src/lib/apps.ts` uses the long form for that
  reason; don't "tidy" the tiles back to the short one.
- **Hand-written widgets run in `/widget-frame/`, never in the admin page.**
  The owner's HTML/script (`backend/src/look.rs`, `GET/POST /api/look*`)
  is drawn by `<iframe sandbox="allow-scripts" src="/widget-frame/">`
  (`admin-ui/app/src/widgets/hand-frame.tsx`, page in
  `admin-ui/app/public/widget-frame/`). Two things are load-bearing: no
  `allow-same-origin` (the frame is an opaque origin with no token and no
  API), and the `~^/widget-frame/` arm of nginx's header map in
  `containers.nix`, the one path served under a permissive CSP. An
  `<iframe srcdoc>` or a `blob:` document would inherit the admin page's
  strict policy and refuse the inline script just the same, so don't
  "simplify" the frame into one. `tests/front-vhost.nix` asserts the arm and
  that the strict policy still covers everything else.
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
- **The market is the registrar's third opt-in.** `/market/*` (Stripe Connect,
  `backend-registrar/src/market.rs`, `wiki/Market.md`) answers 503 unless
  `losos.edge.market.enable`, and 403 unless `losos.edge.tenants.<id>.market`.
  `modules/edge.nix`'s `tenantsJson` hardcodes its attributes, so the `market`
  key must stay listed there or every trade silently 403s. A paid order is an
  entitlement: storage orders get a namespace and PVC on the mesh (idempotent,
  409 counts as done, nothing is ever deleted), compute is a ledger credit
  only. Listing and ordering are gated on what the seller's node already shares
  (`Sharing`, from the registry's compute windows) — the market monetises the
  mesh, it is not a second product. The admin UI reaches it through lososd's
  `/api/market*` relay (`backend/src/market.rs`), which answers 200
  `{available:false}` rather than 404 when the market is off, because the SPA
  latches a 404 as "route not served". The Stripe key is held only by
  the `losos-stripe-gate` unit (`losos-registrar stripe-gate`), which gets the
  sealed blobs (`losos.edge.market.{stripeSecretKey,webhookSecret}Sealed`) by
  `LoadCredentialEncrypted=`; the registrar talks to it over
  `/run/losos-stripe-gate/gate.sock` and never sees the key, so don't add an
  operation that forwards arbitrary Stripe calls, and keep the gate's request
  validation (destination, currency, fee ceiling, session lifetime, https endpoint) tighter than what the
  registrar happens to send. A missing blob skips the gate (`ConditionPathExists`)
  and `/market/*` answers 503. Onboarding writes the box UUID
  (`backend/src/boxid.rs`, a SHA-256 derivative of the recovery code — never the
  code itself) onto the Stripe account.
- **Any edge opens sharing; only an *official* edge opens the market.**
  lososd scans for edges (`backend/src/edge.rs`: DNS-SD `_losos-edge._tcp`
  plus `losos.proxy.registrarUrl`, `/health` probed) and refuses to turn
  `sharingMyStorage`/`cluster.enable` on with none in reach (409
  `edgeRequired`; enable-only, so a box can always leave). On top of that,
  each answering edge is challenged (`GET /identity?nonce=`) and counts as
  official only if its certificate is signed by the LosOS root key in
  `keys/official-edge-root.pub` (`losos.proxy.officialRootKeyFile`), names
  that URL, is unexpired, and the nonce is signed by the certificate's key.
  The market relay refuses everything else (`{available:false,
  reason:"noOfficialEdge"}`, 409 `officialEdgeRequired`). The committed key
  file is **empty on purpose** until the owner writes the public key: no
  root, nothing official, market off. The private key lives offline with the
  owner and is never committed; `losos-registrar identity
  {keygen,sign,show,verify}` is the whole ceremony (`backend-registrar/src/identity.rs`),
  `losos.edge.identity.{keyFile,certFile}` the edge side. `tests/edge-lan.nix`
  builds its own root at build time rather than trusting the shipped file.
  The ceremony is meant to run *from the box*: `provisioning/edge-identity/`
  is a Forgejo Actions repository (root key made on the box, kept as an
  Actions secret, edges provisioned over SSH), executed by the on-box runner
  in `modules/git-runner.nix` — host mode, registered with a shared secret
  (`forgejo-cli actions register --secret-file`, idempotent per boot, UUID
  derived from the secret) rather than a one-shot token, and working against
  the container pod's app.ini on the host filesystem in container mode.
  `losos.forgejo.runner.enable` also drives the pod's `actions.ENABLED`, so
  Actions is never on without a runner. `tests/git-runner.nix` covers the
  native path end to end; the container path shares the script untested.
- **`system.stateVersion = "26.11"` is set-once** — matches the nixos-unstable
  this flake tracks; don't change it.
- **The `result` symlink is a `nix build` artifact** (pointing into
  `/nix/store`), gitignored, never committed.