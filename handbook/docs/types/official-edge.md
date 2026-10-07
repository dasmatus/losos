---
title: A box with the LosOS edge
sidebar_position: 2
---

# A box with the LosOS edge

An **edge** is a server on the internet that a box keeps a tunnel open to.
Through it, the box gets a public address with no port opened at home, joins
the mesh of other boxes, and (once it opens) trades on the market. The
**LosOS edge** is the one the project runs; it is the only edge whose market
pays out.

## What the edge does for a box

| Feature                | How                                                                                                        |
| ---------------------- | ---------------------------------------------------------------------------------------------------------- |
| **Remote access**      | `https://<your-name>.<edge domain>` reaches LosOS cloud and LosOS Git through an encrypted tunnel the box opens outwards. The admin pages are never published: they stay LAN-only even through the tunnel. |
| **The mesh**           | The box joins a Kubernetes cluster the edge runs. Its spare disk becomes part of a shared, replicated pool; its spare CPU runs other people's work inside the hours you allow, only while the box is idle. |
| **The market**         | Being paid for the disk and CPU you share, and buying some from others. Payment is by card through Stripe; the edge keeps a 4 % fee. Built end to end, not open yet: a platform needs a registered business behind it. The Market tab shows as "soon" until then. |
| **Find my box**        | The edge hosts the page that finds a box on your LAN from a browser.                                        |

## How a box finds its edge

A box looks for an edge in two ways: the address configured in the Network
pane, and an announcement on the local network (the way it announces its own
name). What it finds is shown on the Mesh pane. Until an edge answers, every
feature that needs one is held back: the mesh switches cannot be turned on,
and storage sharing is refused, so a box never promises room to a pool it
cannot reach.

:::info Being built
Edge discovery and the sharing gate are landing in the repository as this
page is written (the "Edge proxy discovery" work of 7 October 2026). If your
box's Mesh pane does not show an edge state yet, it predates that change;
the nightly update brings it.
:::

## Official or not

Only the LosOS edge may run the market, and a box has to be able to tell it
from any other edge. Every box carries a **LosOS root public key**, baked into
the system image. An official edge proves itself by presenting an identity
signed with the matching private key, which only the project holds, offline.
A box that cannot verify that signature treats the edge as a
[company edge](company-edge): discovery, remote access and local sharing work,
trading does not. Nothing a company configures can make its own edge
official, by design.

Enrolment at the edge (which box may tunnel through it) is a separate matter:
the edge keeps an allow-list of boxes, each with a token, and a box is listed
there by whoever runs the edge.

## Settings that matter

| Pane    | Setting                    | What it does                                                    |
| ------- | -------------------------- | --------------------------------------------------------------- |
| Network | Reachable from outside     | opens the tunnel to the edge                                    |
| Network | Edge address               | where to look, besides the local network                        |
| Mesh    | Join the mesh              | enrols this box in the edge's cluster                           |
| Mesh    | Share my spare time        | lends CPU inside the window below, while idle                   |
| Mesh    | Hours                      | the daily window, in this box's own time zone                   |
| Market  | Share my disk              | lends spare room to the pool; greyed out with the market        |

## What the edge sees

The box's name, its heartbeat, which features it has on, and the traffic it
tunnels (encrypted end to end with a key the box pins on first contact). It
does not see the admin pages, the owner's password or the disk key. The
[security model](../reference/security-model) lists the trust boundaries.

:::note The demo edge
For presentations, the edge's control plane also runs on Vercel at
`losos-edge.dasmat.us`, with its registry in a small database. That host can
register boxes and show them on a status page, but it cannot carry a tunnel,
run the mesh or take payments; those need the real edge server.
:::
