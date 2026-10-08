# Architecture

## Systems in the flake

| Output                        | What                                             |
| ----------------------------- | ------------------------------------------------ |
| `nixosConfigurations.iso`     | Live installer ISO (`options`, `disko`, `installer`) |
| `nixosConfigurations.install` | The installed appliance (all modules)            |
| `nixosModules.edge`           | The VPS side of the [master proxy](Master-Proxy) and mesh |

All project options live under `losos.*` in `modules/options.nix`, with
defaults in `defaults.nix`. Modules read `config.losos.*`.

## Impermanence

`/` is a tmpfs. `/persist` is LUKS-encrypted ext4. `impermanence` bind-mounts
these back from `/persist`: `/nix`, `/var`, `/etc/ssh`, `/etc/keys`,
`/etc/nixos`, `/etc/rancher`, the two data homes and `machine-id`.

**Anything not on that list is lost on reboot.** New state must be added to
`modules/impermanence.nix`.

## Control plane

| Component      | Role |
| -------------- | ---- |
| `lososd`       | Root systemd daemon (Rust). Sole writer of `/var/lib/losos/state.json`. D-Bus service `org.losos1` and a token-authenticated JSON API on `127.0.0.1:8082`. |
| `losos-ctl`    | CLI that forwards each subcommand to `lososd` over D-Bus. `losos-ctl install` runs standalone on the ISO. |
| `losos-admin-ui` | React + Vite + Tailwind SPA, served by nginx, which proxies `/api/*` to `lososd`. |

Rebuilds run as transient systemd units (`losos-rebuild-<job>`).
`nixos-rebuild switch` restarts `lososd`, so on startup it re-attaches to any
rebuild still recorded as `building`.

See [`backend/README.md`](https://github.com/dasmatus/losos/blob/main/backend/README.md)
for the crate layout and [`backend/schema.json`](https://github.com/dasmatus/losos/blob/main/backend/schema.json)
for the wire format.

## Workloads

Nextcloud and Forgejo run in the box's own k3s cluster
(`losos.<svc>.mode = "container"`) or natively on the host. The pods use
`hostNetwork`. nginx is the only service on public ports and routes by path.

## Repository layout

```
modules/                 NixOS modules; options in options.nix
backend/                 lososd + losos-ctl (Rust)
backend-registrar/       master-proxy edge registrar (Rust)
admin-ui/app/            the admin SPA
admin-ui/themes/         LosOS cloud and LosOS Git: themes on the SPA's tokens.css, logos, names
admin-ui/design-system/  dev-only tokens and React wrapper; not shipped
tests/                   NixOS VM tests
docs/                    security model, design specs and plans
wiki/                    source of this wiki
```

`CLAUDE.md` is the compressed form of this page and of the gotchas in
[Development](Development#gotchas-that-bite-silently).

## In depth

The cross-file picture, one paragraph per mechanism: why each piece is where
it is and what a well-meaning change there breaks.

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
both get logos drawn from `admin-ui/themes/brand/` (`marks.py` wraps
`plate.png`, the plate of salmon cut out of Matus's own photo, into the
SVGs and copies it to the SPA and the handbook; every PNG and the `.ico` is
rendered from those at build time). The old pixel-art fish was dropped over
a copyright worry; nothing drawn by anyone else goes back in. The rest of the upstream marketing goes by config (no skeleton
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
the [gotcha](Development#gotchas-that-bite-silently) about what that costs the `lanOnly` guard.

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
