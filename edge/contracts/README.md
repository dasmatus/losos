# LososPay

One contract, no admin key. A buyer approves USDC and calls `pay`. The
contract forwards the platform fee to `treasury` and the rest to the seller,
then emits `Paid`. The edge checks that log. It never holds a signing key,
and nothing here can be upgraded.

The fee may be at most half of the payment, which is the fixed split of a
machine order. The edge checks the exact fee of each order, and before it
trusts a log it reads `usdc()` and `treasury()` back from the contract and
refuses one deployed over any other token or treasury.

Deploy once per EVM chain the edge settles on (Base, Arbitrum One and OP
Mainnet; their Sepolia networks for a test edge) with Circle's USDC on that
chain and the treasury address. The two addresses go in the edge config, not
in a secret:

```nix
losos.edge.market.usdc = {
  enable = true;
  networks.base = {
    pay = "0x…";       # this contract
    treasury = "0x…";  # platform fee
  };
};
```

Each network's HTTPS RPC URL is a secret (it usually carries an API key),
sealed with systemd-creds as `usdc-rpc-<network>`; see
[CI and releases](../../docs/ci-and-releases.md#usdc-networks). The registrar is
the only unit that reads it, and the browser never does: chain reads are
proxied through the box. On Solana the escrow is a program,
[`solana/`](solana/README.md); without one the buyer's transaction splits the
payment itself and nothing is held.

Circle USDC:

| Network          | chain id | USDC |
| ---------------- | -------- | ---- |
| Base             | 8453     | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` |
| Base Sepolia     | 84532    | `0x036CbD53842c5426634e7929541eC2318f3dCF7e` |
| Arbitrum One     | 42161    | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` |
| Arbitrum Sepolia | 421614   | `0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d` |
| OP Mainnet       | 10       | `0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85` |
| OP Sepolia       | 11155420 | `0x5fd84259d66Cd46123540766Be93DFE6D43130D7` |

The edge knows these and picks the token by the network's name; the contract
must pull the same one, which the edge checks before it counts a payment.

## Wallet

Buyers and sellers use a BIP-39 phrase on MetaMask's path
`m/44'/60'/0'/0/0` (the same path Rabby, Rainbow and Trust Wallet use).
Create one in the Market pane, import an existing phrase, or connect the
wallet already in the browser. The phrase stays in the browser, encrypted
with a password. The box never sees it.

## Tests

```sh
# Local transactions, then a real pay() on anvil, checked by the registrar,
# then a tap topped up, settled by the edge key within its cap and closed.
# If https://sepolia.base.org answers, the same payment runs against Circle's
# Base Sepolia USDC on a fork.
./test-payment.sh
```

`devenv`'s `test-evm-escrow` runs it with Foundry from nixpkgs, as CI's
`evm-escrow` job does.

The Base Sepolia fork uses forge-std (`forge install foundry-rs/forge-std`
from this directory). The script installs it if it is missing. It deals
Circle's test USDC at `0x036CbD53842c5426634e7929541eC2318f3dCF7e` and pays
through `LososPay` on a fork of `https://sepolia.base.org`.



# LososEscrow

The held version of `LososPay`. A buyer approves USDC and calls
`pay(orderId, seller, gross, fee, deadline, buyerBox, sellerBox)`. Instead of
forwarding the money, the contract holds it as one deposit and emits `Held`,
which names the two box keys. The edge keeps it there for as long as what was
bought runs, watching that the seller delivers it, and then one of three
moves ends it:

- `release(depositId, sigs)` sends the fee to `treasury` and the rest to the
  seller recorded in `pay`,
- `refund(depositId, sigs)` sends the whole deposit back to the payer, or
- `settle(depositId, refundAmount, sigs)`, when the seller stopped
  delivering partway: `refundAmount` goes back to the payer, and the rest
  is released, with the fee scaled down to the seller's share and rounded
  down. It emits `Settled`, and the deposit ends in the `Split` state.

Each move needs two of three keys: the edge's (`releaser`), the buyer's box
(`buyerBox`) and the seller's box (`sellerBox`). The sender counts as one key
when it is one of the three, and `sigs` carries the others: EIP-712
signatures of `Decision(uint256 depositId,uint256 refund)` in the domain
`LososEscrow`, version `2`, where `refund` is 0 for a release, the gross for
a refund and `refundAmount` for a split (`decisionDigest` computes it). A
signature is 65 bytes, `r ‖ s ‖ v`, with a low `s` and `v` 27 or 28; a
signature from any other key reverts. The edge sends the transaction and the
box that the move pays signs it: the seller's for a release, the buyer's for
a refund or a split. The two boxes together can also end a deposit without
the edge.

No key can change a deposit's seller, amount or payer, and `pay` refuses box
keys that are zero, equal, or the releaser. If no two keys agree in time, the
payer calls `reclaim(depositId)` after the deadline, which must lie between
one hour and 35 days after the payment: a month of storage or machines, plus
the days the edge has to settle once the month is over. Paying the same order
twice makes two deposits.

`treasury` is the only account that can replace `releaser` or call `rescue`.
`rescue` moves tokens that arrived by plain transfer instead of `pay`, and for
USDC only the balance above what open deposits are owed. Keep `treasury` on a
hardware wallet or a multisig, never on a server.

## Balances and taps

The same contract also holds prepaid balances for pay-as-you-go. A buyer
calls `topUp(amount)` to move USDC onto their balance, one balance per buyer.
To use listings the buyer calls `openTaps(ids, sellers, rates, feeBps)`,
which opens up to 16 taps in one call. A tap names one seller, the most the
edge may bill per hour (`rate`, in USDC base units) and the fee, at most
half. The id the buyer passes is keccak256 of the edge's tap id. The contract
keeps the tap under `tapKey(buyer, id)`, `keccak256(abi.encode(buyer, id))`,
so a key opens once, and another wallet that sends the same ids first only
opens taps of its own. The Solana program does the same with the buyer in the
tap account's seeds, `["tap", config, buyer, id]`.

The edge meters what the buyer used and calls `settleTap(key, amount)` from
time to time. The contract takes `amount` from the buyer's balance, sends the
fee (rounded down) to `treasury` and the rest to the tap's seller, and
refuses an amount over the balance or over the tap's rate for the time since
its last settle. A new tap starts one hour into that time, so the first hour
of use can be billed as soon as the tap opens. The time counts up to the
tap's close, and at most 7 days of it in one settle, and cap left unused is
not carried over. So a stolen edge key can take, from one tap, at most its
rate times the time since it opened plus one hour, never more than a week of
it at once, and only to the seller the buyer named.

The buyer or the edge can close a tap with `closeTap(key)`, once. The edge can
still bill a closed tap for the time before it closed. To take money back the
buyer calls `askWithdrawal(amount)`, which replaces any earlier ask, waits 48
hours so the edge can bill the last slice, then calls `withdraw` and gets the
amount asked for, or what is left of the balance if that is less. Withdrawing
never needs the edge. Balances count towards `held`, so `rescue` cannot move
them.

Deploy it from a fresh key whose first transaction on every chain is this
deployment. A contract's address depends only on the deployer and its nonce,
so the escrow then has the same address on Base, Arbitrum and OP, and USDC
sent to it on the wrong one of those chains can be rescued. Never send any
other transaction from that key.

```sh
forge test --match-contract LososEscrow
BASE_SEPOLIA_RPC=https://sepolia.base.org forge test --match-contract LososEscrowFork
```
