# The map

One page of "what lives where", for the first hour in this tree. The
details live in [development.md](development.md), the recorded decisions
in [decisions/](decisions/), the box's trust boundaries in
[security-model.md](security-model.md); this page only says where things
are and which gate is theirs.

## The two systems

`flake.nix` builds the NixOS configurations out of the modules in
`modules/`:

- `install` (and its `iso` twin): the appliance on the mini-PC. No SSH and
  no login shell; lososd is the control plane, and every change an owner
  makes ends in `POST /api/apply`, which writes Nix and rebuilds.
- `edge-gateway`: the edge VPS side — the registrar's mesh, the tunnel's
  far end.

The VM tests under `nix/tests/` boot both for real; the flake exports them
as `checks`, and `devenv shell vm-tests` runs them.

## The Rust

No Cargo workspace, on purpose
([why](decisions/0001-not-a-cargo-workspace.md));
[`crates.json`](../crates.json) is the one list, and devenv, the git
hooks and CI take their crate lists from it.

| Crate | Is | Eaten by |
|---|---|---|
| `backend` | `lososd`, the control plane; `losos-ctl`, its command half | the appliance |
| `backend/access` | the terminal envelope: OpenPGP, signed calls, tokens | `backend`, `edge/registrar`, `edge/vercel`, `admin-ui/tui`, LosOS Desktop (by git) |
| `backend/edge-path` | the edge path rule lososd ships to the edge | `backend`, LosOS Desktop (by git) |
| `backend/mdx` | Markdown and MDX to sanitized HTML | LosOS Office and Notes, as wasm in the browser |
| `backend/plugins` | the plugin SDK and one crate per area (virtual workspace) | lososd loads them as wasm components |
| `edge/registrar` | the master-proxy registrar: orders, the market, passports | the edge |
| `edge/vercel` | the registrar's Vercel demo host | Vercel builds it; linted and tested here |
| `edge/contracts/solana` | the escrow program | the registrar settles through it |
| `admin-ui/tui` | `losos`, the terminal client | you, from your own machine |
| `admin-ui/lab/{core,virt-rpc,render}` | LosOS Lab's wasm pieces | the Lab page |

## Where a change lands

| You are changing | It lives in | Its gate |
|---|---|---|
| an appliance service or option | `modules/*.nix` | the VM test that boots it (`nix/tests/`), `check-eval` |
| a lososd route | `backend/src/`, permissions in `backend/sections.json` | `backend`'s tests, the `discovery` and admin VM tests |
| an admin page or pane | `admin-ui/app/src/` | Playwright (`bun run test:browser`), the `ui` lane |
| a `losos-ctl` command | `backend/src/bin/losos-ctl.rs` | `backend`'s tests, the `ctl` lane |
| the installer or boot path | `backend/src/installer*`, `modules/installer.nix`, `modules/disko.nix` | `nix/tests/iso-install.nix` (the real install), the `install` lane |
| the terminal client | `admin-ui/tui/` | its TLS suite, `test-tui` |
| the edge | `edge/registrar/`, `modules/edge*.nix` | the registrar's regtest suites, the gateway tests |
| the escrow | `edge/contracts/` | `test-solana-escrow`, `test-evm-escrow` |
| the handbook an owner reads | `handbook/docs/` | the handbook build, the `handbook` lane |
| developer notes | `docs/` | nothing builds it; the review does |

The lane table is `.github/scripts/ci-changes.ts`; the git hooks pick the
same lanes for the files you touch, so what passes locally is what CI
runs.

## The habits

- Comments explain why, not what; the gotchas in
  [development.md](development.md) each carry the test that goes red when
  someone "simplifies" them away.
- A regression gets a VM test, the way the `notshared`/`shared` boundary
  got `nix/tests/impermanence.nix`.
- The lists a gate iterates are read from the thing they iterate
  (`flakeAttrs` in devenv.nix, [`crates.json`](../crates.json)), so a new
  entry cannot be forgotten in a hand-typed copy.
