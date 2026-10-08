---
title: Types of setup
sidebar_position: 0
sidebar_label: Overview
slug: /types
---

# Types of setup

"Which LosOS do I have?" has five answers at once, because a box varies
along five independent lines. This page sums up each line and links to its
own page.

![The three deployment types side by side: a box on its own on a home network; a box that opens a tunnel to the official LosOS edge; several boxes on an office network sharing one pool through the company's own edge.](../img/setup-types.svg)

## 1. How the box is deployed

| Type                                   | Who it is for                                   | Remote access | Mesh                | Market |
| -------------------------------------- | ----------------------------------------------- | ------------- | ------------------- | ------ |
| [A box on its own](./single-box.md)         | A home, a flat, one desk                        | no            | no                  | no     |
| [A box with the LosOS edge](./official-edge.md) | Owners who want a public address and the mesh | yes           | yes, with others    | yes, when it opens |
| [A company with its own edge](./company-edge.md) | A company running several boxes on one network | yes, through its own edge | yes, among its own boxes | no |

Every box starts as the first type. You reach the other two by changing
settings, not by reinstalling.

## 2. How the disk is unlocked, and how the machine boots

[Install variants](./install-variants.md) covers both. The installer picks
TPM or keyfile unlock from the hardware it finds, and takes **UEFI** or
**BIOS** from the firmware menu. There are three install media: the small
installer ISO, the ISO with the full system on it, and the demo disk image for
virtual machines.

## 3. How the apps run

[App modes](./app-modes.md) explains the two modes. By default, LosOS cloud and LosOS
Git run as workloads in the box's own small Kubernetes cluster. They can also
run natively on the host. You see the same apps either way.

## 4. What the disk is for

[Storage modes](./storage-modes.md) covers the disk and the CPU. A box keeps
its disk to itself or shares spare room with the mesh. It can also lend spare
CPU inside a daily window, only while idle.

## 5. What is on the board

[Widget types](./widget-types.md) lists the three kinds on the overview
page's board: built-in widgets, widgets built from the box's readings without
code, and widgets written by hand in HTML and JavaScript, which run in a
sandbox.

## And the three names

Whatever the type, you meet three names. LosOS cloud holds files, photos
and calendars. LosOS Git holds repositories. The admin pages run the box
itself. The [glossary](../reference/glossary.md) has the rest.
