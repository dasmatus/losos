---
title: App modes
sidebar_position: 5
---

# App modes

LosOS cloud and LosOS Git each run in one of two modes. The owner sees the
same app at the same address either way; the Apps pane shows which mode each
one is in.

| Mode            | What runs where                                                                                           | Default |
| --------------- | --------------------------------------------------------------------------------------------------------- | ------- |
| **Workload**    | The app runs in a container inside the box's own small Kubernetes cluster (k3s), from an image built with the system. | yes     |
| **Native**      | The app runs directly on the box as a system service.                                                       | no      |

## Why workload is the default

- The app's image is built and tested with the system, so an update brings a
  known-good pair.
- The box already runs a cluster for the mesh; its own apps use the same
  machinery, with no network plugin (the pods share the box's network, which
  is why they answer on loopback).
- Isolation: the app sees its own data directory and nothing else.

## Why native exists

It is the older shape, still kept so that the two cannot drift: the same
Nextcloud configuration feeds both. Switching needs a rebuild and the
mode is set in Nix, not in the admin pages.

## Two clusters, on purpose

The box's own apps live in **its own** cluster. The [mesh](official-edge)
is a **different** cluster, run by the edge. The box's apps are never placed
in the mesh cluster, because a cluster member cannot start while its server
is unreachable, and the box restarts every night at 00:07: an edge outage
over midnight would otherwise take your own files offline.

![Inside a box: the box's own cluster runs LosOS cloud and LosOS Git, the mesh agent belongs to a different cluster whose server is the edge.](../img/box-architecture.svg)

## Where the apps are

![The Apps pane: LosOS cloud and LosOS Git, each with its mode and whether it is on, and Find more below.](../img/apps.png)

| App          | Address             | Note                                                                                      |
| ------------ | ------------------- | ----------------------------------------------------------------------------------------- |
| LosOS cloud  | `/nextcloud`        | Trusts the box's name and the address each request arrived on, never a wildcard.          |
| LosOS Git    | `/forgejo/`         | Clone URLs use the box's name; turn it off in the Apps pane if you do not want it.         |
| Admin pages  | `/`                 | LAN only.                                                                                 |
| Handbook     | `/handbook/`        | LAN only.                                                                                 |

The admin pages' app tiles open LosOS cloud's apps through `index.php`
(`/nextcloud/index.php/apps/<app>/`), which every configuration answers.
