# 0001 · This repository is deliberately not a Cargo workspace

Status: accepted

## Context

The tree carries twelve Cargo manifests: `backend`, four of its satellites
(`access`, `edge-path`, `mdx`, the `plugins` virtual workspace), the
registrar and its Vercel host, the Solana escrow program, the Lab's three
wasm crates and the terminal client. Cargo would happily make them one
workspace: one `Cargo.lock`, one feature resolution, one target directory.
Three forces in this repository say no.

1. **Consumers by git path.** `backend/access` and `backend/edge-path` are
   read by git path by LosOS Desktop's network machine, and `access` is a
   path dependency of `backend`, `edge/registrar` and `admin-ui/tui`, each
   at its own release cadence. A shared lock couples the vendored
   dependency set of every consumer to the most eager one — one
   Dependabot bump becomes everybody's rebuild.
2. **Nix vendors per lock.** Each crate's `Cargo.lock` is vendored by its
   own fixed-output derivation with its own pinned hash in
   `nix/packages.nix`, which `.github/scripts/fix-hashes.py` rewrites. One
   bump is then one hash and one crate to rebuild; a single workspace lock
   would make the crate list itself a build input of every package.
3. **Feature and target isolation.** `backend/mdx` builds for wasm32 with
   a `wasm` feature; `admin-ui/lab/render` swaps features per backend and
   carries wasm32-only Bevy; the plugins are a virtual workspace whose
   members never link into lososd. One workspace's unified resolution
   drags all of that into builds that want none of it — and even
   `cargo fmt` of a parent would reach into the envelope, which its
   consumers format on their own schedule.

## Decision

Keep the workspaces apart. The cost is that every Rust command carries
`--manifest-path`, and that "which crates are there" has to be written
down somewhere — so it is written down exactly once:
[`crates.json`](../../crates.json) is the machine-readable list,
`devenv.nix` and the git hooks (`nix/hooks/lib.ts`) derive their crate
lists from it, and [`nix/check-crates.ts`](../../nix/check-crates.ts)
refuses a tree whose `Cargo.toml` files the manifest does not cover. A new
crate is one manifest entry, a lock and a pinned hash; nothing else holds
a copy of the list to forget.

## Consequences

- Every cargo invocation is explicit about its crate. The devenv scripts
  and the hooks loop over the manifest, so nobody types the list.
- The crate list cannot drift between the tools that use it: they read the
  same file, and the check pins the file to the tree.
- Adding a crate touches `crates.json` and, if nix builds it,
  `nix/packages.nix` — and nothing else.

## What would change it

- A consumer that needs one dependency version shared across several
  crates *and* their parents at runtime — a protocol upgrade the envelope
  cannot carry across separate locks.
- Nix-side vendoring moving to a per-workspace lock that is cheaper than
  the per-crate hashes it replaces.
- The git-path consumers going away, leaving only in-tree dependencies.

If one of those lands, the decision is revisited here, not in a comment.
