# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Context

`losos` is a stateless NixOS appliance for repurposed mini-PCs: tmpfs root,
encrypted `/persist` bind-mounted back by impermanence, Nextcloud for the
owner, spare disk and CPU lent to a mesh, no SSH and no shell. The owner
reaches it only through web UIs and one admin endpoint, so a bad nightly
upgrade has no one to notice it. Docs live in the handbook, `handbook/`,
published to losos.dasmat.us on push to `main` and served by every box at
`/handbook/`. Its In depth chapter (`handbook/docs/in-depth/architecture.md`)
and the developer notes kept out of it (`docs/development.md`,
`docs/ci-and-releases.md`) carry the long form of what this file compresses.

## Commands

```sh
devenv shell                 # or: direnv allow. Scripts: fmt, lint, lint-wasm,
                             # test-rust, check-flake, check-eval, check-pins,
                             # build-pkgs, build-iso, vm-tests
devenv test                  # the gate. Everything but the VMs and the ISO
cargo test   --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
nix build .#losos-ctl .#losos-admin-ui
nix build .#nixosConfigurations.install.config.system.build.toplevel
nix build .#nixosConfigurations.iso.config.system.build.isoImage
nix build .#checks.x86_64-linux.losos-admin-daemon  # lososd: D-Bus, HTTP, token
nix build .#checks.x86_64-linux.losos-install       # installer, keyfile, BIOS
nix build .#checks.x86_64-linux.losos-tpm-unlock    # installer, swtpm unlock
npm run dev | typecheck | test:browser              # in admin-ui/app
```

Six Rust crates: `backend/` (lososd + losos-ctl), `backend-registrar/`,
`edge-vercel/` (outside the flake), and the Lab's `admin-ui/lab/core`,
`admin-ui/lab/virt-rpc` and `admin-ui/lab/render` (built for wasm32). All
six must pass clippy with `-D warnings` and rustfmt; the devenv pre-commit
hooks, `lint` and CI each check it. Bevy is a wasm32-only dependency of
`render`, so `lint-wasm` and CI's `clippy-wasm` lint its WebGL2 and WebGPU
builds too, one after the other. `nix develop` is a toolchain-only shell for people without devenv.
Inside the shell `bcd` and `rcd` cd into the two in-flake crates.
`admin-ui/design-system/react` has its own `npm run typecheck` and `npm test`
and is never served or built by nix.

## Layout

- `flake.nix` builds two systems from one module set: `iso` (options, disko,
  installer) and `install` (everything). `specialArgs.self` lets
  `defaults.nix` wire `losos-ctl` and `losos-admin-ui` in.
- `modules/options.nix` declares every knob under `options.losos`;
  `defaults.nix` sets them. Modules read `config.losos.X`, never bare
  `config.X`.
- `impermanence.nix` + `disko.nix` + `boot.nix`: tmpfs root, LUKS ext4
  `/persist` formatted unattended from `/etc/keys/persist-keyfile`. Unlock is
  TPM2 with no PCR binding by default; without a TPM the keyfile rides in the
  initrd on the ESP. The installer writes `modules/install-target.nix` (the
  box's drives, firmware mode, unlock mode) onto the box; it is not in the
  tree, and `flake.nix` imports the live copy from `/etc/nixos` on rebuilds.
- `configuration.nix`: `notshared` (uid 1000, Nextcloud) and `shared`
  (uid 1001, mesh storage), each with its own group and a `700` home, no
  passwords, `openssh` off.
- `daemon.nix` + `backend/`: `lososd` is a root daemon owning
  `/var/lib/losos/state.json`, exporting one D-Bus method per subcommand
  (`org.losos1`, `/org/losos1`, `org.losos.Control1`) and a Bearer-authed
  JSON API on `127.0.0.1:8082`. Rebuilds are `systemd-run` units
  `losos-rebuild-<job>` watched by a thread. `losos-ctl` relays to it over
  D-Bus; `losos-ctl install` is local. Commands are written against the
  `Losos` trait (`io_backend` real, `fake` in memory); the installer's plan
  is data, so step order is unit-tested. Wire contract: `backend/schema.json`.
- `admin-ui/app`: React 19 + Vite + Tailwind SPA on real paths. Its `dist/`
  is the front nginx vhost's root (`try_files … /index.html`), `/api/*` is
  proxied to lososd. Palette in `src/styles/tokens.css`.
- LosOS Lab, the setup visualizer at `/lab/` (sidebar entry Lab): the
  admin UI's second Vite page (`admin-ui/app/lab/`, `src/lab/`) over a Rust
  core in WebAssembly (`admin-ui/lab/core`, `losos-lab-core`, contract in
  its README), under its own CSP arm (the admin one plus
  `'wasm-unsafe-eval'`). The box copy has simulated consoles, or libvirt
  guests through `losos-registrar lab` with `losos.lab.libvirt.enable`;
  `admin-ui/lab/engine/build.sh` and `lab.yml` build the hosted copy with
  qemu-wasm guests and deploy it to Vercel when the secrets exist.
  `admin-ui/lab/virt-rpc` is a WebAssembly libvirt client for the helper's
  relay. `admin-ui/lab/render` (`losos-lab-render`) is the GPU canvas: Bevy
  0.19.1 on WebGPU or WebGL2, the two views as a 3D scene under an
  orthographic camera, in one module with the core. The page draws with the
  SVG canvas first, then loads one module (`src/lab/render.ts`), replays the
  core's calls into that module's `Lab` and swaps `bevy-canvas.tsx` in;
  anything that fails leaves it on SVG. No Python in the Lab.
- `containers.nix`, `workloads.nix`, `cluster.nix`, `nextcloud-common.nix`:
  Nextcloud and Forgejo run as hostNetwork pods in the box's own k3s server;
  `services.rke2` (agent) joins the edge's mesh. Nginx is the only public
  listener: `<host>.local/nextcloud`, `/forgejo`, admin SPA on `/`.
- `admin-ui/themes/`: the SPA's `tokens.css` is shipped byte for byte as
  `losos-tokens.css` into the Nextcloud theme folder and Forgejo's custom
  CSS, so one colour change moves all three. `default.nix` is the data every
  caller imports. Owners see LosOS cloud and LosOS Git.
- `hardening.nix` (install only): KSPP primitives, on by default, with
  `apparmor`, `malloc`, `nosmt`, `usbguard` opt-in. `tests/hardening.nix`
  asserts both halves.
- `secure-boot.nix` (iso only): the medium's `EFI/BOOT/BOOTX64.EFI` is a
  signable UKI (ukify: kernel + initrd + cmdline) in place of GRUB, with
  the squashfs's SHA-256 on the signed cmdline and a stage-1 unit that
  checks it. `losos-sign-iso` (`flake/sign-iso.sh`) signs the finished
  ISO in place via the El Torito EFI image; CI and the release job run it
  with the secret, `tests/secure-boot.nix` under OVMF with a throwaway key.
  The certificate is `keys/secure-boot-db.pem` (comment-only until the
  ceremony).
- `branding.nix` (both systems) and `live-branding.nix` (iso only): LosOS
  and the release tag from `flake/version.nix` on the boot menus, the boot
  pictures (`brand-art.nix`, drawn from the salmon), os-release, the console
  palette and banners. The installed box keeps `ID=nixos`. Bump the file in
  the commit that gets the tag; the release job stamps the tag over it.
- `updates.nix`: `system.autoUpgrade` at 03:00 from `losos.upgradeFlakeUri`
  (default `git+file:///etc/nixos#install`, set a `github:` URI to really
  upgrade), `nix.gc` at 04:30 with `--delete-older-than 14d`, five boot
  generations, unconditional reboot at 00:07. `tests/invariants.nix` pins
  these at eval time.
- `grow.rs`: `losos-ctl grow` runs lvextend, `cryptsetup resize`, `resize2fs`
  online, asserted against the plan and in `tests/resize.nix`.
- `backup.nix` + `backup.rs` + `erase.rs`: restic backups to an S3 bucket,
  encrypted on the box with the recovery code as the password, run as
  `losos-backup-<job>` units outside lososd's sandbox; the fscrypt domain is
  unlocked for the copy. The erase is phased in `state.json` (backup,
  cancellable countdown, leave the edge, reset, reboot) and
  `losos-factory-wipe.service` deletes the data before sysinit on the next
  boot. `tests/erase.nix` runs the cycle against MinIO.
- `edge-vercel/`: the registrar's router as one Vercel Function over Neon
  Postgres, for demos. `/cluster/join` and `/market/*` answer 503 there.
- `backend-registrar/src/{domains,zone}.rs` + `losos.edge.dns`: an official
  edge serves its own zone with Knot and routes an owner's domain once its
  TXT token and CNAME check out, only for a box with a ready Stripe account.
  lososd writes the live names to `/var/lib/losos-public-names`, which the
  Nextcloud pod reads for `trusted_domains`.
- `backend-registrar/src/routes.rs`: custom domains of mesh boxes behind a
  local edge. The official edge keeps the routes in the registrar's own
  state (`relay-routes.json` beside the registry, no etcd) and binds a box
  to a local edge only by the box's relay pass, an HMAC the local edge
  forwards but cannot make.
- `backend-registrar/src/market.rs`: Stripe Connect, third opt-in of the
  registrar. The key lives only in the `losos-stripe-gate` unit, reached over
  `/run/losos-stripe-gate/gate.sock`.
- `backend-registrar/src/{vms,server/vm}.rs` + `modules/edge-vms.nix`:
  virtual machines on the mesh, sold per replica-month through the market
  with a fixed 50/50 fee the Stripe gate enforces. KubeVirt and CDI from
  pinned manifests, disks on the hosting box in `losos-vm-local` (Retain).
  Offered only to a box with `sharingMyStorage` (lososd's `/api/vms`, the
  SPA's `/machines`); uploads stream through lososd to the edge's merged
  transfer router, outside the guard's body limit.
- `flake/fast-build.nix`: every Rust binary links with mold and compiles
  through ccache and sccache, in shells and in `nix build` alike.
- CI is `.github/workflows/ci.yml`, kept in step with `devenv.nix` by hand.
  `fmt-bot` pushes `cargo fmt` to main, `iso` imports `losos-ctl` as an
  artifact and boots the ISO under OVMF and SeaBIOS, tags run
  `release-media`. `history-scrub.yml` rewrites the history after every push
  to main (rules in `.github/scripts/scrub-message.py`), pushing branches
  with the `HISTORY_SCRUB_DEPLOY_KEY` deploy key and tags with the CI token.
  The VM tests are not in CI; there is no KVM there.

## Rules

1. Anything that must survive a reboot goes into
   `environment.persistence."/persist".directories`, or it is gone next boot.
   `/etc/rancher` stays listed: the k3s agent's node password lives there and
   the server rejects a node that regenerates it.
2. Run `devenv test` before trusting a change. `doCheck` is off in both
   crates, so a green `nix build` ran no tests.
3. Bump `flake.lock`, the rev in `devenv.yaml` and `devenv.lock` together.
   `check-pins` and CI's `pins` job fail when they differ.
4. Keep `--impure` on both rebuild paths (`system.autoUpgrade.flags`,
   `supervisor.rs`) and the `#install` fragment on the flake URI. Pure
   evaluation cannot see `/etc/nixos/modules/install-target.nix`, and without
   the fragment `nixos-rebuild` asks for `nixosConfigurations.$(hostname)`,
   which does not exist.
5. Keep the `rp_filter = 2`, non-zero `user.max_user_namespaces` and
   executable `/tmp`. Strict rp_filter drops the multicast replies that make
   the box reachable and breaks Calico, zero namespaces stops both kubelets,
   noexec `/tmp` stops nix builds. `tests/hardening.nix` asserts all three.
6. Gate `--disable`, `--flannel-backend` and `--disable-network-policy` on
   the server role. `k3s agent` crash-loops on an unknown flag.
7. Leave the two Kubernetes instances as two clusters. The box's own apps in
   the edge's cluster would go down for any edge outage spanning the 00:07
   reboot.
8. Never write a `deny <podCidr>` nginx rule. hostNetwork pods arrive from
   `127.0.0.1` or the LAN address.
9. `lanOnly` denies loopback on purpose: rathole's tunnel delivers from
   `127.0.0.1`. Test against lososd's `:8082` or with a LAN source address.
10. Nextcloud trusts the box's own IP through the `X-Losos-Server-Addr`
    header, IP literals only. Nextcloud's `*` matches `[-.a-zA-Z0-9]*`.
11. App tiles link `/nextcloud/index.php/apps/<id>/`. The short form is an
    Apache 404 unless the pod rewrites pretty URLs.
12. Sign-in sends the password to Nextcloud over loopback and keeps no hash.
    Keep the spare-key path in the unlock dialog, never `curl -u`, and let
    `signin::validate_candidate` check shape only, or old passwords lock
    their owner out.
13. The keyfile is 4096 hex characters, no NUL, no newline, at
    `/etc/keys/persist-keyfile`. disko reads it through
    `<(echo -n "$(cat FILE)")`, the initrd and cryptenroll read it raw, and
    `ProtectHome=true` hides `/root` and `/home` from lososd.
14. `losos-ctl grow` is correct only in the order lvextend with `+`,
    `cryptsetup resize`, `resize2fs`. Any binary lososd shells out to
    (lvm2, cryptsetup, e2fsprogs, curl) must be on its unit `path`; `path`
    replaces PATH.
15. Keep `start_supervisor`'s re-attach in `supervisor.rs`. `nixos-rebuild
    switch` restarts lososd mid-rebuild and a rebuild would otherwise read
    `building` forever.
16. `losos.admin.tokenFile` is written by lososd on first start. Do not
    declare it from nix.
17. The compute window's time zone travels with its bounds and the edge
    evaluates `now` per node in `modules/edge.nix`. Hoisting it enforced a
    Berlin window in UTC and slid an hour at each DST change.
18. `tenantsJson` in `modules/edge.nix` must keep the `market` key or every
    trade 403s. The `/api/market*` relay answers 200 `{available:false}`
    when the market is off; the SPA latches a 404 as "not served". Never add
    an operation that forwards arbitrary Stripe calls through the gate.
19. In `flake/fast-build.nix` the stdenv is mold first with ccache around it,
    and `RUSTC_WRAPPER` is a script not named `sccache`. Either way round
    silently caches nothing.
20. Don't add `restrict=on` to the ISO boot test's netdev; QEMU then offers
    no DNS server and the test never sees its query. The ISO's UEFI loader
    is a UKI (`modules/secure-boot.nix`), signed after `nix build` by
    `losos-sign-iso` with the `SECURE_BOOT_DB_KEY` secret, never inside a
    derivation: a key in the store would reach the public cache. The
    production key is made only by `provisioning/secure-boot/keygen.sh` on
    the owner's machine; `tests/secure-boot.nix` uses a throwaway one.
21. `system.stateVersion = "26.11"` is set once. `result` is never committed.
22. Docs go in `handbook/`; there is no wiki. The In depth pages, TPM and
    the security model have Slovak and German copies under `handbook/i18n/`:
    change all three together.
23. No `Co-Authored-By`, `Generated with` or session links in commits, PRs
    or branch names. The commit-msg hook refuses them and the scrub bot
    rewrites what gets through. After a rewrite, `git pull --rebase`.
24. Match `.github/workflows/ci.yml` and `devenv.nix` by hand when adding a
    gate. Neither calls the other.
25. PR descriptions follow `.github/pull_request_template.md`, every section
    filled: Before / After prose, a screenshots table (one Before and one
    After per screen, same window size and state) for anything a person can
    see, or "No visible change." and why; under Tested, name what did not
    run (a session has no KVM, so the VM tests are usually "not run").
    Screenshots are committed under `.github/screenshots/<topic>/` and
    embedded by a short-SHA raw URL; an attribution block, when one is
    required, goes above the template as the first two lines.

## Rationalizations to reject

| Excuse | Why it fails |
|---|---|
| "`allow 127.0.0.1` fixes the 403 in the VM test" | It opens the admin surface to the internet whenever the proxy tunnel is on. |
| "Strict rp_filter is on every hardening checklist" | It drops the mDNS replies of an SSH-less box and Calico stops working. |
| "One cluster is simpler" | Midnight reboots plus an edge outage take the owner's Nextcloud down. |
| "Raw random bytes are a stronger key" | disko's command substitution drops NULs; the volume is formatted with one key and unlocked with another. |
| "Pure evaluation is cleaner, drop `--impure`" | A `github:` upgrade flips a keyfile box to TPM, forgets its drives and resets every setting. |
| "`nix build` passed" | It compiled. The suites run in `devenv test` and CI only. |
| "A `192.168.*` wildcard is simpler than a header" | It trusts `192.168.attacker.example` too. |
| "A pod-CIDR deny adds depth" | There is no pod CIDR to match; the pods share the host's netns. |
| "Attribution trailers are standard practice" | Not here. The hook refuses them and the bot rewrites history to remove them. |

## Before you finish

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
screens, the two app themes, the handbook and docs pages — one Before and one
After per screen, taken at the same window size and in the same state so
the only difference is the change. A change with nothing visible says
"No visible change." under that heading and why, instead of the table.
Commit the PNGs under `.github/screenshots/<topic>/{before,after}/` and
embed them in the table by
`raw.githubusercontent.com/dasmatus/losos/<short-sha>/…` URLs, pinned to a
commit so they outlive the branch; images and Markdown are the only
non-code files that go into the repository. An attribution block, when one
is required, goes above the template as the first two lines. Under Tested, say what did not run and why — a session
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
both get logos drawn from `admin-ui/themes/brand/` (`marks.py` wraps
`salmon.png`, a live coho salmon cut out of a NOAA Fisheries photo, into
the SVGs and copies it to the SPA and the handbook; every PNG and the
`.ico` is rendered from those at build time). `plate.png`, the plate of
salmon from Matus's own photo, is the Halloween logo. The SPA and the
handbook each swap it in on 31 October by the viewer's local date, in their
own `src/lib/logo.ts`. The apps' marks never change. `brand/CREDITS.md`
records both sources. A new picture needs a licence that allows use in a
logo, and Pixabay's does not. The old
pixel-art fish was dropped over a copyright worry and does not go back in. The rest of the upstream marketing goes by config (no skeleton
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

**Edge federation and the box's path rule** (`handbook/docs/in-depth/edge-federation.md`,
`backend-registrar/src/relay.rs`, `modules/edge-gateway.nix`,
`modules/proxy.nix`): a user-hosted edge (a *spoke*) relays its boxes to an
official edge (a *hub*) with `POST /relay`; the hub lists the spoke as a
tenant with a `relayZone` and routes `<spoke>.<box>` with the spoke's token.
A spoke with `losos.edge.lan.openEnrolment` enrols unknown boxes
trust-on-first-use under `/var/lib/losos-registrar/enrolled/`; the option
asserts `lan.advertise`, never set it on a VPS. `losos.edge.gateway.enable`
is the preset the `losos-disk-edge-qcow2` image boots. On the box, `lososd`
picks the path (LAN edge, else the configured one, else none) and drives
`losos-rathole-client` + `losos-registrar-announce` through
`/run/losos/edge-path.env` and the `/run/losos/edge-none` marker, so a box
with no edge in reach runs no tunnel; both files are on `/run`, so the
units dial the configured edge until the first scan, which is what
`tests/edge-vm.nix` (no lososd) relies on. `tests/edge-federation.nix` is the
three-VM acceptance test; it is not run by CI.

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
  `backend-registrar/src/market.rs`, `handbook/docs/in-depth/market.md`) answers 503 unless
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
  {keygen,sign,show,verify}` are the primitives (`backend-registrar/src/identity.rs`,
  used by `tests/edge-lan.nix`, which builds its own root at build time
  rather than trusting the shipped file), `losos.edge.identity.{keyFile,certFile}`
  the edge side. The ceremony an operator runs is `losos-registrar provision
  {whoami,root-keygen,publish,edge,verify}` (`backend-registrar/src/provision.rs`),
  **on the operator's own machine**: every verb that makes or uses the root
  key first signs the operator in with GitHub's OAuth device flow (public
  client id only, no secret) and refuses unless the account's *numeric id*
  is in `backend-registrar/operators.json`, which is compiled into the
  binary. `provision edge` reads the edge's own public key from
  `GET /identity/public-key` (the registrar makes the key on its first
  start; the private half never leaves the edge), signs the certificate
  and pushes it to `POST /identity/cert` with the sign-in's GitHub token,
  which the edge resolves against the same compiled-in allowlist before it
  installs anything (`server::identity_push`), then runs the four checks;
  no SSH; `root-keygen --publish` / `publish` open
  the PR that fills `keys/official-edge-root.pub` with the same sign-in
  (`public_repo`). `tests/provision.rs` runs all of it against a fake GitHub
  and a real registrar. The gate decides whom the tooling serves; the root
  key stays the whole secret. Forgejo Actions is **off** in both Forgejo
  modes and the box runs no runner: the earlier from-the-box ceremony
  (Actions workflows plus `modules/git-runner.nix`) is gone, and
  `provisioning/edge-identity/README.md` is the operator runbook. The
  operator's machine has no Nix store, so `.#losos-registrar-static`
  (`pkgsStatic`, musl, same `cargoHash`) is the binary for it: CI's
  `publish-tool` job pushes it to GHCR as `images:<channel>-x86_64`
  (`losos-registrar` + `SHA256SUMS`, one layer each) and the proxy serves
  it at `/updates/<channel>/x86_64/<file>`, the one route of the LosOS
  Desktop proxy that hands out plain files; the runbook has the curl line.
- **`system.stateVersion = "26.11"` is set-once** — matches the nixos-unstable
  this flake tracks; don't change it.
- **The `result` symlink is a `nix build` artifact** (pointing into
  `/nix/store`), gitignored, never committed.
- [ ] `devenv test` passed, not only `nix build`?
- [ ] New persistent path listed in `impermanence.nix`?
- [ ] New option declared under `options.losos`, default in `defaults.nix`?
- [ ] New binary lososd calls added to the unit `path`?
- [ ] VM tests run locally if the installer or control plane changed?
- [ ] CI and `devenv.nix` still in step; the three nixpkgs pins agree?
- [ ] Docs changed in `handbook/`, in every language the page has?
- [ ] Commit message carries no trailer and no session link?
- [ ] PR body follows the template; screenshots, or "No visible change." and why?
