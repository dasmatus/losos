---
title: Sharing and editing together
sidebar_position: 4.45
slug: /manual/sharing-and-editing-together
---

# Sharing and editing together

![LosOS Cloud's files.](../img/cloud-files.png)

In [LosOS Cloud](losos-cloud), text files open in an editor that several
people can type in at once. Each person sees the others' faces at the top,
their cursors in their own colour with their name beside them, and who
edited last under the text. Plain text opens this way: `.txt`, `.csv`,
`.json`, `.yaml` and similar. A Markdown file opens in
[Notes](notes-and-knowledge-bases), and its **Edit** brings you here.

On one box this works for anyone who can open the file. Once the box is in a
mesh, you can also share a file or a folder with a person on another box,
and write in it together across the two boxes.

## Open a file to edit it

Click a text file in **Files**. The editor shows:

- the people in the file right now, as round faces;
- **Live** while your browser is connected to the box;
- **Edited by** the last person who changed it, and when.

The file on the box is saved a moment after anyone stops typing, so the
desktop and phone apps see the change too. Each save is a version you can
go back to. If an app changes the file while people have it open, the
change appears in their editors.

## Share with someone on another box

Each person has an address of the form `name@box`, for example
`eva@kitchenbox.local`. Yours is at the bottom of the share window.

1. In **Files**, tick the file or folder and press **Share**.
2. Type the other person's address.
3. Choose **Can edit** or **Can view**, and press **Share**.

The list in the same window shows each person you shared with and where the
offer stands: waiting, accepted or declined. Change what they can do there,
or press the cross to stop sharing. Stopping takes the file away from their
box too.

## What you receive

Files shared with you are under **Shared with you**. A new one waits for
you to **Accept** or **Decline** it. After that:

- a text file opens in the editor, together with its owner and anyone else
  it is shared with;
- a folder opens as a list; its files open or download from there;
- any other file downloads.

The file stays on its owner's box. Your box keeps a copy of a text file you
edit, so you can keep writing while the other box is out of reach. The
editor then says so, and **Shared with you** shows that your changes are
waiting. When the boxes reach each other again, the changes from both
sides come together without anyone losing a line.

## How boxes find each other

A box answers other boxes at `https://<name>.local/mesh/v1/link`, or at the
public name the edge gives it. Each box has its own key. The first time two
boxes talk, each remembers the other's key, and from then on refuses a box
that comes with a different one under the same name. Nobody signs in to
this address; the keys decide who is talking.

## Settings

These are on the **Advanced** pane:

- `losos.cloud.mesh.enable` turns sharing across boxes on or off. It is on
  when the box shares its storage or computing with a mesh. Editing
  together on one box works either way.
- `losos.cloud.mesh.name` is the name other boxes reach this box by, the
  part after the `@` in people's addresses. It is `<name>.local` on the
  box's network, or the edge's name for the box when it is reached through
  an edge. Other boxes remember this box's key under that name, so a new
  name means sharing again.
- `losos.cloud.mesh.selfSigned` lets the box accept another box's
  self-signed certificate. It is on unless the box is reached through an
  edge.
