# SkyPilot Phase 1 — Mesh Serving Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove SkyPilot can serve a catalogue model across the mesh: lenders' RPC servers become reachable over the cluster fabric, the edge runs a pinned SkyPilot API server, and one served model runs as a SkyPilot serving job reachable through the existing proxy with per-token billing unchanged.

**Architecture:** Keep llama.cpp RPC as the memory-pooling substrate. Make each lender's `ggml-rpc-server` listen on its mesh-node address (fenced to the cluster CIDR) instead of edge-loopback, retire the `:llm` tunnel service. Add a new module `modules/skypilot.nix` that deploys a pinned SkyPilot API server onto the edge's rke2 cluster. Convert the first catalogue model from a registrar-managed head (systemd unit on the edge host) to a SkyPilot serving job whose llama-server points at the group's RPC ports, with the model file seeded into a Longhorn volume. Buyers' requests still flow through the registrar proxy; token counting and per-token billing stay at the proxy; only the proxy's backend changes (head socket → SkyPilot endpoint).

**Tech Stack:** NixOS appliance modules, llama.cpp (ggml-rpc-server, llama-server), SkyPilot (pinned from nixpkgs), k3s local cluster, rke2 mesh cluster, Longhorn, the registrar (`backend/` Rust, `edge/registrar/` Rust), the admin UI (React).

**Spec:** docs/superpowers/specs/2026-11-15-skypilot-mesh-design.md

## Global Constraints

- `losos.sharingMyStorage`, `losos.cluster.enable`, `losos.cluster.shareCompute` gates unchanged; no new capability is implied by this phase.
- Everything pinned and offline-buildable (pinned upstream manifests fetched at build time, like `modules/edge-vms.nix`).
- No SSH, no root processes for jobs; `shared` uid (1001) remains the mesh-storage user.
- The LOCAL k3s and this box's apps/cloud/Git untouched.
- Per-token pricing of the served catalogue unchanged.

## Review Focus

- A lender's RPC server reachable over the cluster fabric must still be unreachable from the LAN, the owner's home network, and any non-cluster source (owner iptables rule widened to cluster CIDR only, not opened broadly).
- A serving job that starts when a lender drops mid-group must not leave the proxy hanging or double-bill; the proxy's existing failure handling is the backstop.
- The model file seeded into Longhorn must not be re-downloaded or re-fetched at job start (offline stance).
- Compute-window taints must still repel new serving-job pods even though heads now float.
- A box whose `sharingLocally` is on but `cluster.enable` is off must not accidentally contribute RPC capacity to the official edge's cluster.

---

### Task 1: Lender RPC server listens on mesh-node address

**Files:**
- Modify: `modules/inference-lend.nix` (ExecStart `-H`, firewall rule, tunnel service removal)
- Modify: `modules/proxy.nix` (retire `:llm` tunnel service — or gate it behind the old path until Phase 2)
- Test: `nix/tests/inference-lend.nix` (if exists) or a new `nix/tests/skypilot-rpc-reachability.nix`

**Interfaces:**
- Produces: a lender RPC server reachable at `<mesh-node-ip>:50052` from within the cluster CIDR, fenced to that CIDR, still running as `shared` uid with MemoryMax.

- [ ] **Step 1: Write the failing test** — a NixOS VM test asserting the RPC port answers from a cluster-internal address and is refused from the LAN address.
- [ ] **Step 2: Run test to verify it fails** — expect connection refused on the mesh-node address today.
- [ ] **Step 3: Implement** — change `-H 127.0.0.1` to the mesh-node address; widen the owner iptables rule from `127.0.0.1` to the cluster CIDR; keep MemoryMax and the `shared` user; remove the `:llm` tunnel service from `modules/proxy.nix` (or gate it behind `losos.inference.lend.legacyTunnel` for one release).
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: lender RPC server reachable over cluster fabric`.

### Task 2: Pin and package SkyPilot API server on the edge

**Files:**
- Create: `modules/skypilot.nix` (edge-side deployment)
- Modify: `modules/options.nix` (`losos.edge.skypilot.enable`, package pin)
- Modify: `flake.nix` (wire the module)
- Test: `nix/tests/skypilot-edge.nix` (smoke: API server answers /health)

**Interfaces:**
- Consumes: `losos.cluster.enable` (the edge runs rke2 server), existing edge manifests dir pattern from `modules/edge-vms.nix`.
- Produces: `losos.edge.skypilot.enable` option; SkyPilot API server answering on a known port on the edge.

- [ ] **Step 1: Write the failing test** — smoke test that the API server's health endpoint returns 200 on the edge VM.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — pin SkyPilot from nixpkgs the way other upstream artifacts are pinned; deploy via the rke2 manifests directory (offline, fetched at build time); expose the port through the edge's existing reverse proxy with LosOS Login auth.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: pinned SkyPilot API server on the edge`.

### Task 3: Seed catalogue model into a Longhorn volume

**Files:**
- Modify: `edge/registrar/src/server/llm.rs` (group bookkeeping gains a volume reference)
- Modify: `modules/edge-inference.nix` (model file → Longhorn seed, not a store path env var)
- Test: unit test in `edge/registrar` for volume seeding idempotency

**Interfaces:**
- Consumes: catalogue model id + GGUF store path.
- Produces: a Longhorn volume per model id, seeded once from the pinned store path, mounted by serving jobs.

- [ ] **Step 1: Write the failing test** — registrar unit test: seeding the same model twice is idempotent and does not re-download.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — seed volume from the Nix store path at edge build/activation time; record the volume name in the group's state; serving jobs mount it read-only.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: catalogue models seeded into Longhorn volumes`.

### Task 4: First catalogue model as a SkyPilot serving job

**Files:**
- Modify: `edge/registrar/src/server/llm.rs` (start/stop a serving job instead of a systemd head unit)
- Create: serving-job spec template in `edge/registrar/` (fenced: non-root, caps, no host mounts)
- Test: integration test — buyer request through the proxy returns a completion for the model.

**Interfaces:**
- Consumes: group's RPC port list, seeded volume name, SkyPilot API server endpoint.
- Produces: a SkyPilot serving job per group whose llama-server points at the RPC ports and mounts the model volume; proxy backend switched from head socket to the SkyPilot endpoint.

- [ ] **Step 1: Write the failing test** — proxy request for the model returns a completion (today returns 503/no head).
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — replace `losos-llm-head@<group>.service` start/stop with SkyPilot serve create/terminate; keep the proxy's token counter and per-token billing untouched; the proxy reads the endpoint from the group state.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: first catalogue model served via SkyPilot`.

### Task 5: Compute-window eviction for serving pods

**Files:**
- Modify: `modules/cluster.nix` (NoExecute taint at window close, or reconciler pod deletion)
- Modify: `edge/registrar/src/server/llm.rs` (group dissolution on taint)
- Test: VM test — a pod admitted in-window is evicted when the window closes.

**Interfaces:**
- Consumes: `losos.cluster.computeWindow`, existing taint logic.
- Produces: serving-job pods evicted at window close; in-flight requests fail through the proxy.

- [ ] **Step 1: Write the failing test** — pod admitted at 22:50 with a 23:00 window close is gone by 23:01.
- [ ] **Step 2: Run test to verify it fails.**
- [ ] **Step 3: Implement** — apply NoExecute taint (or delete pods) at window close; registrar dissolves the group and the proxy returns its usual failure.
- [ ] **Step 4: Run test to verify it passes.**
- [ ] **Step 5: Commit** `feat: evict serving jobs at compute-window close`.
