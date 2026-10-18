# losos-escrow

The Solana twin of `LososEscrow`: one USDC payment held per deposit for as
long as what it bought runs. The edge then releases it to the seller and the
treasury recorded at payment, refunds it to the buyer, or splits it between
the two when the seller stopped delivering partway. The edge's key can do
nothing else, and if the edge never answers, the buyer takes the deposit
back after its deadline. Written with [pinocchio](https://github.com/anza-xyz/pinocchio),
no Anchor, about 40 KB built.

## Accounts

| Account | Address | Holds |
| ------- | ------- | ----- |
| Config  | `["config", treasury, mint]` | the mint, the treasury, the releaser |
| Deposit | `["deposit", config, nonce]` | payer, seller, order, gross, fee, deadline, paid at |
| Vault   | `["vault", deposit]` | the USDC, in a token account the deposit owns |

The nonce is 32 random bytes the wallet picks for each payment. The order is
the keccak-256 of the edge's order id, the invoice's `order_id_hash`. Every
integer is little-endian; `src/state.rs` has the byte layouts.

## Instructions

| Tag | Call | Signer | What it does |
| --- | ---- | ------ | ------------ |
| 0 | `initialize(releaser)` | treasury | Makes the config, once per treasury and mint. |
| 1 | `pay(nonce, order, gross, fee, deadline)` | payer | Opens the deposit and its vault and moves the gross in. |
| 2 | `release` | releaser | Fee to the treasury, the rest to the seller; closes both accounts. |
| 3 | `refund` | releaser, or the payer after the deadline | Everything back to the payer; closes both. |
| 4 | `settle(back)` | releaser | `back` to the payer, the rest released with the fee scaled down. |
| 5 | `set_releaser(releaser)` | treasury | Names another releaser. |

The rules are the EVM contract's: a fee of at most half the gross, a deadline
from one hour to 35 days after payment, a seller that is not the treasury,
the config, the deposit or the vault. Only the classic SPL token program and
the config's mint are taken. A release or split pays only token accounts of
the seller and treasury the deposit names, and a refund only the payer's.
Closing a deposit gives its rent back to the payer, and any tokens sent to a
vault by plain transfer go to the treasury on a release or split and to the
payer on a refund, so nothing can keep a deposit open.

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
