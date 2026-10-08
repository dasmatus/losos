---
title: A box on its own
sidebar_position: 1
---

# A box on its own

This is the simplest LosOS, and every box starts here after the first run.
It is one machine on one local network. The computers and phones on that
network can reach it, and nothing else can.

## What you get

- LosOS cloud and LosOS Git at the box's address, for everyone on the
  network who has an account.
- The admin pages, unlocked with the owner's password.
- Nightly self-repair and self-update, an encrypted disk, the hardening
  baseline.
- The handbook you are reading, at `/handbook/`.

## What you do not get

- **No access from outside.** The box publishes nothing and opens no port on
  your router. To reach your files from a café, you need the
  [LosOS edge](official-edge) or a VPN of your own into the home network.
- **No mesh.** The box has nobody to lend disk or CPU to, and the Mesh pane's
  switches stay off. With no edge in reach, the box refuses network-dependent
  storage sharing, so you cannot turn the switch on by mistake.
- **No market.**

![The Mesh pane on a box with no edge: No edge proxy found, with what the box tried, and the mesh switches greyed with Needs an edge proxy in reach.](../img/mesh-none.png)

## When this is the right type

Most homes. A box on its own has the fewest ways in. The only one is a
browser on your LAN. Updates still arrive, because the box fetches them
itself, and the update path needs no edge.

## Settings that matter

| Pane         | Setting                     | Default                                          |
| ------------ | --------------------------- | ------------------------------------------------ |
| Network      | Name                        | `losos` until you set one in the wizard           |
| Network      | Encrypt the connection      | on; the certificate is the box's own              |
| Network      | Reachable from outside      | off, which is what makes it a box on its own      |
| Apps         | LosOS Git                   | on                                                |
| Hardware     | Graphics                    | off unless the box has a GPU you want pods to use |
| Security     | the four extra protections  | off; each one costs something                     |

Switch **Reachable from outside** on while an edge is in reach, and the box
becomes the next type. Nothing is reinstalled.
