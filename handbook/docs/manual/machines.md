---
title: Machines
sidebar_position: 4.9
slug: /manual/machines
---

# Machines

**Machines** under Mesh in the sidebar (at `/machines`) is where a box
rents virtual machines on the mesh and hosts machines for other boxes.
A machine is sold by the **replica**: each replica is one copy of the
machine with its own disk, running for a month on the box that hosts it.

Payment goes through the [Market](../in-depth/market). Every sale is split
half and half between the owner of the hosting box and LosOS. The
hypervisor is KubeVirt on the edge's mesh cluster; the edge's registrar
sells the replicas and creates the machines.

![The Machines pane under Mesh.](../img/mesh-none.png)

## Who can use it

The box offers machines, to rent or to host, only while it shares its disk
with the mesh. A box that does not share sees one greyed sentence and a link
to the Market pane, where the disk-sharing switch lives. Turn sharing on
there, apply the change, and the Machines page offers the form.

## Renting

The catalogue lists the sizes an edge sells. Pick one, say how many
replicas, and pay. The edge creates the machines; each replica gets its
own disk imported from the image the listing names. The page shows the
machines you rent, their state, and when the period ends.

## Hosting

A box that shares can also host. The page shows what you are hosting and
what you earn. Hosting uses the same Market account as selling storage or
compute.

See [Virtual machines](../in-depth/virtual-machines) for the edge side:
how the registrar sells replicas, how disks are imported, and the reasons
the page can give when machines are not available.
