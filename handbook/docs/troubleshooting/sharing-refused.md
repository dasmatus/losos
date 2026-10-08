---
title: Sharing is refused
sidebar_position: 12
slug: /troubleshooting/sharing-refused
---

# Sharing is refused

<div className="losos-symptom">

**What you see:** the switch for sharing your disk, joining the mesh or
lending spare time cannot be turned on, is greyed out, or Apply refuses the
change with a reason.

</div>

## The reasons, and which one you have

![The Mesh pane with no edge in reach: the sharing switches are greyed with Needs an edge proxy in reach.](../img/mesh-none.png)

| What you see                                           | Reason                                                                                      | What to do                                                                 |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Switch greyed, "Needs an edge proxy in reach."; the Mesh pane says **No edge proxy found** | No edge was found. Storage and compute sharing need somewhere to share *to*. | [No edge found](edge-not-found).                                            |
| Apply refused: "no edge proxy is reachable from this box, so `losos.sharingMyStorage` cannot be turned on …" | The edge answered when you flipped the switch and had gone by the time you applied. The box refuses to promise room to a pool it cannot reach. | Wait for the Mesh pane to show the edge again (it rechecks every 20 seconds) and apply again. Nothing was changed. |
| **Market** tab greyed out with "soon", disk sharing with it | The market is not open yet, and disk sharing lives on its pane because lending disk and being paid for it are one decision. | Nothing today. The nightly update turns it on when the market opens.       |
| The edge's row wears a warning sign; Market says "The edge proxy this box found is not run by LosOS" | The edge is a company's own, or the LosOS root key is not published yet. Sharing through it works; trading is refused by design. | Use the LosOS edge for trading. See [official or not](../types/official-edge#official-or-not). |
| "Join the mesh first."                                 | Lending spare time needs the box in the edge's cluster.                                      | Turn **Join the mesh** on, then set the hours; both go out in one apply.     |
| Hours set, nothing is ever lent                        | The window starts and ends at the same minute, or the box is never idle inside it.           | Set a real window; the caption under the hours says how long it is.          |
| Apply fails on a mesh change                           | The rebuild itself failed, after the box accepted the change.                                 | [Apply fails](apply-fails), then try again with the edge up.                 |

Only *turning on* is ever refused. A box whose edge went away keeps the
sharing it already had, can still change every other setting, and can turn
sharing off at any time.

## What sharing never does

It never shares your own files. Shared room is a separate account on the
box with its own encrypted directory; your files and the mesh's copies cannot
read each other. Turning sharing off makes that directory unreadable again,
even to the box.
