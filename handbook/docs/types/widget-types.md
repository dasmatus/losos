---
title: Widget types
sidebar_position: 7
---

# Widget types

The overview page has a **board** of widgets: small tiles that keep an eye
on the things you care about. **Add a widget** opens the gallery, which has
three kinds.

| Kind                     | Where it lives     | Can it run code?                 | How it gets the box's readings                       |
| ------------------------ | ------------------ | -------------------------------- | ---------------------------------------------------- |
| **Built-in**             | in the system      | yes, compiled into the admin page | directly                                             |
| **Built from readings**  | in this browser    | no                               | one reading, a few expressions in a small language    |
| **Written by hand**      | on the box         | yes, in a sandbox                | through the `losos` object, over messages             |

## Built-in

Five ship with the box: **Answering** (a year of days, shaded by how much
of each day the box answered), **Room** (disk in use and the reserve),
**Sharing** (hours lent against hours kept), **Apps** (which apps answered
just now) and **Changes** (settings changes applied, newest first). Each draws
in one of four shapes: heatmap, number, bar or list. Built-ins are written in
the admin page's own code and arrive with system updates.

![The widget gallery: the built-in widgets, Build one and Write one.](../img/widget-gallery.png)

## Built from readings

**Build one** composes a widget without code: pick a reading the box
publishes (room on the disk, the mesh window, the settings, and so on), a
shape, and write what to show with short expressions such as
`m.free / m.total`. The widget is checked and previewed before it is kept. It
lives in the browser you made it in, changes nothing on the box and cannot
reach the network, the page or anything but its one reading. That is also
its limit: no branching across readings, no loops, no memory between runs.

![Build one: a reading, a shape, the expression to show and a preview of the tile.](../img/widget-build-one.png)

## Written by hand

**Write one** takes HTML, style and script of your own. The widget is kept on
the box, so every browser that opens the box sees it, and it is listed on
**Settings → Look**, where it can be edited and deleted.

![Write one: the source of a hand-written widget beside its live preview in the sandboxed frame.](../img/widget-write-one.png)

A hand-written widget runs inside a **sandboxed frame**: an origin of its
own with no access to the admin pages, the admin key or the box's API. It
talks to the box through a small `losos` object the frame provides:

| Call                        | What it gives                                                                 |
| --------------------------- | ----------------------------------------------------------------------------- |
| `losos.metric(name)`        | a promise of one of the box's readings, the same names the built-ins use     |
| `losos.theme`, `losos.lang` | `light`/`dark` and the language the admin page is in                           |
| `losos.palette`             | the box's colours, also set as CSS variables (`var(--accent)` works)           |
| `losos.onTheme(fn)`         | called whenever the owner switches the theme                                  |
| `losos.resize()`            | asks the board to re-measure the tile                                         |

The editor's **Help** tab repeats this with an example, and its preview is
the real frame, so what it shows is what the tile will show. A hand-written
widget **may** fetch the internet (a weather tile, say), **may not** reach
the box's API, and runs in every browser that opens the box, so read anything
you paste from the internet before you keep it. Up to 24 widgets of 64 KiB
each.


See [Look and widgets](../manual/look-and-widgets) for the board itself,
backgrounds and the veil.
