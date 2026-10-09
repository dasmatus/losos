---
title: Widget builder
sidebar_position: 4.7
mdx:
  format: md
---

# Widget builder

An owner describes a widget in plain words and Claude writes it. The
widget editor (**Add a widget**, then **Write one**) has a **Build with
Claude** tab next to **Write**. The widget's files land in the editor,
where the owner can read them, try them in the preview and change them.
Nothing is saved until the owner presses Save, and a widget Claude wrote
runs in the same sandboxed frame as one written by hand (see
[Administration](administration.md#widgets-written-by-hand)).

The description and the files sent for a change go to Anthropic and are
subject to Anthropic's [Usage Policy](https://www.anthropic.com/legal/aup) and
[Commercial Terms](https://www.anthropic.com/legal/commercial-terms). The tab says so under the Build button.

The builder is a feature of the edge. It is off by default, at three
levels.

|                         | Default | Switch                                   |
| ----------------------- | ------- | ---------------------------------------- |
| The edge serves it      | off     | `losos.edge.builder.enable`              |
| A box may use it        | off     | `losos.edge.tenants.<id>.market`         |
| A build can start       | no      | the box's balance covers one             |

A box asks only an official edge, like the market, because the request
carries the box's proxy token.

## How a build runs

The edge runs a [Claude Managed Agent](https://platform.claude.com/docs/en/managed-agents/overview):
an agent with a fixed system prompt and the widget contract, made once by
the operator, and a cloud container per build that Anthropic hosts. For each
build the edge opens a session with the owner's description, the page's
language and, when the owner asks for a change, the widget's current files.
The agent writes `index.html`, any files it links (a stylesheet, a script,
data) and a short `summary.txt`. When the session goes idle the edge
downloads them, deletes the session and its files, and hands the widget's
files to the box. Files with names a widget cannot have are left out, and a
widget over 12 files or 128 KiB is refused like one typed by hand.

The agent has file and shell tools in its own container and no web access.
Its container cannot reach the box. Builds that run longer than 15 minutes
are stopped.

## What it costs

The owner pays Anthropic's list price per million input and output tokens
of the model (Claude Opus 5.5, $4 in and $20 out), plus a markup the
operator sets (`markupBps`, default 2000, which is 20%). Cache reads and
writes are charged at their own list prices with the same markup. Container
time is not passed on. The edge converts dollars to the market's currency
with `usdRate`.

Payment is prepaid. A box tops up its balance with one of the edge's credit
packs (`packs`, default 5, 10 and 20 in the market's currency) through
Stripe Checkout on the edge's own Stripe account, the one the market uses.
The balance moves when Stripe's signed `checkout.session.completed` webhook
reaches the edge.

Each build runs under a hard budget on the session: what the balance
covers at list price, capped by `maxBuildCents` (default $3). Anthropic
stops the agent when the budget is reached, and the owner sees that the
widget may be unfinished. The charge is what the session used, rounded up
to a cent, also when the agent ends without a usable widget. A build whose
session the edge can no longer read is not charged.

## Operator setup

1. Create an API key in the Claude Console, in a workspace of its own with a
   spend limit. That limit is the last word on what a bug can cost.
2. Seal the key with `systemd-creds`, reading it from stdin so the plaintext
   never touches the disk:

   ```sh
   systemd-creds encrypt --name=claude-api-key - /var/secrets/losos-claude-api-key.cred
   ```

   The path is `losos.edge.builder.claudeKeySealed`. The name must be
   exactly `claude-api-key`.
3. Make the agent and its environment once:

   ```sh
   systemd-run --pipe --wait \
     -p LoadCredentialEncrypted=claude-api-key:/var/secrets/losos-claude-api-key.cred \
     sh -c 'losos-registrar builder-setup --key-file "$CREDENTIALS_DIRECTORY/claude-api-key"'
   ```

   It prints the two lines to put in the edge's configuration:
   `losos.edge.builder.agentId` and `losos.edge.builder.environmentId`. Run
   it again with `--agent-id` and `--environment-id` after an upgrade to
   update the agent in place.
4. Turn on the market (`losos.edge.market.enable`, see [Market](market.md#operator-setup)),
   whose Stripe gate takes the top-ups, then set
   `losos.edge.builder.enable = true`. The registrar unit does not start
   without the sealed key, so seal it first.
5. Set `losos.edge.tenants.<id>.market = true` for each box allowed to use
   it.

Start in Stripe **test mode**. The edge's tests run against stand-ins for
Stripe and Anthropic, which check what the edge sends and how it reacts but
cannot say whether either service accepts it.

## Options

| Option                               | Default                                  |
| ------------------------------------ | ---------------------------------------- |
| `losos.edge.builder.enable`          | `false`                                  |
| `losos.edge.builder.claudeKeySealed` | `/var/secrets/losos-claude-api-key.cred` |
| `losos.edge.builder.agentId`         | none, required                           |
| `losos.edge.builder.environmentId`   | none, required                           |
| `losos.edge.builder.markupBps`       | `2000` (20%)                             |
| `losos.edge.builder.usdRate`         | `"1.0"`                                  |
| `losos.edge.builder.packs`           | `[ 500 1000 2000 ]`                      |
| `losos.edge.builder.maxBuildCents`   | `300`                                    |

## Safety properties

- The Anthropic key is held by the edge's registrar unit only, as a
  systemd credential read per request. Boxes and browsers never see it.
- The Stripe key stays in the market's gate. The gate accepts a credit
  checkout only for an amount among the operator's packs, in the edge's
  currency, with no destination account and no fee.
- The balance moves only on a signed webhook that matches the recorded
  order's id, session, amount and currency, and each order is credited once.
- A box runs one build at a time, and the edge runs at most eight.
- The widget is text. The box puts it in the editor and nothing runs it
  outside the sandboxed widget frame.

## API

The box reaches the builder through lososd, which adds its proxy token:

| Box route                           | Edge route              |
| ----------------------------------- | ----------------------- |
| `GET /api/builder`                  | `POST /builder/account` |
| `POST /api/builder/credits`         | `POST /builder/credits` |
| `POST /api/builder/builds`          | `POST /builder/builds`  |
| `GET /api/builder/builds/{id}`      | `POST /builder/build`   |

The shapes are in `backend/schema.json`.
