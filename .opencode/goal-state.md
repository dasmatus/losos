# Goal

**Measurable outcome (this session):** Bring SkyPilot **Phase 1 Tasks 3–5** and **Phases 2–4** to a completed, verifiable state on branch `t3/skypilot-ai-clustering-shared-kindashed-users` / PR [#692](https://github.com/dasmatus/losos/pull/692): design intent from `docs/superpowers/specs/2026-11-15-skypilot-mesh-design.md` and phase plans reflected in code/modules, touched Nix/Rust formatted (`nixfmt`, `rustfmt`), `nix eval .#nixosConfigurations.iso.config.system.build.toplevel.drvPath` passes, structural checks (`rg`/inspection) confirm required scaffolding/hooks for each phase, and CI failures introduced by this work are fixed with evidence (push + CI results). Local `cargo` build/test/clippy not feasible (OOM); CI is the authoritative Rust verifier.

**Status:** in progress (resumed session)

## Milestones (ordered, verified with real checks)

| # | Milestone | Status | Verification check |
|---|-----------|--------|--------------------|
| 1 | Phase 1 Task 3 — Volume seeding: model volume identity/bookkeeping and Nix hook for seeding path | pending | `rg` shows `Group.volume`/`HeadSpec.volume`/`model_volume_name` in `edge/registrar/src/inference.rs` and seeding hook/service or manifest reference in `modules/edge-inference.nix` or `modules/skypilot.nix`; `rustfmt --check` + `nixfmt --check` on touched files; `nix eval` ISO toplevel passes |
| 2 | Phase 1 Task 4 — First-model SkyPilot serving: endpoint-aware serving path and start/health/call plumbing for HTTP endpoints | pending | `rg` confirms `Group.endpoint`, `http_request`, endpoint-aware `health`/`call` and start path in `edge/registrar/src/inference.rs`; formatted; `nix eval` passes; CI clippy/test for registrar addressed for changes made |
| 3 | Phase 1 Task 5 — Compute-window eviction (serving evicted, VMs survive) | verified | `modules/edge.nix` applies `closed:NoExecute`; `edge/registrar/src/vms.rs` + `modules/edge-vms.nix` tolerate all effects on `losos.dev/compute-window`; `nix eval` pass (evidence: `/nix/store/gpcs8wvn9haq5f92fm6p90zixgqa5xjw-...drv`) |
| 4 | Phase 2 — Registrar retirement/generalisation: legacy head path gated/retired when skypilot enabled, proxy/ops use endpoint-capable path | pending | `rg "losos-llm-head@" modules/edge-inference.nix` shows gated/retired for skypilot mode; inference/llm server paths structurally endpoint-capable; `nix eval` passes; CI clippy/tests addressed |
| 5 | Phase 3 — Gateway-local SkyPilot for kinda-shared (`sharingLocally`): module/options wired, local scheduling gate/hooks | pending | `modules/skypilot-gateway.nix` present and wired in `flake.nix`/options; Nix/module hooks for gateway-local/kinda-shared separation present; `nix eval` passes; structural checks for gate/config present |
| 6 | Phase 4 — Rented compute scaffolding + enforcement hooks: rentable options, market op, admission/allow-list hooks | pending | `edge.skypilot.rentableImages`/pricing options in `modules/options.nix`; `Op::RentCompute` in `backend/src/market.rs` with validate/route/body; admission/allow-list config or module hook present; `rustfmt`/`nixfmt`; `nix eval` passes; CI clippy/tests addressed |
| 7 | CI/push verification for this session's changes | pending | `git push` succeeds, `gh pr checks 692` inspected; failures caused by this session's changes fixed and re-pushed with evidence; remaining failures (if any) identified as pre-existing/unrelated with evidence |

## Key decisions
- No local `cargo build/test/clippy` (OOM). Use `rustfmt`, structural review, and CI runs as Rust verification evidence.
- `modules/skypilot.nix` manifest hash left for GH Actions pin-update bot.
- Compute-window: NoExecute evicts serving pods; VMs/CDI tolerate all effects.
- Legacy head path gated by `!edge.skypilot.enable` for migration (structural retirement in skypilot mode).

## Blockers
- CI environment performs authoritative Rust/Nix checks; local feedback limited.

## Progress log
- Resumed: read prior state, ran rustfmt on touched Rust, inspected CI (nix flake check, rust clippy/test failures on PR). Updating plan to fix failures and complete milestones 1,2,4,5,6.