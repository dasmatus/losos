# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Context

`losos` is a stateless NixOS appliance for repurposed mini-PCs: tmpfs root,
encrypted `/persist` bind-mounted back by impermanence, Nextcloud for the
owner, spare disk and CPU lent to a mesh, no SSH and no shell. The owner
reaches it only through web UIs and one admin endpoint, so a bad nightly
upgrade has no one to notice it. User docs live in `wiki/`, published to the
GitHub wiki on push to `main`; `wiki/Architecture.md` and
`wiki/Development.md` carry the long form of what this file compresses.

## Commands

```sh
devenv shell                 # or: direnv allow. Scripts: fmt, lint, test-rust,
                             # check-flake, check-eval, check-pins, build-pkgs,
                             # build-iso, vm-tests
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

Three Rust crates: `backend/` (lososd + losos-ctl), `backend-registrar/`,
`edge-vercel/` (outside the flake). All three must pass clippy with
`-D warnings` and rustfmt; the devenv pre-commit hooks, `lint` and CI each
check it. `nix develop` is a toolchain-only shell for people without devenv.
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
- `updates.nix`: `system.autoUpgrade` at 03:00 from `losos.upgradeFlakeUri`
  (default `git+file:///etc/nixos#install`, set a `github:` URI to really
  upgrade), `nix.gc` at 04:30 with `--delete-older-than 14d`, five boot
  generations, unconditional reboot at 00:07. `tests/invariants.nix` pins
  these at eval time.
- `grow.rs`: `losos-ctl grow` runs lvextend, `cryptsetup resize`, `resize2fs`
  online, asserted against the plan and in `tests/resize.nix`.
- `edge-vercel/`: the registrar's router as one Vercel Function over Neon
  Postgres, for demos. `/cluster/join` and `/market/*` answer 503 there.
- `backend-registrar/src/market.rs`: Stripe Connect, third opt-in of the
  registrar. The key lives only in the `losos-stripe-gate` unit, reached over
  `/run/losos-stripe-gate/gate.sock`.
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
22. Edit `wiki/`, not the web wiki; the workflow overwrites it.
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
    Sessions keep their screenshots under `/mnt/project-files/demo/<topic>/`
    and name the folder in the table; the project's attribution block goes
    above the template as the first two lines.

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

- [ ] `devenv test` passed, not only `nix build`?
- [ ] New persistent path listed in `impermanence.nix`?
- [ ] New option declared under `options.losos`, default in `defaults.nix`?
- [ ] New binary lososd calls added to the unit `path`?
- [ ] VM tests run locally if the installer or control plane changed?
- [ ] CI and `devenv.nix` still in step; the three nixpkgs pins agree?
- [ ] Docs changed in `wiki/`, not the web wiki?
- [ ] Commit message carries no trailer and no session link?
- [ ] PR body follows the template; screenshots, or "No visible change." and why?
