# losos-escrow

The Solana twin of `LososEscrow`: one USDC payment held per deposit for as
long as what it bought runs. The edge then releases it to the seller and the
treasury recorded at payment, refunds it to the buyer, or splits it between
the two when the seller stopped delivering partway. Each of those needs two
of three keys, the edge's, the buyer's box's and the seller's box's, so the
edge's key alone moves nothing, and if no two agree, the buyer takes the
deposit back after its deadline. It also keeps prepaid balances that the edge
draws on as the buyer uses something, through rate-capped taps. Written with [pinocchio](https://github.com/anza-xyz/pinocchio),
no Anchor, about 65 KB built.

## Accounts

| Account | Address | Holds |
| ------- | ------- | ----- |
| Config  | `["config", treasury, mint]` | the mint, the treasury, the releaser |
| Deposit | `["deposit", config, nonce]` | payer, seller, order, gross, fee, deadline, paid at, buyer's box, seller's box |
| Vault   | `["vault", deposit]` | the USDC, in a token account the deposit owns |
| Balance | `["balance", config, buyer]` | config, buyer, asked amount, not before |
| Vault   | `["vault", balance]` | the buyer's prepaid USDC, in a token account the balance owns |
| Tap     | `["tap", config, tap id]` | config, buyer, seller, tap id, rate, fee bps, settled at, closed at |

The nonce is 32 random bytes the wallet picks for each payment. The order is
the keccak-256 of the edge's order id, the invoice's `order_id_hash`. The
tap id is 32 bytes the edge picks for each tap and hands to the wallet; the
program only uses it as a seed. Every integer is little-endian;
`src/state.rs` has the byte layouts.

## Instructions

| Tag | Call | Signer | What it does |
| --- | ---- | ------ | ------------ |
| 0 | `initialize(releaser)` | treasury | Makes the config, once per treasury and mint. |
| 1 | `pay(nonce, order, gross, fee, deadline, buyer_box, seller_box)` | payer | Opens the deposit and its vault and moves the gross in. |
| 2 | `release` | two of the three keys | Fee to the treasury, the rest to the seller; closes both accounts. |
| 3 | `refund` | two of the three keys, or the payer after the deadline | Everything back to the payer; closes both. |
| 4 | `settle(back)` | two of the three keys | `back` to the payer, the rest released with the fee scaled down. |
| 5 | `set_releaser(releaser)` | treasury | Names another releaser. |
| 6 | `top_up(amount)` | buyer | Opens the buyer's balance and its vault on first use, moves `amount` in. |
| 7 | `open_tap(tap_id, rate, fee_bps)` | buyer | Opens one tap against the buyer's balance, billable from an hour before now; several fit in one transaction. |
| 8 | `close_tap` | buyer, or releaser | Stops the tap's billing at now, once. |
| 9 | `settle_tap(amount)` | releaser | Bills `amount` from the balance: the fee to the treasury, the rest to the tap's seller. |
| 10 | `ask_withdrawal(amount)` | buyer | Asks to take `amount` out after 48 hours, replacing any earlier ask. |
| 11 | `withdraw` | buyer | After the notice, pays the smaller of the ask and the balance to the buyer. |

Each instruction is its tag byte, then the data below. Signers are marked
(s), writable accounts (w).

| Tag | Data | Accounts, in order |
| --- | ---- | ------------------ |
| 0 | `releaser: 32` | treasury (s, w), config (w), mint, system program |
| 1 | `nonce: 32, order: 32, gross: u64, fee: u64, deadline: i64, buyer_box: 32, seller_box: 32` | payer (s, w), payer's token account (w), config, deposit (w), vault (w), seller, mint, token program, system program |
| 2 | none | authority (s), config, deposit (w), vault (w), seller's token account (w), treasury's token account (w), payer (w), mint, token program, instructions sysvar |
| 3 | none | authority or payer (s), config, deposit (w), vault (w), payer's token account (w), payer (w), mint, token program, instructions sysvar |
| 4 | `refund: u64` | authority (s), config, deposit (w), vault (w), payer's token account (w), seller's token account (w), treasury's token account (w), payer (w), mint, token program, instructions sysvar |
| 5 | `next: 32` | treasury (s), config (w) |
| 6 | `amount: u64` | buyer (s, w), buyer's token account (w), config, balance (w), vault (w), mint, token program, system program |
| 7 | `tap_id: 32, rate: u64, fee_bps: u16` | buyer (s, w), config, balance, tap (w), seller, system program |
| 8 | none | buyer or releaser (s), config, tap (w), buyer (w) |
| 9 | `amount: u64` | releaser (s), config, tap (w), balance, vault (w), seller's token account (w), treasury's token account (w), buyer (w), mint, token program |
| 10 | `amount: u64` | buyer (s), config, balance (w) |
| 11 | none | buyer (s), config, balance (w), vault (w), buyer's token account (w), mint, token program |

For tags 2 to 4 the authority, one of the three keys, signs the transaction,
and the second key agrees through an Ed25519 program instruction earlier in
the same transaction, which the program reads back through the instructions
sysvar (`Sysvar1nstructions1111111111111111111111111`). That instruction must
carry one signature with its key, signature and message all in its own data,
over the 64 bytes `"losos-escrow decision v1"`, the deposit's address and the
refund as a little-endian u64: 0 for a release, the gross for a refund, `back`
for a split. The edge signs as the authority, and the box the move pays
supplies the Ed25519 check: the seller's for a release, the buyer's for a
refund or a split. `pay` refuses box keys that are zero, equal, or the
releaser.

The rules are the EVM contract's: a fee of at most half the gross, a deadline
from one hour to 35 days after payment, a seller that is not the treasury,
the config, the deposit or the vault. Only the classic SPL token program and
the config's mint are taken. A release or split pays only token accounts of
the seller and treasury the deposit names, and a refund only the payer's.
Closing a deposit gives its rent back to the payer, and any tokens sent to a
vault by plain transfer go to the treasury on a release or split and to the
payer on a refund, so nothing can keep a deposit open.

## Balances and taps

A tap is the buyer's consent for the releaser to bill one seller, at up to
`rate` base units of USDC an hour, with `fee_bps` of each bill going to the
treasury. A new tap starts one hour into its billing (`HEADSTART`):
`settled_at` is set to an hour before it opened, so the first hour of use
can be billed as soon as the tap is open. A `settle_tap` of `amount` is
taken only when:

- `amount` is at most `rate × min(end − settled_at, 7 days) / 3600`,
  rounded down, where `end` is the time the tap closed, or now while it is
  open;
- `end` is later than `settled_at`, so the same second is never billed twice;
- `amount` is not more than the balance holds, and not 0 on an open tap.

The settle then moves `settled_at` to `end`, so cap that went unused is not
carried over, and a settle that comes more than a week after the last one
bills at most a week of the time between. The fee is `amount × fee_bps / 10000`, rounded down,
and `fee_bps` is at most 5000. The seller may not be the treasury, the
config, the balance, its vault or the tap.

The buyer or the releaser closes a tap at any time. After that the releaser
can bill it only up to the moment it closed. Once a closed tap is settled up
to its close, its account is closed and its rent goes back to the buyer. A
tap closed in the same second as its last settle has nothing left to bill
and is closed at once. A fresh tap closed at once still has its hour of
headstart to bill; a settle of 0 on a closed tap finishes one that used
nothing.

A withdrawal is asked for, and paid 48 hours later, to the buyer's own token
account: the smaller of the amount asked and what the balance still holds.
A new ask replaces the old one and waits again. The releaser can keep
settling during the notice, which is how it bills the last of what was used.
A withdrawal never needs the edge, so an edge that stops answering costs the
buyer only the notice.

A stolen edge key can therefore bill only at the rates the buyer agreed to,
and only to the sellers the buyer named. Over a tap's whole life it can take
at most the rate times the time since the tap opened plus one hour, and never
more than a week of it in one settle. Lamports sent ahead to a balance, vault or tap address do not block
it: the program tops the account up to its rent and takes it over. Tokens
sent to a balance's vault by plain transfer count as the buyer's balance.

The program keeps no id of its own, so the same build serves any address it
is deployed to.

## Build and test

Anza's tool suite builds it for the Solana VM (`cargo-build-sbf`, v4.0.3 in
CI). The tests load the built program into
[LiteSVM](https://github.com/LiteSVM/litesvm):

```sh
cargo build-sbf
cargo test
```

`edge/registrar/tests/solana_escrow.rs` then runs a real edge against
`solana-test-validator` with the program loaded:

```sh
LOSOS_ESCROW_SO=$PWD/target/deploy/losos_escrow.so \
LOSOS_SOLANA_TEST_VALIDATOR=$(command -v solana-test-validator) \
  cargo test --manifest-path ../../registrar/Cargo.toml --test solana_escrow
```

`devenv`'s `test-solana-escrow` runs all three, as CI's `solana-escrow` job
does.

## Deploy

1. Build, deploy and compare what is on chain with what you built:

   ```sh
   cargo build-sbf
   solana program deploy target/deploy/losos_escrow.so --program-id escrow-keypair.json
   solana program dump <PROGRAM> on-chain.so
   cmp target/deploy/losos_escrow.so on-chain.so
   ```

   `solana program dump` pads the program to its account size; compare the
   first `stat -c %s target/deploy/losos_escrow.so` bytes when it does.

2. Remove the upgrade authority, so that nobody can ever replace the code.
   A live edge counts no payment into a program that still has one:

   ```sh
   solana program set-upgrade-authority <PROGRAM> --final
   ```

3. Make the treasury's config. `losos edge escrow releaser` on the edge shows
   the address the edge signs with on Solana. The treasury signs, pays the
   fee and the config's rent (about 0.0016 SOL), and should be a key kept
   off any server:

   ```sh
   RPC=https://api.mainnet-beta.solana.com
   MINT=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v   # Circle's USDC
   rpc() { curl -s "$RPC" -H 'content-type: application/json' -d "$1"; }
   HASH=$(rpc '{"jsonrpc":"2.0","id":1,"method":"getLatestBlockhash","params":[{"commitment":"confirmed"}]}' \
     | jq -r .result.value.blockhash)
   TX=$(cargo run -q --example config -- initialize <PROGRAM> treasury.json "$MINT" <RELEASER> "$HASH")
   rpc "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"sendTransaction\",\"params\":[\"$TX\",{\"encoding\":\"base64\",\"preflightCommitment\":\"confirmed\"}]}"
   ```

   `set-releaser` in place of `initialize` names a new releaser the same
   way, after the edge's key was replaced. Wait for one call to confirm
   before sending the next.

4. Fund the releaser with a little SOL: it pays the fee of each release
   and refund, and opens the seller's and treasury's USDC accounts when they
   have none (about 0.002 SOL each).

5. Name the program on the edge:

   ```nix
   losos.edge.market.usdc.networks.solana = {
     pay = "<PROGRAM>";
     treasury = "<TREASURY>";
   };
   ```

   The edge checks the config (Circle's mint, this treasury, its own key)
   and that the program is final before it counts a payment, and
   `losos edge escrow releaser` reports both.

For a test edge, use `solana-devnet`, devnet's USDC mint
`4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`, and skip step 2 while the
program may still change.
