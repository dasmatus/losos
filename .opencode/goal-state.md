# Goal

**Measurable outcome (this session):** Implement SkyPilot runtime, orchestration, registrar, and backend behavior on branch `t3/skypilot-ai-clustering-shared-kindashed-users` (PR [#692](https://github.com/dasmatus/losos/pull/692)): serving-job lifecycle/orchestration and endpoint discovery in inference, Longhorn model-seeding runtime manifests/hooks, registrar handling for rented compute (`/market/compute/rent`) with admission/allow-list/metering scaffolding, backend market flow/gate hooks (including kinda-shared separation/owner-free intent), and endpoint-preferred proxy/retirement behavior. Each milestone verified with real checks (`rustfmt`/`nixfmt`/`bun nix/lint-nix.ts`, `nix eval` of relevant derivations/packages, structural `rg`/command output, and CI submission/status capture).

**Status:** in progress

## Milestones (ordered, verified with real checks)

| # | Milestone | Status | Verification check |
|---|-----------|--------|--------------------|
| 1 | Inference runtime/orchestration: serving-job lifecycle hooks and endpoint discovery/registration for SkyPilot-managed groups | verified | `rustfmt --edition 2021` applied to `edge/registrar/src/inference.rs`; `rg` shows `skypilot_serve_up/down/endpoint` and lifecycle wiring in `edge/registrar/src/inference.rs` (group start/restart/stop + endpoint discovery in `pass()`); `nix eval --raw .#packages.x86_64-linux.losos-registrar.drvPath` → `/nix/store/ws9v8k5swnf6zd5fbjlh4qwa1d8yh7ry-losos-registrar-0.1.0.drv` |
| 2 | Longhorn model seeding runtime: idempotent data-copy Job/manifests or seeder hook for catalogue models into PVCs | verified | Added per-model runtime seed `batch/v1 Job` manifests (copy from pinned Nix store hostPath into Longhorn PVC, idempotent `test -s || cp`) alongside PVCs in `modules/edge-inference.nix`; `nixfmt` + `bun nix/lint-nix.ts` clean; `nix eval --raw .#nixosConfigurations.iso.config.system.build.toplevel.drvPath` → `/nix/store/w1bb0az0xv7rh2h35cwzslczask86bic-nixos-system-losos-installer-v0.6.1.drv` |
| 3 | Registrar rented-compute behavior: `/market/compute/rent` handler with admission/allow-list and metering/billing scaffolding | verified | Added route and `market_compute_rent` handler in `edge/registrar/src/server.rs` (loads `/etc/losos/skypilot-rentable-images.json`, enforces allow-list/resource bounds, submits fenced `batch/v1 Job` to `skypilot-jobs` with security/cap/resource/deadline constraints, returns metering metadata). `rustfmt` applied; `rg -n "market/compute/rent|market_compute_rent"` confirms wiring; `nix eval --raw .#packages.x86_64-linux.losos-registrar.drvPath` → `/nix/store/2lzzpqhamh62gzliikmzy2q2qy1r7qxf-losos-registrar-0.1.0.drv` |
| 4 | Backend rented-compute and gate behavior: market flow wiring, kinda-shared eligibility/owner-free hooks | verified | `backend/src/market.rs` includes `Op::RentCompute` with validation (incl. `network: None` owner-local), route (`POST /market/compute/rent`), timeout/max-reply/body serialization; registrar enforces owner-free metering/pricing when `network` absent and records `losos.dev/owner-free`. Kinda-shared separation/market ineligibility signaled via gateway-local config (`modules/skypilot-gateway.nix`: `marketEligible=false`). `rustfmt` clean; `nix eval --raw .#packages.x86_64-linux.losos.drvPath` → `/nix/store/0iajnilawgb3b2117d5p84k2npk795if-losos-0.1.0.drv` |
| 5 | Proxy/retirement behavior: endpoint-preferred LLM serving path when SkyPilot enabled; legacy path constrained | verified | `edge/registrar/src/inference.rs` uses endpoint-aware `health`/`call` and SkyPilot lifecycle when `skypilot_api` set; `edge/registrar/src/server/llm.rs` routes through inference (no direct socket coupling). `modules/edge-inference.nix` gates legacy `losos-llm-head@` on `!skypilotOn`. `rustfmt`/`nixfmt` clean; `nix eval` drvPaths for `losos-registrar` and ISO toplevel succeed. |
| 6 | Final verification, commit, push, and CI handoff | pending | `bun nix/lint-nix.ts` clean, key `nix eval` checks pass, `git push` succeeds, `gh pr checks 692` captured for pushed HEAD |

## Key decisions
- Local `cargo` build/test not feasible (OOM); use `rustfmt`, `nix eval` of package derivations, structural checks, and CI as verification.
- Prefer implementable, testable scaffolding with real control flow over stubs where full cluster/runtime is unavailable.
- SkyPilot API interaction implemented as HTTP client calls against configured API base when available; runtime cluster side remains deploy-time concern.

## Blockers
- No running k8s/SkyPilot/Longhorn cluster in-session for end-to-end runtime validation.
- Local Rust compilation via cargo unavailable; Nix package builds may be expensive but derivation evals are used as checks.

## Progress log
- Resumed: read existing state and codebase (`inference.rs`, `server/llm.rs`, `market.rs`, modules). Planning implementation.