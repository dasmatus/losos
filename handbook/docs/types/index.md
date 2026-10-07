---
title: Types of setup
sidebar_position: 0
sidebar_label: Overview
slug: /types
---

# Types of setup

"Which LosOS do I have?" has several answers at once, because the box is
shaped along five independent lines. This page is the map; each line has its
own page.

## 1. How the box is deployed

| Type                                   | Who it is for                                   | Remote access | Mesh                | Market |
| -------------------------------------- | ----------------------------------------------- | ------------- | ------------------- | ------ |
| [A box on its own](./single-box.md)         | A home, a flat, one desk                        | no            | no                  | no     |
| [A box with the LosOS edge](./official-edge.md) | Owners who want a public address and the mesh | yes           | yes, with others    | yes, when it opens |
| [A company with its own edge](./company-edge.md) | A company running several boxes on one network | yes, through its own edge | yes, among its own boxes | no |

Every box starts as the first type. The other two are settings, not
reinstalls.

## 2. How the disk is unlocked, and how the machine boots

[Install variants](./install-variants.md): the installer decides **TPM** or
**keyfile** unlock from the hardware it finds, and **UEFI** or **BIOS** from
the firmware menu. There are three install media: the small installer ISO,
the ISO with the full system on it, and the demo disk image for virtual
machines.

## 3. How the apps run

[App modes](./app-modes.md): LosOS cloud and LosOS Git run as **workloads** in the
box's own small Kubernetes cluster (the default) or **natively** on the host.
The owner sees the same apps either way.

## 4. What the disk is for

[Storage modes](./storage-modes.md): a box keeps its disk **to itself** or
**shares** spare room with the mesh, and lends spare CPU inside a daily
**window**, only while idle.

## 5. What is on the board

[Widget types](./widget-types.md): the overview page's board holds **built-in**
widgets, widgets **built from the box's readings** without code, and widgets
**written by hand** in HTML and JavaScript that run in a sandbox.

## And the three names

Whatever the type, the owner meets three names: **LosOS cloud** (files,
photos, calendar), **LosOS Git** (repositories) and **the admin pages** (the
box itself). The [glossary](../reference/glossary.md) has the rest.
