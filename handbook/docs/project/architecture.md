---
title: Architecture
sidebar_position: 1
---

# Architecture

Requirement 1: *design an OS architecture for mesh storage.* This is the
design, from the outside in.

## The idea

LosOS starts from many small boxes. Each one has an owner, holds that
owner's files, and has spare room and spare time. An **edge** server ties the
boxes of one group into a **mesh**: a replicated storage pool across their
spare disks and a scheduler for their spare CPU. The mesh uses both only
within what each owner allows. The owner's own files never enter the pool,
which is made of what the owner is not using.

## The box

![Inside a box: nginx is the one front door, with the admin pages and the handbook for the LAN only; behind it lososd, the box's own k3s cluster running LosOS cloud and LosOS Git, and the rke2 agent of the mesh, whose server is the edge; under them a RAM system disk and one encrypted volume unlocked by the TPM.](../img/box-architecture.svg)

Six decisions carry the design:

1. **Stateless root.** The system disk is a RAM filesystem that the box
   rebuilds from a declarative NixOS description on every boot. Only a
   listed set of directories survives, on an encrypted volume. The
   configuration therefore cannot drift, and the box needs no shell. A
   restart is the repair.
2. **Two accounts, two data domains.** `notshared` owns the owner's files.
   `shared` owns the mesh's copies, in a directory encrypted with its own
   key, which the box loads only while sharing is on. Neither can read the
   other.
3. **Two clusters.** The box's own apps run in its own Kubernetes cluster:
   k3s with no network plugin, the pods on the host's network. The mesh is a
   *different* cluster, rke2, with the agent on the box and the server on
   the edge. An edge outage therefore never takes the owner's apps down, not
   even across the unconditional midnight restart.
4. **One front door.** nginx holds the only public ports and routes by path.
   A check on the source address and a strict content-security policy guard
   the admin side. The apps are reachable from anywhere the box is.
5. **A root daemon with a typed API.** `lososd` owns the box's state. It
   exposes one method per operation on the system bus and a loopback JSON
   API, runs rebuilds as transient units, and re-attaches to them after its
   own restart. Its command layer is written against an interface with a
   real and an in-memory implementation, so the tests run the state machine
   without a filesystem.
6. **Everything is a rebuild.** Settings, updates and resets all take the
   same path: write the description, build the system, switch. When any of
   them fails, the previous system keeps running. The nightly update runs
   the same code that owners run by hand.

## The mesh

![The mesh: each box keeps its owner's files and lends the room it is not using to one replicated pool; spare time is lent inside the owner's window, only while idle.](../img/mesh.svg)

- **Storage.** Longhorn on the edge's cluster replicates volumes across the
  boxes that share room. A box shares only the room it is not using, in its
  `shared` domain. When a box stops sharing, its node leaves and Longhorn
  re-replicates its copies elsewhere.
- **Compute.** The edge taints each node outside its owner's window or while
  the box is busy, so other people's pods run only inside the hours the
  owner chose, on an idle box. The edge evaluates the window per node, in
  the box's own time zone.
- **Gating.** A box with no reachable edge refuses to turn sharing on, so it
  never promises room to a pool it cannot reach. An edge proves it is the
  LosOS edge with a signature, which the box checks against a root public
  key it carries. Any other edge gets discovery, remote access and local
  sharing, but not trading.

## The edge

![How a box finds an edge and decides whether it is official.](../img/edge-discovery.svg)

The edge is a NixOS module, `nixosModules.edge`. Traefik with automatic TLS
sits in front, with a rathole tunnel server behind it. The registrar,
`losos-registrar`, is written in Rust and keeps both configured from the
boxes that register and send heartbeats. The module can also run the mesh
control plane, an rke2 server with Longhorn, and the market. The market uses
Stripe Connect, and a separate gate process holds the Stripe key and never
shows it to the registrar. For presentations, the registrar's router also
runs as one serverless function with its registry in a small database.

## Why NixOS

The whole box, apps included, is one description that can be built, tested in
virtual machines and rebuilt on the box unattended. The repository carries
thirteen VM tests and a set of evaluation-time invariants that fail the
build when a load-bearing default drifts. That is what makes "no shell"
possible. The box is never in a state nobody wrote down.
