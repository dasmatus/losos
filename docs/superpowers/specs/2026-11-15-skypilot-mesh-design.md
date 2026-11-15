# SkyPilot as the mesh's AI job scheduler — design

Date: 2026-11-15
Status: design

## Goal

Make SkyPilot (https://github.com/skypilot-org/skypilot) the one scheduler for
AI work on the mesh, replacing the bespoke llama.cpp lending pipeline
(`modules/inference-lend.nix` + `modules/edge-inference.nix` + the registrar's
head grouping), while keeping the market, the sharing model and the appliance's
security stance intact.

What SkyPilot must do, as decided with the user:

1. **Mesh-wide AI job scheduler** — AI jobs (inference, fine-tuning, batch)
   scheduled across shared and kinda-shared boxes' CPU/memory.
2. **Edge-managed** — the edge VPS runs the SkyPilot API server; the mesh
   rke2 cluster is the compute substrate.
3. **Marketable** — shared capacity appears on the market; buyers may also
   rent **raw compute** and submit their own fenced SkyPilot jobs; the served
   models catalogue keeps **per-token pricing**.

## Context: what exists today

- **Two Kubernetes instances per box** (`modules/cluster.nix`): a LOCAL k3s
  server for this box's own `shared` apps (no off-box dependency), and a MESH
  rke2 agent joining the edge VPS's cluster (`losos.cluster.enable`, off by
  default) for Longhorn storage and mesh compute. The two-instances split is
  deliberate; do not collapse it.
- **Compute sharing** (`losos.cluster.shareCompute`): "share my compute when
  I sleep" — the edge holds a NoSchedule taint on the box's mesh node outside
  the compute window or while the box is busy.
- **LLM pipeline (to be replaced)**: each lending box runs a llama.cpp RPC
  server (`ggml-rpc-server`) as the `shared` uid, loopback-only, fenced with
  MemoryMax and an iptables owner rule (`modules/inference-lend.nix`); its
  tunnel client carries it to the edge as the service
  `<appliance id>:llm`; the registrar groups lenders until a catalogue model
  fits, runs a llama-server head per group as `losos-llm`, proxies buyers'
  requests, counts tokens, and bills per model `priceIn`/`priceOut` through
  the market (escrow, `POST /market/llm/models`, `/market/llm/chat`).
- **Sharing spectrum**: `losos.sharingMyStorage` (full mesh sharing, market
  eligible), `losos.sharingLocally` (kinda-shared: this network's local
  gateways only, **refused to the market** — `backend/src/apps.rs` tests
  that kinda-shared is offered but `check_local_gate` /
  `local_sharing_tick` keep it off the market).
- **The market already sells compute-adjacent things**: VMs hosted on boxes
  (`losos.vms.host`, half earnings to the owner, disks in the shared user's
  home) and disk replicas on the rke2 cluster (`modules/edge-vms.nix`).
- **Security stance**: stateless appliance, tmpfs root, no SSH, everything
  sandboxed and non-root; "the other side is not trusted" is the standing
  assumption (inference-lend's three fences exist because the head is not
  fully trusted).

## Decisions made with the user

- **Replace, not coexist**: SkyPilot becomes the one scheduler; the
  registrar's grouping/billing-of-heads logic is retired or reworked. The
  llama.cpp RPC lending primitive itself survives as the memory-pooling
  substrate (see "How it hangs together").
- **Jobs are fenced**: a SkyPilot job arriving from another box runs as a
  non-root pod (the `shared` uid pattern), no host mounts, no devices,
  network-restricted, with resource caps — the existing inference-lend
  fencing generalized into scheduler admission rules.
- **Buyers can rent raw compute** and submit their own fenced jobs to mesh
  boxes, beside the served-models catalogue.
- **Per-token pricing stays** for the served catalogue.

## Goals

- One AI scheduler on the mesh: model serving, fine-tuning and batch jobs
  scheduled over shared and kinda-shared boxes.
- The edge runs the SkyPilot API server; boxes contribute capacity to it
  through the mesh rke2 cluster they already join.
- The sharing model is the admission model: shared capacity is market
  eligible, kinda-shared capacity is schedulable only within its local-gateway
  scope and never sold.
- Buyers get the served-models catalogue (per-token, as today) **and** can
  rent raw compute to run their own fenced jobs.
- Everything pinned and offline-buildable the way the rest of LosOS is
  (pinned upstream manifests fetched at build time, like
  `modules/edge-vms.nix` does).

## Non-goals

- The LOCAL k3s, this box's own apps, cloud, Git: untouched. They live on the
  local cluster with no taint and no scheduler.
- No GPU support in this design: these boxes are CPU/memory lenders. SkyPilot
  is chosen for scheduling, not for GPUs.
- No SSH-based SkyPilot clusters and no cloud-provider backends; the mesh
  rke2 cluster is the only executor.
- No new pricing model for served inference: per-token stays.

## Anti-goals (a failure even if it works)

- Any path where a job landing on a box runs as root, sees the owner's home,
  devices or host mounts, or is not capped in memory/CPU.
- Any path where kinda-shared capacity is schedulable from outside its local
  network scope or is sold on the market.
- Making the box's own services depend on the edge (the two-cluster lesson in
  `modules/cluster.nix`).

## How it hangs together

The mechanism that must survive replacement is llama.cpp RPC memory pooling:
a model larger than one box's RAM is split across boxes' memories over RPC.
SkyPilot's k8s scheduler does not do "one process spanning three boxes'
RAM" natively, so the RPC servers stay as the capacity primitive and the
scheduling layers change around them:

- **Boxes** keep running `ggml-rpc-server` as `shared`, fenced exactly as
  today (loopback, tunnel service `<appliance id>:llm`, MemoryMax, owner
  rule) and keep announcing their lent memory in the heartbeat. `sharingLocally`
  and `cluster.shareCompute`/compute-window tainting continue to gate
  visibility, now as scheduler visibility.
- **The edge** runs the SkyPilot API server against the rke2 mesh cluster.
  A model of the catalogue becomes a serving job (SkyPilot serve) whose
  llama-server points at the group's RPC ports; the grouping that the
  registrar did with systemd heads becomes the scheduler's placement of
  those serving jobs. Buyers' requests flow through the registrar's proxy
  as today — token counting and per-token billing stay at the proxy — with
  the proxy's backend now a SkyPilot-managed endpoint instead of a
  registrar-managed head socket.
- **Job submission** has a single front door on the edge (the registrar or a
  sibling service in front of the SkyPilot API server). Nobody talks to the
  cluster directly: submission, authentication (LosOS Login/passport
  identity), market billing and the kinda-shared admission check all happen
  at that front door.
- **Fenced jobs** are enforced by cluster-level admission on the rke2 server
  (Pod Security `restricted`, NetworkPolicies, resource limits, no host
  mounts or devices) — enforced by the cluster, not promised by the
  submitter. Kinda-shared placement is enforced at the same layer: a
  kinda-shared box's mesh node is only schedulable for jobs admitted through
  its local-gateway scope (mechanism — taint + admission-gated tolerations
  or namespace scoping — is the builder's to settle).
- **Raw compute rentals** appear as market listings over the same capacity,
  submitted through LosOS ops with the edge constructing the fenced job
  spec, metered per vCPU/MiB-hour. Kinda-shared capacity is rented and
  scheduled only through its network's gateway-local SkyPilot server, never
  through the official edge. Serving keeps per-token billing; a box owner's
  jobs on their own box's lent share are free.

## Calls made during the builder check (the user may veto)

These were raised by the builder check as plan-shaping questions; the answers
follow from the design's own commitments and are marked as calls:

- **Serving placement is real, not pinned to the edge.** A head pod can only
  float across the mesh if lenders' RPC servers are reachable from the
  cluster fabric. Call: RPC servers listen on the lender's mesh-node address
  (the rke2 agent connectivity the box already has) instead of edge-loopback
  via the tunnel binding, fenced by owner rules from edge-loopback-only to
  the cluster CIDR. `modules/proxy.nix`'s `:llm` tunnel service retires with
  the rest of the old pipeline.
- **Catalogue models reach head pods through Longhorn volumes** seeded from
  the edge's pinned catalogue — no runtime downloads, no Nix store paths
  inside pods, no hostPath pinning heads to the edge node.
- **The compute window evicts, not just repels.** The NoSchedule taint only
  blocks new placement; at window close the edge also applies a NoExecute
  taint (or deletes the pods), so "share my compute when I sleep" keeps its
  meaning once AI jobs live on lenders. In-flight buyer requests fail through
  the proxy's existing failure handling; per-token billing bills what was
  served.
- **Lender-churn regrouping stays a custom reconciler.** SkyPilot serve
  autoscales on request load, not on "how many boxes are currently lending";
  the registrar's continuous re-grouping (heartbeat TTL, busy/window state)
  survives as the reconciler that adjusts the serving jobs' replica/RPC
  targets. SkyPilot provides scheduling and job plumbing, not lender
  lifecycle.

## What's left to the builder

- Packaging and pinning of the SkyPilot API server for the edge (nixpkgs
  provides SkyPilot; pin it the way other upstream artifacts are pinned) and
  the offline story for anything it would fetch at runtime.
- The exact admission mechanism for fenced jobs and kinda-shared placement
  on rke2.
- How the registrar's market ops (`Op::LlmModels`/`Op::LlmChat`) map onto
  SkyPilot-managed serving endpoints, how head/group bookkeeping in
  `edge/registrar/src/server/llm.rs` retires, and the new op shape for
  submitting rented jobs (the constructed-fenced-spec flow).
- The gateway-local SkyPilot server: packaging for gateway boxes (which are
  smaller than the edge VPS), its cluster bootstrap, and what the mesh page
  shows for each.
- Admin UI surfaces (Mesh page states, job views) in the shape the existing
  pages follow.

## Open questions for the user (brought back once, after the builder check)

All answered:

- **Kinda-shared topology: gateway-local.** Each network's local gateway
  runs its own SkyPilot API server over a gateway-local cluster; kinda-shared
  boxes never join the official edge's cluster, and the official edge never
  touches kinda-shared capacity. The official edge runs SkyPilot only over
  fully-shared mesh capacity. Heavy on gateway boxes by design: that is the
  price of keeping `sharingLocally`'s promise.
- **Buyer submission surface: LosOS operations.** Buyers submit rented jobs
  through registrar-style ops (the shape of the VM order flow); the edge
  constructs the fenced job spec, so nobody points a stock `sky` CLI at the
  edge and nobody authors raw YAML or brings third-party images.
- **Rental pricing: compute-seconds.** Rented jobs are metered per
  vCPU/MiB-hour by the scheduler and listed beside the catalogue; served
  models keep per-token billing unchanged.
- **Owner jobs: own share free.** A box owner's jobs drawing only on their
  box's own lent share are free; anything beyond it draws on the market
  account.
