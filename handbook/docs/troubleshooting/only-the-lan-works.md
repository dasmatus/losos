---
title: Only the local network works
sidebar_position: 1
slug: /troubleshooting/only-the-lan-works
---

# Only the local network works

<div className="losos-symptom">

**What you see:** the box's own pages open at its `http://192.168.…` address
or its `.local` name, and everything inside them works. Remote access from
outside does not, the Mesh pane finds no edge, or nothing on the internet
loads from any computer at home.

</div>

The box is built for this case. **Everything that lives on the box keeps
working on the local network**, whatever took the outside away. Nothing is
lost or paused, and you set nothing up again when the outside comes back.

## What can cause it

Any of these leaves the box reachable from the LAN and nothing else. The
steps below are the same for all of them. The cause only decides who fixes
it.

| Cause                                   | How to recognise it                                                                                     |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| The provider's line is down             | No website opens from any device at home, and the router's status light shows it. You can only wait.    |
| The router has no uplink                | The router lost its connection through an unplugged cable, a reboot in progress or a changed setting. The LAN still works because the switch does. |
| The edge is down or unreachable         | The internet works from your laptop, but the Mesh pane finds no edge and the public name does not answer. See [No edge found](./edge-not-found.md). |
| The provider's DNS is broken            | Pages by name fail, the box by IP works. Switch the router or the computer to another resolver.          |
| The box was never joined to an edge     | Not an outage. A [single box](../types/single-box) has no remote access and no mesh by design.           |

## What still works

![A box whose internet is down but whose company edge on the LAN still answers: the Mesh pane lists it On this network.](../img/mesh-company.png)

| What                                | Why it does not need the internet                                                                                |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Signing in to the admin pages       | The box checks the password against LosOS cloud on the box itself, over its own loopback. The printed spare key works too. |
| LosOS cloud and LosOS Git           | Both run on the box, so files, calendars, contacts, repositories and the mobile apps on the same Wi-Fi all stay local. |
| Your own files                      | They are on the box's encrypted disk, in every storage mode.                                                     |
| Local sharing between people at home | The box decides shares, links and permissions inside LosOS cloud.                                             |
| The whole admin UI                  | Apply, Settings, the Look pane, the Storage pane and restarts all talk to the box over the LAN.                |
| Built-in widgets and hand-written widgets | They render on the box, from the box's own readings. A hand-written widget that *chooses* to fetch something from the internet shows that part empty until the outside is back, and draws the rest. |
| This handbook                       | The box serves its own copy at `/handbook/`, search included. If you opened this page from Settings → About, you are reading that copy. |
| The nightly restart and the 03:00 update with the default source | The default source is the box's own copy of its description, on the disk. The rebuild runs and finishes offline. |
| A company edge on the same LAN      | A [company deployment](../types/company-edge) keeps its edge, its mesh and its sharing on the local network. The ISP is not in that path. |

## What waits for the outside

| What                                        | What you see meanwhile                                                                                              |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Remote access through the LosOS edge        | Your box's public address does not answer from outside. The tunnel reconnects on its own when the outside is back.     |
| The [LosOS edge](../types/official-edge) itself: the mesh, compute sharing, the market | The Mesh pane says no edge was found. The box refuses a switch that needs the edge with a message, instead of leaving it half on. |
| Room **shared with the mesh**               | The pool spreads across the edge's boxes, so whatever it holds is out of reach while the edge is. Your own files are never in the pool. They stay on the box, untouched. |
| **Find more** on the Apps page              | The box fetches the catalogue from the internet, so the search shows nothing until the internet is back. Apps already installed keep running. |
| The 03:00 update with a `github:` source    | It cannot fetch, and the **Changes** widget records the night's rebuild as failed. The box keeps running on what it has and tries again the next night. |
| The binary cache                            | A rebuild that needs new software builds it on the box instead, slowly, or fails for the night. The next successful night catches up. |
| Letting a visitor in through a public link  | Public share links go through the box's public address; they work again with the outside.                             |

## How to tell an outage from a box fault

The box has no indicator light for "the internet works". What it has is the
things listed above.

1. Open the box's address. If the Overview says **answering** and the app
   tiles open, the box and the LAN are fine. If even that does not open,
   the ISP is not the problem. See [Cannot reach the box](./cannot-reach-the-box.md).
2. Open the **Mesh** pane. A box joined to the LosOS edge says no edge was
   found while the outside is down. A box with a company edge on the LAN
   still shows it as found. A box that was never joined shows nothing either
   way, which is normal for a [single box](../types/single-box). If the
   internet can reach an edge but the box cannot, the problem is the edge,
   not the line. See [No edge found](./edge-not-found.md).
3. Look at the **Changes** widget on the Overview after 03:00. On a box with
   a `github:` update source, a rebuild that failed during an outage is the
   outage, not a fault on the box. The next night's entry shows it.
4. Open any website from a phone on the same Wi-Fi. If that fails too, the
   line or the router is the problem and the box is waiting with you. If it
   works, the box's own uplink or the edge is what is missing.

## When the outside comes back

Nothing to do. The tunnel to the edge reconnects, the shared pool comes
back, the next 03:00 run fetches what the last one could not, and the
Mesh pane goes back to its usual state. You do not re-enter anything, and
you do not restart the box. If the Mesh pane still shows no edge an hour
after the internet is back, see [No edge found](./edge-not-found.md).

## What to avoid during the outage

- **Do not reinstall or factory-reset** because remote access stopped.
  Neither brings the outside back, and both throw away local state.
- **Do not turn sharing off and on** to wake the mesh. The box refuses to
  turn it on while no edge answers, and the pool comes back on its own.
- **Do not change the update source** to work around a failed night. The
  default source works offline, and a `github:` source waits.
