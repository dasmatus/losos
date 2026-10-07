---
title: Sharing is refused
sidebar_position: 12
---

# Sharing is refused

<div className="losos-symptom">

**What you see:** the switch for sharing your disk, joining the mesh or
lending spare time cannot be turned on, is greyed out, or Apply refuses the
change with a reason.

</div>

## The reasons, and which one you have

| What you see                                           | Reason                                                                                      | What to do                                                                 |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Mesh switches off and disabled, "no edge"              | No edge was found. Storage and compute sharing need somewhere to share *to*.                | [No edge found](edge-not-found).                                            |
| **Market** tab greyed out with "soon", disk sharing with it | The market is not open yet, and disk sharing lives on its pane because lending disk and being paid for it are one decision. | Nothing today. The nightly update turns it on when the market opens.       |
| Market says this edge is not official                  | The edge is a company's own. Local sharing works; trading is refused by design.              | Use the LosOS edge for trading. See [official or not](../types/official-edge#official-or-not). |
| "Join the mesh first"                                  | Lending spare time needs the box in the edge's cluster.                                      | Turn **Join the mesh** on, apply, then the hours.                            |
| Hours set, nothing is ever lent                        | The window starts and ends at the same minute, or the box is never idle inside it.           | Set a real window; the caption under the hours says how long it is.          |
| Apply fails on a mesh change                           | The edge was reachable when you flipped the switch and not when the rebuild ran.             | [Apply fails](apply-fails), then try again with the edge up.                 |

## What sharing never does

It never shares your own files. Shared room is a separate account on the
box with its own encrypted directory; your files and the mesh's copies cannot
read each other. Turning sharing off makes that directory unreadable again,
even to the box.
