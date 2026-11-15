# Goal

**Measurable outcome:** Implement SkyPilot Phase 1 Tasks 3–5 and Phases 2–4 in the LosOS codebase such that (a) Nix module wiring evaluates (`nix eval` for relevant configurations/derivations), (b) Nix files pass `nixfmt`/lint on touched files, (c) Rust changes compile under CI (local `cargo` build/check not feasible due to RAM; evidence is CI on pushed branch), and (d) design intent from spec/plans is reflected in code (structural verification via inspection/grep/commits). Pins for SkyPilot artifacts may be filled by GH Actions pin-update bot as noted by user.

**Status:** partially completed (pushed to PR; CI/pin-bot validation in progress)

## Milestones

| # | Milestone | Status | Verification / Evidence |
|---|-----------|--------|-------------------------|
| 1 | Phase 1 Task 3: Longhorn volume seeding for catalogue models | partial | `nix eval .#nixosConfigurations.iso.config.system.build.toplevel.drvPath` → `/nix/store/gpcs8wvn9haq5f92fm6p90zixgqa5xjw-nixos-system-losos-installer-v0.6.1.drv` (pass). `Group.volume`, `HeadSpec.volume`, `model_volume_name` added in `edge/registrar/src/inference.rs` (commit `6c13e453`). Full Longhorn PVC seed job in Nix not implemented; Rust correctness deferred to CI. |
| 2 | Phase 1 Task 4: SkyPilot serving job path scaffolding (endpoint in group state, HTTP request path) | partial | `Group.endpoint`, `http_request` (reqwest) and endpoint-aware `health`/`call` in `edge/registrar/src/inference.rs` (commit `6c13e453`). `nix eval` passes. Full SkyPilot serve create/terminate orchestration and endpoint registration not fully wired; Rust correctness deferred to CI. |
| 3 | Phase 1 Task 5: Compute-window eviction for serving pods (NoExecute), VMs survive | verified | `modules/edge.nix` taint script now applies `losos.dev/compute-window=closed:NoExecute`; `edge/registrar/src/vms.rs` and `modules/edge-vms.nix` tolerations set to `operator: Exists` (all effects) so VMs/CDI survive. `nixfmt` applied; `nix eval` passes. Commits `6c13e453`. |
| 4 | Phase 2: Registrar retirement scaffolding | partial | `modules/edge-inference.nix` gates `losos-llm-head@` service on `!edge.skypilot.enable` (commit `262a31b1`); endpoint/volume scaffolding in inference. Full proxy-endpoint-only switch and market-op backend rework incomplete; CI validates. |
| 5 | Phase 3: Gateway-local SkyPilot module/options wired | partial | `modules/skypilot-gateway.nix` added (k3s server + manifest skeleton + options `losos.edge.gateway.skypilot.{enable,port}`), wired in `flake.nix` (commit `262a31b1`). `nix eval` passes. Kinda-shared agent join, market gate extension, admin UI distinction not implemented. |
| 6 | Phase 4: Rented compute scaffolding (options + market op types) | partial | `modules/options.nix`: `edge.skypilot.rentableImages`, `computePricePerCpuHour`, `computePricePerMibHour`. `backend/src/market.rs`: `Op::RentCompute` variant with validation, route (`POST /market/compute/rent`), timeout/reply/body serialization (commit `262a31b1`). `nix eval` passes. Registrar handler, metering/billing, admission policies, owner-free and routing logic not implemented; Rust correctness deferred to CI. |
| 7 | Push, update PR, let CI and pin bot run | verified | Commits pushed to `t3/skypilot-ai-clustering-shared-kindashed-users` (HEAD `262a31b1`), PR #692 updated. `gh pr checks 692` shows checks pending/running (CodeQL, label, gate, etc.) at time of capture — CI and GH Actions pin-update bot can act on the branch. |

## Key decisions

- Local cargo build/test not possible (OOM). Rust verification delegated to CI ("build it and let CI check it"). Nix verified locally with `nix eval` and `nixfmt`.
- Compute-window eviction: taint changed to `NoExecute` for serving-job eviction; VMs/CDI tolerations changed to tolerate all effects on `losos.dev/compute-window` so long-lived hosted workloads survive.
- SkyPilot manifest/hash placeholders left for GH Actions pin-update bot.
- `RentCompute` op added structurally in backend; registrar execution/metering/billing handlers are follow-up for CI iteration.
- Legacy systemd head path retained but gated by `!edge.skypilot.enable`.

## What shipped (commits this effort on branch)

- `9f19cffa` — lender RPC reachable over cluster fabric (Task 1, VM test green earlier)
- `0d75b920` — SkyPilot edge module skeleton (Task 2)
- `db0feb28` — nixfmt fix
- `6c13e453` — Phase 1 Tasks 3–5 scaffolding + compute-window NoExecute eviction
- `262a31b1` — Phases 2–4 scaffolding (gateway module, rentable compute options/op, head unit gating) [rebased on remote merge]
- PR: https://github.com/dasmatus/losos/pull/692

## Deliberately not done / remaining

- Full Longhorn PVC seeding automation and SkyPilot serve create/terminate + endpoint registration in registrar.
- Complete registrar retirement (proxy endpoint-only, market LLM ops fully backed by SkyPilot endpoints).
- Kinda-shared gateway cluster join logic, market gate extension for SkyPilot capacity, admin UI scheduler distinction.
- RentCompute registrar handler, compute-second metering/billing, admission/fencing policies for rented jobs, owner-free enforcement, gateway routing for kinda-shared rentals.
- End-to-end integration/VM tests for new paths (rely on CI).
- Local Rust compile verification (environment constraint).