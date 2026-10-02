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
`losos-registrar`.

**The market sells what you already share, and nothing else.** It is a way to
be paid for the storage and compute a box contributes to the mesh, not a
separate product. A box can list storage only while its node is enrolled in the
mesh, and compute only while it is also sharing compute
(`losos.cluster.shareCompute`). Stop sharing and your listings leave the shelf
immediately (they return if you resume; orders already paid are unaffected).

## How the money moves

The edge is the Stripe **platform**. Money never touches a losos box.

1. A seller onboards: the edge creates a Stripe Express connected account for
   it, **tags it with the box's UUID** (metadata `losos_box_uuid`), and returns
   Stripe's hosted onboarding link. A box that was onboarded before this
   existed is tagged the next time its owner opens the page.
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

## Fulfilment

A paid order is a **30 day entitlement** (`expires_at`). Units return to the
listing when it lapses.

- **Storage.** The edge creates the namespace `market-<buyer id>` in the mesh
  cluster and a `PersistentVolumeClaim` named after the order (`ord-...`) of the
  purchased size, from `losos.edge.market.storageClass` (default `longhorn`).
  It runs after every reconcile pass and straight after a paid webhook, is
  idempotent (an `AlreadyExists` answer counts as done), and retries every
  reconcile interval if the apiserver refuses. The order counts as fulfilled
  only once the claim is `Bound`; a claim left `Pending` (no such storage
  class, or no capacity) is checked again on every pass. That assumes an
  `Immediate`-binding class, as Longhorn's default is: a `WaitForFirstConsumer`
  class never binds, because nothing mounts the claim yet. The account view
  reports the claim as `volume: "<namespace>/<claim>"`.
- **Compute.** A credit in vCPU-hours, listed under `entitlements`. Nothing
  meters or schedules against it yet.

Limits to know about, none of which are enforced yet:

- The claim is not pinned to the seller's node: Longhorn places replicas across
  the pool, so the seller is paid for contributing to it, not for hosting
  that particular volume.
- The buyer has no access path to the claim; it exists in the cluster for a
  workload to mount.
- Nothing is ever deleted. A lapsed entitlement stops being reported, but the
  volume holds the buyer's data and removing it is an operator decision.
- The registrar's ServiceAccount gains cluster-wide `get`/`create` on
  namespaces and PersistentVolumeClaims when the market is enabled. Kubernetes
  RBAC cannot scope either to `market-*` names.

## Admin UI

Owners reach the market from **Settings → Market** in the admin UI. The pane
shows what the owner has bought (with expiry and volume), the shelf to buy
from, and, for selling, Stripe payout setup, a listing form and the owner's
own listings and sales.

- The listing form only offers what is already shared: storage once the box
  has joined the mesh, compute once it also shares compute. Anything else is
  greyed with the reason. The edge enforces the same rule; the pane only
  explains it.
- Payment and Stripe onboarding open Stripe's own pages in a new tab. Nothing
  in the admin UI sees a card, and only `https://checkout.stripe.com` and
  `https://connect.stripe.com` pages are opened, checked by lososd and again
  by the page, so a compromised edge cannot send the owner to a look-alike
  card form. Custom Checkout domains are not supported.
- The pages cannot call the edge themselves (the admin UI's CSP is
  `connect-src 'self'`), so `lososd` relays: `GET /api/market` and
  `POST /api/market/{onboard,listings,listings/close,orders}`. The relay uses
  the registrar's URL, the appliance id and the proxy token that
  `losos.proxy.enable` already provides, requires https, and sends the token
  to `curl` on stdin rather than on the command line.
- Where the market is off (no proxy, the edge has it disabled, or this tenant
  is not opted in) `GET /api/market` answers `{"available": false}` with a 200
  and the pane says so. It is deliberately not a 404: the admin UI treats a 404
  as "this box does not serve the route" for the rest of the session.

## API

All routes live on the registrar's public API (`register.<publicDomain>`).
Authenticated routes take the same `appliance_id` and `token` as `/register`.
Bodies are JSON.

| Route                          | Auth       | Purpose                                         |
| ------------------------------ | ---------- | ----------------------------------------------- |
| `GET /market/listings`         | none       | What can be bought now. Names no seller         |
| `POST /market/account`         | token      | Your listings, purchases, sales, entitlements   |
| `POST /market/seller/onboard`  | token      | Start or resume Stripe onboarding               |
| `POST /market/listings`        | token      | `kind`, `unit_price`, `capacity`                |
| `POST /market/listings/close`  | token      | `listing_id`; paid orders keep their units      |
| `POST /market/orders`          | token      | `listing_id`, `quantity`; returns a Checkout URL |
| `POST /market/webhook`         | signature  | Stripe events                                   |

Every route answers 503 when the market is off. A tenant without the `market`
bit gets 403. Neither party learns the other's appliance id, and the public
listing view never shows one.

Capacity is reserved when the Checkout Session is created and held until
Stripe says how it ended: paid (`checkout.session.completed`) or abandoned
(`checkout.session.expired`, sent as the session's 31 minutes run out). It is
released at once if Stripe cannot be reached to create the session. If neither
event ever arrives, the hold lapses after Stripe's three-day retry window, so
a payment delayed by an edge outage still finds its units unsold. Two buyers racing for the last units cannot both get
a session.

## Operator setup

1. In the Stripe dashboard, enable **Connect** on the platform account.
2. Seal the secret key (a restricted key works) with `systemd-creds`, reading
   it from stdin so the plaintext never touches the disk:

   ```sh
   systemd-creds encrypt --name=stripe-secret-key - \
     /var/secrets/losos-stripe-secret-key.cred
   ```

   The blob's path is `losos.edge.market.stripeSecretKeySealed`, and only the
   gate unit ever decrypts it. The name must be exactly `stripe-secret-key`: a blob only decrypts under the name it
   was sealed with.
3. Add two webhook endpoints, both at
   `https://register.<publicDomain>/market/webhook`: one for events on your
   account (`checkout.session.completed`, `checkout.session.expired`) and
   one that listens to **events on Connected accounts** (`account.updated`).
   Stripe signs each with its own secret, so seal both `whsec_...` secrets,
   one per line, under the name `stripe-webhook-secret` into
   `losos.edge.market.webhookSecretSealed` (default
   `/var/secrets/losos-stripe-webhook-secret.cred`); the edge accepts either.
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
- **The registrar never holds the Stripe key.** A separate unit,
  `losos-stripe-gate` (`losos-registrar stripe-gate`), is the only process that
  has it. The registrar talks to the gate over a Unix socket
  (`/run/losos-stripe-gate/gate.sock`, 0600) and may ask only for: create an
  account, tag an account, check an account, make an onboarding link, start a
  Checkout Session, verify a webhook signature. The gate refuses a checkout
  whose destination is not an `acct_...` id, whose currency differs from the
  configured one, whose fee exceeds the 20% ceiling (or the whole amount),
  whose return URL is not a plain http(s) URL, or whose session lifetime is more than a day.
  It also refuses a non-`https` Stripe endpoint. There is no "forward this to
  Stripe" operation. A compromised registrar therefore cannot read the key, cannot
  send money anywhere but a connected account at the gate's fee limits, and
  cannot issue refunds or payouts; it can still ask for checkouts, because
  that is its job. Both units run as processes on the same machine, and root
  on the edge can still reach both. The registrar also has the sealed blobs and
  the gate's credential directory marked inaccessible, as a second layer.
- The Stripe secrets are never stored in plaintext on the edge. They live as
  `systemd-creds` blobs (TPM2 where the edge has one, otherwise the host key)
  handed to the gate with `LoadCredentialEncrypted=`, so they are plaintext only
  in that unit's private credential tmpfs and a copy of `/var` carries
  ciphertext only. Rotating a secret means sealing a new blob and restarting
  `losos-stripe-gate`. A missing blob skips the gate, which turns the market
  off (503) and nothing else; the gate is its own unit, so the master proxy in
  the registrar is never affected.
- **The Stripe account carries the box's UUID, not its recovery code.** The
  recovery code is a credential and stays on the box. What is sent is a
  one-way value derived from it (`SHA-256("losos-box-id-v1:" + code)`, first
  16 bytes, formatted as a UUID), which is stable for the life of the
  installation and reveals nothing from which the code could be recovered. The
  registrar checks it is a canonical UUID and the browser cannot choose it:
  lososd adds it itself.
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
- **Enforcing the entitlement.** Fulfilment creates the claim; it does not
  stop a buyer using more than they bought, meter compute, or revoke access at
  expiry.
- **Tax, invoicing and seller verification** beyond what Stripe's onboarding
  does. Running a marketplace has legal duties that depend on where you
  operate; this is an experiment, not advice.
