---
title: A company with its own edge
sidebar_position: 3
---

# A company with its own edge

A company runs several boxes on one network and an edge server of its own,
either on that network or on a server it rents. Everything stays inside the
company: the boxes find the edge on the LAN, share disk and CPU among
themselves, and are reachable under the company's domain through the
company's edge.

## What is the same as the LosOS edge

- Remote access under the edge's domain, no ports opened on the boxes.
- The mesh: a shared, replicated disk pool and spare compute across the
  company's boxes, scheduled inside each box's own hours.
- Discovery on the LAN: a box on the same network finds the edge without
  being told its address.
- The admin pages of every box stay LAN-only.

## What is different

- **No market.** A company's edge cannot present the LosOS signature (see
  [official or not](official-edge#official-or-not)), so every box treats it
  as unofficial: trading is refused, and the Market pane says so. Storage and
  compute are shared among the company's own boxes for free, which is the
  point.
- **The company holds the keys.** The tunnel key pair, the cluster token and
  the allow-list of boxes live on the company's edge. Nothing about the
  company's data or traffic touches the LosOS project.
- **Updates still come from the project**, unless the company points its
  boxes at a fork (the upgrade source is a setting).

## Setting it up

The edge is a NixOS module shipped with LosOS (`nixosModules.edge`), so the
edge server is installed the same way a box is configured: a short Nix file
naming the public domain, the boxes allowed to connect and whether to run
the mesh control plane. The operator's walkthrough, with a script that stands
up an edge and a box on one network for a demonstration, is the
[Master proxy](https://github.com/dasmatus/losos/wiki/Master-Proxy) and
[Mesh](https://github.com/dasmatus/losos/wiki/Mesh) wiki pages; the demo
script is being written in the same change as edge discovery.

In short:

1. Install NixOS on the edge server and import the module.
2. Set `losos.edge.publicDomain`, list each box under `losos.edge.tenants`
   with a token file, and turn `losos.edge.cluster.enable` on for the mesh.
3. On each box, turn **Reachable from outside** on and, if the edge is not on
   the same LAN, enter its address.

## Who this is for

Small companies that want the "one box per team, one pool for all" setup
with nothing leaving the building, and that have someone who can run one
NixOS server. It is the deployment LosOS is meant to be sold as.
