# Mesh

A box can lend spare disk and CPU to other losos boxes. Joining is off by
default. The Mesh pane has two switches: join the mesh, and share compute. The
third, sharing this box's disk, sits on the [Market](Market) pane, because
lending disk and being paid for it are one decision; while the market is
planned, that switch is out of reach with it.

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

The mesh is other boxes behind an edge, so a box with no edge in reach cannot
share anything, whatever its switches say. `lososd` therefore looks for one
every 20 seconds, on two roads:

- **on the local network**, by DNS-SD: an edge configured with
  `losos.edge.lan.advertise = true` publishes `_losos-edge._tcp` over mDNS
  with a `url=` record naming its registrar API, and the box's Avahi finds it;
- **at the configured address**, `losos.proxy.registrarUrl`, which on a box
  with internet is the public edge. It is probed whether or not the master
  proxy is switched on.

A candidate counts only once its `/health` answers. The Mesh pane shows the
result at the top ("Edge proxy found: …" or "No edge proxy found", with what
was tried), and `GET /api/edge` serves it.

While nothing answers, the box **refuses to turn network-dependent sharing
on**: switching to mesh mode, and any apply that turns `losos.sharingMyStorage`
or `losos.cluster.enable` on, is answered 409 with the reason, and the
switches are greyed with the same sentence. Settings that are already on are
left alone, so a box whose edge went away keeps its configuration and can
still change anything else, and turning sharing off is always allowed. Your
own files and apps never depend on the edge.

`demo/edge-lan/run.sh` boots an edge and a box on one virtual network and
walks through exactly this: found, allowed, edge gone, refused.

## Sharing storage

Contributed storage lives in the `shared` user's home under an fscrypt policy.
The key is sealed to the TPM. When sharing is off, the key is not loaded and
the directory is unreadable, even to root on the running box. LUKS protects
the box when it is off; fscrypt protects this directory while it is on.

Contributed files are namespaced per machine.

## Selling

Storage and compute will also be sellable to other boxes through the optional
Stripe Connect market, which is built but not open yet (the admin UI shows its
tab greyed out as "soon(TM)"). See [Market](Market).

## Persistence

The mesh agent writes `/etc/rancher/node/password` on first join. That path
must stay in the persistence list, or the server rejects the box after the
next reboot.
