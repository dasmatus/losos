# Development

## Shell

```sh
devenv shell        # or: direnv allow
devenv test         # pin checks, lint, rust tests, flake eval (no builds)
```

`nix develop` gives the toolchain only, for people without devenv.

| Script                         | Does |
| ------------------------------ | ---- |
| `fmt` · `lint` · `test-rust`   | Format, lint and test both Rust crates |
| `check-flake` · `check-eval`   | Evaluate the flake; force the full module merge |
| `check-pins`                   | Fail if `flake.lock` and `devenv.lock` pin different nixpkgs |
| `build-pkgs` · `build-iso`     | Build the packages; build the installer ISO |
| `build-images` · `build-media` | OCI images; demo QCOW2 and closure ISO (large, opt-in) |
| `vm-tests`                     | All NixOS VM tests (needs `/dev/kvm`) |

`devenv test` does not build packages or the installer ISO, or run VM tests.
Those are separate opt-in scripts.

## Rust

```sh
cargo test   --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```

Same for `backend-registrar/`. Both crates must be clippy-clean and
rustfmt-clean. The Nix packages set `doCheck = false`, so `nix build` does
not run tests.

On pushes to `main`, a CI job runs `cargo fmt` and commits the result. Clippy
is not auto-fixed.

Another job then rewrites the history so that no commit credits Claude or
links a claude.ai session, and force-pushes the branches and tags that
moved. If `main` changed under you, `git pull --rebase` picks the rewritten
commits up cleanly; a plain `git pull` would merge the two histories.

## Admin UI

From `admin-ui/app/`:

```sh
npm run dev           # set LOSOS_API_ORIGIN to a real box
npm run typecheck
npm run test:browser
```

`nix build .#losos-admin-ui` runs `npm ci` then `tsc --noEmit && vite build`,
so the typecheck is part of the package; `npm run test:browser` is the
playwright check that `.#checks.x86_64-linux.losos-admin-ui` wraps.

`tests/advanced.browser.mjs` renders every option the box declares. Outside
nix it reads `tests/fixtures/options.json`, a copy of the document
`flake/options-doc.nix` generates; refresh it after changing
`modules/options.nix` with

```sh
nix build --no-link --print-out-paths .#checks.x86_64-linux.losos-options-doc
cp "$(nix build --no-link --print-out-paths .#checks.x86_64-linux.losos-options-doc)" \
   admin-ui/app/tests/fixtures/options.json
```

(`nix flake check` and `tests/admin-ui.nix` use the freshly generated one,
so a stale fixture only affects a run on a dev machine.)

## VM tests

CI cannot run these (no KVM on hosted runners). Run them locally before
merging changes to the modules, daemon or installer:

```sh
devenv shell vm-tests
nix build .#checks.x86_64-linux.losos-admin-daemon
nix build .#checks.x86_64-linux.losos-install
```

Also build the system closure locally:

```sh
nix build .#nixosConfigurations.install.config.system.build.toplevel
```

One flake check is not a VM: `losos-invariants` (`tests/invariants.nix`)
evaluates the published `install` configuration and asserts the option values
the appliance cannot afford to lose by a default drifting (garbage collection,
the boot-menu cap, the unlock mode, the `#install` fragment). `nix flake check
--no-build` runs it, so CI's eval job fails on it at zero build cost.

## Pull requests

The template in `.github/pull_request_template.md` fills the description:
Before / After prose, a screenshots table, How, Tested, notes for the
reviewer. Keep every section.

Screenshots are required for any change a person can see: the admin UI and
the wizard, the installer and the tty1 banner, the Nextcloud and Forgejo
themes, the wiki and docs pages. Take a Before and an After per screen at
the same window size and in the same state, so the only difference is the
change, and drag them into the table. A change with nothing visible says
"No visible change." under that heading, and why.

Under Tested, list what ran and what did not. The VM tests need KVM; if
they were not run, say so.

## Lock files and hashes

- `flake.lock` pins the nixpkgs that builds the appliance; `devenv.lock` the
  one that lints and tests it. Update them together.
- After changing `Cargo.lock` or `admin-ui/app/package-lock.json`, update
  `cargoHash`/`npmDepsHash` in `flake/packages.nix` (the admin UI hash is
  also in `tests/admin-ui.nix`).

## Wiki

This wiki is generated from `wiki/` in the repository. Edit pages there; the
`wiki` workflow publishes them on push to `main`. Edits made in the GitHub
wiki editor are overwritten.

## Gotchas that bite silently

Each of these was paid for once. `CLAUDE.md` carries the one-line rule; this
is the reasoning behind it.

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
  {keygen,sign,show,verify}` are the primitives (`backend-registrar/src/identity.rs`,
  used by `tests/edge-lan.nix`, which builds its own root at build time
  rather than trusting the shipped file), `losos.edge.identity.{keyFile,certFile}`
  the edge side. The ceremony an operator runs is `losos-registrar provision
  {whoami,root-keygen,publish,edge,verify}` (`backend-registrar/src/provision.rs`),
  **on the operator's own machine**: every verb that makes or uses the root
  key first signs the operator in with GitHub's OAuth device flow (public
  client id only, no secret) and refuses unless the account's *numeric id*
  is in `backend-registrar/operators.json`, which is compiled into the
  binary. `provision edge` makes the edge key in memory, signs its
  certificate, ships both over one SSH session on stdin (the remote script
  frames them with `read -r`; nothing secret in argv or on the operator's
  disk) and runs the four checks; `root-keygen --publish` / `publish` open
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
