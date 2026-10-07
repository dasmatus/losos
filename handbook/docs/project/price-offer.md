---
title: The price offer
sidebar_position: 3
---

# The price offer

Requirement 4: *prepare a price offer.* The project's answer is not a quote
for a box but a **market**: owners are paid for the disk and CPU their boxes
share, buyers pay for what they use, and the platform keeps a fee. The
software for it is finished end to end; opening it needs a registered
business to stand behind the Stripe platform, which comes after the
matriculation.

## What is sold

| Kind      | Unit       | What the buyer gets                                                      |
| --------- | ---------- | ------------------------------------------------------------------------ |
| Storage   | GiB-month  | A replicated volume of that size on the mesh, for 30 days               |
| Compute   | vCPU-hour  | Scheduling on the seller's box, inside its window, for 30 days          |

A box can list only what it already shares: storage while it is enrolled in
the mesh, compute while it also shares compute. Stop sharing and the listing
leaves the shelf.

## How the money moves

1. A seller onboards: the edge creates a Stripe Express account for the box,
   tagged with the box's identifier, and returns Stripe's hosted onboarding.
2. Stripe reports the account can receive transfers; only then can the
   seller list.
3. A buyer orders units. The edge reserves them and creates a Stripe
   Checkout session as a destination charge: the buyer pays the platform,
   the sale is forwarded to the seller's account, the fee stays.
4. Stripe reports the payment; the order is `paid` and fulfilled: a
   namespace and a volume claim on the mesh for storage, a ledger credit for
   compute.

The fee is **4 %** of the gross amount (400 basis points, capped at 20 %),
rounded half up: on a 20.00 EUR sale the platform keeps 0.80 and the seller
receives 19.20. Prices are in cents of one currency per edge; an order is at
least 0.50 EUR, Stripe's floor. Money never touches a box.

## What a company pays

A company running its own edge shares among its own boxes for free; the
market is not available to it, by design. What it buys is the setup: the
boxes, the edge, and the installation. That offer, with a reproducible
deployment script and an operator's walkthrough, is being written as the
edge discovery work lands, and will be priced once the business exists.

## Why no bill of materials

A LosOS box is a repurposed mini-PC; the hardware is whatever the owner has
or buys second-hand, and the software is free. The thing that has a price is
the capacity owners share with each other, and the platform that makes
sharing it safe. That is what the market prices.
