# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Context

`losos` is a stateless NixOS appliance for repurposed mini-PCs: tmpfs root,
encrypted `/persist` bind-mounted back by impermanence, LosOS Cloud for the
owner, spare disk and CPU lent to a mesh, no SSH and no shell. The owner
reaches it only through web UIs and one admin endpoint, so a bad nightly
upgrade has no one to notice it. Docs live in the handbook, `handbook/`,
published to losos.dasmat.us on push to `main` and served by every box at
`/handbook/`. Its In depth chapter (`handbook/docs/in-depth/architecture.md`)
and the developer notes kept out of it (`docs/development.md`,
`docs/ci-and-releases.md`) carry the long form of what this file compresses.

## Commands

```sh
devenv shell                 # or: direnv allow. Scripts: fmt, lint, lint-nix, lint-wasm,
                             # test-rust, test-tui, check-flake, check-eval, check-pins,
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
nix build .#checks.x86_64-linux.losos-iso-install   # the real installer offline, then boot the disk
nix build .#checks.x86_64-linux.losos-network-unlock # --tang: TPM2 + Tang, recovery code
nix build .#checks.x86_64-linux.losos-identity      # LosOS Login (Authelia), sections
npm run dev | typecheck | test:browser              # in admin-ui/app
cargo test --manifest-path admin-ui/tui/Cargo.toml  # losos, the terminal client
```

Eleven Rust crates: `backend/` (lososd + losos-ctl), `backend/access`
(the terminal envelope, its own workspace, a path dependency of lososd, the
registrar and the client), `backend/mdx` (`losos-mdx`, MDX and Markdown to
sanitized HTML for Office and Notes, its own workspace, also built for the
browser with its `wasm` feature), `backend/plugins` (lososd's WebAssembly plugins,
their SDK and WIT, its own workspace, linted for the host and wasm32), `edge/registrar/`, `edge/vercel/` (outside the
flake), the terminal client `admin-ui/tui`, and Diagrams' `admin-ui/lab/core`,
`admin-ui/lab/virt-rpc` and `admin-ui/lab/render` (built for wasm32), and
the Solana escrow program `edge/contracts/solana` (`cargo build-sbf`, outside
the flake). All eleven must pass clippy with `-D warnings` and rustfmt; the git hooks
in `nix/hooks`, `lint` and CI each check it. Bevy is a wasm32-only dependency of
`render`, so `lint-wasm` and CI's `clippy-wasm` lint its WebGL2 and WebGPU
builds too, one after the other. `nix develop` is a toolchain-only shell for people without devenv.
The git hooks (`nix/hooks`, `docs/development.md`) replay CI's lanes for
the files a change touches: pre-commit runs rustfmt, clippy, the Nix lints,
the pins and hashes, the admin UI typecheck and CI's whole eval job
(`check-flake`); pre-push adds `cargo test`, wasm32 clippy, the Playwright
check and the VM tests the change touches (TCG without KVM), logging each
check under `.git/losos-hooks/logs/`. devenv, `nix develop` and a Claude
Code session (`.claude/settings.json`) set `core.hooksPath`; elsewhere run
`bash nix/hooks/install`. Never commit or push with `--no-verify` or
`LOSOS_HOOKS_SKIP` to get past a red check you caused; skip only a check
that is red on `main` too, and say which in the PR.
Inside the shell `bcd` and `rcd` cd into the two in-flake crates.
`admin-ui/design-system/react` has its own `npm run typecheck` and `npm test`
and is never served or built by nix.

## Layout

- The root is kept short so GitHub shows the README without a long scroll:
  `admin-ui/`, `backend/`, `edge/` (`registrar/`, `vercel/`, `contracts/`),
  `modules/`, `nix/` (packages, images, helpers, `statix.toml`, the VM tests
  in `nix/tests/`, `nix/keys/`, the LAN demo in `nix/demo/`), `handbook/`
  and `docs/` (developer notes, `docs/provisioning/` runbooks, the quickemu
  file in `docs/vm/`). Put new things inside one of them. `modules/` stays
  at the root: every box keeps its own `modules/install-target.nix` and
  `modules/overrides.nix` there (`config_repo::BOX_FILES`).
- `flake.nix` builds two systems from one module set: `iso` (options, disko,
  installer) and `install` (everything). `specialArgs.self` lets
  `defaults.nix` wire `losos-ctl` and `losos-admin-ui` in.
- `modules/options.nix` declares every knob under `options.losos`;
  `defaults.nix` sets them. Modules read `config.losos.X`, never bare
  `config.X`.
- `impermanence.nix` + `disko.nix` + `boot.nix`: tmpfs root, LUKS ext4
  `/persist` formatted unattended from `/etc/keys/persist-keyfile`. Unlock is
  TPM2 with no PCR binding by default; without a TPM the keyfile rides in the
  initrd on the ESP. With `losos.tpm.tang` (`--tang URL#THP`) the key is a
  Clevis JWE sealed to the TPM2 AND a Tang server instead
  (`network-unlock.nix`, initrd DHCP), and the recovery code is added as the
  console passphrase. The installer writes `modules/install-target.nix` (the
  box's drives, firmware mode, unlock mode) onto the box; it is not in the
  tree, and `flake.nix` imports the live copy from `/etc/nixos` on rebuilds.
  `nix/tests/iso-install.nix` (CI's `install` job, and pre-push on the same
  lane) runs that installer offline and boots the disk it wrote.
- `configuration.nix`: `notshared` (uid 1000, the owner's apps and LosOS Cloud name) and `shared`
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
- `backend/plugins/` + `backend/src/plugin/`: lososd's work moves into
  WebAssembly components (Wasmtime, contract `backend/plugins/wit/plugin.wit`,
  SDK `losos-plugin`), built for wasm32-unknown-unknown and wrapped by
  `wasm-tools component new` (`.#losos-plugins`). No WASI: every file,
  program, unit and HTTP call is a host import checked against the plugin's
  manifest. lososd loads them only from `losos.backend.plugins` in the
  read-only store and refuses to start on anything else. New lososd routes
  go in a plugin, each with its `sections.json` line; sign-in, sections and
  D-Bus stay in the host. The look, the pins and the statistics
  (`/api/statistics`, Traefik's metrics, a minute ring in its cache) are
  plugins so far;
  `nix/tests/plugins.nix` boots them with a browser that takes the PR's screenshots
  (`nix/tests/screens/`, reusable by any VM test).
- `admin-ui/app`: React 19 + Vite + Tailwind SPA on real paths. Its `dist/`
  is the front nginx vhost's root (`try_files … /index.html`), `/api/*` is
  proxied to lososd. Palette in `src/styles/tokens.css`.
- `admin-ui/app/src/games/` + `components/play-while-waiting.tsx`: the
  long waits (a rebuild, the wizard's two, LosOS Login starting, an app
  installing) offer a random game of five on canvas, a lazy chunk that
  fetches nothing; turn-based only under reduced motion, and the end of
  the wait stops it. `nix/tests/wait-game.nix` plays it during a real
  claim wait and a real rebuild unit, and films it.
- `identity.nix` (`losos.identity.enable`): sign-in is LosOS Login
  (Authelia) on `127.0.0.1:9091`, its page at `https://<host>/auth/` (https
  only), over the people file `/var/lib/losos-identity/users.yml` that
  lososd alone writes (`backend/src/people.rs`, argon2id). nginx asks it
  before every `/api/` request and passes `Remote-User`; a Bearer request
  (the spare key) skips it. `backend/sections.json`, read by
  `backend/src/sections.rs` and nginx, maps each route to a section
  (`losos-<section>` groups, `losos-admins`, `losos-viewers`); a missing
  route is admin only. The claim makes the first person an administrator
  and puts them in `losos-owner`. `nix/tests/identity.nix` boots it.
  LosOS Cloud and LosOS Git take the person from nginx in
  `X-Losos-Cloud-*` headers (`modules/app-sign-in.nix`: LosOS Cloud reads
  them in `backend/src/cloud/auth.rs`, LosOS Git in
  `backend/src/forge/auth.rs`); the owner maps to `notshared`. nginx sets
  the headers on every request to either app, and each app believes them
  only on a Unix socket nginx alone opens (LosOS Git's `127.0.0.1:3000`
  takes tokens and nothing else); drop either and anything on the box can
  name anyone. `nix/tests/app-sign-in.nix` boots LosOS Git on the headers
  and `nix/tests/cloud.nix` LosOS Cloud.
- Terminal access (`docs/terminal-access.md`, `docs/edge-terminal.md`):
  `losos`, a Rust client in `admin-ui/tui` (clap derive, miette reports,
  tracing without time, ratatui) with the web UI's panes, catalogue and
  palette, read from `admin-ui/app` by its `build.rs`. It pins a box's
  self-signed certificate by SHA-256 at login. `.#losos-static` (musl) is
  installed by `install.sh` out of CI's `losos-pages-updates` artifact,
  linked through nightly.link (CI's `tui` and `publish-tool` jobs). Each call is one OpenPGP
  message signed by a key seeded at install (`losos.access.gpgKeys`,
  installer-fixed, never added at runtime) and encrypted to the host's own
  key, carrying a personal access token from the Access tokens pane;
  `backend/access` opens and seals it. A token carries its owner, and
  lososd's `/api/access/call` replays the call against its own loopback API
  with the admin token as `X-Losos-As: <owner>`, never `/api/access` or
  `/api/setup`. An edge
  (`losos.edge.access.gpgKeys`, `edge/registrar/src/operator.rs`) answers
  four paths and seals secrets through `losos-seal-credential` under
  `systemd-run`; its tokens come from `provision token` behind the GitHub
  allowlist. The client keeps keys and tokens in the Secret Service through
  `secret-tool`, never a file; `losos ci setup` pushes a separate CI key with
  `gh`, never an edge-made key. `losos cloud setup` and `losos git setup` mint an
  app password and a LosOS Git token per computer with the owner's password
  (`backend/src/devices.rs`); `losos git credential` is git's helper.
- Calls (`modules/calls.nix`, `losos.calls.enable`, only while chat runs):
  `losos-calls` (`backend/src/calls/`, str0m, ICE-lite, VP8 and Opus) is an
  SFU as `shared` on `/run/losos-calls/http.sock` (nginx's group only),
  `/calls/api/` behind LosOS Login with no spare-key bypass. nginx hands
  the SFU the signed-in person as `Remote-User` (`calls/person.rs`), the
  Matrix id `@<name>:<server>`. Calls are a MatrixRTC application of
  their own, `org.losos.call`, one slot per room (`org.losos.call#room`,
  `src/chat/rtc.ts`): `m.call` is LiveKit-only, and this one is str0m.
  A membership is a sticky `org.matrix.msc4143.rtc.member` event (MSC4354)
  with a delayed leave (MSC4140), both of which only Synapse has; each
  publishes an `org.losos.sfu` transport, and the page always dials its own
  `/calls/api/ws`. The SFU
  owns the call id: `POST /calls/api/calls` hands back the room's live call
  (`created:false`), so two starts never split. Ringing is MSC4075
  (`org.matrix.msc4075.rtc.notification`, the SDK's receiving rules),
  decline MSC4310. New private rooms and DMs carry the open slot; in an
  older room an admin's first call opens it. A start also sends an `m.notice` with a
  `/chat/?join=` link for other Matrix apps, which LosOS Chat hides.
  MatrixRTC's own media keys are off: every frame is AES-GCM sealed by the
  browser's encoded transform (`admin-ui/app/src/calls/e2ee.ts`, worker
  `e2ee-worker.ts`), and its sender keys travel as MatrixRTC per-member
  keys (`io.element.call.encryption_keys` over Olm) to the devices the
  SFU lists in the call, since a membership can sync later than its sender
  joins. The legacy `org.losos.call` timeline event is kept for
  end markers and old history. In Chromium a worker held
  by an `RTCRtpScriptTransform` loses its own `postMessage` and timers, so
  stats go over a MessagePort from the frame path. `--address IP=OFFERED`
  offers a forwarded port's address; `nix/tests/calls.nix` reaches the SFU
  that way and reads its `--tap` (`lososInternal.callsTap`) to prove the
  box holds only ciphertext. Behind an edge (`losos.proxy.enable`) the SFU
  keeps one QUIC link to the edge's relay (`calls/relay.rs`,
  `edge/registrar/src/calls_relay.rs`) and opens no port. The screen is a
  third transceiver (`p<pid>-screen`); the black background is MediaPipe's
  selfie segmenter in the browser before the seal (`calls/background.ts`,
  files under `/chat/calls-ml/`, model from `LOSOS_SEGMENTER_MODEL`).
  Podcast mode composites on the host's canvas and sends WebM to
  `/calls/api/stream`, which ffmpeg passes to RTMP: the one plaintext path,
  marked public in the UI. Guests: only an official edge answers
  `POST /calls/guest-link`; the box serves `/calls/guest/` (its own Vite
  build, `vite.guest.config.ts`) and `/calls/guest/ws` without LosOS Login,
  and the link's `#` secret, which no server sees, keys an ECDH pair per
  guest (`calls/guests.ts`). lososd runs `HTTP_WORKERS` (8) and a 120 s
  `client_request_timeout` because its handlers block; one worker per core
  and the 5 s default 408'd chat on a two-core box.
- LosOS Diagrams at `/diagrams/` (sidebar entry Diagrams; `/lab/` 301s
  there; internal names stay `lab`: the section, `/api/lab/*`, the crates,
  `src/lab/`): a screen of the admin SPA at `/diagrams`, laid out like Mail
  (`src/lab/app.tsx`, a lazy chunk; the shell widens on that route) over a
  Rust core in WebAssembly (`admin-ui/lab/core`, `losos-lab-core`, contract
  in its README). The admin CSP carries `'wasm-unsafe-eval'` for it; the
  hosted copy alone builds `admin-ui/app/diagrams/index.html`
  (`VITE_LAB_HOSTED=1`). Its default view is a JSON Canvas
  1.0 editor (`core/src/canvas/`, `src/lab/canvas/`): files are `.canvas`
  with Advanced Canvas `styleAttributes`, unknown keys kept on save;
  devices are cards with `losos.device`, cables edges with `losos.link`,
  machine cards `losos.machine` over `/api/vms`. It opens and saves in LosOS
  Cloud with `ifMatch`, and Cloud opens `.canvas` in it
  (`src/cloud/open-diagrams.ts`). The box copy has simulated consoles, or libvirt
  guests through `losos-registrar lab` with `losos.lab.libvirt.enable`;
  `admin-ui/lab/engine/build.sh` and `lab.yml` build the hosted copy with
  qemu-wasm guests and deploy it to Vercel when the secrets exist.
  `admin-ui/lab/virt-rpc` is a WebAssembly libvirt client for the helper's
  relay. `admin-ui/lab/render` (`losos-lab-render`) is the GPU canvas: Bevy
  0.19.1 on WebGPU or WebGL2, the two views as a 3D scene under an
  orthographic camera, in one module with the core. The page draws with the
  SVG canvas first, then loads one module (`src/lab/render.ts`), replays the
  core's calls into that module's `Lab` and swaps `bevy-canvas.tsx` in;
  anything that fails leaves it on SVG. No Python in Diagrams.
- `cloud.nix` + `backend/src/cloud/` (`losos-cloud`): LosOS Cloud, the
  owner's files, photos, calendars, contacts and boards, in the browser at
  `/cloud/` (`admin-ui/app/cloud/`) and to the Nextcloud desktop and phone
  apps over the addresses they know (`/status.php`, `/remote.php/`,
  `/ocs/`, `/index.php/`, and `/nextcloud/` for apps set up against the old
  Nextcloud). SQLite plus files under `/var/lib/losos-cloud/<login>/files`,
  a Unix socket only nginx opens. `losos-cloud-from-nextcloud` moves a box
  that ran Nextcloud over once, files, calendars, contacts, boards, app
  passwords and trash, and leaves Nextcloud's leftovers for the erase;
  `nix/tests/cloud-from-nextcloud.nix` boots a real Nextcloud and checks it.
  LosOS Cloud draws the admin SPA's own top bar, sidebar and palette
  (`components/app-shell.tsx`, `src/lib/site.tsx` for links across the two
  routers); `nix/tests/cloud-shell.nix` measures that frame on every page
  and copies 4.5 GiB in over WebDAV. Every auth_request location keeps
  `client_max_body_size 0`: nginx holds the request's length to the
  asking location's limit, and the server's 10m turned every upload over
  10 MB into a 500.
- `containers.nix`, `cluster.nix`: the box's own k3s server runs only the
  apps of its shared side (sharing on and in a mesh); `services.rke2`
  (agent) joins the edge's mesh. nginx holds 80 and 443: `/cloud/`,
  `/git/`, admin SPA on `/`. Traefik holds `losos.apps.ports` (apps.nix).
- `git.nix` + `backend/src/forge/` (`losos-git`, a second binary of the
  backend crate): LosOS Git, the box's Git server in place of Forgejo.
  Smart HTTP (`git upload-pack`/`receive-pack --stateless-rpc`, protocol
  v2), repositories in `/var/lib/losos-git/repositories/<owner>/<name>.git`,
  accounts and SHA-256-hashed scoped tokens in SQLite. Two listeners: the
  unix socket nginx reaches (headers trusted) and `127.0.0.1:3000`
  (tokens only; lososd uses the admin token in
  `/var/lib/losos-git/admin-token` with `X-Losos-As`). Issues, PRs,
  comments, labels and attachments are in the same SQLite. Pull requests
  are writable: compare, open, line comments in threads, reviews with a
  verdict, requested reviewers, merge (merge, squash, fast-forward) in a
  temporary worktree (`forge/pulls.rs`). Worktrees are marked by push
  options `worktree=` and `computer=` (`forge/worktrees.rs`). The pages
  are a fifth Vite page of the admin UI (`admin-ui/app/git/`, `src/git/`)
  at `/git/`: nginx serves them, strips `/git` from `/git/api/` and the
  smart-HTTP paths to the socket. A write with sign-in headers and no
  token is refused when `Sec-Fetch-Site`/`Origin` say another site sent
  it (`server::cross_site`). A box upgraded from Forgejo runs `losos-git-forgejo-import`
  before LosOS Git: it moves the repositories, exports Forgejo's Postgres
  (a private copy of the data dir when Postgres is off) or SQLite, keeps a
  full dump, and `forge/import.rs` takes in accounts, tokens (Forgejo's
  PBKDF2 hash and salt, so old values work), issues and the rest in one
  transaction, idempotent through `forgejo_ids`. Anything not carried over
  is named in `/var/lib/losos-git/forgejo/import.json`; nothing of
  Forgejo's is deleted. `nix/tests/git-forgejo.nix` switches a real
  Forgejo box to LosOS Git and checks every record. The old
  `losos.forgejo.*` names stay as hidden aliases (`lib.doRename`,
  `options.rs` `ALIASES`). `nix/tests/git.nix` boots it with a laptop.
- LosOS Wiki (`backend/src/forge/wiki.rs`, `admin-ui/app/src/{wikis,wiki}/`):
  a repository's wiki is `<name>.wiki.git` beside it, Forgejo's layout, so
  a box moved from Forgejo keeps its wikis (`names::repo` refuses `.wiki`
  names, adoption skips them). The repository's people read and write it;
  a save is a commit made with plumbing, refused with 409 over a page that
  changed meanwhile. `/wikis` in the admin SPA is the box's directory of
  wikis (spaces, LosOS Cloud's knowledge bases from `GET /kb/api/`, the
  handbook) and the editor; `/wiki/<owner>/<name>/` is each space as a
  site, its own Vite page under nginx's `/wiki/` location beside `/git/`,
  not LAN-guarded. Pictures come from `…/wiki/raw/`, raster only.
  `nix/tests/wiki.nix` boots it.
- `apps.nix` + `backend/src/apps.rs` + `backend/src/compose.rs`: the run-as
  user picks the runtime. `notshared` apps (a chart from the Apps pane's
  search, a Compose file in the LosOS app format from `POST
  /api/apps/compose`, the box's own `losos.apps.builtin`) run on rootless
  Podman Compose as `notshared`, `losos-podman-app@<name>`, no cluster
  needed; `compose.rs` turns `helm template` output into Compose and
  refuses privileges. Each one's Compose file is committed, as `notshared`,
  to that user's own Git repo `/var/lib/losos-podman/notshared/apps`, apart
  from `/etc/nixos`. `shared` apps are Helm releases in the box's own k3s,
  only where that cluster runs. `losos-app-<name>` transient units do the
  work; the `losos-shape` rules (`losos-app-shape`) put every pod on the
  host network without root, turn claims into folders in that user's data
  domain and refuse a chart that needs more. `losos-traefik` (its own
  user) holds `losos.apps.ports`, an entrypoint `port-<n>` each, LAN only
  through the `lan-only@file` ipAllowList (RFC 1918 minus 10.42/16), IPv4
  only (Go listens on both families, so the firewall resets IPv6 to the
  range; an IPv6 LAN address looks like anyone's). Routes
  are labels `compose.rs` writes on every service (`app-<name>`, dropping an
  app's own `traefik.*`/`losos.*`), read through `losos-podman-api`, a
  socket-activated Podman API of `notshared` only Traefik's group opens; a
  `shared` app gets a route file in `/var/lib/losos-apps/traefik/`. No
  nginx forward, no reload. API and metrics on `127.0.0.1:8089`
  (`losos.apps.traefikPort`), the job waits on it. Settings › Statistics and
  the Overview's Statistics tile draw `src/statistics/` (shared with the
  edge's manage page). `nix/tests/podman-apps.nix` boots it.
- `chat.nix` + `backend/src/chat.rs` + `admin-ui/app/src/chat/` (LosOS
  Chat, `/chat/`, its own Vite page of the admin UI): Synapse (unit
  `losos-chat`, SQLite) as `shared` in `/home/shared/data/chat/synapse`
  (a box that ran Dendrite keeps its files beside it, unread), only while
  `lososInternal.sharedSide` (`sharingMyStorage` and `cluster.enable`, the
  gate mail shares); off, nginx
  sends `/chat/` home and `/_matrix/` 404s. No Matrix passwords: lososd is an
  application service over every user and the only way in
  (`/api/chat/session`), and nginx asks `/internal/chat/authz` before every
  client and media request (LosOS Login cookie plus a token of that same
  person). The application service's user namespace is not exclusive:
  Synapse leaves an exclusive one's users out of its directory. The client is matrix-js-sdk with its Rust
  crypto under the Lab's CSP arm. `nix/tests/chat.nix` boots it and drives
  two browsers (`chat-browser.mjs`); its output holds the screenshots.
- `mail.nix` + `backend/src/mail.rs` (`losos.mail.*`): LosOS Mail. Stalwart
  as `shared`, mailboxes in `/home/shared/data/mail`, loopback HTTP only (no
  SMTP or IMAP listener), its own unit rather than nixpkgs' module because a
  tmpfiles line inside the locked fscrypt domain fails at boot. lososd is the
  only client, over JMAP as `<person>%losos` (master user) with a secret from
  `losos-mail-secret`. All of it, `/mail` (a screen of the SPA, inside its
  top bar and sidebar like every pane, `src/mail/`) and
  `/api/mail*` exist only while `lososInternal.mail.live` (the shared side,
  `lososInternal.sharedSide`: sharing on and `cluster.enable`);
  otherwise `/mail`, `/mail/` and the API answer 404 and `/api/me`'s `shared` omits
  `mail`, which hides the sidebar entry. Mail stays between people on the
  box (`local_only`). `nix/tests/mail.nix` boots it; `losos-mail-screens`
  is the same run with the real UI and Chromium, for screenshots.
- `search.nix` + `backend/src/textsearch/` (`losos-search`): text inside
  files for the palette. A timer pass keeps an SQLite FTS5 index of the
  owner's cloud files in `/var/lib/losos-search` as `losos-cloud` (only what
  changed is re-read); Git is `git grep` per query as `losos-git`. Both answer
  on Accept=yes sockets under `/run/losos-search` that only root (lososd's
  `/api/{cloud,git}/text`) can open. `nix/tests/search.nix` boots it.
- `ci.nix` + `backend/src/ci/` (`losos-ci`, page at `/ci`): Mesh CI, the
  GitHub Actions workflows of LosOS Git's repositories, only on a box that
  shares and is in a mesh. `losos-ci serve` (user `losos-ci`) plans runs
  and hands jobs to the edge's `/ci/*` queue; `losos-ci runner` (as
  `shared`) takes them and starts each job's containers through pm
  (`nix/pm.nix`, flake input `pm`, never patched) with Mesh CI's signed pm
  plugin (`backend/plugins/pm-ci`, `nix/ci-pm-plugin.nix`), which answers
  for `lci-*` containers alone. pm has no exec, so the job container runs
  an agent that takes steps through a shared folder. `uses:
  losos-project/pm` builds a signed recipe in pm's jail.
  `nix/tests/mesh-ci.nix` boots an edge and two boxes.
- `windows.nix` + `backend/src/windows.rs` + `backend/src/cloud/watch.rs`
  (`losos.cloud.windows.enable`, on by default): the box under Network in
  File Explorer (wsdd, LLMNR) and each person's LosOS Cloud files as
  `\\<box>\<name>` over Samba, SMB2 and up, LAN only; ksmbd stays
  blocked. NTLM needs the NT hash, so lososd makes one whenever a LosOS
  password passes through it in the clear (`People` add, set, check) and
  writes Samba's `smbpasswd`, shares and name map, and the
  `losos-win-<name>` accounts as userdb records on /run. Samba writes as
  `losos-cloud`; `losos-cloud serve --watch` scans what changed so ids and
  ETags follow. While on, `_smb._tcp` replaces the WebDAV mDNS entry so
  Finder and GNOME list the box once. `nix/tests/windows.nix` boots it.
- `hidden-services.nix` + `hidden-lib.nix` (`losos.hidden.{tor,i2p}.enable`)
  and `edge-hidden.nix` (`losos.edge.hidden.*`): Tor and I2P addresses.
  Each side writes a list of services; `losos-hidden-sync` turns it into a
  torrc fragment (`%include` of `/var/lib/losos-hidden/tor/`) and an i2pd
  tunnels file, reloads the daemon and collects the names into
  `/var/lib/losos-hidden/addresses.json`. The box's address lands on the
  front vhost's loopback listeners 8480 (http) and, through nginx's stream
  TLS on 8443, 8481 (taken as https by `$losos_proto`), never on port 80,
  where the manage vhost trusts loopback; the leaf certificate is signed
  by the box's own. An edge makes one address per box and routes it on
  Traefik's loopback entry points `losos-hidden`/`losos-hidden-tls` only
  (`edge/registrar/src/hidden.rs`), and the heartbeat's answer hands the
  names to the box (`edge.json`). LosOS Login takes both as cookie
  domains; `GET /api/hidden` is the `hidden` plugin.
  `nix/tests/hidden-services.nix` boots private Tor and I2P networks.
- `admin-ui/themes/`: the SPA's `tokens.css` is shipped byte for byte as
  `losos-tokens.css` into LosOS Login's page; LosOS Cloud and LosOS Git are
  part of the SPA's own build, so one colour change moves all of them.
  `default.nix` is the data every caller imports. Owners see LosOS Cloud and
  LosOS Git.
- `credentials.nix` + `backend/src/credentials.rs` (`losos.credentials.entries`):
  every secret of the box (admin token, LosOS Login's keys, lososd's OIDC
  client secret, the mail master secret, the backup bucket's keys) is a
  systemd-creds file in `/etc/credstore.encrypted`, sealed to the TPM with no
  PCRs or to the host key, persisted, wiped on erase, and handed to its units
  by `LoadCredentialEncrypted=`. The boot unit `losos-credentials` makes the
  box's own values, adopts an older build's plaintext and deletes it, and
  sets aside a file that no longer opens. lososd lists them
  (`GET /api/credentials`, the Credentials pane, `losos credentials`) and
  makes new values for the rotatable ones; no value is ever in the API.
  `nix/tests/credentials.nix` boots it with and without a TPM.
- `hardening.nix` (install only): KSPP primitives, on by default, with
  `apparmor`, `malloc`, `nosmt`, `usbguard` opt-in. `nix/tests/hardening.nix`
  asserts both halves.
- `secure-boot.nix` (iso only): the medium's `EFI/BOOT/BOOTX64.EFI` is a
  signable UKI (ukify: kernel + initrd + cmdline) in place of GRUB, with
  the squashfs's SHA-256 on the signed cmdline and a stage-1 unit that
  checks it. `losos-sign-iso` (`nix/sign-iso.sh`) signs the finished
  ISO in place via the El Torito EFI image; CI and the release job run it
  with the secret, `nix/tests/secure-boot.nix` under OVMF with a throwaway key.
  The certificate is `nix/keys/secure-boot-db.pem` (comment-only until the
  ceremony).
- `branding.nix` (both systems) and `live-branding.nix` (iso only): LosOS
  and the release tag from `nix/version.nix` on the boot menus, the boot
  pictures (`brand-art.nix`, drawn from the salmon), os-release, the console
  palette and banners. The installed box keeps `ID=nixos`. Nobody bumps the
  file: `version.yml` commits the newest tag to it on main, and the
  release and `tui` jobs stamp the tag over it before they build.
- `splash.nix` (both systems, `losos.splash.enable`): Plymouth with the
  LosOS theme in `modules/splash/losos.script`. `console.nix` sends it the
  tty1 rows (`plymouth update --status=losos:…`) and keeps drawing the text
  banner underneath; plymouth-quit is out of multi-user.target, so the
  splash is the box's screen after boot. `nix/tests/console.nix` asserts it.
  On the ISO the installer is a tty1 service (`installer.nix`), not a
  login shell, and runs under the splash. `install_screen.rs` builds the
  panel's rows, keys come from `plymouth watch-keystroke` (Plymouth holds
  the keyboard), and a getty's hangup on tty1 would pull the terminal from
  under it. `nix/tests/installer-screen.nix` asserts both modes.
- `startup-sound.nix` (install only, `losos.startupSound.enable`): the end
  of the launch film's score, synthesised at build time by
  `modules/startup-sound/render.py` (no recording in the tree), played on
  every ALSA output once `losos-console` is up. A reboot leaves a marker so
  the nightly reboots stay quiet.
- `builder.nix` + `backend/src/builder.rs` (install only,
  `losos.builder.enable`): Build with AI, for widgets and for the owner's
  own apps. Ollama on `127.0.0.1:11434` serves `losos.builder.model`
  (qwen2.5-coder:1.5b, sized for a 4 GiB two-core box, pulled once by
  `ollama-model-loader` into `/var/lib`); lososd streams one build at a
  time from `/api/chat`, parses the answer (a fenced block plus `Name:` and
  `Summary:` lines) and runs the loop: write, check (`jsx::check`, the
  widget checks, an example from the prompt sent back), try, fix, up to
  three fixes for an app and one for a widget, keeping the best answer.
  The try runs in the owner's browser: the build sets `trial`, the page's
  app frame fills every field, presses the first button and posts what was
  kept to `/api/builder/builds/{id}/tried`; with no page within
  `LOSOS_BUILDER_TRY_SECS` the step is skipped. The box drives the loop on
  purpose: the 1.5B model cannot call tools. The prompts are chat turns with
  examples: `backend/src/builder-{widget,app}-prompt.md`. No edge, no key,
  no charge.
- Your apps (`backend/src/{user_apps,jsx}.rs`, `admin-ui/app/src/apps/`,
  `src/app-frame/`): an app is one React component in JSX that lososd
  parses, checks for names that do not exist and compiles with oxc, then
  keeps in `user-apps.json` with its data under `user-apps/<id>.json`.
  It runs only in the browser, in `<iframe sandbox="allow-scripts
  allow-forms" src="/app-frame/">` under nginx's `/app-frame/` CSP arm,
  with the runtime (React, the 16 LosOS components, `useCollection`,
  `useStored`, `losos`) as one classic script; data goes through
  postMessage to `/api/user-apps/<id>/data`. No app code runs on the box.
- `inference-lend.nix` (box, `losos.inference.lend.*`) + `edge-inference.nix`
  (edge, `losos.edge.inference.*`) + `edge/registrar/src/{inference,meter}.rs`
  + `market/taps.rs`: AI models split across boxes. A lending box runs
  `ggml-rpc-server -d CPU` (the default device is BLAS, which crashes) as
  `shared` on loopback, reached only by root through a second tunnel
  service `<id>:llm`. The registrar groups lenders until a model fits and
  runs one `losos-llm-head@<group>` llama-server per group; it counts the
  tokens itself and bills them through escrow taps (a USDC balance per
  buyer, one rate-capped tap per seller, a one-hour head start, a 48 h
  withdrawal notice). The Market pane is `ai-models.tsx`.
  `nix/tests/inference-lend.nix` and `inference-payg.nix` boot it.
- `updates.nix`: `system.autoUpgrade` at 03:00 from `losos.upgradeFlakeUri`
  (default `git+file:///etc/nixos#install`, set a `github:` URI to really
  upgrade), `nix.gc` at 04:30 with `--delete-older-than 14d`, five boot
  generations, unconditional reboot at 00:07. `nix/tests/invariants.nix` pins
  these at eval time.
- `logs.nix` (install and edge) caps every log: the journal at
  `losos.logs.journalMaxUse` and a month, pod logs at three files of 10 MiB
  per container in all three clusters, logrotate for lososd's `audit.log`
  and `rebuild.log` and a size bound on nixpkgs' nginx rule, Nix build logs
  after 30 days. A new service that writes its own log file gets a cap
  here or in its own flags. `nix/tests/invariants.nix` pins it.
- `grow.rs`: `losos-ctl grow` runs lvextend, `cryptsetup resize`, `resize2fs`
  online, asserted against the plan and in `nix/tests/resize.nix`.
- `backup.nix` + `backup.rs` + `erase.rs`: restic backups to an S3 bucket,
  encrypted on the box with the recovery code as the password, run as
  `losos-backup-<job>` units outside lososd's sandbox; the fscrypt domain is
  unlocked for the copy. The erase is phased in `state.json` (backup,
  cancellable countdown, leave the edge, reset, reboot) and
  `losos-factory-wipe.service` deletes the data before sysinit on the next
  boot. `nix/tests/erase.nix` runs the cycle against MinIO.
- `edge/vercel/`: the registrar's router as one Vercel Function over Neon
  Postgres, for demos. `/cluster/join` and `/market/*` answer 503 there.
- `edge/registrar/src/{domains,zone}.rs` + `losos.edge.dns`: an official
  edge serves its own zone with Knot and routes an owner's domain once its
  TXT token and CNAME check out, only for a box with a ready Stripe account.
  lososd writes the live names to `/var/lib/losos-public-names`, which
  LosOS Login reads for its cookie domains.
- Traefik finds the boxes through its HTTP provider: it polls the
  registrar's loopback provider listener (`server/provider.rs`,
  `losos.edge.traefikProviderPort`, 8090) every 2 s for `config::Routes` as
  JSON. The registrar writes nothing into `/etc/traefik/dynamic` (NixOS-owned
  `register*.yml` only; tmpfiles removes an old edge's `losos.yml`). No routes
  is the document `{}`: an empty `routers: {}` is refused and Traefik keeps
  the old routes, as it does on any non-200. Box routers carry the
  `losos_offline` errors middleware (502/504 → the edge's page).
  `websecure` also answers HTTP/3 on UDP 443. `nix/tests/edge-vm.nix` and
  `edge-federation.nix` curl real HTTPS and HTTP/3 through Traefik with -k.
- `edge/registrar/src/routes.rs`: custom domains of mesh boxes behind a
  local edge. The official edge keeps the routes in the registrar's own
  state (`relay-routes.json` beside the registry, no etcd) and binds a box
  to a local edge only by the box's relay pass, an HMAC the local edge
  forwards but cannot make.
- `edge/registrar/src/market.rs`: Stripe Connect, third opt-in of the
  registrar. The key lives only in the `losos-stripe-gate` unit, reached over
  `/run/losos-stripe-gate/gate.sock`.
  The key's prefix is the mode (`stripe_mode.rs`): test and live each keep
  their own ledger (`market-test.json`, `market.json`), and a signed event
  from the other mode is refused. `modules/edge-credentials.nix` and
  `.github/workflows/edge-credentials.yml` seal `STRIPE_KEY` and the webhook
  secrets from Actions secrets over a forced-command SSH key.
  USDC is held in `LososEscrow` on EVM networks and in the `losos-escrow`
  program on Solana (`edge/contracts/solana`, `solana_escrow.rs`, signed by
  a key derived from the EVM releaser's); a live edge counts no payment into
  a program that still has an upgrade authority.
- `edge/registrar/src/{vms,server/vm}.rs` + `modules/edge-vms.nix`:
  virtual machines on the mesh, sold per replica-month through the market
  with a fixed 50/50 fee the Stripe gate enforces. KubeVirt and CDI from
  pinned manifests, disks on the hosting box in `losos-vm-local` (Retain).
  Offered only to a box with `sharingMyStorage` (lososd's `/api/vms`, the
  SPA's `/machines`); uploads stream through lososd to the edge's merged
  transfer router, outside the guard's body limit.
- `nix/fast-build.nix`: every Rust binary links with mold and compiles
  through ccache and sccache, in shells and in `nix build` alike.
- CI is `.github/workflows/ci.yml`, kept in step with `devenv.nix` by hand.
  A pull request runs only the jobs its files reach: the `changes` job maps
  them to lanes with `.github/scripts/ci-changes.sh`, and `result` is the
  one check that sums the run up. Main, tags, the weekly run and the
  `ci: full` label run everything. A new file a job reads from outside its
  own folders goes into that job's lane in the script.
  `fmt-bot` pushes `cargo fmt` to main, `iso` imports `losos-ctl` as an
  artifact and boots the ISO under OVMF and SeaBIOS, tags run
  `release-media`. `history-scrub.yml` rewrites the history after every push
  to main (rules in `.github/scripts/scrub-message.py`), pushing branches
  with the `HISTORY_SCRUB_DEPLOY_KEY` deploy key and tags with the CI token.
  `stale-branches.yml` deletes, each Sunday, branches with no commit in 30
  days that no open PR uses as head or base.
  `failed-runs.yml` deletes, each day, unsuccessful workflow runs whose
  commit is no longer a branch tip or an open PR's head.
  `dependency-hashes.yml` (`pull_request_target`, branches of this repository
  only) runs `.github/scripts/fix-hashes.py` on a pull request that changes
  a `Cargo.lock` or `package-lock.json`, Dependabot's included, and pushes the
  new `cargoHash`/`npmDepsHash` back to the branch, and on a push to main
  that still carries a stale one opens a `bot/dependency-hashes-*` pull
  request; run the script yourself after touching a lock file.
  `discovery` boots the netboot box `nix/tests/discovery/box.nix` on Linux,
  macOS and Windows runners, and `nix/tests/discovery/check.py` has each
  find it and read and write files over SMB and WebDAV.
  The nixos-test VM tests are not in CI.

## Rules

1. Anything that must survive a reboot goes into
   `environment.persistence."/persist".directories`, or it is gone next boot.
   `/etc/rancher` stays listed: the k3s agent's node password lives there and
   the server rejects a node that regenerates it.
2. Run `devenv test` before trusting a change, and let the git hooks run. `doCheck` is off in both
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
   noexec `/tmp` stops nix builds. `nix/tests/hardening.nix` asserts all three.
6. Gate `--disable`, `--flannel-backend` and `--disable-network-policy` on
   the server role. `k3s agent` crash-loops on an unknown flag.
7. Leave the two Kubernetes instances as two clusters. The box's own apps in
   the edge's cluster would go down for any edge outage spanning the 00:07
   reboot.
8. Never write a `deny <podCidr>` nginx rule. hostNetwork pods arrive from
   `127.0.0.1` or the LAN address.
9. `lanOnly` denies loopback on purpose: the edge's tunnel delivers from
   `127.0.0.1`. Test against lososd's `:8082` or with a LAN source address.
10. LosOS Cloud listens on a Unix socket only nginx's group may open; that
    is what makes `X-Losos-Cloud-User` believable. Never give it a TCP port.
11. Keep `/status.php`, `/remote.php/`, `/ocs/`, `/index.php/`,
    `/public.php/` and `/nextcloud/` routed to LosOS Cloud. Phones and
    desktops set up against Nextcloud keep calling them.
12. Sign-in is LosOS Login (Authelia) at `/auth/` over the people file
    lososd writes. Keep the spare-key Bearer bypass in the authz location
    and the spare-key path in the dialog. lososd trusts `Remote-User` only
    from loopback with `LOSOS_IDENTITY=authelia`. Authelia must start only
    after `users.yml` exists: an empty users map is fatal and a missing file
    makes it write an example account.
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
16. A secret the box makes for itself is a credential in
    `/etc/credstore.encrypted` (`modules/credentials.nix`), the admin token
    included: a module declares it under `losos.credentials.entries` with
    the units that read it, not as a new plaintext file under `/var/secrets`. Seal with
    `--tpm2-pcrs=` (no PCRs), and list an old plaintext path under `legacy`
    so the boot unit adopts and deletes it.
17. The compute window's time zone travels with its bounds and the edge
    evaluates `now` per node in `modules/edge.nix`. Hoisting it enforced a
    Berlin window in UTC and slid an hour at each DST change.
18. `tenantsJson` in `modules/edge.nix` must keep the `market` key or every
    trade 403s. The `/api/market*` relay answers 200 `{available:false}`
    when the market is off; the SPA latches a 404 as "not served". Never add
    an operation that forwards arbitrary Stripe calls through the gate.
19. In `nix/fast-build.nix` the stdenv is mold first with ccache around it,
    and `RUSTC_WRAPPER` is a script not named `sccache`. Either way round
    silently caches nothing.
20. Don't add `restrict=on` to the ISO boot test's netdev; QEMU then offers
    no DNS server and the test never sees its query. The ISO's UEFI loader
    is a UKI (`modules/secure-boot.nix`), signed after `nix build` by
    `losos-sign-iso` with the `SECURE_BOOT_DB_KEY` secret, never inside a
    derivation: a key in the store would reach the public cache. The
    production key is made only by `docs/provisioning/secure-boot/keygen.sh` on
    the owner's machine; `nix/tests/secure-boot.nix` uses a throwaway one.
21. `system.stateVersion = "26.11"` is set once. `result` is never committed.
22. Docs go in `handbook/`; there is no wiki. Every handbook page has Slovak
    and German copies under `handbook/i18n/`: change all three together.
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
| "One cluster is simpler" | Midnight reboots plus an edge outage take the owner's LosOS Git down. |
| "Raw random bytes are a stronger key" | disko's command substitution drops NULs; the volume is formatted with one key and unlocked with another. |
| "Pure evaluation is cleaner, drop `--impure`" | A `github:` upgrade flips a keyfile box to TPM, forgets its drives and resets every setting. |
| "`nix build` passed" | It compiled. The suites run in `devenv test` and CI only. |
| "A `192.168.*` wildcard is simpler than a header" | It trusts `192.168.attacker.example` too. |
| "A pod-CIDR deny adds depth" | There is no pod CIDR to match; the pods share the host's netns. |
| "Attribution trailers are standard practice" | Not here. The hook refuses them and the bot rewrites history to remove them. |

## Before you finish

One flake check is not a VM: `losos-invariants` (`nix/tests/invariants.nix`)
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
nix build .#checks.x86_64-linux.losos-iso-install    # losos-install from the medium's system onto an empty disk, offline, then that disk boots and answers on :80
```

Inside the dev shell, `bcd` → `cd backend`, `rcd` → `cd edge/registrar`.

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
  `cloud`, `containers`, `daemon`, `overrides`, `updates`, `defaults`. The
  flake passes `specialArgs.self = self` so `defaults.nix`
  can reach `self.packages.${system}.{losos-ctl,losos-admin-ui}` to wire the
  control plane and the admin UI.

**Stateless-by-impermanence model** (`impermanence.nix` + `disko.nix` +
`boot.nix`): the root is tmpfs; `/persist` is LUKS-encrypted ext4 (with the
`encrypt` feature, because fscrypt needs it and btrfs cannot provide it). Only the
dirs in `environment.persistence."/persist".directories` survive a reboot
(`/nix`, `/var`, `/etc/ssh`, `/etc/keys`, `/etc/nixos`, `/etc/rancher`,
`/etc/credstore.encrypted`, the two data homes, `machine-id`). **Anything new that must persist across reboot must be added
to that list** or it silently vanishes on the next boot. `/persist` is
`neededForBoot` so impermanence bind-mounts resolve before the sysroot is
populated. The volume is always formatted, unattended, from a random keyfile
the installer generates at `/etc/keys/persist-keyfile`. Unlock is TPM2 by
default (`losos.tpm.enable = true`): the installer runs `systemd-cryptenroll
--tpm2-device=auto --tpm2-pcrs= --unlock-key-file=…` right after `disko`, so
the initrd carries no secret and the keyfile stays only inside `/persist` as
the recovery slot (`losos-ctl grow` authenticates with it). No PCR binding, on
purpose: the box updates itself unattended with no shell to recover a lockout
from. Where the installer medium sees no
`/dev/tpmrm0`, or with `losos-ctl install --no-tpm`, the box is in keyfile
mode: `tpm.enable = false` in `install-target.nix` and the keyfile injected
into the initrd as `/crypto_keyfile.bin`, on the unencrypted ESP. The two
crypttab shapes are exclusive: systemd-cryptsetup given a key file *and*
`tpm2-device=` reads the file as a sealed blob. `nix/tests/invariants.nix` pins the
default and that the TPM path declares no initrd secret; `nix/tests/tpm.nix` boots
it end to end under swtpm. A remote `github:` upgrade URI evaluates a tree
without `install-target.nix`, so `flake.nix` prefers the live
`/etc/nixos/modules/{install-target,overrides}.nix` whenever it can read them,
and both rebuild paths (`updates.nix`, `supervisor.rs`) pass `--impure` for
that; see the auto-upgrade section.

**Two isolated data domains, no shell** (`configuration.nix`): `notshared`
(uid 1000) is the owner's account (their LosOS Cloud files carry the name), `shared` (uid 1001) owns the contributed mesh
storage domain; both homes are
mode `700`, each with its own primary group, so neither can read the other —
`isNormalUser` without an explicit `group` puts both in `users`, and a `750`
home then grants that group r-x, which silently defeated the isolation until
`nix/tests/impermanence.nix` caught it. Neither has a password, and
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
(`nix/options-doc.nix` + `modules/config-repo.nix` + `backend/src/options.rs`
+ `backend/src/config_repo.rs` + `admin-ui/app/src/screens/settings/pane-advanced.tsx`
+ `pane-history.tsx`): the `losos.*` declarations are walked at build time
into `/etc/losos/options.json` (name, editor kind, default, description, the
running value, danger and read-only flags); lososd joins the current
`overrides.nix` into it at `GET /api/options`, and the Advanced pane draws a
typed editor per row. `classify` in the Nix file is the gate: a declaration
with a type it does not know is a `throw`, so the eval job goes red rather
than an empty row landing on the pane, and `tests/advanced.browser.mjs`
renders the real document (`nix/tests/admin-ui.nix` passes the
`losos-options-doc` check) and fails on any row the pane cannot draw.
`POST /api/apply` is now checked line by line against the document
(`check_body`): undeclared, read-only (installer-fixed, packages) and
ill-typed values, and any `${`, are refused with the key named. Every
apply, mode change and reset is a commit in `/etc/nixos` (the repository
the installer made), and a reconciler thread in lososd polls every 30 s:
it creates the owner's LosOS Git account (`notshared`, no password: the
owner signs in with LosOS Login) and a private `losos-config` repository
through LosOS Git's admin API (admin token at
`/var/lib/losos-git/admin-token`, `X-Losos-As` to act as the owner), pushes
when behind, and when a clone has pushed ahead fast-forwards, gates
the new `overrides.nix` the same way, and rebuilds; a diverged history is
reported, never touched. There is deliberately **no CI runner**
behind this. `POST /api/config/sync` runs one pass on demand. The fifteen settings the other panes own are linked
from the Advanced pane, not edited twice; `OWNED` in
`admin-ui/app/src/lib/option-value.ts` and `OWNED_NAMES` in `lib/api.ts`
are that list, and `buildOverridesNix` filters extras against it so the
file never carries a key twice. Two parser facts matter here:
`overrides.rs`'s `parse_line` cuts a value at the first `#` *outside*
quotes (it used to cut inside them, which mangled the default
`upgradeFlakeUri`), and a list is one `[ "a" "b" ]` line, strings only.

**Workloads and the single front door** (`modules/containers.nix` +
`modules/cluster.nix` + `modules/cloud.nix` + `modules/git.nix`):
systemd-nspawn is gone, and so are Nextcloud and Forgejo. LosOS Cloud and
LosOS Git are host services of their own on Unix sockets, and the box's own
k3s cluster runs only the apps of its shared side. Nginx holds the web ports
(Traefik holds only the installed apps' LAN range): path-based routing on the appliance's mDNS name
(`/cloud/` plus the Nextcloud client addresses, `/git/`) and the admin SPA on
the same vhost. `admin-ui/design-system/` (`tokens.css` + `losos.css`, plus
a `react/` wrapper package for claude.ai/design) is **dev-machine-only and no
longer served at all** — the shipped SPA carries its own palette in
`admin-ui/app/src/styles/tokens.css`, which `index.css` republishes as
Tailwind v4 theme keys. It stays out of the Nix closure
because `losos-admin-ui`'s source *root* is `admin-ui/app`; that is a stronger
guarantee than the `design-system` name filter it replaced, which would have
stopped matching on a rename.

**One look across the admin pages, LosOS Cloud, LosOS Git and LosOS Login** (`admin-ui/themes/`):
LosOS Cloud and LosOS Git are pages of the admin UI's own build
(`admin-ui/app/cloud/`, `admin-ui/app/git/`), so they share the components
and `tokens.css`. `tokens.css` is plain CSS on purpose, because LosOS
Login's page ships it byte for byte (as `losos-tokens.css`) and maps its own
variables onto it — change a colour there and everything moves together.
Owners see the two apps as **LosOS Cloud** and **LosOS Git**, and both get
logos drawn from `admin-ui/themes/brand/` (`marks.py` wraps
`salmon.png`, a live coho salmon cut out of a NOAA Fisheries photo, into
the SVGs and copies it to the SPA and the handbook; every PNG and the
`.ico` is rendered from those at build time). `plate.png`, the plate of
salmon from Matus's own photo, is the Halloween logo. The SPA and the
handbook each swap it in on 31 October by the viewer's local date, in their
own `src/lib/logo.ts`. The apps' marks never change. `brand/CREDITS.md`
records both sources. A new picture needs a licence that allows use in a
logo, and Pixabay's does not. The old
pixel-art fish was dropped over a copyright worry and does not go back in. The rest of the upstream marketing goes by config (no skeleton
files, no help or sign-up links, no "Powered by").
`admin-ui/themes/default.nix` is plain data every caller imports. The workload pods are
`hostNetwork` (the local cluster runs no CNI), so they answer on loopback — see
the gotcha about what that costs the `lanOnly` guard.

**The Vercel demo host** (`edge/vercel/`): a third Rust crate, outside the
flake, that runs the registrar's router as one Vercel Function with the
registry as one `jsonb` row in a Neon Postgres (tokio-postgres over rustls;
`DATABASE_URL` from Vercel's marketplace integration, loaded before and
upserted after every request) and two demo-only routes (`/status`,
`/status/traefik`) plus a static page. It exists for presentations and is
meant to be deleted afterwards: the only changes it needed in
`edge/registrar` are `server::build` (the router and reconciler without
the listener and the timer; `serve` calls it) and `Registry::{export,import}`
(a snapshot with ages, so the registry can travel between processes;
`import` applies the heartbeat TTL because no reconciler ran in between).
The tunnel, Traefik, the mesh and the Stripe gate cannot run there, so on
that host `/cluster/join` and `/market/*` answer 503 and `/tunnel-pin`
404. `devenv.nix`'s `crates` list and CI lint/test it like the other two;
`nix build` never sees it. Its `tests/postgres.rs` is gated on
`LOSOS_TEST_DATABASE_URL` (CI's test job provides a Postgres service); the
rest of its suite runs on the in-memory store. Its second static page,
`public/find.html`, is the "find my box" flow: Chrome's Local Network Access
permission lets it read a box's `/setup/state.json` from the public origin,
which `modules/setup.nix` allows for exactly the origins in
`losos.setup.finderOrigins` (an nginx map, asserted by `nix/tests/setup.nix`),
never `*` — the document is LAN inventory.

**Edge federation and the box's path rule** (`handbook/docs/in-depth/edge-federation.md`,
`edge/registrar/src/relay.rs`, `modules/edge-gateway.nix`,
`modules/proxy.nix`): a user-hosted edge (a *spoke*) relays its boxes to an
official edge (a *hub*) with `POST /relay`; the hub lists the spoke as a
tenant with a `relayZone` and routes `<spoke>.<box>` with the spoke's token.
A spoke with `losos.edge.lan.openEnrolment` enrols unknown boxes
trust-on-first-use under `/var/lib/losos-registrar/enrolled/`; the option
asserts `lan.advertise`, never set it on a VPS. `losos.edge.gateway.enable`
is the preset the `losos-disk-edge-qcow2` image boots. On the box, `lososd`
picks the path (LAN edge, else the configured one, else none) and drives
`losos-tunnel-client` + `losos-registrar-announce` through
`/run/losos/edge-path.env` and the `/run/losos/edge-none` marker, so a box
with no edge in reach runs no tunnel; both files are on `/run`, so the
units dial the configured edge until the first scan, which is what
`nix/tests/edge-vm.nix` (no lososd) relies on. `nix/tests/edge-federation.nix` is the
three-VM acceptance test; it is not run by CI.

**The `losos.*` option namespace** (`options.nix`): all project-specific
knobs (`targetDrive`, `tpm.enable`, `sharingMyStorage`, `git.enable`,
`cloud.*`, `cluster.*` (mesh join, compute window), `shared.fscrypt.*`,
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
sandboxing on `nginx`/`lososd`. mDNS is systemd-resolved (`mdns.nix`,
`losos.mdns.*`, set by modules and kept off the Advanced pane), on every
physical interface and with LLMNR off; there is no Avahi. Four opt-in flags — `apparmor`,
`malloc`, `nosmt`, `usbguard` — carry what can break something. Not imported by
`iso`: the live medium needs squashfs. `nix/tests/hardening.nix` asserts both halves,
including that nothing blocks the kernel surface k3s, rke2, containerd and
Longhorn need.

**Online growth** (`grow.rs` + `losos.storage.fillPercent`): the LV deliberately
stops short of the whole volume group, so `/persist` — which *is* `/nix`, via
impermanence — can be grown without opening the box. `losos-ctl grow` runs
lvextend, then `cryptsetup resize`, then `resize2fs`, in that order, online.
The order is the whole correctness argument and the failure is silent, so it is
asserted against the *plan* in `grow.rs` and end-to-end in `nix/tests/resize.nix`.

**BIOS and the tty1 banner** (`losos.bios`, `modules/console.nix`): the
installer ISO's firmware menu writes `losos.bios` into `install-target.nix`:
BIOS and UEFI can be chosen explicitly, or autodetect uses the absence of
`/sys/firmware/efi` (also available noninteractively with `--bios`/`--uefi`).
The menu exists only on the installer ISO, not the installed system, and
takes autodetect after 30 s unanswered: the medium is meant to install
unattended, and `nix/tests/iso-boot.py` never types. Under the splash the
menu is on the panel and answered with 1, 2 or 3. BIOS
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
pure and see the in-tree file or the defaults. `nix/tests/invariants.nix` pins
the flag.
`nix.gc` runs at 04:30 with `--delete-older-than 14d`, and `boot.nix` caps
both loaders at five generations: `/nix` *is* `/persist`, the ESP is 500 MiB,
and `linuxPackages_latest` lands a new kernel there most nights, so without
both the box fills itself up and the 03:00 switch fails on the bootloader
step. `nix/tests/invariants.nix` asserts both at eval time.

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
  rebuild is the only self-repair path. `nix/tests/hardening.nix` asserts all three
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
  `/proc/devices`, and vgs exits 4 without it. `nix/tests/invariants.nix` pins
  both.
- **The LUKS keyfile is hex text, and must stay NUL-free.** disko hands
  `passwordFile` to `luksFormat` as `<(echo -n "$(cat FILE)")`, a command
  substitution that drops NUL bytes and a trailing newline, while the initrd
  (`/crypto_keyfile.bin`) and `systemd-cryptenroll --unlock-key-file` read
  the file raw. The installer used to write 4096 random bytes, so the volume
  was formatted with one key and unlocked with another on practically every
  install; nothing reached that failure until `nix/tests/tpm.nix` enrolled the
  chip. `ensure_keyfile` now writes 2048 random bytes as 4096 hex characters.
  Don't "harden" it back to raw bytes, and don't put a newline in it.
- **The LUKS key file must not live under `/root` or `/home`.** `lososd` runs
  with `ProtectHome=true` — on purpose, so a compromised request handler cannot
  read either data domain — and that hides `/root` too. `cryptsetup resize`
  then fails with "Failed to open key file", *after* `lvextend` has already
  grown the volume. `/etc/keys/persist-keyfile` is the path everything agrees
  on: `disko.nix` formats from it, the installer keeps it inside `/persist`
  on both unlock paths, `daemon.nix` passes it as `LOSOS_LUKS_KEYFILE`, and
  `nix/tests/resize.nix` uses it rather than a fixture of its own, so they cannot
  drift apart unnoticed.

- **`lososd` is restarted mid-rebuild.** `nixos-rebuild switch` restarts the
  changed `lososd.service`, killing the watcher thread that was tracking the
  rebuild. `Daemon.startSupervisor` re-attaches on daemon startup: if
  `state.json` says `Building`, it spawns a fresh watcher for
  `losos-rebuild-<job>`. Without this the rebuild records `building`
  forever. Don't drop the re-attach.
- **The admin routes deny loopback on purpose** (`lanOnly` in
  `containers.nix`): master-proxy tunnel traffic reaches the front vhost *from
  127.0.0.1* (the tunnel client's `local`), so the LAN-only guard on `/`,
  `/assets/`, `/setup/` and `/api/` must not allow loopback — a
  `curl localhost/` on the box (or in a VM test) gets 403 by design. "Fixing"
  it with `allow 127.0.0.1` exposes the whole admin surface to the internet
  whenever `losos.proxy.enable` is on. Test against lososd's :8082 directly,
  or curl with a LAN source address.
- **Sign-in is LosOS Login (Authelia), over a people file lososd writes.**
  nginx asks Authelia (`modules/identity.nix`) before every `/api/` request
  and passes `Remote-User`, which lososd believes only from loopback with
  `LOSOS_IDENTITY=authelia`. lososd alone writes
  `/var/lib/losos-identity/users.yml` (`backend/src/people.rs`), and
  Authelia starts only once it exists: an empty users map is fatal, and with
  no file Authelia writes an example account. A Bearer request skips
  Authelia and lososd checks the token itself, so the spare key opens the
  box while Authelia is down; keep that bypass and the dialog's spare-key
  path. `backend/sections.json` decides who may call what, for lososd and
  nginx alike; a route missing from it is admin only. `signin.rs` is left
  for the shape check of a password one of the owner's computers offers
  (`crate::devices`) and the LosOS Git requests: never `curl -u`, and
  `signin::validate_candidate` checks shape only.
- **LosOS Cloud keeps answering where Nextcloud did.** `modules/cloud.nix`
  routes `/status.php`, `/remote.php/`, `/ocs/`, `/index.php/`,
  `/public.php/` and the whole `/nextcloud/` prefix to the same socket, and
  `/.well-known/{caldav,carddav}` redirect to `/remote.php/dav/`. A phone
  set up against the old Nextcloud signs in again with its app password,
  which the migration carried over. Dropping a route strands those
  devices silently.
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
  "simplify" the frame into one. `nix/tests/front-vhost.nix` asserts the arm and
  that the strict policy still covers everything else.
- **The admin token is a credential, not a NixOS file.** `modules/credentials.nix`
  makes it at first boot and seals it as `admin-token` in
  `/etc/credstore.encrypted`; lososd reads `%d/admin-token`.
  `losos.admin.tokenFile` is only the old plaintext path, adopted once and
  deleted. Don't declare it as a store path.
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
  `services.k3s` (role `server`) runs *this box's* shared apps;
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
  `edge/registrar/src/market.rs`, `handbook/docs/in-depth/market.md`) answers 503 unless
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
  `edgeRequired`; enable-only, so a box can always leave). A sharing box
  that then finds no edge for `losos.proxy.pauseSharingAfter` (600 s) turns
  `sharingMyStorage` off itself and back on a minute after one answers
  (`edge::sharing_step`, `State::sharing_paused_at`, `/api/edge`'s
  `sharingPausedAt`); only its own pause is undone, and a box that never
  reached an edge while sharing (the shared side's VM tests) is never
  paused. On top of that,
  each answering edge is checked as an open gateway (`GET /passport?nonce=`,
  `POST /passport/attest`; `backend/access/src/gateway/check.rs`, the edge's
  half `edge/registrar/src/gateway.rs`, `losos.edge.passport.enable`) and
  counts as official only if its owner's OpenPGP passport holds, its TPM's
  endorsement certificate chains to `losos.proxy.gateways.tpmRoots`, its
  quote over PCRs 4, 11 and 15 shows a published image
  (`losos.proxy.gateways.images`) and the settings it measured at boot, and
  those settings pay LosOS's 2% to `losos.proxy.gateways.payouts`. The
  market relay refuses everything else (`{available:false,
  reason:"noOfficialEdge"}`, 409 `officialEdgeRequired`); proof of
  tampering disables the edge on that box for good
  (`/var/lib/losos/gateways.json`). `nix/gateway/images.nix` and
  `payouts.nix` are **empty on purpose** until the image and the addresses
  are published: nothing official, market off. There is no root key.
  `nix/tests/open-gateway.nix` is the VM tests' fixture (a throwaway TPM
  maker, swtpm EK certificates, passports). `losos-registrar provision
  {whoami,token}` (`edge/registrar/src/provision.rs`) runs **on the
  operator's own machine** behind GitHub's OAuth device flow (public client
  id only, no secret) and refuses unless the account's *numeric id* is in
  `edge/registrar/operators.json`, which is compiled into the binary. The
  box runs no CI runner, and
  `docs/provisioning/edge-identity/README.md` is the operator runbook. The
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
