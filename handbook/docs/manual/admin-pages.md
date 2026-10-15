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

The sidebar on the left has five sections, and under them the things you
pinned. Collapsed, it shrinks to a rail of icons. On a phone it opens as a
sheet.

![The Overview: the sidebar with the five sections on the left, the box's name and whether it is answering, the app tiles, the storage card and the board.](../img/overview.png)

| Section      | What is there                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------------- |
| **Overview** | Whether the box is answering and for how long, the storage card, the app tiles and your board of widgets.  |
| **Apps**     | The apps on this box with their mode, a search of Artifact Hub, and the apps installed from it.            |
| **Storage**  | How full the disk is, what is holding the room, and the reserve to claim.                                   |
| **Mesh**     | Joining other boxes, the hours this one lends its spare time, and **Market** with disk sharing.            |
| **Settings** | Network, Look, Hardware, Security, Advanced, History, About and Reset, each a pane. A search box filters the panes.             |

<img src={require("../img/overview-phone.png").default} alt="The Overview on a phone: the sections fold into a menu button." width="260" /> <img src={require("../img/overview-phone-sheet.png").default} alt="The same phone with the menu open as a sheet over the page." width="260" />

## Pins

Pin the apps, files and repositories you open most, and they sit under
**Pinned** in the sidebar. A pin opens in its own app: a file or folder in
LosOS cloud, a repository on LosOS Git, an app on the Overview, or an app
you installed from the search on its own port. The box keeps the list, so
every browser you sign in from shows the same pins.

- On the Overview, point at an app tile and click the pin at its corner. A
  pinned tile keeps the pin, in colour.
- On the Apps page, **Pin** sits beside each installed app.
- The pencil beside **Pinned** opens the full list. There you move a pin up
  or down, unpin it, and pin from three tabs: **Apps**, **Files**, which
  walks the folders of LosOS cloud, and **Repositories**.
- Point at a pin in the sidebar and click the crossed-out pin to unpin it.
  The search at the top of the sidebar finds pins too.

![The Overview with six pins under Pinned in the sidebar, and the pinned Calendar and Notes tiles marked.](../img/pins-overview.png)

![The pins list with its Files tab open on the Documents folder of LosOS cloud.](../img/pins-dialog.png)

The box lists your files with an app password it asks LosOS cloud for when
you sign in with your password. Until you have done that once, the
**Files** tab asks you to. The app password shows up in LosOS cloud's
security settings as "LosOS admin page", and revoking it there stops the
listing until your next sign-in. The sidebar holds 24 pins.

## The tour

The first time you reach these pages after setup, a tour of six short steps
opens over the Overview. **Take the tour** under Settings, About opens it
again. With "reduce motion" set in your system, each step shows a still
picture instead of a moving one.

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
LosOS cloud; the **Files** tab of the pins list shows folders only so you can
pin from them. The box has no shell, so neither do the pages. The box reports
on itself through the Overview, the Storage pane and the About pane. When
those are not enough, [When something goes wrong](../troubleshooting) says
what else there is.
