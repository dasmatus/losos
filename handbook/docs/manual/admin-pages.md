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
| **Mesh**     | Joining other boxes, the hours this one lends its spare time, and **Market** with disk sharing.            |
| **Settings** | Network, Look, Hardware, Security, Advanced, History, About and Reset, each a pane.                         |

<img src={require("../img/overview-phone.png").default} alt="The Overview on a phone: the sections fold into a menu button." width="260" /> <img src={require("../img/overview-phone-sheet.png").default} alt="The same phone with the menu open as a sheet over the page." width="260" />

## Search

The search box at the top of the sidebar opens one list that finds pages,
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
- apps to install from Artifact Hub; picking one opens the Apps pane with
  its install dialog
- a few actions: the light or dark theme, the language, the tour, this
  handbook and signing out

The arrow keys move through the list, Enter opens the highlighted entry and
Escape closes the list. Files and repositories open in their own app, and
Ctrl+Enter or Cmd+Enter opens one in a new tab.

File search needs one sign-in with your password, not the spare key. Until
then the list says so in place of the files.

To search inside your LosOS cloud files, the box keeps an index of their
text. It checks for new and changed files every two minutes and reads only
those. It reads plain text and code, Markdown, CSV, JSON and HTML,
OpenDocument files, Word, Excel and PowerPoint files in their current formats
(.docx, .xlsx, .pptx), and PDFs. Pictures, audio, video and archives are skipped, and only the first
megabyte of text in each file is kept. Right after the first start the list
says the box is still reading your files. The index stays on the box, a
factory erase deletes it, and `losos.search.enable` on the Advanced pane turns
it off. LosOS Git needs no index: each search reads the repositories
directly.

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
LosOS cloud. The box has no shell, so neither do the pages. The box reports
on itself through the Overview, the Storage pane and the About pane. When
those are not enough, [When something goes wrong](../troubleshooting) says
what else there is.
