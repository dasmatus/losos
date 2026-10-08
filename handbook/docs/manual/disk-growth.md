---
title: Disk growth
sidebar_position: 7
---

# Disk growth

The installer leaves **10 % of the disk unused** on purpose. The box keeps its
own software on the same encrypted volume as your files, and that software
grows with every update, so a volume that could never be grown would one day
fill up on a box nobody can open a shell on.

## Claiming the reserve

**Storage pane → Use the reserve.** A dialog says how much this adds. The
box grows the volume, the encryption layer and the filesystem, in that order,
with everything still mounted; nothing stops and nothing is unmounted. The
pane's capacity meter shows the reserve as a hatched slice until it is
claimed.

![The Use the reserve dialog: how much room it adds, with Cancel first.](../img/storage-use-reserve.png)

![The three layers grown in order: the logical volume, then the encryption layer, then the filesystem.](../img/disk-growth.svg)

Once the reserve is used up, the button says there is nothing left to claim.

## Adding a disk

A box is one volume group across every fixed disk the installer found. Adding
a disk later means adding it to that group and growing again. That needs the
box's command line, which only exists on the installer medium:

```sh
pvcreate /dev/sdX && vgextend persist-vg /dev/sdX && losos-ctl grow
```

It is a job for whoever maintains the box, not an owner's button, and a
full reinstall with the new disk present is the simpler route for most
people (back up first: the installer wipes everything).

## Running out of room

The Overview shows **Almost full** and then **Out of room**. A full disk
stops LosOS cloud from accepting uploads and, worse, can stop the nightly
rebuild. [Out of room](../troubleshooting/out-of-room) has the steps.

![The Overview on a box that is almost full: the Files tile and the storage card warn.](../img/overview-almost-full.png)
