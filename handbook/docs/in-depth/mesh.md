---
title: Mesh
sidebar_position: 3
mdx:
  format: md
---

# Mesh

![The Mesh pane on a box with no edge proxy in reach.](../img/mesh-none.png)

A box can lend spare disk and CPU to other LosOS boxes. Joining is off by
default. The Mesh pane has two switches: join the mesh, and share compute. The
third switch, sharing this box's disk, sits on the [Market](market.md) pane,
because lending disk and being paid for it are one decision. The switch works
whether or not the edge offers the market to this box.

## Two clusters

Each box runs two Kubernetes instances:

|                     | Local (k3s)                 | Mesh (rke2 agent)                |
| ------------------- | --------------------------- | -------------------------------- |
| Runs                | apps installed as shared    | Longhorn storage, shared compute |
| Server              | this box                    | the edge VPS                     |
| Needs the network   | no                          | yes                              |

:::warning[Keep the two clusters separate]
A Kubernetes agent cannot start while its server is unreachable, and the box
reboots every night. If your own services ran in the mesh cluster, an edge
outage over midnight would take them offline. Keep them separate.
:::

## Sharing compute

The box takes mesh work only when both are true:

1. The current time is inside the window you set
   (`losos.cluster.computeWindow`).
2. The box is idle (`losos.cluster.idleLoadThreshold`).

Idleness can close the window early. It cannot open it outside the window.

:::note[Unknown counts as busy]
Anything unknown counts as busy: no idleness report, a stale report, or an
edge that just restarted. The edge evaluates the window in the appliance's
time zone.
:::

## Finding the edge

The mesh is other boxes behind an edge. A box with no edge in reach cannot
share anything, whatever its switches say, so `lososd` looks for one every
20 seconds, in two places:

- **on the local network**, by DNS-SD. An edge configured with
  `losos.edge.lan.advertise = true` publishes `_losos-edge._tcp` over mDNS
  with a `url=` record naming its registrar API, and the box finds it
  through systemd-resolved.
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
LAN](master-proxy.md#edge-on-the-same-lan).

While nothing answers, the box refuses to turn network-dependent sharing on.
Switching to mesh mode, and any apply that turns `losos.sharingMyStorage` or
`losos.cluster.enable` on, is answered 409 with the reason, and the switches
are greyed with the same sentence. Turning sharing off is always allowed,
and the box can change anything else during an outage. Your own files and
apps never depend on the edge.

A box that shares its storage and then finds no edge for ten minutes turns
sharing off by itself, the way its switch would, and the Mesh pane says
**Sharing paused**. Once an edge has answered for a minute, the box turns
sharing back on. It undoes only its own pause: if you switch sharing on or
off in the meantime, your choice stays. `losos.proxy.pauseSharingAfter` sets
the wait in seconds, and `0` turns the pause off.

Finding an edge and trusting it with money are two different questions. Any
edge that answers opens sharing. Trading on the market is allowed only
through an **official** edge, a gateway that passes the box's checks: its
owner's signed passport, a TPM that shows a published LosOS gateway image
booted untouched, and LosOS's fee addresses. The box checks this on every
scan. Each edge's row carries a sign beside its name, a check for a checked
gateway and a warning for any other. The warning's tooltip lists what
that edge cannot do for this box: buying on the market, and selling this
box's spare storage and compute. A company's own edge wears the warning, and
that is not a fault. See [Master proxy, Official
edges](master-proxy.md#official-edges).

:::note[Try it on virtual machines]
`nix/demo/edge-lan/run.sh` boots two boxes and an edge on one virtual network and
walks through exactly this in the order of the verification checklist
(`nix/demo/edge-lan/CHECKLIST.md`). First with no edge anywhere: nothing found,
sharing refused, local use intact, a reboot changes nothing. Then with the
edge on: found on both boxes within a scan, allowed, edge gone, refused,
back. `nix/demo/edge-lan/record.sh` records the same walk from the boxes' admin
UI.
:::

## Sharing storage

Contributed storage lives in the `shared` user's home under an fscrypt
policy. The key is sealed to the TPM. When sharing is off, the key is not
loaded and the directory is unreadable, even to root on the running box. LUKS
protects the box while it is off; fscrypt protects this directory while it is
on.

Contributed files are namespaced per machine.

The edge runs Longhorn in the mesh cluster, installed from a pinned chart
(`losos.edge.cluster.longhornChart`). Only the boxes hold its disks: a box
that joins the mesh is labelled `node.longhorn.io/create-default-disk=true`,
and the edge, which has no such label, stores no replicas. The market's
storage orders claim their volumes from Longhorn's `longhorn` storage class.
The fscrypt key protects the lender's disk, but it lives on the lender's
box, so a lender can read what lands in its share. A buyer's backups on such
a volume are therefore encrypted on the buyer's box first, with restic and
the buyer's recovery code ([Market](/in-depth/market.md)).
Longhorn on the edge does not check for updates or send usage reports, and
its web UI is not deployed.

## Selling

Storage and compute can also be sold to other boxes through the optional
market, under Mesh, Market in the sidebar. Buyers pay in USDC, Bitcoin or Monero,
or by card where the edge takes cards. A box shows the market only when an
official LosOS edge is in reach. See [Market](market.md).

## Persistence

The mesh agent writes `/etc/rancher/node/password` on first join. That path
must stay in the persistence list, or the server rejects the box after the
next reboot.
