---
title: App modes
sidebar_position: 5
---

# App modes

LosOS Git runs in one of two modes. You see the same app at the same
address either way, and the Apps pane shows which mode it is in. LosOS Cloud
has no modes: it is a service of the box's own, on the box itself.

| Mode            | What runs where                                                                                           | Default |
| --------------- | --------------------------------------------------------------------------------------------------------- | ------- |
| **Workload**    | The app runs in a container inside the box's own small Kubernetes cluster (k3s), from an image built with the system. | yes     |
| **Native**      | The app runs directly on the box as a system service.                                                       | no      |

## Why workload is the default

- The app's image is built and tested together with the system, so an update
  brings a known-good pair.
- The box already runs a cluster for the mesh, and its own apps use the same
  machinery. Their cluster has no network plugin. The pods share the box's
  network, which is why they answer on loopback.
- Each app sees its own data directory and nothing else.

## Why native exists

Native is the older shape. It stays because the same Forgejo
configuration feeds both modes, so the two cannot drift apart. You set the
mode in Nix, not in the admin pages, and switching needs a rebuild.

## Two clusters, on purpose

The box's own apps live in its own cluster. The [mesh](official-edge) is a
different cluster, and the edge runs it. LosOS never places the box's apps
in the mesh cluster. A cluster member cannot start while its server is
unreachable, and the box restarts every night at 00:07, so an edge outage
over midnight would take your own files offline.

![Inside a box: the box's own cluster runs LosOS Git and the apps installed as shared, the mesh agent belongs to a different cluster whose server is the edge.](../img/box-architecture.svg)

## Where the apps are

![The Apps pane: LosOS Git with its mode, an installed app, and Find more below.](../img/apps.png)

| App          | Address             | Note                                                                                      |
| ------------ | ------------------- | ----------------------------------------------------------------------------------------- |
| LosOS Cloud  | `/cloud/`           | The desktop and phone apps take the box's address without a path.                         |
| LosOS Git    | `/forgejo/`         | Clone URLs use the box's name; turn it off in the Apps pane if you do not want it.         |
| Admin pages  | `/`                 | LAN only.                                                                                 |
| Handbook     | `/handbook/`        | LAN only.                                                                                 |

