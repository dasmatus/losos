---
title: The admin pages
sidebar_position: 1
---

# The admin pages

The admin pages are the box's own web page at `http://<address>/`. The box
serves them to your local network only, even when an edge publishes the box.
Every tab asks for the owner's password before it shows anything you can
change.

## The sections

The sidebar on the left has five sections. Collapsed, it shrinks to a rail of
icons. On a phone it opens as a sheet.

![The Overview: the sidebar with the five sections on the left, the box's name and whether it is answering, the app tiles, the storage card and the board.](../img/overview.png)

| Section      | What is there                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------------- |
| **Overview** | Whether the box is answering and for how long, the storage card, the app tiles and your board of widgets.  |
| **Apps**     | The apps on this box with their mode, a search of Artifact Hub, and the apps installed from it.            |
| **Storage**  | How full the disk is, what is holding the room, and the reserve to claim.                                   |
| **Mesh**     | Joining other boxes, the hours this one lends its spare time, and **Market** with disk sharing, greyed out. |
| **Settings** | Network, Look, Hardware, Security, Advanced, History, About and Reset, each a pane. A search box filters the panes.             |

<img src={require("../img/overview-phone.png").default} alt="The Overview on a phone: the sections fold into a menu button." width="260" /> <img src={require("../img/overview-phone-sheet.png").default} alt="The same phone with the menu open as a sheet over the page." width="260" />

## Notifications

Short status messages appear at the top right: a change being applied, a
change that took, a widget added. A question that needs your answer, such as
resetting the box or using the reserve, opens a dialog instead. The safe
choice comes first.

![A settings pane with a change not applied yet: the apply bar at the bottom with Discard and Apply.](../img/settings-apply-bar.png)

## Theme and language

The bar at the top switches between English, Slovak and German, and between
"match the browser", light and dark. This browser remembers both.

![The Overview in German and in the dark theme.](../img/overview-de-dark.png)

## Sign out

**Sign out** forgets the admin key in this tab. Closing the tab does the
same. See [Sign-in and the spare key](sign-in-and-spare-key).

## What the admin pages cannot do

There is no file browser, no terminal and no log viewer. Your files live in
LosOS cloud. The box has no shell, so neither do the pages. The box reports
on itself through the Overview, the Storage pane and the About pane. When
those are not enough, [When something goes wrong](../troubleshooting) says
what else there is.
