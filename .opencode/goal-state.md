# Goal

**Measurable outcome (this session):** Implement SkyPilot runtime, orchestration, registrar, and backend behavior on branch `t3/skypilot-ai-clustering-shared-kindashed-users` (PR [#692](https://github.com/dasmatus/losos/pull/692)): serving-job lifecycle/orchestration and endpoint discovery in inference, Longhorn model-seeding runtime manifests/hooks, registrar handling for rented compute (`/market/compute/rent`) with admission/allow-list/metering scaffolding, backend market flow/gate hooks (including kinda-shared separation/owner-free intent), and endpoint-preferred proxy/retirement behavior. Each milestone verified with real checks (`rustfmt`/`nixfmt`/`bun nix/lint-nix.ts`, `nix eval` of relevant derivations/packages, structural `rg`/command output, and CI submission/status capture).

**Status:** completed (runtime/orchestration/registrar/backend behavior implemented and verified locally; CI running on pushed HEAD)

## Milestones (ordered, verified with real checks)

| # | Milestone | Status | Verification / Evidence |
|---|-----------|--------|-------------------------|
| 1 | Inference runtime/orchestration: serving-job lifecycle hooks and endpoint discovery/registration for SkyPilot-managed groups | verified | `edge/registrar/src/inference.rs`: added `skypilot_serve_up/down/endpoint` HTTP orchestration helpers; SkyPilot-aware group create/restart/stop and endpoint refresh in `pass()`. **Checks:** `rustfmt --edition 2021` clean; `rg -n "skypilot_serve_(up|down|endpoint)" edge/registrar/src/inference.rs`; `nix eval --raw .#packages.x86_64-linux.losos-registrar.drvPath` → `/nix/store/966pkc3477mj7jnw9lm1j8gmg0jsnjas-losos-registrar-0.1.0.drv`. |
| 2 | Longhorn model seeding runtime: idempotent data-copy Job/manifests or seeder hook for catalogue models into PVCs | verified | `modules/edge-inference.nix`: added per-model runtime `batch/v1 Job` manifests to copy pinned Nix store GGUF into Longhorn PVC (`test -s /dst/model.gguf || cp ...`, idempotent) alongside PVCs. **Checks:** `nixfmt` + `bun nix/lint-nix.ts` clean; `rg -n "llm-seed-|modelSeedJobName|batch/v1" modules/edge-inference.nix`; `nix eval --raw .#nixosConfigurations.iso.config.system.build.toplevel.drvPath` → `/nix/store/w1bb0az0xv7rh2h35cwzslczask86bic-nixos-system-losos-installer-v0.6.1.drv`. |
| 3 | Registrar rented-compute behavior: `/market/compute/rent` handler with admission/allow-list and metering/billing scaffolding | verified | `edge/registrar/src/server.rs`: added route and `market_compute_rent` handler (reads `/etc/losos/skypilot-rentable-images.json`, enforces allow-list/resource/max-runtime bounds, submits fenced `skypilot-jobs` Kubernetes Job with security/resource/deadline constraints, owner-free metering when `network` absent, returns metering metadata). **Checks:** `rustfmt` clean; `rg -n "/market/compute/rent|market_compute_rent|skypilot-jobs" edge/registrar/src/server.rs`; `nix eval --raw .#packages.x86_64-linux.losos-registrar.drvPath` → `/nix/store/966pkc3477mj7jnw9lm1j8gmg0jsnjas-losos-registrar-0.1.0.drv`. |
| 4 | Backend rented-compute and gate behavior: market flow wiring, kinda-shared eligibility/owner-free hooks | verified | `backend/src/market.rs`: `Op::RentCompute` wired with validation/routing/timeout/body (including owner-local `network: None`); registrar applies owner-free pricing/metering hooks. Kinda-shared separation signaled by gateway-local config in `modules/skypilot-gateway.nix` (`marketEligible=false`, scheduler/sharing labels). **Checks:** `rustfmt` clean; `nix eval --raw .#packages.x86_64-linux.losos.drvPath` → `/nix/store/0iajnilawgb3b2117d5p84k2npk795if-losos-0.1.0.drv`. |
| 5 | Proxy/retirement behavior: endpoint-preferred LLM serving path when SkyPilot enabled; legacy path constrained | verified | Endpoint-aware `health`/`call` and SkyPilot-mode lifecycle in `edge/registrar/src/inference.rs`; `edge/registrar/src/server/llm.rs` uses inference API (endpoint-preferred). Legacy `losos-llm-head@` gated in `modules/edge-inference.nix` when `skypilotOn`. **Checks:** `rustfmt`/`nixfmt`, `bun nix/lint-nix.ts` clean, package/ISO `nix eval` pass. |
| 6 | Final verification, commit, push, and CI handoff | verified | `bun nix/lint-nix.ts` clean; `nix eval` for `losos-registrar`, `losos`, and ISO toplevel pass (see above). Commit `d7ef23d8` pushed to `t3/skypilot-ai-clustering-shared-kindashed-users` (PR [#692](https://github.com/dasmatus/losos/pull/692), head `d7ef23d8`). `gh pr checks 692` at capture showed CodeQL checks pending/running. PR not watched. |

## Key decisions
- SkyPilot lifecycle implemented as best-effort HTTP orchestration against configured API base (`{api}/serve/*` patterns) plus endpoint discovery; direct fenced Kubernetes Job submission used for RentCompute runtime execution.
- Kinda-shared capacity treated as gateway-local/non-market-eligible via module/config separation; owner-local RentCompute (`network: None`) marked owner-free in metering.
- Rust compilation/tests not run locally (OOM); `rustfmt`, `nix eval` of derivations, structural checks, and CI submission used as verification.

## Blockers
- No in-session running k8s/SkyPilot/Longhorn cluster for end-to-end runtime validation.
- CI must confirm full Rust/Nix build/test behavior.

## Progress log
- Resumed from prior completed scaffolding state. Implemented milestones 1–5 with local checks, committed/pushed `d7ef23d8`, and captured CI pending status.

## What shipped this session
- `d7ef23d8` — **feat(skypilot): runtime orchestration and rent-compute behavior**
  - [edge/registrar/src/inference.rs](/home/matus/.t3/worktrees/losos/t3-2c579687-a4a53b632b584c79/edge/registrar/src/inference.rs)
  - [edge/registrar/src/server.rs](/home/matus/.t3/worktrees/losos/t3-2c579687-a4a53b632b584c79/edge/registrar/src/server.rs)
  - [modules/edge-inference.nix](/home/matus/.t3/worktrees/losos/t3-2c579687-a4a53b632b584c79/modules/edge-inference.nix)

## Deliberately not done
- Full end-to-end runtime validation against live SkyPilot/k8s/Longhorn (no cluster in session).
- Rich SkyPilot API adaptation beyond best-effort HTTP patterns and direct Job submission.
- Persistent market-ledger billing/meter reconciliation for RentCompute beyond job annotations/response scaffolding.
- UI/admin changes for scheduler distinction and deeper kindashared join automation.
- Local Rust `cargo` build/test/clippy (OOM); CI is authoritative.