---
title: The project
sidebar_position: 0
sidebar_label: Overview
slug: /project
---

# The project

LosOS is Matúš Maštena's matriculation (maturita) project at SPŠE Hálova,
Bratislava, defended in April 2027, and the seed of a product: a full setup
(boxes plus an edge on one network) to offer to companies afterwards. The
official assignment (received 7 October 2026) names four requirements. This
chapter is where each is answered, in the handbook's own words rather than a
separate report; the Slovak hand-in document, the
[technical documentation (KOP)](./kop/index.md), is built from this handbook
and adds what the assignment asks for beyond it.

![A box's admin pages in October 2026, with the Tide background and a hand-written widget.](../img/overview-tide.png)

| Requirement                                                              | Where it is answered                                                          |
| ------------------------------------------------------------------------ | ----------------------------------------------------------------------------- |
| 1. Examine and describe the system's main functions and options           | [Architecture](./architecture.md), [What LosOS is](../start/what-is-losos.md), [Types of setup](../types/index.md); in Slovak, [Funkcionality](./kop/funkcionality.md) and [Architektúra](./kop/architektura.md) |
| 2a. Choose a suitable kernel                                              | [The kernel](./kernel.md); in Slovak, [Výber kernelu](./kop/kernel.md)        |
| 2b. Install the system                                                    | [Install](../start/install.md); demonstrated in a virtual machine; in Slovak, [Inštalácia](./kop/instalacia.md) |
| 2c. Perform the basic configuration                                       | [The first run](../start/first-run.md), [Settings and Apply](../manual/settings-and-apply.md); in Slovak, [Základná konfigurácia](./kop/konfiguracia.md) |
| 2d. Verify the configuration and that the system works                    | in Slovak, [Overenie](./kop/overenie.md): the test suites, a checklist for the VM demo, the independent review |
| 3. Prepare a price offer for the hardware and software                    | [The price offer](./price-offer.md); in Slovak, [Cenová ponuka](./kop/cenova-ponuka.md); the priced tables are kept with the project's materials |
| 4. Write the technical documentation                                      | This handbook, the developer [wiki](https://github.com/dasmatus/losos/wiki), and the [KOP](./kop/index.md) itself |

## What is done and what is planned

**Done and tested**: the stateless appliance with encrypted persistence,
TPM and keyfile unlock, UEFI and BIOS installs, the admin pages with one
password and a spare key, LosOS cloud and LosOS Git as workloads, nightly
self-repair and self-update, disk growth without opening the box, the
hardening baseline, the edge with tunnel, mesh and compute window, the market
end to end against Stripe's test mode, the edge's control plane on Vercel for
demonstrations, and this handbook on every box.

**Planned**: opening the market once a business exists to be the Stripe
platform; edge discovery with the official-edge trust anchor (in progress
as this is written); Slovak and German translations of this handbook; a
packaged company deployment with a reproducible demo script.

## Source and licence

The whole project, this handbook included, is at
[github.com/dasmatus/losos](https://github.com/dasmatus/losos). Every design
choice carries its reasoning in a comment at the top of the file that makes
it, which is where the examiner's "why" questions are answered first.
