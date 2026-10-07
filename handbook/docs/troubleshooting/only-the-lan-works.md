---
title: Only the local network works
sidebar_position: 1
---

# Only the local network works

<div className="losos-symptom">

**What you see:** the box's own pages open at its `http://192.168.…` address
or its `.local` name, and everything inside them works. Remote access from
outside does not, the Mesh pane finds no edge, or nothing on the internet
loads from any computer at home.

</div>

The box is designed for exactly this: **everything that lives on the box
keeps working on the local network**, whatever took the outside away.
Nothing is lost, nothing is paused, and nothing needs to be set up again
when the outside comes back.

## What can cause it

Any of these leaves the box reachable from the LAN and nothing else. The
steps below are the same for all of them; the cause only decides who fixes
it.

| Cause                                   | How to recognise it                                                                                     |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| The provider's line is down             | No website opens from any device at home; the router's status light says so. Nothing to do but wait.    |
| The router has no uplink                | The router lost its connection (unplugged cable, a reboot in progress, a changed setting). The LAN still works because the switch does. |
| The edge is down or unreachable         | The internet works from your laptop, but the Mesh pane finds no edge and the public name does not answer: [No edge found](./edge-not-found.md). |
| The provider's DNS is broken            | Pages by name fail, the box by IP works. Switch the router or the computer to another resolver.          |
| The box was never joined to an edge     | Not an outage: a [single box](../types/single-box) has no remote access and no mesh by design.           |

## What still works

| What                                | Why it does not need the internet                                                                                |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Signing in to the admin pages       | The password is checked against LosOS cloud on the box itself, over the box's own loopback. The printed spare key works too. |
| LosOS cloud and LosOS Git           | Both run on the box. Files, calendars, contacts, the mobile apps on the same Wi-Fi, repositories, all local.   |
| Your own files                      | They are on the box's encrypted disk, in every storage mode.                                                     |
| Local sharing between people at home | Shares, links and permissions inside LosOS cloud are decided on the box.                                       |
| The whole admin UI                  | Apply, Settings, the Look pane, the Storage pane, restarts, all of it talks to the box over the LAN.           |
| Built-in widgets and hand-written widgets | They render on the box, from the box's own readings; a hand-written widget that *chooses* to fetch something from the internet shows that part empty until the outside is back, the rest of it draws. |
| This handbook                       | The box serves its own copy at `/handbook/`, search included, which is what you are reading if you opened it from Settings → About. |
| The nightly restart and the 03:00 update with the default source | The default source is the box's own copy of its description, on the disk. The rebuild runs and finishes offline. |
| A company edge on the same LAN      | A [company deployment](../types/company-edge) keeps its edge, its mesh and its sharing on the local network; the ISP is not in that path. |

## What waits for the outside

| What                                        | What you see meanwhile                                                                                              |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Remote access through the LosOS edge        | Your box's public address does not answer from outside. The tunnel reconnects on its own when the outside is back.     |
| The [LosOS edge](../types/official-edge) itself: the mesh, compute sharing, the market | The Mesh pane says no edge was found; a switch that needs the edge is refused with a message rather than left half on. |
| Room **shared with the mesh**               | The pool lives across the boxes of the edge, so whatever is kept in it is out of reach while the edge is. Your own files are never in the pool; they are on the box and unaffected. |
| **Find more** on the Apps page              | The catalogue is fetched from the internet; the search shows nothing until it is back. Apps already installed keep running. |
| The 03:00 update with a `github:` source    | It cannot fetch and the night's rebuild records as failed in the **Changes** widget. The box keeps running on what it has and tries again the next night. |
| The binary cache                            | A rebuild that needs new software builds it on the box instead, slowly, or fails for the night. The next successful night catches up. |
| Letting a visitor in through a public link  | Public share links go through the box's public address; they work again with the outside.                             |

## How to tell an outage from a box fault

The box has no indicator light for "the internet works"; what it has is the
things above.

1. Open the box's address. The Overview **answering** and the app tiles
   opening means the box and the LAN are fine. If even that does not open,
   this is not an ISP problem: [Cannot reach the box](./cannot-reach-the-box.md).
2. Open the **Mesh** pane. A box joined to the LosOS edge says no edge was
   found while the outside is down; a box with a company edge on the LAN still
   shows it as found. A box that was never joined shows nothing either way,
   which is normal for a [single box](../types/single-box). An edge the
   internet can reach but the box cannot is the edge's problem, not the
   line's: [No edge found](./edge-not-found.md).
3. Look at the **Changes** widget on the Overview after 03:00. A failed
   rebuild on a box with a `github:` update source during an outage is the
   outage, not a fault on the box; the next night's entry says so.
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
- **Do not turn sharing off and on** to wake the mesh: turning it on is
  refused while no edge answers, and the pool comes back on its own.
- **Do not change the update source** to work around a failed night. The
  default source works offline; a `github:` source simply waits.
