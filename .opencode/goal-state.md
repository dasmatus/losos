# Goal: substituter defaults + drop the Pages proxy

## Goal statement

The LosOS GHCR binary-cache proxy (`http://127.0.0.1:8787`) is used during an
install and by every user on the box with no confirmation prompt, and
`https://losos.dasmat.us/proxy` (the GitHub Pages copy) is gone from every
substituter list.

Measurable outcome:

1. The appliance and the installer medium carry `accept-flake-config = true`,
   so Nix's "do you want to allow configuration setting 'extra-substituters'
   …? (y/N)" question (default: no) neither blocks an unattended install nor
   silently drops the cache for any user.
2. `trusted-substituters` lists the GHCR proxy, so an unprivileged user may
   pass it as a substituter instead of Nix answering "ignoring untrusted
   substituter … you are not a trusted user".
3. No substituter list in the tree names `https://losos.dasmat.us/proxy`.

## Milestones

| # | Milestone | Status |
|---|-----------|--------|
| 1 | Reproduce the bug: flake `nixConfig` prompt / untrusted-user drop, and confirm `accept-flake-config` + `trusted-substituters` fix it | verified |
| 2 | `modules/cache.nix`: `accept-flake-config = true` and `trusted-substituters = cfg.substituters` (both systems import it) | pending |
| 3 | Remove `https://losos.dasmat.us/proxy` from `flake.nix`, `.github/actions/setup-nix/action.yml`, `nix/dev-cache.sh`; fix the docs that call it a substituter | pending |
| 4 | Tests: assertions in `nix/tests/invariants.nix` (pure eval) and `nix/tests/tls.nix` (booted box) | pending |
| 5 | Verification: evaluate `config.nix.settings.{accept-flake-config,trusted-substituters,substituters,upstreams}` from the flake, run the checks, grep the tree | pending |

## Key decisions

- The bug is the flake `nixConfig` confirmation, not a missing URL: Nix asks
  y/N per setting on first evaluation (root, trusted) and *ignores* the settings
  for untrusted users. Evidence recorded for milestone 1.
- Fix at the configuration layer (`nix.settings`), which both the installer
  medium and the installed box import via `modules/cache.nix`, rather than
  pre-seeding `~/.local/share/nix/trusted-settings.json` per user (the WIP on
  the checkpoint ref would only ever cover root).
- `trusted-users` is *not* relaxed (docs: equivalent to root). Only
  `trusted-substituters` names the proxy.
- The Pages copy is dropped from substituters only; the publishing pipeline in
  `.github/workflows/handbook.yml` / `.github/scripts/pages-cache.ts` is left
  alone (out of scope, still documents the copy).

## Blockers

None.
