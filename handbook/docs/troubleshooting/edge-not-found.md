---
title: No edge found
sidebar_position: 11
slug: /troubleshooting/edge-not-found
---

# No edge found

<div className="losos-symptom">

**What you see:** the Mesh pane says **No edge proxy found** and **Sharing is
off**; the mesh and disk-sharing switches are greyed with "Needs an edge
proxy in reach."; an apply was refused with "no edge proxy is reachable from
this box"; the public name of the box stops answering.

</div>

## What the box looks for

![What the box tries every 20 seconds: the LAN by DNS-SD and the configured address; no answer from either means No edge proxy found.](../img/edge-discovery.svg)

Every 20 seconds the box looks for an edge in two places. It listens for an
announcement on the local network, which is how it finds a
[company edge](../types/company-edge) on the same LAN. It also asks the
configured address, which is how it reaches the
[LosOS edge](../types/official-edge) or a company edge elsewhere. An edge
counts once its `/health` answers. The Mesh pane lists every edge the box
found, each with a sign that says whether it proved itself official. If it
found none, the sentence under **No edge proxy found** says what the box
tried:

| The pane says                                                              | Meaning                                                                       |
| -------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| "This box searched its network and asked `<address>`; nothing answered."   | The box tried both. The address is the configured one.                        |
| "This box searched its network and nothing answered."                      | No address is configured, so the box searched only the local network.         |
| "This box could not search its network, and nothing answered elsewhere."   | The box's mDNS service was not running, so it could not find a LAN edge even if one was there. A reboot brings the service back. |

## From the LAN

![The Mesh pane saying No edge proxy found, with what the box searched and asked.](../img/mesh-none.png)

1. **Does the box have the internet?** An edge on the internet needs it, and
   a company edge on the LAN does not. The Overview says whether the box is
   answering, but the box has no "internet" light. Try the edge's address in
   your own browser from the same network. If only the local network works,
   see [Only the local network works](only-the-lan-works).
2. **Is the address right?** Open Settings → Advanced and find
   `losos.proxy.registrarUrl`. It is the base address of the edge's API,
   `https://losos-edge.dasmat.us` for the LosOS edge's control plane. A
   company edge on the LAN announces its own, usually
   `http://<edge>.local:8443`.
3. **Company edge on the LAN: same network segment?** mDNS announcements
   do not cross routers or VLANs. Enter the edge's address instead of relying
   on discovery.
4. **Is the edge announcing itself?** Boxes find a company edge on the LAN
   only when `losos.edge.lan.advertise` is on. Without it, boxes need the
   edge's address. Whoever runs the edge sets that option.
5. **Is this box enrolled?** An edge only accepts boxes on its allow-list,
   each with a token. Discovery does not need enrolment, but the tunnel
   does. A reinstalled box has a new identity, and whoever runs the edge
   must enrol it again.
6. **Was the edge reinstalled?** The box pins the edge's tunnel key on first
   contact and refuses a rebuilt edge with a new key, on purpose. The
   operator either clears the pinned key on the box or re-enrols the box.
   The pinned key is a file under the box's secrets, and only a rebuild can
   reach it.

## The box reconnects on its own

Once the edge answers again, the next scan finds it within 20 seconds, and
the Mesh pane shows it a few seconds later. The tunnel comes back without a
restart. An outage turns off nothing you turned on. While the box finds no
edge it refuses only *turning on*, and
[Sharing is refused](sharing-refused) covers that.
