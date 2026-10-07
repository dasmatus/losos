---
title: Settings and Apply
sidebar_position: 3
---

# Settings and Apply

A LosOS box does not have settings in the usual sense. It has a **description
of itself**, written in Nix, and the settings panes edit a small part of that
description. Applying a change means rebuilding the box from the new
description. That is why:

- A change takes a **rebuild**, a few minutes, not a restart. The box stays
  reachable while it works, and a notification says when it is done.
- A change either takes completely or not at all. A rebuild that fails
  leaves the box running its previous settings.
- Every box can be rebuilt from the LosOS source plus one file holding its
  choices (`modules/overrides.nix`). A setting the panes do not write is
  reset on the next save.

## The apply bar

Edit any field and a bar appears at the bottom: *N changes not applied
yet*, with **Discard** and **Apply**. A field that fails its check (a name
with a space, a port out of range) is marked, and Apply waits until it is
fixed. Nothing reaches the box until you press Apply.

## What Apply does

1. The box writes your choices into its overrides file.
2. It starts a rebuild as a background job (`losos-rebuild-<job>`). The
   Overview's **Changes** widget lists these jobs and whether each one took.
3. The new system is switched in. Some changes restart the box's own control
   daemon mid-way; it picks the job back up on its own.
4. A notification says *Changes applied* or *The changes could not be
   applied* with the reason the box gave.

## Two settings that change the address

- **Name**: the box answers to `<name>.local`, and the apps hand out links on
  that name. After renaming, reach the box at its IP address (unchanged) or
  the new name.
- **Encrypt the connection**: off makes the box plain HTTP only; the
  certificate step and passkeys go with it.

## Settings that are not Nix

Two things on the admin pages take effect at once, without a rebuild,
because they are documents the box keeps rather than parts of its
description:

- **Look** (background picture, veil, hand-written widgets): kept by the
  box's control daemon, shared by every browser.
- **Your board** of widgets: kept in the browser you arranged it in.

## The full list

The [settings-to-options table](../reference/settings-to-options) maps every
pane field to the Nix option behind it, for the day you want to change
something the panes do not offer.
