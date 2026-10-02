# Mesh

A box can lend spare disk and CPU to other losos boxes. Joining is off by
default. The settings page has two switches: join the mesh, and share compute.

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

## Sharing storage

Contributed storage lives in the `shared` user's home under an fscrypt policy.
The key is sealed to the TPM. When sharing is off, the key is not loaded and
the directory is unreadable, even to root on the running box. LUKS protects
the box when it is off; fscrypt protects this directory while it is on.

Contributed files are namespaced per machine.

## Selling

Storage and compute can also be sold to other boxes through the optional
Stripe Connect market. See [Market](Market).

## Persistence

The mesh agent writes `/etc/rancher/node/password` on first join. That path
must stay in the persistence list, or the server rejects the box after the
next reboot.
