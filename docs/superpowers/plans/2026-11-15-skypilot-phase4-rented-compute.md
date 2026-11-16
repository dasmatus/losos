# SkyPilot Phase 4 — Rented Raw Compute Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let market buyers rent raw compute and submit their own fenced SkyPilot jobs, metered per vCPU/MiB-hour, submitted through LosOS operations (the edge constructs the fenced job spec). A box owner's jobs on their own box's lent share are free; anything beyond draws on the market account. Serving keeps per-token billing.

**Architecture:** Add a new market op (shape of the existing VM order flow) for renting compute: the buyer specifies resources and a container image from an allow-list; the edge constructs a fenced SkyPilot job spec (non-root, caps, no host mounts, no devices, network-restricted) and submits it to the appropriate scheduler (official edge for shared capacity, gateway-local for kinda-shared). Metering runs per vCPU/MiB-hour via the scheduler's usage; billing settles through the existing escrow/account plumbing. Owners' jobs on their own box's share bypass billing.

**Tech Stack:** Rust (backend + edge/registrar), NixOS modules (fencing admission), market escrow (existing), admin UI.

**Spec:** docs/superpowers/specs/2026-11-15-skypilot-mesh-design.md
**Depends on:** Phase 1 (serving) and Phase 2 (registrar retirement); Phase 3 for kinda-shared rentals.

## Global Constraints

- Fenced jobs only: non-root (`shared` uid), no host mounts, no devices, network-restricted, resource-capped — enforced by cluster admission, not by the submitter.
- Buyers never author raw YAML and never supply arbitrary images; images come from an allow-list pinned at build time.
- Offline stance: no runtime image pulls; images loaded from pinned archives like apps (`x-losos.images`).
- Compute-second metering; served models keep per-token billing.
- Owners' jobs on their own box's lent share are free.

## Review Focus

- A buyer must not be able to escape the fence (no hostPath, no privileged, no host network, no device access) — cluster admission is the enforcement, and a misconfigured admission policy would silently allow escape.
- A buyer's image must not be pulled from a registry at runtime (offline stance + supply-chain risk).
- Metering must not bill a job that failed to start or was evicted by the compute window (only bill served compute).
- An owner must not be able to bill the market for jobs on their own box's share (free path must be enforced server-side, not client-side).
- Kinda-shared rentals must never route through the official edge.

---

### Task 1: Fenced-job admission policy on the clusters

**Files:**
- Modify: `modules/cluster.nix` (Pod Security `restricted`, NetworkPolicies, resource limits, no host mounts/devices)
- Modify: `modules/skypilot-gateway.nix` (same policy on gateway-local cluster)
- Test: VM test — a privileged/hostPath job submission is rejected by admission.

**Interfaces:**
- Produces: cluster-level admission enforcing the fence on both official and gateway-local clusters.

- [ ] **Step 1: Write the failing test** — submitting a privileged job spec is rejected with an admission error.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — Pod Security `restricted`, default-deny NetworkPolicy, resource quotas, no hostPath/device volumes.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: fenced-job admission policy on mesh clusters`.

### Task 2: Pinned image allow-list

**Files:**
- Modify: `modules/options.nix` (allow-list of rentable images, pinned archives)
- Modify: `modules/defaults.nix` (load images into the cluster like apps)
- Test: unit test — an image not on the allow-list is rejected at submission.

**Interfaces:**
- Consumes: image archive paths.
- Produces: an allow-list the submission op validates against; images loaded offline.

- [ ] **Step 1: Write the failing test** — submitting with an unlisted image returns a refused error.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — allow-list option + validation in the submission op + offline image loading.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: pinned rentable image allow-list`.

### Task 3: Market op for renting compute

**Files:**
- Modify: `backend/src/market.rs` (new `Op::RentCompute` or equivalent, shaped like the VM order flow)
- Modify: `backend/src/losos.rs` (`cmd_market` path + new command)
- Modify: `edge/registrar/src/server/manage.rs` (construct the fenced job spec, submit to the right scheduler)
- Test: integration test — a buyer rents compute, a fenced job runs, usage is recorded.

**Interfaces:**
- Consumes: resources (vCPU, MiB), image (from allow-list), duration/limits.
- Produces: a job id + usage record; job spec constructed server-side.

- [ ] **Step 1: Write the failing test** — buyer rents 1 vCPU; a fenced job appears on the cluster and runs.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — op + command + server-side spec construction + submission to official/gateway scheduler based on capacity chosen.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: market op for renting raw compute`.

### Task 4: Compute-second metering and billing

**Files:**
- Modify: `edge/registrar/src/server/llm.rs` or a new metering module — record vCPU/MiB-seconds per job
- Modify: market billing path to settle compute-seconds via escrow
- Test: unit test — a job running 60s at 1 vCPU bills 60 compute-seconds; a failed/evicted job bills 0.

**Interfaces:**
- Consumes: job start/stop events from the scheduler.
- Produces: usage records settled through escrow.

- [ ] **Step 1: Write the failing tests** — 60s run bills 60s; failed job bills 0.
- [ ] **Step 2: Run tests to verify they fail.**
- [ ] **Step 3: Implement** — meter on job lifecycle events; settle through existing escrow; no billing for failed/evicted jobs.
- [ ] **Step 4: Run tests to verify they pass.**
- [ ] **Step 5: Commit** `feat: compute-second metering and billing`.

### Task 5: Owner share is free

**Files:**
- Modify: submission path — detect owner-of-the-lender and skip billing when the job fits the owner's lent share
- Test: unit test — owner's job on own share bills 0; beyond the share bills normally.

**Interfaces:**
- Consumes: box identity (appliance id), lent share, job resources.
- Produces: free path enforced server-side.

- [ ] **Step 1: Write the failing tests** — owner job within share bills 0; beyond share bills normally.
- [ ] **Step 2: Run tests to verify they fail.**
- [ ] **Step 3: Implement** — server-side check; never trust the client.
- [ ] **Step 4: Run tests to verify they pass.**
- [ ] **Step 5: Commit** `feat: owner jobs on own share are free`.

### Task 6: Kinda-shared rentals route through gateway

**Files:**
- Modify: submission op to pick the gateway-local scheduler when the rented capacity is kinda-shared
- Test: integration test — a kinda-shared rental never touches the official edge.

**Interfaces:**
- Consumes: capacity's sharing mode.
- Produces: routing decision: shared → official edge scheduler; kinda-shared → gateway-local scheduler.

- [ ] **Step 1: Write the failing test** — a kinda-shared rental's job appears only in the gateway's cluster.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — routing by sharing mode; official edge path rejects kinda-shared capacity.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: kinda-shared rentals route through gateway`.
