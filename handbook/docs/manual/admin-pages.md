---
title: The admin pages
sidebar_position: 1
---

# The admin pages

The admin pages are the box's own web page at `http://<address>/`. The box
serves them to your local network only, even when an edge publishes the box.
Every tab asks you to sign in with your own name and password on LosOS
Login, the box's sign-in page at `https://<address>/auth/`, before it shows
anything you can change.

## The sections

The sidebar on the left has six sections, and under them the things you
pinned. Collapsed, it shrinks to a rail of icons. On a phone it opens as a
sheet.

![The Overview: the sidebar with the six sections on the left, the box's name and whether it is answering, the app tiles, the storage card and the board.](../img/overview.png)

| Section      | What is there                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------------- |
| **Overview** | Whether the box is answering and for how long, the storage card, the app tiles and your board of widgets.  |
| **Apps**     | The apps on this box with their mode, a search of Artifact Hub, and the apps installed from it.            |
| **Storage**  | How full the disk is, what is holding the room, and the reserve to claim.                                   |
| **Mesh**     | Joining other boxes, the hours this one lends its spare time, **Market** with disk sharing, and **Machines**. |
| **Lab**      | LosOS Lab, which draws this box and its network as a diagram.                                              |
| **Settings** | Network, Look, Hardware, Security, Users, Access tokens, Advanced, History, Backup, About and Reset, each a pane.  |

<img src={require("../img/overview-phone.png").default} alt="The Overview on a phone: the sections fold into a menu button." width="260" /> <img src={require("../img/overview-phone-sheet.png").default} alt="The same phone with the menu open as a sheet over the page." width="260" />

## Search

The magnifier at the top of the sidebar opens one list that finds pages,
apps, files and repositories. Ctrl+Shift+P on any admin page opens it too
(Cmd+Shift+P on a Mac), and so does typing a colon anywhere outside a text
field. It finds:

- the sections and settings panes, by name or by a word such as "gpu" or
  "hostname", in the language on screen or in English
- the apps in LosOS cloud, and LosOS Git
- files and folders in LosOS cloud, by name
- repositories in LosOS Git
- text inside files, once you type three letters: in your LosOS cloud files,
  and on the main branch of every repository in LosOS Git
- your pins
- apps to install from Artifact Hub; picking one opens the Apps pane with
  its install dialog
- a few actions: the light or dark theme, the language, the tour, this
  handbook and signing out

The arrow keys move through the list, Enter opens the highlighted entry and
Escape closes the list. Files and repositories open in their own app, and
Ctrl+Enter or Cmd+Enter opens one in a new tab.

:::note[File search needs the box's app password]
File search uses an app password the box gets from LosOS cloud when the
owner's password is set, which the wizard does. Without it, the list says so
in place of the files; see [How the box lists your files](#pins) below.
:::

To search inside your LosOS cloud files, the box keeps an index of their
text. It checks for new and changed files every two minutes and reads only
those. It reads plain text and code, Markdown, CSV, JSON and HTML,
OpenDocument files, Word, Excel and PowerPoint files in their current
formats (.docx, .xlsx, .pptx), and PDFs. Pictures, audio, video and archives
are skipped, and only the first megabyte of text in each file is kept.
Right after the first start the list says the box is still reading your
files. The index stays on the box, a factory erase deletes it, and
`losos.search.enable` on the Advanced pane turns it off. LosOS Git needs no index: each search reads the repositories
directly.

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
  The [search](#search) finds pins too.
- Right-click a file, folder or repository in the search, an app tile on
  the Overview, or a pin in the sidebar, and choose **Pin to sidebar** or
  **Unpin from sidebar**. On a touch screen, press and hold instead.

![The Overview with six pins under Pinned in the sidebar, and the pinned Calendar and Notes tiles marked.](../img/pins-overview.png)

![The pins list with its Files tab open on the Documents folder of LosOS cloud.](../img/pins-dialog.png)

:::note[How the box lists your files]
The box lists your files with an app password it asks LosOS cloud for when
the owner's password is set, which the wizard does. The app password shows
up in LosOS cloud's security settings as "LosOS admin page". Revoking it
there stops the listing, and file search, until the owner's password is set
again, through the spare-key route in
[Forgot the password](../troubleshooting/forgot-password#you-have-the-spare-admin-key). The sidebar holds 24 pins.
:::

## The tour

The first time you reach these pages after setup, a short tour opens over
the Overview: six steps on every box, and one more for each feature this
box has ([The first run](../start/first-run) lists them). **Take the tour** under Settings, About opens it
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

**Sign out** ends your sign-in on LosOS Login too, so the next visit asks
for your name and password again. If you opened the pages with the spare
key, it forgets the key in this tab instead, and closing the tab does the
same. See [Sign-in and the spare key](sign-in-and-spare-key).

## What the admin pages cannot do

There is no file browser, no terminal and no log viewer. Your files live in
LosOS cloud; the **Files** tab of the pins list shows folders only so you can
pin from them. The box has no shell, so neither do the pages. The box reports
on itself through the Overview, the Storage pane and the About pane. When
those are not enough, [When something goes wrong](../troubleshooting) says
what else there is.
