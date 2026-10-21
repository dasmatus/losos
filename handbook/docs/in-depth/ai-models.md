---
title: AI models
sidebar_position: 4.6
mdx:
  format: md
---

# AI models

Boxes on the mesh can lend memory to language models that are too large for
any one of them. The edge puts lending boxes together into a group until a
model fits, runs the model across the group, and sells its answers by the
token. The buyer pays as they go in USDC, from a balance held by the
[escrow](market.md#paying-with-usdc); every bill is split between the boxes
that did the work and the LosOS treasury.

The runtime is [llama.cpp](https://github.com/ggml-org/llama.cpp). Each
lending box runs its RPC server, which holds a share of the model's layers
and computes on them. The edge runs one llama.cpp server per group, which
reaches every box's share through that box's tunnel and answers the buyer.

## Lending memory

Lending is its own switch, `losos.inference.lend.enable`, with
`losos.inference.lend.memoryMiB` for how much memory the box lends, 1024 MiB
by default, and `losos.inference.lend.threads` for how many processor
threads it computes with. Without a thread count it takes half of the box's.
On the Market pane the switch is **Lend memory to AI models**, under
**Spare memory**, with the amount in GiB.

- The RPC server runs as the `shared` user, the same user as mesh storage,
  and never sees the owner's files.
- It listens on loopback only. A firewall rule lets only root, which runs
  the tunnel, reach its port; every other user on the box is refused.
- It cannot take more than the memory the owner set. The unit has a hard
  memory limit, and when the server grows past it, systemd stops the
  server, never LosOS Cloud.
- It runs at a low processor and disk weight, so the owner's own work comes
  first.
- The edge reaches it through a second service on the box's existing
  tunnel. A box that has no tunnel to the edge cannot lend.

The compute window does not apply: a box lends while the switch is on.

To be picked, a box also needs a payout address on the network the buyer
pays on, EVM or Solana, and its edge must let it sell.
A box reached through someone's own edge on the LAN cannot lend yet.

## Groups

For each model and payment network the edge keeps at most one group. It
picks lending boxes with the most memory first, up to 16, until their
memory together covers what the model needs. A box is in one group at a
time. Every buyer of the same model on the same network uses the same
group.

| State   | Meaning                                                           |
| ------- | ----------------------------------------------------------------- |
| Loading | The edge's server is starting and sending the model to the boxes. |
| Ready   | The model answers.                                                |
| Down    | A box dropped out. The group restarts once every box is back.     |

A group that restarts more than five times, or takes longer than 30 minutes
to load, is given up, and its boxes are left out of new groups for an hour.
A box missing for more than 15 minutes ends the group. A group nobody has
used for an hour is taken apart.

The first load sends the model's layers to every box. The RPC server keeps
them in its cache, so the next load of the same model is faster.

## Paying as you go

A buyer tops up a USDC balance in the escrow once, on one network, and can
then use any model on that network without another payment.

1. The buyer picks a model and how much they will spend at most per hour,
   between $0.01 and $100.
2. The edge answers with a quote: one **tap** per box in the group. A tap
   names one seller and the most the edge may bill it per hour, the buyer's
   hourly limit times that box's share of the memory. The quote is valid
   for 30 minutes.
3. The buyer's wallet opens every tap in one transaction.
4. The edge reads the taps back from the chain, checks that each one names
   the seller, rate and fee it quoted, and starts serving.
5. The edge bills each answer by its tokens, at the model's price for a
   million prompt tokens and a million generated tokens, and splits the
   bill between the taps by each box's share.

The edge counts the tokens itself. It renders and counts the prompt before
it sends it, and limits the answer to the number of tokens the request
asked for at most, or 256 when it names none. The boxes only compute layers
and report nothing, so no box can raise a bill. An answer that fails is not billed. One meter
answers one request at a time.

Before an answer the edge checks that the balance covers the most that
answer can cost, and that every tap still has room under its hourly rate.
A new tap starts with an hour of room, so the buyer is served at once.

### Settling

The edge settles each tap on the chain from time to time:

| Network | When                                          |
| ------- | --------------------------------------------- |
| Solana  | Every hour.                                   |
| EVM     | Once a day, or as soon as $1 is owed on a tap. |

A tap that is closed, a meter that ends and a balance the buyer asked to
withdraw settle at once. Each settle moves the amount out of the buyer's
balance, sends the 4% fee to the treasury and the rest to the box's
payout address.

### What the escrow enforces

The escrow does not trust the edge with more than the buyer allowed:

- A settle can only pay the seller the tap names.
- It can only bill up to the tap's rate for the time since the last
  settle, plus the hour a new tap starts with, and never more than seven
  days of it at once.
- It can only bill what the balance holds.
- After a tap is closed, it can only bill time up to the close.

So a stolen edge key costs a buyer at most their own hourly limit for the
time until they close their taps.

### Closing and withdrawing

The buyer can stop a meter on the Market pane or close any tap from their
wallet; the edge closes taps of a meter that ends. To take money out, the
buyer asks for a withdrawal, waits 48 hours, then withdraws. During those
48 hours the edge can still settle what was used. A withdrawal never needs
the edge, so a balance stays reachable even if the edge disappears.

The Market pane warns when the balance covers less than 24 hours at the
current hourly limit.

## What it cannot do

- **The boxes in a group see the work.** Each box computes on the prompt as
  it passes through the model's layers. Do not send anything a stranger's
  box must not see.
- **It is slow over home links.** Every token passes through every box in
  the group, so a model split across boxes on home connections answers far
  more slowly than the same model on one machine.
- **Large models load slowly.** The first load sends gigabytes to each box.
- **Groups are capped at 16 boxes**, so the largest model an edge can offer
  is bounded by what 16 boxes lend.
- **Cards are not offered** for models. Storage, compute and machines are
  still sold by the paid period, as the [market](market.md) describes.
