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
assignment names five requirements. This chapter is where each is answered,
in the handbook's own words rather than a separate report; the rest of the
handbook is the documentation the fifth requirement asks for.

| Requirement                                        | Where it is answered                                                          |
| -------------------------------------------------- | ----------------------------------------------------------------------------- |
| 1. Design an OS architecture for mesh storage      | [Architecture](./architecture.md)                                                   |
| 2. Choose a suitable kernel                         | [The kernel](./kernel.md)                                                           |
| 3. Install and configure the OS                    | [Install](../start/install.md), [The first run](../start/first-run.md), [Settings and Apply](../manual/settings-and-apply.md); demonstrated in a virtual machine |
| 4. Prepare a price offer                            | [The price offer](./price-offer.md)                                                 |
| 5. Write the project documentation                 | This handbook, and the developer [wiki](https://github.com/dasmatus/losos/wiki) |

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
