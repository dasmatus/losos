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

Same for `edge/registrar/`. Both crates must be clean under clippy and
rustfmt. The Nix packages set `doCheck = false`, so `nix build` runs no
tests.

On pushes to `main`, a CI job runs `cargo fmt` and commits the result. Clippy
is not auto-fixed.

Another job then rewrites the history so that no commit credits Claude or
links a claude.ai session, and force-pushes the branches and tags that
moved. If `main` changed under you, `git pull --rebase` picks up the
rewritten commits cleanly. A plain `git pull` would merge the two histories.

## Admin UI

From `admin-ui/app/`:

```sh
npm run dev           # set LOSOS_API_ORIGIN to a real box
npm run typecheck
npm run test:browser
```

`nix build .#losos-admin-ui` runs `npm ci` then `tsc --noEmit && vite build`,
so the typecheck is part of the package. `npm run test:browser` is the
playwright check that `.#checks.x86_64-linux.losos-admin-ui` wraps.

`tests/advanced.browser.mjs` renders every option the box declares. Outside
nix it reads `tests/fixtures/options.json`, a copy of the document
`nix/options-doc.nix` generates; refresh it after changing
`modules/options.nix` with

```sh
nix build --no-link --print-out-paths .#checks.x86_64-linux.losos-options-doc
cp "$(nix build --no-link --print-out-paths .#checks.x86_64-linux.losos-options-doc)" \
   admin-ui/app/tests/fixtures/options.json
```

`nix flake check` and `nix/tests/admin-ui.nix` use a freshly generated
document, so a stale fixture affects only a run on a dev machine.

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

One flake check is not a VM. `losos-invariants` (`nix/tests/invariants.nix`)
evaluates the published `install` configuration and asserts the option values
the appliance cannot afford to lose to a drifting default: garbage
collection, the boot-menu cap, the unlock mode, the `#install` fragment.
`nix flake check --no-build` runs it, and so does `nix/check-flake.sh`, the
same evaluation a few attributes per process, which CI's eval job and
`check-flake` use: one process evaluating every VM test passes 13 GiB, more
than a GitHub runner has. Either way it fails at no build cost.

## Pull requests

The template in `.github/pull_request_template.md` shapes the description:
Before / After prose, a screenshots table, How, Tested, notes for the
reviewer. Keep every section.

Screenshots are required for any change a person can see: the admin UI and
the wizard, the installer and the tty1 banner, LosOS Cloud and the Forgejo
theme, the handbook and docs pages. Take a Before and an After per screen at
the same window size and in the same state, so the only difference is the
change, and drag them into the table. A change with nothing visible says
"No visible change." under that heading, and why.

Under Tested, list what ran and what did not. The VM tests need KVM, so
say so when they did not run.

## Lock files and hashes

- `flake.lock` pins the nixpkgs that builds the appliance; `devenv.lock` the
  one that lints and tests it. Update them together.
- After changing `Cargo.lock` or `admin-ui/app/package-lock.json`, update
  `cargoHash` or `npmDepsHash` in `nix/packages.nix`. The admin UI hash is
  also in `nix/tests/admin-ui.nix`.

## Handbook

The handbook is `handbook/` in the repository.
[For developers](../handbook/docs/reference/for-developers.md) says how to
preview and build it, take its pictures and translate a page. Every push to
`main` publishes it, and every box builds its own copy. This page and
[CI and releases](ci-and-releases.md) stay out of it: they are notes for
the people working on LosOS, not for owners.

## Gotchas that bite silently

Each of these was paid for once. `.claude/CLAUDE.md` carries the one-line rule, and
this is the reasoning behind it.

- **Three hardening settings are deliberately *not* applied**, and all three
  are on every checklist you will be tempted to copy from. `rp_filter` is `2`
  (loose), not `1`. Strict mode drops the multicast replies that make an
  SSH-less box reachable, and Calico does not work under it either.
  `user.max_user_namespaces` stays non-zero, because zero stops both kubelets
  and containerd. `/tmp` is not `noexec`, because nix builds execute there and
  the nightly unattended rebuild is the box's only way to repair itself.
  `nix/tests/hardening.nix` asserts all three absent, so "fixing" one turns a test
  red instead of bricking a box.
- **`fileSystems."/tmp"` does not mount anything.** NixOS masks `tmp.mount`
  unless `boot.tmp.useTmpfs` is set. The declaration produces no mount and no
  error, and `/tmp` inherits the root filesystem's options. Finding this cost
  one VM-test round trip.
- **The allocator preload file is `/etc/ld-nix.so.preload`**, read by NixOS's
  patched loader. The glibc-standard `/etc/ld.so.preload` never exists here.
  An assertion on the standard path passes when the allocator is off and
  fails when it is on.
- **`losos-ctl grow` runs three commands, and only their order matters.**
  `resize2fs` asks the LUKS *mapping* for its size. Run before `cryptsetup
  resize`, it reads the pre-grow size, prints "Nothing to do!" and exits 0.
  And `lvextend -l 77` without the `+` is an absolute extent count, which
  *shrinks* a larger volume and destroys a mounted ext4 on it.
  - `lososd` needs lvm2, cryptsetup and e2fsprogs on its unit `path`, or the
    first step fails with "No such file or directory". The same goes for
    `curl`, which `GET /api/apps/search` shells out to. `path` **replaces**
    PATH, so no binary is on it by accident, and a missing one reaches the
    owner as a feature that never works and never says why.
  - `nixos-rebuild` is on that list for the same reason. systemd-run
    resolves a bare command against the *caller's* PATH, so without it every
    Apply failed before the rebuild unit existed.
  - lososd must not get `ProcSubset=pid`. It hides `/proc/devices`, and vgs
    exits 4 without it.
  - `nix/tests/invariants.nix` pins both of the last two.
- **The LUKS keyfile is hex text and must stay NUL-free.** disko hands
  `passwordFile` to `luksFormat` as `<(echo -n "$(cat FILE)")`, a command
  substitution that drops NUL bytes and a trailing newline. The initrd
  (`/crypto_keyfile.bin`) and `systemd-cryptenroll --unlock-key-file` read
  the file raw. The installer used to write 4096 random bytes, so on nearly
  every install the volume was formatted with one key and unlocked with
  another. Nothing hit that failure until `nix/tests/tpm.nix` enrolled the chip.
  `ensure_keyfile` now writes 2048 random bytes as 4096 hex characters. Don't
  "harden" it back to raw bytes, and don't put a newline in it.
- **The LUKS key file must not live under `/root` or `/home`.** `lososd` runs
  with `ProtectHome=true`, on purpose, so a compromised request handler
  cannot read either data domain. That hides `/root` too. `cryptsetup resize`
  then fails with "Failed to open key file", *after* `lvextend` has already
  grown the volume. `/etc/keys/persist-keyfile` is the path everything agrees
  on. `disko.nix` formats from it, the installer keeps it inside `/persist`
  on both unlock paths, `daemon.nix` passes it as `LOSOS_LUKS_KEYFILE`, and
  `nix/tests/resize.nix` uses it instead of a fixture of its own, so the four
  cannot drift apart unnoticed.
- **`lososd` is restarted mid-rebuild.** `nixos-rebuild switch` restarts the
  changed `lososd.service` and kills the watcher thread that was tracking the
  rebuild. `Daemon.startSupervisor` re-attaches on daemon startup: if
  `state.json` says `Building`, it spawns a fresh watcher for
  `losos-rebuild-<job>`. Without this the rebuild reads `building` forever.
  Don't drop the re-attach.
- **The admin routes deny loopback on purpose** (`lanOnly` in
  `containers.nix`). Master-proxy tunnel traffic reaches the front vhost
  *from 127.0.0.1* (rathole's `local_addr`), so the LAN-only guard on `/`,
  `/assets/`, `/setup/` and `/api/` must not allow loopback. A
  `curl localhost/` on the box, or in a VM test, gets 403 by design.
  "Fixing" it with `allow 127.0.0.1` exposes the whole admin interface to the
  internet whenever `losos.proxy.enable` is on. Test against lososd's :8082
  directly, or curl with a LAN source address.
- **Sign-in is LosOS Login (Authelia), over a people file lososd writes.**
  People sign in at `https://<host>/auth/` (`modules/identity.nix`). nginx asks Authelia
  before every `/api/` request (`auth_request`) and passes `Remote-User`,
  which lososd believes only from loopback and only with
  `LOSOS_IDENTITY=authelia`. lososd is the only writer of
  `/var/lib/losos-identity/users.yml` (`backend/src/people.rs`, argon2id
  hashes), and Authelia must start only after that file exists: its file
  backend refuses an empty users map, and with no file it writes an example
  account of its own. A request with `Authorization: Bearer` skips Authelia
  in the authz location and lososd checks the token itself, which is what
  keeps the spare key working while Authelia is down; keep that bypass, and
  keep the spare-key path in the sign-in dialog. Who may call what is
  `backend/sections.json`, read by lososd (`backend/src/sections.rs`) and by
  nginx; a route missing there is admin only. `nix/tests/identity.nix` boots it.
  `backend/src/signin.rs` now only holds the LosOS Cloud login helpers that
  device setup (`losos cloud setup`, `losos git setup`) and the pin picker
  use: keep the password in a Basic header over loopback, never a `curl -u`
  (it would be in an argv), and let `signin::validate_candidate` check shape
  only.
- **Hand-written widgets run in `/widget-frame/`, never in the admin page.**
  `<iframe sandbox="allow-scripts" src="/widget-frame/">` draws the owner's
  HTML and script (`backend/src/look.rs`, `GET/POST /api/look*`). The frame
  lives in `admin-ui/app/src/widgets/hand-frame.tsx` and its page in
  `admin-ui/app/public/widget-frame/`. Two things carry the weight. There is
  no `allow-same-origin`, so the frame is an opaque origin with no token and
  no API. And the `~^/widget-frame/` arm of nginx's header map in
  `containers.nix` is the one path served under a permissive CSP. An
  `<iframe srcdoc>` or a `blob:` document would inherit the admin page's
  strict policy and refuse the inline script just the same, so don't
  "simplify" the frame into one. `nix/tests/front-vhost.nix` asserts the arm and
  that the strict policy still covers everything else.
- **The admin token is a credential, not a NixOS file.** The boot unit of
  `modules/credentials.nix` makes it at first boot (64 hex characters) and
  seals it as `admin-token` in `/etc/credstore.encrypted`; lososd reads it
  from `$CREDENTIALS_DIRECTORY/admin-token`. `losos.admin.tokenFile` is only
  where an older build kept it, adopted once and deleted. Don't try to
  declare it as a store path. Every other secret of the box is a credential
  the same way: a module declares it under `losos.credentials.entries`,
  names the units that read it, and lists any plaintext file an older build
  used under `legacy`.
- **`/etc/rancher` must stay persisted** (`impermanence.nix`). `/var` covers
  most of both Kubernetes instances' state, but the agent writes
  `/etc/rancher/node/password` on its first join, and the server stores a
  hash of it keyed by node name. On a tmpfs root that file is regenerated
  every boot, and the server then refuses the rejoin ("Node password
  rejected"). Drop the line and the box falls out of the mesh on the first
  reboot after enrolling, without a word.
- **`--disable`, `--flannel-backend` and `--disable-network-policy` are
  server-only flags.** `k3s agent` hard-errors on an unknown flag, so passing
  any of them to an agent crash-loops the unit forever, on a box with no
  shell. nixpkgs' own `nixos/tests/rancher/multi-node.nix` gives its server
  nodes `disable` and its agent node neither, and rke2's `role` description
  says the same. Gate every server-only flag on the role.
- **The two Kubernetes instances are separate clusters on purpose.**
  `services.k3s` (role `server`) runs *this box's* Forgejo and shared apps.
  `services.rke2` (role `agent`) joins the edge's mesh. An agent's kubelet
  cannot start while its server is unreachable, and `midnight-reboot.timer`
  fires unconditionally at 00:07. Putting the box's own services in the
  edge's cluster would take them down for any outage spanning midnight, so
  don't "simplify" this into one cluster. It is rke2 rather than a second k3s
  because nixpkgs builds both from one generator parameterised by name, so
  their state directories (`/var/lib/rancher/{k3s,rke2}`) and unit names do
  not collide. There is no `dataDir` option that would make a second k3s
  work.
- **`hostNetwork` pods have no distinguishable source address.** The local
  cluster runs `--flannel-backend=none`, so its pods share the host's network
  namespace and nginx sees them as `127.0.0.1` or the LAN IP. The old nspawn
  design gave containers their own subnet, which the `lanOnly` guard denied
  first. That layer is gone. Never write a `deny <podCidr>` rule, because it
  cannot match.
- **The edge enforces the compute window, in the appliance's time zone.**
  Only the edge can write the NoSchedule taint (NodeRestriction lets no one
  else), so the comparison runs on a machine that is not the owner's: an edge
  VPS on UTC against an appliance shipping `Europe/Berlin`. The zone
  therefore travels with the two bounds (`--window-tz`, `ComputeWindow.tz`),
  and the edge evaluates each node with `TZ="$tz" date`. Before that it read
  its own clock. A 23:00 to 07:00 window entered in Berlin was enforced from
  00:00 to 08:00 in winter and 01:00 to 09:00 in summer, sliding an hour at
  each DST change. That handed strangers' pods the first hours of the
  owner's working day, the exact thing the feature exists to prevent. Don't
  hoist `now` back out of the per-node loop in `modules/edge.nix`. It is per
  node because the zone is.
- **The market is the registrar's third opt-in.** `/market/*` (Stripe
  Connect, `edge/registrar/src/market.rs`, [Market](../handbook/docs/in-depth/market.md)) answers 503
  unless `losos.edge.market.enable` is set, and 403 unless
  `losos.edge.tenants.<id>.market` is. `modules/edge.nix`'s `tenantsJson`
  hardcodes its attributes, so the `market` key must stay listed there, or
  every trade gets a 403 that nobody explains.
  - A paid order is an entitlement. Storage orders get a namespace and a PVC
    on the mesh; creating them is idempotent, a 409 counts as done, and
    nothing is ever deleted. Compute is a ledger credit only.
  - Listing and ordering depend on what the seller's node already shares
    (`Sharing`, from the registry's compute windows). The market is a way to
    be paid for the mesh, not a second product.
  - The admin UI reaches it through lososd's `/api/market*` relay
    (`backend/src/market.rs`). The relay answers 200 `{available:false}`
    rather than 404 when the market is off, because the SPA latches a 404 as
    "route not served".
  - Only the `losos-stripe-gate` unit (`losos-registrar stripe-gate`) holds
    the Stripe key. It gets the sealed blobs
    (`losos.edge.market.{stripeSecretKey,webhookSecret}Sealed`) through
    `LoadCredentialEncrypted=`. The registrar talks to it over
    `/run/losos-stripe-gate/gate.sock` and never sees the key. Don't add an
    operation that forwards arbitrary Stripe calls, and keep the gate's
    request checks (destination, currency, fee ceiling, session lifetime,
    https endpoint) tighter than whatever the registrar happens to send. A
    missing blob skips the gate (`ConditionPathExists`), and `/market/*`
    answers 503.
  - Onboarding writes the box UUID onto the Stripe account. It is a SHA-256
    derivative of the recovery code (`backend/src/boxid.rs`), never the code
    itself.
- **Any edge opens sharing; only an *official* edge opens the market.**
  lososd scans for edges (`backend/src/edge.rs`: DNS-SD `_losos-edge._tcp`
  plus `losos.proxy.registrarUrl`, probed on `/health`). With none in reach
  it refuses to turn `sharingMyStorage` or `cluster.enable` on (409
  `edgeRequired`). The refusal applies only to turning things on, so a box
  can always leave. Joining the mesh also needs the tunnel on and a name
  other than the stock `mattbox`, the two things `modules/cluster.nix`
  asserts; lososd refuses the apply first (409 `meshNotReady`) and the Mesh
  pane greys the switch with the reason.
  - On top of that, lososd challenges each answering edge
    (`GET /identity?nonce=`). An edge counts as official only if its
    certificate is signed by the LosOS root key in
    `nix/keys/official-edge-root.pub` (`losos.proxy.officialRootKeyFile`), names
    that URL and has not expired, and the certificate's key signed the
    nonce. The market relay refuses everything else (`{available:false,
    reason:"noOfficialEdge"}`, 409 `officialEdgeRequired`).
  - The committed key file is **empty on purpose** until the owner writes
    the public key into it. No root means nothing is official and the market
    is off. The private key lives offline with the owner and is never
    committed.
  - `losos-registrar identity {keygen,sign,show,verify}` are the low-level
    commands (`edge/registrar/src/identity.rs`). `nix/tests/edge-lan.nix`
    uses them to build its own root at build time rather than trusting the
    shipped file. `losos.edge.identity.{keyFile,certFile}` are the edge
    side.
  - The ceremony an operator runs is `losos-registrar provision
    {whoami,root-keygen,publish,edge,verify}`
    (`edge/registrar/src/provision.rs`), **on the operator's own
    machine**. Every verb that makes or uses the root key first signs the
    operator in with GitHub's OAuth device flow (public client id only, no
    secret). It refuses unless the account's *numeric id* is in
    `edge/registrar/operators.json`, which is compiled into the binary.
  - `provision edge` reads the edge's own public key from
    `GET /identity/public-key`; the registrar makes that key on its first
    start, and the private half never leaves the edge. The tool signs the
    certificate, pushes it to `POST /identity/cert` with the sign-in's
    GitHub token, and runs the four checks. The edge resolves the token
    against the same compiled-in allowlist before it installs anything
    (`server::identity_push`). There is no SSH. `root-keygen --publish` and
    `publish` open the PR that fills `nix/keys/official-edge-root.pub`, with the
    same sign-in (`public_repo`). `tests/provision.rs` runs all of it
    against a fake GitHub and a real registrar.
  - The allowlist decides whom the tooling serves. The root key is still the
    whole secret.
  - Forgejo Actions is **off** in both Forgejo modes and the box runs no
    runner. The earlier ceremony from the box (Actions workflows plus
    `modules/git-runner.nix`) is gone, and
    `docs/provisioning/edge-identity/README.md` is the operator runbook.
  - The operator's machine has no Nix store, so `.#losos-registrar-static`
    (`pkgsStatic`, musl, same `cargoHash`) is the binary for it. CI's
    `publish-tool` job pushes it to GHCR as `images:<channel>-x86_64`
    (`losos-registrar` and `SHA256SUMS`, one layer each), and the proxy
    serves it at `/updates/<channel>/x86_64/<file>`. That is the one route
    of the LosOS Desktop proxy that hands out plain files. The runbook has
    the curl line.
- **`system.stateVersion = "26.11"` is set once.** It matches the
  nixos-unstable this flake tracks. Don't change it.
- **The `result` symlink is a `nix build` artifact** pointing into
  `/nix/store`. It is gitignored and never committed.
