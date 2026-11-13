# Contributing to LosOS

LosOS is a NixOS appliance for a mini-PC: your files, no SSH and no login
shell, administered from a web page. The
[security model](docs/security-model.md) says what the box trusts and why;
reading it first is the fastest way to understand why the code looks the
way it does.

## Getting a shell

```sh
devenv shell        # or: nix develop, or: direnv allow
devenv test         # the fast gates: pins, crates, lint, rust tests, flake eval
```

`devenv test` builds nothing and boots nothing; it is what CI's quick
lanes run. The git hooks install themselves (`nix/hooks/`): pre-commit
replays CI's quick gates for the files you touch, pre-push the slower
ones, including the NixOS VM tests your change reaches (they want
`/dev/kvm`; without it they run under TCG). [Development](docs/development.md)
is the long version of all of this.

## Finding your way around

- [docs/map.md](docs/map.md) — the one-page map: what lives where, which
  crate eats which, where a change for feature X lands and which test
  guards it.
- [docs/development.md](docs/development.md) — the toolchain, the gates,
  and "Gotchas that bite silently", each one paid for once.
- [docs/decisions/](docs/decisions/) — recorded decisions and why; for
  example [why there is no Cargo
  workspace](docs/decisions/0001-not-a-cargo-workspace.md).
- `.claude/CLAUDE.md` — the one-line rules behind those gotchas.

## Making the change

1. Branch, change, `devenv test` (the hooks run the same for your files).
2. Open a pull request. The template is the review: Before/After prose, a
   screenshots table (required for anything a person can see — same
   window, same state, Before and After), How, and Tested, which must say
   what did *not* run and why.
3. CI picks the lanes your files reach; a job that shows as skipped is one
   your change does not need. Label `ci: full` re-runs everything when you
   want it anyway.

Comments in this repository explain why, not what, and every gotcha in
development.md is pinned by a test that goes red when someone "simplifies"
it away. If your change hits one, the test will tell you and the text
tells you why it is not a bug.

Commits that credit an AI assistant or link a session are rewritten out of
the history on main, and the `commit-msg` hook refuses them up front. Say
what changed, not what wrote it.

## Security problems

Please do not open an issue. [SECURITY.md](SECURITY.md) has the private
channel; the threat model in
[docs/security-model.md](docs/security-model.md) says what the box already
assumes an attacker can do.

## Starter tasks

Worth a first pull request; each is scoped to one module, page or suite,
with the gate it touches named.

- **A handbook fix from your own install.** Anything the owner's handbook
  (`handbook/docs/`) gets wrong or skips. You just installed a box, so you
  are the expert; [For developers](handbook/docs/reference/for-developers.md)
  says how to build and preview it.
- **A translation.** The admin UI and the handbook carry i18n catalogues
  (`admin-ui/app/src/i18n/`, `handbook/i18n/`); a new language is
  self-contained and the browser test checks the fallbacks.
- **An invariant.** `nix/tests/invariants.nix` pins the option values the
  appliance cannot afford to lose to a drifting default. If you found a
  setting you had to re-apply by hand, pin it there and name the VM test
  that boots it.
- **A per-module exposure row.** `docs/security-model.md` describes the
  box's boundaries in prose. Pick one module under `modules/`, write down
  what it exposes (ports, secrets, network reach) and which VM test guards
  it, and send it as a start on the table that doc is growing into.
- **First-person docs.** [docs/map.md](docs/map.md) and
  [docs/development.md](docs/development.md) are written by the person who
  knows the code; a paragraph you had to ask for in chat belongs in one of
  them.

## Licence

AGPL-3.0-or-later. The tree is REUSE-compliant (`REUSE.toml`), which
covers the whole repository with one licence; a contribution merged here
is licensed the same way, and no per-file header is needed.
