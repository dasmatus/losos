# Goal

**Measurable outcome:** Implement SkyPilot Phase 1 Tasks 3–5 and Phases 2–4 in the LosOS codebase such that (a) Nix module wiring evaluates (`nix eval` for relevant configurations/derivations), (b) Nix files pass `nixfmt`/lint on touched files, (c) Rust changes compile under CI (local `cargo` build/check not feasible due to RAM; evidence is CI on pushed branch), and (d) design intent from spec/plans is reflected in code (structural verification via inspection/grep/commits). Pins for SkyPilot artifacts may be filled by GH Actions pin-update bot as noted by user.

## Milestones

| # | Milestone | Status | Verification |
|---|-----------|--------|--------------|
| 1 | Phase 1 Task 3: Longhorn volume seeding for catalogue models (registrar bookkeeping + spec/Nix hooks) | partial | `nix eval .#nixosConfigurations.iso.config.system.build.toplevel.drvPath` passes; `Group.volume`/`HeadSpec.volume` and `model_volume_name` added in `edge/registrar/src/inference.rs`; full Longhorn PVC seed job in Nix deferred — CI validates Rust |
| 2 | Phase 1 Task 4: SkyPilot serving job path scaffolding (endpoint in group state, HTTP request path) | partial | `Group.endpoint`, HTTP request helper (reqwest) and endpoint-aware `health`/`call` in `inference.rs`; `nix eval` passes; full SkyPilot serve create/terminate orchestration and endpoint registration not fully wired — CI validates Rust |
| 3 | Phase 1 Task 5: Compute-window eviction for serving pods (NoExecute), VMs survive | verified | `modules/edge.nix` taint script applies `closed:NoExecute`; `edge/registrar/src/vms.rs` and `modules/edge-vms.nix` tolerations accept all effects so VMs/CDI survive; `nix eval` passes; code inspection confirms separation |
| 4 | Phase 2: Registrar retirement scaffolding | partial | `modules/edge-inference.nix` gates `losos-llm-head@` on `!skypilotOn`; inference HTTP/endpoint path added; full proxy-endpoint-only and market-op rework incomplete — CI validates Rust |
| 5 | Phase 3: Gateway-local SkyPilot module/options wired | partial | `modules/skypilot-gateway.nix` created with options and k3s/manifest skeleton; wired in `flake.nix`; `nix eval` passes; kinda-shared cluster join, market gate extension, and UI changes not fully implemented |
| 6 | Phase 4: Rented compute scaffolding (options + market op types) | partial | `edge.skypilot.rentableImages`/pricing options added; `Op::RentCompute` variant, validation, routing, timeout/reply/body handling added in `backend/src/market.rs`; `nix eval` passes; registrar handler/metering/admission policies/owner-free routing not fully implemented — CI validates Rust |
| 7 | Push, update PR, let CI and pin bot run; record results | in_progress | push/PR update + CI status to be captured |

## Key decisions

- Local cargo build/test not possible (OOM). Rust verification delegated to CI ("build it and let CI check it"). Nix verified locally with `nix eval` and `nixfmt`.
- Compute-window eviction: changed taint to `NoExecute` so serving pods are evicted at window close; VMs and CDI tolerations changed to tolerate all effects on `losos.dev/compute-window` so hosted VMs/importers survive.
- SkyPilot manifest hash in `modules/skypilot.nix` left for GH Actions pin-update bot.
- `RentCompute` market op added structurally in backend; registrar-side execution/metering/billing handlers are follow-up work for CI iteration.
- Legacy systemd head path kept but gated by `!edge.skypilot.enable` to allow migration.

## Blockers

- No local RAM for `cargo check/build` of registrar/backend — CI is the verifier; Rust additions may need CI-driven fixes.
- Full NixOS VM integration tests not run locally.

## Progress log

- Prior: Tasks 1-2 done (9f19cffa, 0d75b920, db0feb28), PR #692 created.
- This session: Task 5 verified (NoExecute eviction + VM survival). Tasks 3-4/Phases 2-4 partially implemented (inference endpoint/volume scaffolding, gateway module, rentable compute options+op). `nix eval` passes. Pushing for CI.

## Completion status

Not fully completed: milestones 1,2,4,5,6 are partial; milestone 7 pending CI results. Remaining work requires CI feedback and deeper Rust/cluster integration.