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

Every 20 seconds the box looks for an edge on two roads:

- **on the local network**, by DNS-SD: an edge set up to announce itself
  publishes `_losos-edge._tcp` over mDNS with the address of its API, and the
  box finds it the same way it finds its own `.local` name;
- **at the configured address**, the LosOS edge's control plane unless the
  owner changed it (Settings → Advanced, `losos.proxy.registrarUrl`). It is
  asked whether or not **Reachable from outside your home** is on.

An edge counts as found once its `/health` answers. Several can be in reach
at once, a company's edge on the LAN and the LosOS edge over the internet,
say. The **Mesh pane** shows one row per edge, **On this network** or **Over
the internet**, each with a sign saying whether it is official (below), or
**No edge proxy found** with what the box tried: "This box searched its
network and asked `<address>`; nothing answered."

While nothing answers, the box **refuses to turn network-dependent sharing
on**. Switching storage to mesh mode, **Join the mesh** and **Share this
box's disk with the mesh** are greyed with "Needs an edge proxy in reach.",
and an apply that turns one of them on anyway is refused with the same
reason. Settings that are already on are left alone, so a box whose edge
went away keeps its configuration, can still change everything else, and can
always turn sharing *off*. Your own files and apps never depend on the edge.
The tunnel switch is not gated either: it simply reconnects when the edge
answers again.

## Official or not

Any edge that answers opens sharing. Only the LosOS edge may run the market,
and a box has to be able to tell it from any other edge, so finding an edge
and *trusting it with money* are two different questions. Every box carries
a **LosOS root public key**. An official edge proves itself on every scan: it
presents a certificate signed with the matching private key, which only the
project holds, offline, and it signs a fresh challenge the box just made up
with the certificate's own key. Four checks, all or nothing.

The result is the sign beside each edge's name on the Mesh pane: a **check**
for an official edge, a **warning** for any other, with a tooltip listing
what that edge cannot do for this box (buying on the market, selling this
box's spare storage and compute). A box that cannot verify the signature
treats the edge as a [company edge](company-edge): discovery, remote access
and sharing work, trading does not, and the Market pane says "The edge proxy
this box found is not run by LosOS." Nothing a company configures can make
its own edge official, by design.

:::note Until the key is published
The root key file every box ships is empty until the project publishes the
public key. Until then no edge is official, every edge wears the warning sign
and the market is off on every box. That is the safe direction to fail in.
:::

Enrolment at the edge (which box may tunnel through it) is a separate matter:
the edge keeps an allow-list of boxes, each with a token, and a box is listed
there by whoever runs the edge.

## Settings that matter

| Pane     | Setting                                | What it does                                                            |
| -------- | -------------------------------------- | ----------------------------------------------------------------------- |
| Network  | Reachable from outside your home       | opens the tunnel to the edge                                            |
| Advanced | `losos.proxy.registrarUrl`             | the edge address asked over the internet, besides the local network     |
| Mesh     | Join the mesh                          | enrols this box in the edge's cluster; needs an edge in reach           |
| Mesh     | Lend this box while I sleep            | lends CPU inside the hours below, while idle; needs the mesh joined     |
| Mesh     | Hours                                  | the daily window, in this box's own time zone                           |
| Market   | Share this box's disk with the mesh    | lends spare room to the pool; needs an edge in reach; greyed with the market until it opens |

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
