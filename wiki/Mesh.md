# Mesh

A box can lend spare disk and CPU to other losos boxes. Joining is off by
default. The Mesh pane has two switches: join the mesh, and share compute. The
third switch, sharing this box's disk, sits on the [Market](Market) pane,
because lending disk and being paid for it are one decision. While the market
is planned, that switch is out of reach with it.

## Two clusters

Each box runs two Kubernetes instances:

|                     | Local (k3s)                 | Mesh (rke2 agent)                |
| ------------------- | --------------------------- | -------------------------------- |
| Runs                | this box's Nextcloud, Forgejo | Longhorn storage, shared compute |
| Server              | this box                    | the edge VPS                     |
| Needs the network   | no                          | yes                              |

A Kubernetes agent cannot start while its server is unreachable, and the box
reboots every night. If your own services ran in the mesh cluster, an edge
outage over midnight would take them offline. Keep them separate.

## Sharing compute

The box takes mesh work only when both are true:

1. The current time is inside the window you set
   (`losos.cluster.computeWindow`).
2. The box is idle (`losos.cluster.idleLoadThreshold`).

Idleness can close the window early. It cannot open it outside the window.

Anything unknown counts as busy: no idleness report, a stale report, or an
edge that just restarted. The edge evaluates the window in the appliance's
time zone.

## Finding the edge

The mesh is other boxes behind an edge. A box with no edge in reach cannot
share anything, whatever its switches say, so `lososd` looks for one every
20 seconds, in two places:

- **on the local network**, by DNS-SD. An edge configured with
  `losos.edge.lan.advertise = true` publishes `_losos-edge._tcp` over mDNS
  with a `url=` record naming its registrar API, and the box's Avahi finds it.
- **at the configured address**, `losos.proxy.registrarUrl`, which on a box
  with internet is the public edge. The box probes it whether or not the
  master proxy is switched on.

A candidate counts only once its `/health` answers. Several can be in reach
at once, say a company's edge on the LAN and the public one over the
internet. `GET /api/edge` lists them all, LAN first, and the Mesh pane shows
one row per edge, or "No edge proxy found" with what it tried. Sharing is
allowed while any of them answers. Growing a box's own disk needs no edge at
all. Pooling across boxes does, because the mesh's control plane runs on the
edge. On a LAN with no internet, that edge is one always-on PC running the
edge module, see [Master proxy, Edge on the same
LAN](Master-Proxy#edge-on-the-same-lan).

While nothing answers, the box refuses to turn network-dependent sharing on.
Switching to mesh mode, and any apply that turns `losos.sharingMyStorage` or
`losos.cluster.enable` on, is answered 409 with the reason, and the switches
are greyed with the same sentence. Settings that are already on are left
alone, so a box whose edge went away keeps its configuration and can still
change anything else. Turning sharing off is always allowed. Your own files
and apps never depend on the edge.

Finding an edge and trusting it with money are two different questions. Any
edge that answers opens sharing. Trading on the market is allowed only
through an **official** edge, one that presents a certificate signed by the
LosOS root key and answers the box's fresh nonce with it. The box checks this
on every scan. Each edge's row carries a sign beside its name, a check for an
official edge and a warning for any other. The warning's tooltip lists what
that edge cannot do for this box: buying on the market, and selling this
box's spare storage and compute. A company's own edge wears the warning, and
that is not a fault. See [Master proxy, Official
edges](Master-Proxy#official-edges).

`demo/edge-lan/run.sh` boots two boxes and an edge on one virtual network and
walks through exactly this in the order of the verification checklist
(`demo/edge-lan/CHECKLIST.md`). First with no edge anywhere: nothing found,
sharing refused, local use intact, a reboot changes nothing. Then with the
edge on: found on both boxes within a scan, allowed, edge gone, refused,
back. `demo/edge-lan/record.sh` records the same walk from the boxes' admin
UI.

## Sharing storage

Contributed storage lives in the `shared` user's home under an fscrypt
policy. The key is sealed to the TPM. When sharing is off, the key is not
loaded and the directory is unreadable, even to root on the running box. LUKS
protects the box while it is off; fscrypt protects this directory while it is
on.

Contributed files are namespaced per machine.

## Selling

Storage and compute will also be sellable to other boxes through the optional
Stripe Connect market. It is built but not open yet, and the admin UI shows
its tab greyed out as "soon(TM)". See [Market](Market).

## Persistence

The mesh agent writes `/etc/rancher/node/password` on first join. That path
must stay in the persistence list, or the server rejects the box after the
next reboot.
