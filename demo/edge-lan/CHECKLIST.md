# Verification checklist: the mesh, with and without the edge proxy

This is the list the KOP's verification chapter cites ("overte správnosť
konfigurácie a funkčnosť"): what to do, what to look at, and what has to be
there, for two LosOS boxes on one network, first with **no edge proxy**
(scenario A) and then **with one** (scenario B). The same list is what
`demo/edge-lan/run.sh walk` performs over the API and what
`demo/edge-lan/two-boxes.nix` asserts as a check and photographs as a
recording (`record.sh`, `compose.sh`).

Setup for both scenarios: two boxes installed from the stock installer ISO,
claimed in the first-run wizard, on one switch with no internet; one more
machine that can run the edge module (`nixosModules.edge` with
`losos.edge.lan.advertise = true`, or the edge gateway image), switched off
until B1. The Mesh pane is `http://<box>.local/mesh` (or the box's IP) after
signing in; the API lines use the box's admin key as a Bearer token.

## Scenario A: no edge proxy

| # | Do | Look at | Expected |
| --- | --- | --- | --- |
| A1 | Boot both boxes; open each one's admin UI and Mesh pane. | Mesh pane, "Edge proxy" group. `GET /api/edge`. | Each box says **No edge proxy found** / "This box searched its network and asked `<configured URL>`; nothing answered." The API answers `reachable: false`, `edges: []`, `lanSearched: true`. Both admin UIs load; Nextcloud answers on each box. |
| A2 | On each box try to turn **Join the mesh** or disk sharing on. Then `POST /api/change {"mode":"mesh"}`. | The two switches; the API status and body. | The switches are greyed with **Needs an edge proxy in reach.** The API answers **409** with `edgeRequired: true` and the sentence "no edge proxy is reachable from this box … Local use keeps working." `losos-ctl change --mode mesh` prints the same sentence and exits non-zero. `/var/lib/losos/state.json` is byte for byte what it was. |
| A3 | Wait two scan periods (about a minute) and read the pane again. Change something unrelated (`GET /api/settings`, or rename the box). | Mesh pane; `GET /api/settings`. | Nothing changed: still **No edge proxy found**; settings read back; a hostname-only apply is accepted (the gate is about sharing, nothing else). |
| A4 | Reboot one box. | Its Mesh pane and `/api/edge` after the restart. | The same state as before the reboot: local services up, no edge, sharing still refused with the same 409. Nothing tried to join anything. |

Result of A: a box with no edge is a complete local appliance and **never
starts sharing its disk into a network that has no trusted edge in it**.

## Scenario B: with the edge proxy

| # | Do | Look at | Expected |
| --- | --- | --- | --- |
| B1 | Switch the edge on (the same LAN, `lan.advertise` on). | Both Mesh panes; `GET /api/edge` on both. | Within one scan (20 s; the pane refreshes every 10 s) both boxes show **Edge proxy found: losos edge on edge** with **On this network**, a check sign for an official edge or a warning sign for any other. The API lists it with `source: "lan"` and the advertised URL. |
| B2 | On both boxes turn **Join the mesh** on and Apply (or `POST /api/change {"mode":"mesh"}`). | The API status; the box's status line; on the edge `kubectl get nodes`. | **200**; the box rebuilds into mesh mode; afterwards `kubectl get nodes` on the edge lists both boxes (their appliance ids) as Ready. |
| B3 | On both boxes turn disk sharing on. | The edge: `kubectl -n longhorn-system get nodes.longhorn.io`; on a box the Storage pane. | Longhorn lists both boxes as schedulable nodes with their allocated capacity; `/home/shared` on each box is unlocked (fscrypt) while sharing is on. |
| B4 | Create a volume in the pool with two replicas and write data into it. | `kubectl -n longhorn-system get volumes,replicas`; the data after one box is unplugged. | The volume has one replica on each box; the data is still readable with one box off. This is what "storage pooled and visible from both" means. |
| B5 | Switch the edge off. | Both Mesh panes; `POST /api/change {"mode":"mesh"}`; each box's own apps. | Within two scans both boxes say **No edge proxy found** again, sharing is refused with the same 409, and each box's own files and apps keep working (back to scenario A). |
| B6 | Switch the edge back on. | Both Mesh panes; `kubectl get nodes` on the edge. | Found again on both without any hand on the boxes; the gate reopens; already-joined nodes return to Ready without re-adding themselves (`/etc/rancher` is persisted, so the node password survives). |
| B7 | Set the compute window to 23:00–07:00 on a box and look at its node on the edge outside the window. | `kubectl describe node <box>` on the edge. | The node carries the NoSchedule taint outside the window; inside it, while the box is idle, the taint is gone. The edge evaluates the window in the box's time zone. |
| B8 | Point the boxes at an edge nobody signed (a company's own edge, or the demo edge with no `identity`). | The warning sign and its tooltip on the Mesh pane; `GET /api/market`; `POST /api/market/orders`. | Sharing is allowed through it; the tooltip lists what is missing ("buying storage or compute on the market", "selling this box's spare storage and compute"); the market answers `{available: false, reason: "noOfficialEdge"}` and an order is **409** `officialEdgeRequired`. |

Result of B: **two boxes' storage is one pool, seen from both sides**;
membership survives the edge going away and a reboot; trading is allowed
only through an official edge.

## What covers each step today

| Steps | Covered by |
| --- | --- |
| A1–A4, B1, B2 (acceptance), B5, B6, B8 | `checks.losos-edge-lan-two-boxes` (`demo/edge-lan/two-boxes.nix`: two boxes with the real admin UI, one edge, the exact order above; the recording made by `record.sh` is the same run photographed) and `checks.losos-edge-lan` (`tests/edge-lan.nix`, one box). |
| B2 (the rebuild and the node registering), B6 (the node's return) | `checks.losos-cluster` (`tests/cluster-vm.nix`) for the rke2 agent joining the edge's server and surviving the edge's absence; the join route in `backend-registrar/tests/cluster_join.rs`. |
| B3, B4, B7 | Manual on a real site. They are not reachable in the VM walkthrough yet, and not only for lack of KVM: see below. |

### What pooling still needs (B3, B4)

The box side has the Longhorn prerequisites (iSCSI initiator, NFS client,
`dm_crypt`), the edge side has the rke2 server and the Longhorn chart hook
(`losos.edge.cluster.longhornChart`), and the gate decides *whether* a box
may join. Three pieces are not wired, and a two-box pool cannot be shown
before they are:

1. **No Longhorn chart is pinned.** `losos.edge.cluster.longhornChart`
   defaults to `null` and nothing in the tree sets it, so no edge deploys
   Longhorn today; the mesh cluster has nodes and no storage class.
2. **The join does not follow discovery.** `losos-mesh-join` enrols at the
   build-time `losos.proxy.registrarUrl` and `losos.cluster.serverAddr`
   (the public edge), not at the edge the box just found on the LAN; a LAN
   site sets both in its own flake for now. The enrolment credential a box
   needs for that route is being added by open LAN enrolment (PR #81, the
   edge gateway).
3. **The box opens no Longhorn ports.** Joining needs none (the agent dials
   out), replicas need several; `modules/cluster.nix` opens none yet.

Each of the three is a follow-up on top of PR #75 and PR #81; the checklist
above does not change when they land, only the "Covered by" column does.
