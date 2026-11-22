# SkyPilot Phase 3 — Gateway-Local SkyPilot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make kinda-shared capacity schedulable, but only within its network's scope: each local gateway runs its own SkyPilot API server over a gateway-local cluster, and kinda-shared boxes join that cluster instead of the official edge's. The official edge never touches kinda-shared capacity.

**Architecture:** A gateway box (`losos.edge.gateway.enable`) gains a SkyPilot API server and a small local cluster (k3s, since rke2 on a gateway is heavy and the gateway already runs edge services). Kinda-shared boxes, when `losos.sharingLocally` is on, join the gateway's cluster as agents (compute contribution gated by the existing compute window/idle taint) instead of — or in addition to — the official edge's rke2 cluster. The mesh page and the admin UI distinguish "this network's gateway" from "the official edge" as separate schedulers. Kinda-shared boxes keep `cluster.enable` (official edge) independent, exactly as today.

**Tech Stack:** NixOS modules, k3s (gateway-local cluster), SkyPilot (pinned), existing gateway/mDNS discovery (`backend/src/edge.rs` `check_local_gate`, `local_sharing_tick`).

**Spec:** docs/superpowers/specs/2026-11-15-skypilot-mesh-design.md
**Depends on:** Phase 1 (mesh serving) for the serving-job machinery; Phase 2 helpful but not strictly required.

## Global Constraints

- `losos.sharingLocally` and `losos.cluster.enable` remain independent options; kinda-shared does not imply official-edge membership.
- `nix/tests/kindashared.nix`'s VM topology (gateway + box + laptop) must still assert the sharingLocally gate; the test gains a SkyPilot assertion, not a rewrite.
- Gateway boxes are smaller than the edge VPS: the local SkyPilot server must be lightweight enough to coexist with the gateway's existing services.
- No kinda-shared capacity is ever sold on the market.

## Review Focus

- A kinda-shared box must not register with the official edge's cluster merely because it also runs a gateway-local agent (no accidental dual membership leaking capacity upward).
- A buyer on the market must never be routed to kinda-shared capacity (the market gate must reject it, as today).
- Gateway failure must not take kinda-shared boxes' own services down (the two-cluster lesson: a box's own services live on its local cluster, not the gateway's).
- The gateway-local SkyPilot server must not starve the gateway's own services of resources.

---

### Task 1: Gateway-local SkyPilot server + cluster

**Files:**
- Create: `modules/skypilot-gateway.nix` (k3s server + pinned SkyPilot API server on gateway boxes)
- Modify: `modules/options.nix` (`losos.edge.gateway.skypilot.enable` or reuse `losos.edge.skypilot` gated on gateway role)
- Modify: `flake.nix`
- Test: `nix/tests/skypilot-gateway.nix` (smoke: gateway SkyPilot health endpoint)

**Interfaces:**
- Produces: a gateway box running a SkyPilot API server over a local k3s cluster, answering on a known port.

- [ ] **Step 1: Write the failing test** — gateway VM's SkyPilot health endpoint returns 200.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — k3s server on the gateway (separate from any mesh agent), pinned SkyPilot deployed via manifests, resource-capped so gateway services survive.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: gateway-local SkyPilot server`.

### Task 2: Kinda-shared boxes join the gateway's cluster

**Files:**
- Modify: `modules/cluster.nix` (a second, gateway-local agent join, gated on `losos.sharingLocally` and a discovered gateway)
- Modify: `backend/src/edge.rs` (`check_local_gate`, `local_sharing_tick` — announce gateway-local membership)
- Test: extend `nix/tests/kindashared.nix` with a SkyPilot assertion.

**Interfaces:**
- Consumes: gateway discovery (mDNS / configured URL), existing `losos.sharingLocally`.
- Produces: a kinda-shared box registered as an agent of its gateway's cluster, with the compute-window taint applied.

- [ ] **Step 1: Write the failing test** — in the kindashared VM topology, the box's mesh node appears in the gateway's cluster only while sharingLocally is on.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — agent join gated on sharingLocally + gateway presence; taint applied per compute window; no registration with the official edge.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: kinda-shared boxes join gateway-local cluster`.

### Task 3: Market gate rejects kinda-shared capacity

**Files:**
- Modify: `backend/src/edge.rs` / market gate (`check_market_gate`) to ensure kinda-shared capacity is never listed
- Modify: `backend/src/apps.rs` (the test that kinda-shared is offered — keep the offer, keep the refusal)
- Test: unit test — a kinda-shared box's capacity never appears in market listings.

**Interfaces:**
- Consumes: sharing mode of the box.
- Produces: market listings exclude kinda-shared capacity (unchanged behaviour, now covering SkyPilot capacity too).

- [ ] **Step 1: Write the failing test** — market browse from a kinda-shared box returns no SkyPilot capacity listing.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — extend the existing gate to SkyPilot capacity; no new gate, just coverage.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: market excludes kinda-shared SkyPilot capacity`.

### Task 4: Admin UI distinguishes the two schedulers

**Files:**
- Modify: admin UI Mesh page (show "this network's gateway" scheduler vs "official edge" scheduler)
- Test: UI smoke / plan screenshot (reuse `nix/tests/kindashared.nix` shot harness)

**Interfaces:**
- Consumes: edge status (`/api/edge` role: `localGateway` vs official).
- Produces: Mesh page shows which scheduler a box's capacity is contributing to.

- [ ] **Step 1: Write the failing test** — UI shows the gateway scheduler when sharingLocally.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — surface the scheduler role in the Mesh page using existing role strings.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: admin UI shows gateway-local vs official scheduler`.
