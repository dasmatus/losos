# SkyPilot Phase 2 — Registrar Retirement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retire the systemd-head machinery from the registrar: every catalogue model is a SkyPilot serving job, the proxy's backend is always a SkyPilot endpoint, and the market ops (`Op::LlmModels`, `Op::LlmChat`) describe SkyPilot-managed serving rather than registrar-managed heads. Token counting and per-token billing remain at the proxy untouched.

**Architecture:** After Phase 1, one model already flows through SkyPilot. Phase 2 generalises: remove `losos-llm-head@<group>.service` entirely, delete the head env-file writing, and make group lifecycle (create/dissolve/re-group) drive SkyPilot serve create/terminate. The registrar keeps its role as the grouping reconciler (heartbeat TTL, busy/window state) and as the market front door; it stops being a process supervisor for heads. The proxy stops reading head sockets and reads the SkyPilot endpoint from group state exclusively.

**Tech Stack:** Rust (edge/registrar), NixOS modules, SkyPilot CLI/SDK called from the registrar.

**Spec:** docs/superpowers/specs/2026-11-15-skypilot-mesh-design.md
**Depends on:** Phase 1 (mesh serving) complete.

## Global Constraints

- Per-token billing of the served catalogue unchanged; token counting stays at the proxy.
- No change to the market's escrow/account plumbing.
- Offline-buildable: any SkyPilot invocation the registrar makes must work with the pinned install (no runtime pip installs).

## Review Focus

- A group that dissolves must terminate its SkyPilot job; a leaked job keeps billing the lender's memory and the buyer's tokens.
- Re-grouping on lender churn must not create duplicate serving jobs for the same model (idempotency of create).
- The proxy must never fall back to a head socket once Phase 2 lands; stale socket paths would silently 502.
- Escrow/token accounting must be unaffected by the head→job swap (regression risk).

---

### Task 1: Generalise group lifecycle to SkyPilot serve

**Files:**
- Modify: `edge/registrar/src/server/llm.rs` (create/terminate serve jobs for every group, not just the first model)
- Test: unit tests for create-idempotency and terminate-on-dissolve

**Interfaces:**
- Consumes: group RPC port list, seeded volume name, SkyPilot API endpoint (from Phase 1).
- Produces: `create_serve_job(group) -> endpoint`, `terminate_serve_job(group)`; group state carries the endpoint.

- [ ] **Step 1: Write the failing tests** — create called twice for the same group returns the same endpoint without a second job; terminate removes the job.
- [ ] **Step 2: Run tests to verify they fail.**
- [ ] **Step 3: Implement** — generalise Phase 1's single-model path to all catalogue models; keep the reconciler's heartbeat-TTL/busy/window logic as the driver.
- [ ] **Step 4: Run tests to verify they pass.**
- [ ] **Step 5: Commit** `feat: generalise SkyPilot serving to all catalogue models`.

### Task 2: Remove systemd head machinery

**Files:**
- Modify: `modules/edge-inference.nix` (delete `losos-llm-head@.service`, env-file writing, `/run/losos-llm-env`/`/run/losos-llm-heads`)
- Modify: `edge/registrar/src/server/llm.rs` (stop writing env files, stop starting units)
- Test: VM test — no head unit exists; a model still serves.

**Interfaces:**
- Produces: registrar as a pure reconciler + front door, no process supervisor for heads.

- [ ] **Step 1: Write the failing test** — `systemctl status losos-llm-head@*` reports no such unit, yet the proxy serves the model.
- [ ] **Step 2: Run test to verify it fails** (today the unit exists).
- [ ] **Step 3: Implement** — delete the head unit and env-file logic; keep the RPC-fencing and announce heartbeat on boxes.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `chore: retire losos-llm-head systemd units`.

### Task 3: Proxy backend exclusively from group state

**Files:**
- Modify: registrar proxy code (wherever it reads the head socket) to read the SkyPilot endpoint from group state
- Test: regression test — proxy never references a head socket path.

**Interfaces:**
- Consumes: group state endpoint (from Task 1).
- Produces: proxy backend = SkyPilot endpoint only.

- [ ] **Step 1: Write the failing test** — grep/assert no head-socket path in the proxy code path; proxy serves via endpoint.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — remove the socket fallback; fail loudly if group state has no endpoint.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `refactor: proxy backend reads SkyPilot endpoint from group state`.

### Task 4: Market ops describe SkyPilot-managed serving

**Files:**
- Modify: `backend/src/market.rs` (`Op::LlmModels`, `Op::LlmChat` semantics unchanged at the API boundary; internal routing to SkyPilot endpoints)
- Modify: `backend/src/losos.rs` (`cmd_market` LLM paths)
- Test: existing market tests still pass; new test that a served model's endpoint is SkyPilot-backed.

**Interfaces:**
- Consumes: group state (endpoint per model).
- Produces: market API unchanged for buyers; internally backed by SkyPilot.

- [ ] **Step 1: Write the failing test** — market LLM models listing returns endpoints that are SkyPilot serving jobs.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — route `Op::LlmModels`/`Op::LlmChat` through group state; keep escrow/token accounting untouched.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `refactor: market LLM ops backed by SkyPilot serving`.
