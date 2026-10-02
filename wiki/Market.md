# Market (experimental)

An optional marketplace where one box sells spare storage or compute and
another buys it. Payment goes through **Stripe Connect**; the edge keeps a 4%
cut to cover its running costs. It is off by default, at three levels.

|                       | Default | Switch                                        |
| --------------------- | ------- | --------------------------------------------- |
| The edge serves it    | off     | `losos.edge.market.enable`                    |
| A box may trade       | off     | `losos.edge.tenants.<id>.market`              |
| A seller may list     | closed  | Stripe reports the connected account ready    |

A box that never trades is unaffected. The market is a feature of the edge's
`losos-registrar`; nothing on the appliance changes.

## How the money moves

The edge is the Stripe **platform**. Money never touches a losos box.

1. A seller onboards: the edge creates a Stripe Express connected account for
   it and returns Stripe's hosted onboarding link.
2. Stripe tells the edge (`account.updated`) when the account can receive
   transfers. Only then can the seller list anything.
3. A buyer orders units of a listing. The edge reserves them and creates a
   Stripe Checkout Session as a **destination charge**: the buyer pays the
   platform, `transfer_data[destination]` forwards the sale to the seller's
   account, and `application_fee_amount` stays with the platform.
4. Stripe tells the edge (`checkout.session.completed`) that the payment
   cleared. The order becomes `paid`.

The fee is `feeBps` basis points of the gross amount, rounded half up. At the
default 400, a 20.00 EUR sale pays the platform 0.80 and the seller 19.20. The
configured value is capped at 2000 (20%).

## What is for sale

| Kind      | Unit        | Meaning                                              |
| --------- | ----------- | ---------------------------------------------------- |
| `storage` | GiB-month   | Capacity in the mesh's Longhorn pool                 |
| `compute` | vCPU-hour   | Scheduling on the seller's node, inside its window   |

Prices are in the minor unit (cents) of one currency per edge
(`losos.edge.market.currency`). A single order must total at least 50 minor
units, which is Stripe's floor.

**A paid order is the entitlement. It does not provision anything yet.** The
order record is the proof of purchase; turning it into a Longhorn volume or a
scheduled workload is a separate step, kept apart so the payment path can be
run and audited before anything is handed out against it.

## API

All routes live on the registrar's public API (`register.<publicDomain>`).
Authenticated routes take the same `appliance_id` and `token` as `/register`.
Bodies are JSON.

| Route                          | Auth       | Purpose                                         |
| ------------------------------ | ---------- | ----------------------------------------------- |
| `GET /market/listings`         | none       | What can be bought now. Names no seller         |
| `POST /market/account`         | token      | Your listings, purchases and sales              |
| `POST /market/seller/onboard`  | token      | Start or resume Stripe onboarding               |
| `POST /market/listings`        | token      | `kind`, `unit_price`, `capacity`                |
| `POST /market/listings/close`  | token      | `listing_id`; paid orders keep their units      |
| `POST /market/orders`          | token      | `listing_id`, `quantity`; returns a Checkout URL |
| `POST /market/webhook`         | signature  | Stripe events                                   |

Every route answers 503 when the market is off. A tenant without the `market`
bit gets 403. Neither party learns the other's appliance id, and the public
listing view never shows one.

Capacity is reserved when the Checkout Session is created, held for the
session's 30 minutes plus a 5 minute grace, and released if it expires or
Stripe cannot be reached. Two buyers racing for the last units cannot both get
a session.

## Operator setup

1. In the Stripe dashboard, enable **Connect** on the platform account.
2. Place the secret key, 0600, at `losos.edge.market.stripeSecretKeyFile`
   (default `/var/secrets/losos-stripe-secret-key`). A restricted key works.
3. Add two webhook endpoints, both at
   `https://register.<publicDomain>/market/webhook`: one for events on your
   account (`checkout.session.completed`,
   `checkout.session.async_payment_succeeded`, `checkout.session.expired`) and
   one that listens to **events on Connected accounts** (`account.updated`).
   Stripe signs each with its own secret, so put both `whsec_...` secrets in
   `losos.edge.market.webhookSecretFile`, one per line; the edge accepts either.
4. Set `losos.edge.market.enable = true` and `losos.edge.market.returnUrl`.
5. Set `losos.edge.tenants.<id>.market = true` for each box allowed to trade.

Start in Stripe **test mode**. The edge has been tested against a stand-in for
Stripe, which checks what it sends and how it reacts but cannot say whether
Stripe accepts it; the first test-mode run settles that.

## Safety properties

- The webhook is authenticated by Stripe's HMAC signature over the raw body,
  with a five minute replay window and a larger body cap than other routes.
- A payment only counts when the session id, amount and currency equal what
  the edge recorded for the order. A mismatch is logged and not fulfilled.
- Secret files are shape-checked (`sk_`/`rk_`, `whsec_`) before use.
- `market.json` is 0600 and written atomically. A file that does not parse
  stops the edge from starting rather than being treated as empty, because
  empty would forget who has paid for what.
- Stripe's error text and endpoints stay in the journal; callers get a fixed
  message.

## Not covered

- **Refunds and disputes.** Handle them in the Stripe dashboard, using
  `reverse_transfer` and `refund_application_fee` so the seller's share and the
  platform's cut are both returned. The market does not model either.
- **Fulfilment.** See above.
- **Tax, invoicing and seller verification** beyond what Stripe's onboarding
  does. Running a marketplace has legal duties that depend on where you
  operate; this is an experiment, not advice.
- **An admin UI.** The routes are there; the SPA does not use them yet.
