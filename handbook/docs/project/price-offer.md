---
title: The price offer
sidebar_position: 3
---

# The price offer

Requirement 4: *prepare a price offer.* The project answers it with a
market instead of a quote for a box. Owners are paid for the disk and CPU
their boxes share, buyers pay for what they use, and the platform keeps a
fee. The software is finished end to end. Opening the market needs a
registered business to stand behind the Stripe platform, and that comes
after the matriculation.

## What is sold

| Kind      | Unit       | What the buyer gets                                                      |
| --------- | ---------- | ------------------------------------------------------------------------ |
| Storage   | GiB-month  | A replicated volume of that size on the mesh, for 30 days               |
| Compute   | vCPU-hour  | Scheduling on the seller's box, inside its window, for 30 days          |

A box can list only what it already shares: storage while it is enrolled in
the mesh, compute while it also shares compute. When a box stops sharing,
its listing disappears.

## How the money moves

![The Market pane as it will look once it opens: buying storage and compute, and payouts through Stripe with the 4 % platform fee.](../img/market-open.png)

1. A seller onboards. The edge creates a Stripe Express account for the
   box, tagged with the box's identifier, and returns Stripe's hosted
   onboarding.
2. Stripe reports that the account can receive transfers. Only then can the
   seller list.
3. A buyer orders units. The edge reserves them and creates a Stripe
   Checkout session as a destination charge. The buyer pays the platform,
   Stripe forwards the sale to the seller's account, and the fee stays with
   the platform.
4. Stripe reports the payment. The edge marks the order `paid` and fulfils
   it with a namespace and a volume claim on the mesh for storage, or a
   ledger credit for compute.

The fee is 4 % of the gross amount, rounded half up. That is 400 basis
points, and the fee can never exceed 20 %. On a 20.00 EUR sale the platform
keeps 0.80 and the seller receives 19.20. Each edge sets prices in cents of
one currency. An order is at least 0.50 EUR, Stripe's minimum. Money never
touches a box.

## What a company pays

A company running its own edge shares among its own boxes for free. By
design, the market is not available to it. The company pays for the setup:
the boxes, the edge and the installation. That offer, with a reproducible
deployment script and an operator's walkthrough, is being written as the
edge discovery work lands. It will get a price once the business exists.

## Why no bill of materials

A LosOS box is a repurposed mini-PC. The hardware is whatever the owner has
or buys second-hand, and the software is free. What has a price is the
capacity owners share with each other, and the platform that makes sharing
it safe. That is what the market prices.
