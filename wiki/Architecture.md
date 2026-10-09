**English** · [Slovenčina](Architecture-sk) · [Deutsch](Architecture-de)

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
these from `/persist`: `/nix`, `/var`, `/etc/ssh`, `/etc/keys`,
`/etc/nixos`, `/etc/rancher`, the two data homes and `machine-id`.

**Anything not on that list is lost on reboot.** Add new state to
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

One paragraph per mechanism: why each piece sits where it does, and what a
well-meant change there breaks.

**Two NixOS systems share one module set** (`flake.nix`).

- `iso` is a minimal live ISO. It carries the `disko` layout so it can
  format the target disk and enrol encryption keys, and imports only
  `options.nix`, `disko.nix` and `installer.nix`.
- `install` is the system installed on disk. It imports everything:
  `options`, `configuration`, `impermanence`, `disko`, `boot`, `services`,
  `nextcloud-common`, `containers`, `daemon`, `overrides`, `updates`,
  `defaults`. The flake passes `specialArgs.self = self` so `defaults.nix`
  can reach `self.packages.${system}.{losos-ctl,losos-admin-ui}` and wire in
  the control plane and the admin UI.

**Stateless by impermanence** (`impermanence.nix`, `disko.nix`, `boot.nix`).
The root is tmpfs. `/persist` is LUKS-encrypted ext4 with the `encrypt`
feature, because fscrypt needs it and btrfs cannot provide it. Only the
directories in `environment.persistence."/persist".directories` survive a
reboot: `/nix`, `/var`, `/etc/ssh`, `/etc/keys`, `/etc/nixos`, `/etc/rancher`,
the two data homes and `machine-id`. **Anything new that must survive a
reboot goes on that list**, or it vanishes on the next boot without an error.
`/persist` is `neededForBoot`, so the impermanence bind mounts resolve before
the sysroot is populated.

The installer always formats the volume unattended, from a random keyfile it
generates at `/etc/keys/persist-keyfile`. Unlock is TPM2 by default
(`losos.tpm.enable = true`). Right after `disko`, the installer runs
`systemd-cryptenroll --tpm2-device=auto --tpm2-pcrs= --unlock-key-file=…`, so
the initrd carries no secret and the keyfile stays only inside `/persist` as
the recovery slot, which `losos-ctl grow` authenticates with. There is no PCR
binding, on purpose. The box updates itself unattended and has no shell to
recover a lockout from, the same call `keyring.nix` makes.

Where the installer medium sees no `/dev/tpmrm0`, or with `losos-ctl install
--no-tpm`, the box is in keyfile mode. `install-target.nix` sets
`tpm.enable = false`, and the keyfile goes into the initrd as
`/crypto_keyfile.bin`, on the unencrypted ESP. The two crypttab shapes
exclude each other: systemd-cryptsetup given a key file *and* `tpm2-device=`
reads the file as a sealed blob. `tests/invariants.nix` pins the default and
checks that the TPM path declares no initrd secret. `tests/tpm.nix` boots it
end to end under swtpm.

A remote `github:` upgrade URI evaluates a tree without
`install-target.nix`. So `flake.nix` prefers the live
`/etc/nixos/modules/{install-target,overrides}.nix` whenever it can read
them, and both rebuild paths (`updates.nix`, `supervisor.rs`) pass
`--impure` for that. See the auto-upgrade paragraph below.

**Two isolated data domains, no shell** (`configuration.nix`). `notshared`
(uid 1000) owns Nextcloud. `shared` (uid 1001) owns the storage contributed
to the mesh. Both homes are mode `700`, each with its own primary group, so
neither user can read the other's files. `isNormalUser` without an explicit
`group` puts both in `users`, and a `750` home then grants that group r-x.
That quietly defeated the isolation until `tests/impermanence.nix` caught it.
Neither user has a password, and `services.openssh.enable = false`. The only
configuration changes reachable from the running box go through the admin UI:
the Local or Mesh storage toggle, **join the compute mesh**, and **share my
compute when I sleep**.

**The lososd daemon, the losos-ctl facade, and the admin endpoint**
(`modules/daemon.nix`, `backend/`, `admin-ui/`). The privileged logic lives
in `lososd`, a root systemd daemon in Rust (`backend/`, one crate with two
binaries; zbus for D-Bus, actix-web for HTTP, clap for the CLI). The bus
listener and the rebuild supervisor run on a tokio runtime. The HTTP server
runs on its own thread under an actix `System`. The two runtimes are kept
apart on purpose.

lososd owns `/var/lib/losos/state.json` as its sole writer, with atomic
temp-and-rename writes. It exports one method per subcommand on the system
D-Bus (bus `org.losos1`, path `/org/losos1`, interface `org.losos.Control1`)
and serves a Bearer-authenticated JSON HTTP API on
`127.0.0.1:${losos.admin.apiPort}` (default 8082). Rebuilds run as
`systemd-run` transient units (`losos-rebuild-<job>`). A watcher thread polls
the unit and records done or failed. It re-attaches after a daemon restart,
which happens because `nixos-rebuild switch` restarts lososd itself in the
middle of a rebuild.

`losos-ctl` is a thin facade CLI. It keeps the old subcommands and JSON
output (`state`, `change --mode`, `status`, `settings`, `apply` (stdin),
`factory-reset`; `--json` is a no-op) and relays each call to lososd over
D-Bus. `losos-ctl install` stays local, because the ISO installer runs as
root and needs no bus.

The admin UI (`admin-ui/app/`, packaged as `losos-admin-ui` with
`buildNpmPackage`) is a React 19, Vite and Tailwind v4 SPA on real paths
(`/`, `/apps`, `/storage`, `/mesh`, `/settings/<pane>`). Its built `dist/`
**is** the front nginx vhost's document root, served with
`try_files $uri $uri/ /index.html`. Without that line every deep link and
every reload is a 404. The vhost proxies `/api/*` to lososd's loopback API.
The SPA replaced a pair of plain-JS pages, `dashboard/` and `settings/`. Any
reference to those, or to `/ds/` or `/common.js`, is stale.

The SPA's building blocks (`admin-ui/app/src/components/ui/`) are shadcn/ui
components on the box's palette. `components.json` points the shadcn CLI at
`src/styles/index.css`, which maps shadcn's colour names onto `tokens.css`.
`accent` and `muted` are left unmapped on purpose, because the app already
uses both names for something else.

The admin page's `style-src 'self'` decides which flavour each component
comes from. Verified in Chromium, it allows React `style` props and every
other CSSOM write, but refuses `setAttribute("style")` and any `<style>`
element a library creates at runtime. sonner renders unstyled unless its
shipped `styles.css` is imported into the bundle. Radix's scroll lock
(react-remove-scroll) does nothing at all, so nothing modal is Radix. The
Sidebar and its sheet, tooltip and folds come from shadcn's *base* registry
on Base UI, whose scroll lock uses CSSOM. The Switch is shadcn's React Aria
flavour (`react-aria-components`), which `SwitchRow` lays into settings rows
as shadcn's "Switch with a description" Field. Only the confirmation Toast is
Radix, and it needs no scroll lock. Dialog stays on the native `<dialog>`.
`tests/app.browser.mjs` serves the real CSP header and fails on any new
violation, so a library swap that injects styles turns a check red.

The command layer is written against a `Losos` effect trait with a real
implementation (`io_backend`) and an in-memory one (`fake`), so the state
machine is unit-tested with no filesystem. The installer repeats the pattern
with `Install` and `plan_install`. Its plan is data, so a test asserts the
order of the destructive steps without formatting anything. The wire
contract is `backend/schema.json`. Set `losos.backend.package = null` to run
without the daemon.

**Every option on one pane, and the configuration in LosOS Git**
(`flake/options-doc.nix`, `modules/config-repo.nix`,
`backend/src/options.rs`, `backend/src/config_repo.rs`,
`admin-ui/app/src/screens/settings/pane-advanced.tsx`, `pane-history.tsx`).
At build time the `losos.*` declarations are walked into
`/etc/losos/options.json`: name, editor kind, default, description, the
running value, and the danger and read-only flags. lososd joins the current
`overrides.nix` into it at `GET /api/options`, and the Advanced pane draws a
typed editor per row.

`classify` in the Nix file is the gate. A declaration with a type it does not
know is a `throw`, so the eval job goes red instead of an empty row landing
on the pane. `tests/advanced.browser.mjs` renders the real document
(`tests/admin-ui.nix` passes the `losos-options-doc` check) and fails on any
row the pane cannot draw. `POST /api/apply` checks each line against the
document (`check_body`). It refuses undeclared keys, read-only ones
(installer-fixed values, packages), ill-typed values and any `${`, and names
the key it refused.

Every apply, mode change and reset is a commit in `/etc/nixos`, the
repository the installer made. A reconciler thread in lososd polls every
30 s. It creates the admin's Forgejo account (`notshared`, with the owner
password, kept in step on sign-in and set-password) and a private
`losos-config` repository. It does that through a bot account whose token the
Forgejo start-up script mints (`flake/forgejo-bootstrap.nix`, token at
`/var/lib/forgejo/.losos-token`). It pushes when Forgejo is behind. When a
clone has pushed ahead, it fast-forwards, gates the new `overrides.nix` the
same way, and rebuilds. It reports a diverged history and never touches it.
There is deliberately **no Forgejo Actions runner** behind this.

The Advanced pane links the sixteen settings the other panes own instead of
editing them twice. `OWNED` in `admin-ui/app/src/lib/option-value.ts` and
`OWNED_NAMES` in `lib/api.ts` are that list, and `buildOverridesNix` filters
extras against it so the file never carries a key twice. Two parser facts
matter here. `overrides.rs`'s `parse_line` cuts a value at the first `#`
*outside* quotes; it used to cut inside them, which mangled the default
`upgradeFlakeUri`. And a list is one `[ "a" "b" ]` line, strings only.

**Workloads and the single front door** (`modules/containers.nix`,
`modules/workloads.nix`, `modules/cluster.nix`,
`modules/nextcloud-common.nix`). Rootless Podman is gone, and so is
systemd-nspawn. When `losos.<svc>.mode == "container"`, Nextcloud and Forgejo
run as Kubernetes workloads in the box's own local k3s cluster. The shared
Nextcloud configuration still lives in `nextcloud-common.nix`, so the native
and workload modes cannot drift apart. nginx is the only process holding
public ports. It routes by path on the appliance's mDNS name
(`<hostName>.local:80/nextcloud`, `:80/forgejo`) and serves the admin SPA on
the same `:80` vhost.

`admin-ui/design-system/` (`tokens.css` and `losos.css`, plus a `react/`
wrapper package for claude.ai/design) is dev-machine-only and no longer
served at all. The shipped SPA carries its own palette in
`admin-ui/app/src/styles/tokens.css`, which `index.css` republishes as
Tailwind v4 theme keys. The design system stays out of the Nix closure
because the source *root* of `losos-admin-ui` is `admin-ui/app`. That is a
stronger guarantee than the `design-system` name filter it replaced, which a
rename would have broken.

**One look across the homepage, Nextcloud and Forgejo** (`admin-ui/themes/`).
`tokens.css` is plain CSS on purpose. Both apps' themes ship it byte for byte
as `losos-tokens.css` and map their own variables onto it, so a colour
changed there moves all three.

Nextcloud gets a *theme folder*: `nextcloud-stack.nix` copies `themes/losos/`
into the package, and the config sets `theme = "losos"`. Nextcloud resolves
themes against its real server root, so it never finds a symlink beside the
package. Forgejo gets `theme-losos-{auto,light,dark}.css` in
`$FORGEJO_CUSTOM/public/assets/css/`, linked by tmpfiles natively and copied
by the image's entrypoint, with `DEFAULT_THEME = losos-auto`. Forgejo stamps
that on accounts at creation, so an existing account keeps its old theme
until its owner switches.

Owners see the two apps as **LosOS cloud** and **LosOS Git**. The theme
folder's `defaults.php` names Nextcloud and Forgejo's `APP_NAME` names
Forgejo. Both get logos drawn from `admin-ui/themes/brand/`. `marks.py` wraps
`salmon.png`, a live coho salmon cut out of a NOAA Fisheries photo, into the
SVGs and copies it to the SPA and the handbook. The build renders every PNG
and the `.ico` from those. On 31 October, by the viewer's own date, the admin
pages and the handbook show `plate.png` instead, a plate of salmon from
Matus's own photo. `brand/CREDITS.md` says where both pictures come from.
The old pixel-art fish was dropped over a copyright worry and does not go
back in. The rest of the
upstream branding goes by config (no skeleton files, no help or sign-up
links, no "Powered by") and two Forgejo template overrides.

Two details are easy to miss. Nextcloud's header filter inverts any logo it
takes for its own white one, so `server.css` sets
`--image-logoheader-custom`. And the theming app reads a few `core/img/`
files by absolute path, so `nextcloud-stack.nix` overwrites those in the
package. That is why the image sets `integrity.check.disabled`, as
`services.nextcloud` already does. `admin-ui/themes/default.nix` is plain
data that all three callers import, for the same reason `nextcloud-stack.nix`
is. The workload pods use `hostNetwork`, since the local cluster runs no CNI,
so they answer on loopback. The [gotcha](Development#gotchas-that-bite-silently)
about the `lanOnly` guard explains what that costs.

**The Vercel demo host** (`edge-vercel/`). A third Rust crate, outside the
flake, that runs the registrar's router as one Vercel Function. The registry
is one `jsonb` row in a Neon Postgres (tokio-postgres over rustls), with
`DATABASE_URL` from Vercel's marketplace integration, loaded before every
request and upserted after it. It adds two demo-only routes (`/status`,
`/status/traefik`) and a static page. It exists for presentations and is
meant to be deleted afterwards. It needed only two changes in
`backend-registrar`. `server::build` returns the router and reconciler
without the listener and the timer, and `serve` calls it.
`Registry::{export,import}` is a snapshot with ages, so the registry can
travel between processes, and `import` applies the heartbeat TTL because no
reconciler ran in between.

The tunnel, Traefik, the mesh and the Stripe gate cannot run there, so on
that host `/cluster/join` and `/market/*` answer 503 and `/noise-public-key`
404. `devenv.nix`'s `crates` list and CI lint and test it like the other two
crates. `nix build` never sees it. Its `tests/postgres.rs` runs only when
`LOSOS_TEST_DATABASE_URL` is set, which CI's test job provides with a
Postgres service. The rest of its suite runs on the in-memory store.

Its second static page, `public/find.html`, is the "find my box" flow.
Chrome's Local Network Access permission lets it read a box's
`/setup/state.json` from the public origin. `modules/setup.nix` allows
exactly the origins in `losos.setup.finderOrigins` (an nginx map, asserted by
`tests/setup.nix`), never `*`, because the document is LAN inventory.

**The `losos.*` option namespace** (`options.nix`). All project-specific
knobs live under `options.losos`, with defaults in `defaults.nix`:
`targetDrive`, `tpm.enable`, `sharingMyStorage`, `forgejo.enable`,
`nextcloud.*`, `cluster.*` (mesh join, compute window), `shared.fscrypt.*`,
`upgradeFlakeUri`, `backend.package`,
`admin.{enable,port,apiPort,tokenFile,ui}`, `proxy.*` (the appliance side of
the master proxy) and `edge.*` (the VPS side, consumed through the
`nixosModules.edge` flake output). **Modules never read bare `config.X`; they
read `config.losos.X`.** This is the only place to add new options. Earlier
code wrongly declared options inside `config` blocks.

**Hardening** (`hardening.nix`, `install` only). `losos.hardening.*` is built
from KSPP primitives rather than wrapping an upstream profile, because none
is left to wrap. `profiles/hardened.nix` was removed in 26.05, and
`linux_hardened` is now a `throw`. `hardening.enable` (default true) is the
layer that costs nothing: kernel parameters, sysctls, a module blacklist,
`boot.tmp.useTmpfs`, dbus-broker, and systemd sandboxing on `nginx`,
`avahi-daemon` and `lososd`. The blacklist also blocks explicit `modprobe`.
Plain `boot.blacklistedKernelModules` does **not**; it only stops alias
autoloading. Four opt-in flags carry what can break something: `apparmor`,
`malloc`, `nosmt`, `usbguard`. The `iso` system does not import the module,
because the live medium needs squashfs. `tests/hardening.nix` asserts both
halves, including that nothing blocks the kernel features k3s, rke2,
containerd and Longhorn need.

**Online growth** (`grow.rs`, `losos.storage.fillPercent`). The logical
volume stops short of the whole volume group on purpose, so `/persist` can be
grown without opening the box. `/persist` *is* `/nix`, through impermanence.
`losos-ctl grow` runs lvextend, then `cryptsetup resize`, then `resize2fs`,
in that order, online. The order is the whole correctness argument, and
getting it wrong fails silently. So `grow.rs` asserts it against the *plan*,
and `tests/resize.nix` checks it end to end.

**BIOS and the tty1 banner** (`losos.bios`, `modules/console.nix`). The
installer ISO's firmware menu writes `losos.bios` into
`install-target.nix`. BIOS and UEFI can be chosen explicitly. Autodetect
picks BIOS when `/sys/firmware/efi` is missing. `--bios` and `--uefi` choose
without the menu. The menu exists only on the installer ISO, not on the
installed system, and takes autodetect after 30 s unanswered, because the
medium is meant to install unattended and `tests/iso-boot.py` never types.
BIOS switches `boot.nix` to GRUB and adds an EF02 partition to the first
drive in `disko.nix`. The ESP stays `/boot` either way. `console.nix`
replaces getty on tty1 with a banner service showing the LAN IPv4 address and
`<hostName>.local`.

**Auto-upgrade and the nightly reboot** (`updates.nix`). `system.autoUpgrade`
rebuilds from `losos.upgradeFlakeUri` at 03:00. The default,
`git+file:///etc/nixos#install`, only moves the system forward consistently
and does not pull new nixpkgs. Set a `github:` URI to upgrade for real.

**Both rebuild paths are `--impure` on purpose**: `system.autoUpgrade.flags`
and lososd's `rebuild_command`. The published tree carries neither this box's
`modules/install-target.nix` nor its `modules/overrides.nix`, so `flake.nix`
imports the live copies from `/etc/nixos` when it can read them, and only an
impure evaluation can. Drop the flag, and a `github:` upgrade flips a keyfile
box to the TPM shape, forgets its drives and firmware mode, and resets every
setting. Under pure evaluation, `builtins.pathExists` on an absolute path is
`false`, not an error. So CI, `nix flake check` and the installer stay pure
and see the in-tree file or the defaults. `tests/invariants.nix` pins the
flag.

`nix.gc` runs at 04:30 with `--delete-older-than 14d`, and `boot.nix` caps
both loaders at five generations. `/nix` *is* `/persist`, the ESP is 500 MiB,
and `linuxPackages_latest` lands a new kernel there most nights. Without both
limits the box fills itself up, and the 03:00 switch fails at the bootloader
step. `tests/invariants.nix` asserts both at eval time.

**The `#install` fragment is load-bearing.** Without it, `nixos-rebuild`
looks for `nixosConfigurations.$(hostname)`. This flake exports only `iso`
and `install`, so every nightly run died with "flake does not provide
attribute", on a box with no shell to notice it from. The daemon had it right
all along (`io_backend.rs`, `/etc/nixos#install`). Only the NixOS-side
default was wrong.

A separate `midnight-reboot.timer` reboots unconditionally at 00:07, with
`Persistent=true` so a box that was off catches up.
