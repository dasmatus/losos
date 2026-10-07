---
title: No edge found
sidebar_position: 11
---

# No edge found

<div className="losos-symptom">

**What you see:** the Mesh pane says no edge was found; the mesh switches
cannot be turned on; the public name of the box stops answering.

</div>

## What the box looks for

A box looks for an edge in two places: an announcement on the local network
(for a [company edge](../types/company-edge) on the same LAN) and the address
entered in the Network pane (for the [LosOS edge](../types/official-edge) or
a company edge elsewhere). The Mesh pane shows what it found and whether the
edge proved itself official.

:::info Being built
This state and the gate on sharing are landing in the repository (the "Edge
proxy discovery" work of 7 October 2026). A box that shows no edge state on
its Mesh pane predates it and simply tries to connect to the configured
address.
:::

## From the LAN

1. **Does the box have the internet?** An edge on the internet needs it; a
   company edge on the LAN does not. The Overview says whether the box is
   answering; the box has no "internet" light, so try the edge's address in
   your own browser from the same network.
2. **Is the address right?** Network pane → the edge address. It is an
   `https://` address; the box refuses plain HTTP there. The LosOS edge's
   control plane is `https://losos-edge.dasmat.us`.
3. **Company edge on the LAN: same network segment?** mDNS announcements
   do not cross routers or VLANs. Enter the edge's address instead of relying
   on discovery.
4. **Is this box enrolled?** An edge only accepts boxes on its allow-list,
   each with a token. A box that was reinstalled has a new identity and must
   be enrolled again by whoever runs the edge.
5. **Was the edge reinstalled?** The box pins the edge's tunnel key on first
   contact. A rebuilt edge with a new key is refused, on purpose; the
   operator clears the pinned key on the box (a file under the box's
   secrets, reachable only via a rebuild) or re-enrols the box.

## The box reconnects on its own

Once the edge answers again, the tunnel comes back without a restart, and
the Mesh pane updates. Nothing you turned on is turned off by an outage;
only *turning on* is refused while no edge is found.
