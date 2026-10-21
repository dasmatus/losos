---
title: Where the apps run
sidebar_position: 5
---

# Where the apps run

LosOS Cloud and LosOS Git are services of the box itself, like the admin
pages. Each runs as its own system service with its own data directory, and
neither needs a cluster. You see them at the same addresses on every box.

Apps you install from the Apps pane run in one of two places, picked by who
they run as:

| Runs as         | Where it runs                                                                                    |
| --------------- | ------------------------------------------------------------------------------------------------ |
| **notshared**   | Rootless Podman, as you. No cluster is needed.                                                   |
| **shared**      | The box's own small Kubernetes cluster (k3s), which runs only while the box shares its disk with a mesh. |

## Two clusters, on purpose

The box's own shared apps live in its own cluster. The
[mesh](official-edge) is a different cluster, and the edge runs it. LosOS
never places the box's apps in the mesh cluster. A cluster member cannot
start while its server is unreachable, and the box restarts every night at
00:07, so an edge outage over midnight would take those apps offline.

![Inside a box: the box's own cluster runs the apps installed as shared, the mesh agent belongs to a different cluster whose server is the edge.](../img/box-architecture.svg)

## Where the apps are

![The Apps pane: LosOS Cloud, LosOS Git, an installed app, and Find more below.](../img/apps.png)

| App          | Address             | Note                                                                                      |
| ------------ | ------------------- | ----------------------------------------------------------------------------------------- |
| LosOS Cloud  | `/cloud/`           | The desktop and phone apps take the box's address without a path.                         |
| LosOS Git    | `/git/`             | Turn it off in the Apps pane if you do not want it.                                       |
| Admin pages  | `/`                 | LAN only.                                                                                 |
| Handbook     | `/handbook/`        | LAN only.                                                                                 |
