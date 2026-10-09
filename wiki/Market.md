**English** · [Slovenčina](Market-sk) · [Deutsch](Market-de)

# Market (planned)

An optional marketplace where one box sells spare storage or compute and
another buys it. Payment goes through **Stripe Connect**, and the edge keeps a
4% cut to cover its running costs.

**It is not open yet.** The code is finished end to end: the registrar's
`/market/*` routes, the Stripe gate, lososd's relay and the admin UI pane.
A Stripe Connect platform needs a registered business behind it, though, and
there is none yet. Until there is, the admin UI shows the **Market** tab
greyed out with a "soon(TM)" badge. Nobody can click it or reach it by
address, and nothing on a box asks the market anything. The disk-sharing
switch (`losos.sharingMyStorage`) lives on this pane, so sharing the disk with
the mesh opens with the market, and the UI cannot switch it on before then.
The rest of this page describes how the market works once it opens. It is
also off by default, at three levels.

|                       | Default | Switch                                        |
| --------------------- | ------- | --------------------------------------------- |
| The edge serves it    | off     | `losos.edge.market.enable`                    |
| A box may trade       | off     | `losos.edge.tenants.<id>.market`              |
| A seller may list     | closed  | Stripe reports the connected account ready    |

A box that never trades is unaffected. The market is a feature of the edge's
`losos-registrar`.

**The market sells what you already share, and nothing else.** It pays a
box for the storage and compute it contributes to the mesh. It is not a
separate product. A box can list storage only while its node is enrolled in
the mesh, and compute only while it also shares compute
(`losos.cluster.shareCompute`). Stop sharing and your listings leave the shelf
at once. They return if you resume, and orders already paid are unaffected.

## How the money moves

The edge is the Stripe **platform**. Money never touches a losos box.

1. A seller onboards. The edge creates a Stripe Express connected account
   for it, **tags it with the box's UUID** (metadata `losos_box_uuid`), and
   returns Stripe's hosted onboarding link. A box onboarded before tagging
   existed gets tagged the next time its owner opens the page.
2. Stripe tells the edge (`account.updated`) when the account can receive
   transfers. Only then can the seller list anything.
3. A buyer orders units of a listing. The edge reserves them and creates a
   Stripe Checkout Session as a **destination charge**. The buyer pays the
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

A paid order is a **30 day entitlement** (`expires_at`). Compute units return
to the listing when it lapses. Storage units return only once the claim is
gone, because the claim still holds Longhorn capacity after the month ends.

- **Storage.** The edge creates the namespace `market-<buyer id>` in the
  mesh cluster and a `PersistentVolumeClaim` of the purchased size, named
  after the order (`ord-...`), from `losos.edge.market.storageClass` (default
  `longhorn`). This runs after every reconcile pass and straight after a paid
  webhook. It is idempotent, since an `AlreadyExists` answer counts as done,
  and it retries every reconcile interval if the apiserver refuses. The order
  counts as fulfilled only once the claim is `Bound`. A claim left `Pending`
  (no such storage class, or no capacity) is checked again on every pass.
  That assumes an `Immediate`-binding class, which Longhorn's default is. A
  `WaitForFirstConsumer` class never binds, because nothing mounts the claim
  yet. The account view reports a bound claim as
  `volume: "<namespace>/<claim>"`. The edge writes the claim's name down
  before it asks for it, so a claim that never binds, or one created just
  before the edge stopped, still keeps its units sold after the month ends.
- **Compute.** A credit in vCPU-hours, listed under `entitlements`. Nothing
  meters or schedules against it yet.

Limits to know about, none of which are enforced yet:

- The claim is not pinned to the seller's node. Longhorn places replicas
  across the pool, so the seller is paid for contributing to it, not for
  hosting that particular volume.
- The buyer has no access path to the claim; it exists in the cluster for a
  workload to mount.
- Nothing is ever deleted. A lapsed entitlement stops being reported, but
  the volume holds the buyer's data, and removing it is the operator's call.
  Until the operator deletes the claim, its GiB stay sold, so the same
  capacity is never sold twice. That holds for a claim still `Pending` too,
  since it may yet bind. The reconcile pass checks each lapsed claim and puts
  its units back on sale once the apiserver answers 404 for it.
- The registrar's ServiceAccount gains cluster-wide `get`/`create` on
  namespaces and PersistentVolumeClaims when the market is enabled. Kubernetes
  RBAC cannot scope either to `market-*` names.

## Admin UI

Today the **Settings, Market** row is greyed out and labelled "soon(TM)". It
is a disabled button that the keyboard skips, and `/settings/market` opens
the default pane instead. Opening it takes one flag, `planned` on the row in
`admin-ui/app/src/screens/settings/panes.ts`, plus the matching switch in
`admin-ui/app/tests/app.browser.mjs`, which holds the pane's browser checks
until then.

Once open, the pane starts with the disk-sharing switch. It moved here from
Storage because lending disk to other boxes and being paid for it are one
decision. Below it come what the owner has bought, with expiry and volume,
and the shelf to buy from. For selling there is Stripe payout setup, a
listing form, and the owner's own listings and sales. The switch shows
whether or not the edge offers the market to this box, because it is a
setting of the box, not of the edge.

- The listing form offers only what is already shared: storage once the box
  has joined the mesh, compute once it also shares compute. Anything else is
  greyed with the reason. The edge enforces the same rule, and the pane only
  explains it.
- Payment and Stripe onboarding open Stripe's own pages in a new tab.
  Nothing in the admin UI sees a card. lososd, and then the page again, check
  that the link is a `https://checkout.stripe.com` or
  `https://connect.stripe.com` page, so a compromised edge cannot send the
  owner to a look-alike card form. Custom Checkout domains are not
  supported.
- The pages cannot call the edge themselves, since the admin UI's CSP is
  `connect-src 'self'`. `lososd` relays instead: `GET /api/market` and
  `POST /api/market/{onboard,listings,listings/close,orders}`. The relay uses
  the registrar's URL, the appliance id and the proxy token that
  `losos.proxy.enable` already provides. It requires https and sends the
  token to `curl` on stdin rather than on the command line.
- Where the market is off (no proxy, the edge has it disabled, or this
  tenant is not opted in), `GET /api/market` answers `{"available": false}`
  with a 200 and the pane says so. It is not a 404 on purpose. The admin UI
  treats a 404 as "this box does not serve the route" for the rest of the
  session.

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

Every route answers 503 when the market is off. On the token routes, a
tenant without the `market` bit gets 403. The public listing view and the
webhook belong to no tenant, so they never do. `POST /market/account` returns the 100
most recent purchases and sales on each side, live ones first. A closed
listing nobody ordered from is deleted; one with orders stays as long as they
do. Neither party learns the other's appliance id, and the public
listing view never shows one.

The edge reserves capacity when it creates the Checkout Session and holds it
until Stripe says how the session ended: paid (`checkout.session.completed`)
or abandoned (`checkout.session.expired`, sent as the session's 31 minutes
run out). An `expired` event counts only if it names the session the order
recorded. If Stripe cannot be reached to create the session, the edge
releases the capacity at once, unless a payment for it has already landed.
If neither event ever arrives, the hold lapses after Stripe's three-day retry
window, so a payment delayed by an edge outage still finds its units unsold.
The next reconcile pass then records the order as `expired`. A payment that
arrives even later is honoured if the units are still free. Otherwise the
edge logs it with its session id so the operator can refund it. Two buyers
racing for the last units cannot both get a session.

## Operator setup

1. In the Stripe dashboard, enable **Connect** on the platform account.
2. Seal the secret key (a restricted key works) with `systemd-creds`, reading
   it from stdin so the plaintext never touches the disk:

   ```sh
   systemd-creds encrypt --name=stripe-secret-key - \
     /var/secrets/losos-stripe-secret-key.cred
   ```

   The blob's path is `losos.edge.market.stripeSecretKeySealed`, and only
   the gate unit ever decrypts it. The name must be exactly
   `stripe-secret-key`, because a blob decrypts only under the name it was
   sealed with.
3. Add two webhook endpoints, both at
   `https://register.<publicDomain>/market/webhook`: one for events on your
   account (`checkout.session.completed`, `checkout.session.expired`) and
   one that listens to **events on Connected accounts** (`account.updated`).
   Stripe signs each with its own secret, so seal both `whsec_...` secrets,
   one per line, under the name `stripe-webhook-secret` into
   `losos.edge.market.webhookSecretSealed` (default
   `/var/secrets/losos-stripe-webhook-secret.cred`). The edge accepts either.
4. Set `losos.edge.market.enable = true` and `losos.edge.market.returnUrl`,
   an absolute http(s) URL with no credentials and no `#fragment` (the order
   and status are appended as query parameters).
5. Set `losos.edge.tenants.<id>.market = true` for each box allowed to trade.

Start in Stripe **test mode**. The edge has only been tested against a
stand-in for Stripe. That checks what the edge sends and how it reacts, but
cannot say whether Stripe accepts it. The first test-mode run settles that.

Steps 2 and 3 can also come from GitHub Actions secrets instead of a shell on
the edge; see [Keys from GitHub Actions](#keys-from-github-actions).

### Test mode and live mode

The edge reads the mode off the key. `sk_test_` and `rk_test_` keys run the
market in test mode, `sk_live_` and `rk_live_` keys in live mode, and there is
no separate setting that could disagree with the key. The gate logs the mode
when it starts and refuses to start on a key whose mode it cannot read. Every
market answer (`/market/listings`, `/market/account`, onboarding and
checkout) carries `"mode": "test"` or `"mode": "live"`.

Each mode keeps its own ledger: `market.json` for live and `market-test.json`
beside it for test. Going live is sealing a live key and restarting the gate.
The test sellers, listings and orders stay in the test ledger, the live shelf
starts empty, and each seller onboards again with a live Stripe account. A
test key brings the test ledger back as it was. The edge reads a `market.json`
written before it kept the two apart as the live ledger. Volumes claimed by
test orders stay on the mesh until the operator removes them.

Webhook secrets carry no mode, but every Stripe event does (`livemode`). A
signed event from the other mode means the sealed webhook secret and key come
from different modes, so the gate refuses it and logs which is which. Stripe
retries a refused event for three days, so sealing the matching pair within
that time loses nothing.

### Keys from GitHub Actions

The `edge-credentials` workflow (`.github/workflows/edge-credentials.yml`)
seals the keys from the repository's Actions secrets, so no one pastes them
on the edge.

1. Make an SSH key for the workflow
   (`ssh-keygen -t ed25519 -N "" -f edge-credentials`) and set
   `losos.edge.credentials.deployKey` to its public half. The edge lets that
   key in as root only to run `losos-seal-credential`: no shell, no
   forwarding, no terminal.
2. Add the Actions secrets:

   | Secret                  | Holds                                                   |
   | ----------------------- | ------------------------------------------------------- |
   | `STRIPE_KEY`            | the Stripe secret or restricted key, test or live       |
   | `STRIPE_WEBHOOK_SECRET` | optional: both `whsec_` secrets, one per line           |
   | `CLAUDE_KEY`            | the Claude API key (`sk-ant-...`)                       |
   | `EDGE_SSH_KEY`          | the private half of the key from step 1                 |
   | `EDGE_SSH_HOST`         | the edge's address (a secret or a variable)             |
   | `EDGE_SSH_KNOWN_HOSTS`  | the edge's line from `ssh-keyscan` (secret or variable) |
   | `EDGE_SSH_PORT`         | optional, 22 when unset                                 |

3. Run **edge-credentials** on `main` from the Actions tab. It prints whether
   the Stripe key is a test or a live key, never the key itself, and pipes
   each secret that is set into `losos-seal-credential`. That checks the
   secret's shape, seals it with `systemd-creds` under its credential name
   and restarts the gate. Rotating a key is changing the secret and running
   the workflow again.

The Claude key is sealed as `claude-key` to
`/var/secrets/losos-claude-key.cred`
(`losos.edge.credentials.secrets.claude-key.sealed`). The registrar reads it
with `LoadCredentialEncrypted=claude-key:<that path>`, and sealing a new key
restarts `losos-registrar.service`.

## Safety properties

- The webhook is authenticated by Stripe's HMAC signature over the raw body,
  with a five minute replay window and a larger body cap than other routes.
- A payment only counts when the session id, amount and currency equal what
  the edge recorded for the order. A mismatch is logged and not fulfilled. If
  the edge stopped before it wrote the session id down, the signed event's id
  is adopted, since only this edge's gate can have named the order in it.
- **The registrar never holds the Stripe key.** A separate unit,
  `losos-stripe-gate` (`losos-registrar stripe-gate`), is the only process
  that has it. The registrar talks to the gate over a Unix socket
  (`/run/losos-stripe-gate/gate.sock`, 0600) and may ask only to create an
  account, tag an account, check an account, make an onboarding link, start a
  Checkout Session, or verify a webhook signature. The gate refuses a
  checkout whose destination is not an `acct_...` id, whose currency differs
  from the configured one, whose fee exceeds the 20% ceiling or the whole
  amount, whose return URL is not a plain http(s) URL, or whose session
  lifetime is more than a day. It also refuses a non-`https` Stripe endpoint.
  There is no "forward this to Stripe" operation. A compromised registrar
  therefore cannot read the key, cannot send money anywhere but a connected
  account within the gate's fee limits, and cannot issue refunds or payouts.
  It can still ask for checkouts, because that is its job. Both units run as
  processes on the same machine, and root on the edge can still reach both.
  As a second layer, the registrar's unit marks the sealed blobs and the
  gate's credential directory inaccessible.
- The edge never stores the Stripe secrets in plaintext. They live as
  `systemd-creds` blobs, sealed with TPM2 where the edge has one and the host
  key otherwise, and reach the gate through `LoadCredentialEncrypted=`. They
  are plaintext only in that unit's private credential tmpfs, and a copy of
  `/var` carries ciphertext only. Rotating a secret means sealing a new blob
  and restarting `losos-stripe-gate`. A missing blob skips the gate, which
  turns the market off (503) and nothing else. The gate is its own unit, so
  the master proxy in the registrar never notices.
- **The Stripe account carries the box's UUID, not its recovery code.** The
  recovery code is a credential and stays on the box. The box sends a one-way
  value derived from it, the first 16 bytes of
  `SHA-256("losos-box-id-v1:" + code)` formatted as a UUID. It is stable for
  the life of the installation and reveals nothing the code could be
  recovered from. The registrar checks that it is a canonical UUID. The
  browser cannot choose it, because lososd adds it itself.
- Secret files are shape-checked (`sk_`/`rk_`, `whsec_`) before use.
- `market.json` is 0600 and written atomically. A file that does not parse
  stops the edge from starting. Treating it as empty would forget who has
  paid for what.
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
  operate. This is an experiment, not advice.
