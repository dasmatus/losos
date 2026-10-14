# LososPay

One contract, no admin key. A buyer approves USDC and calls `pay`. The
contract forwards the platform fee to `treasury` and the rest to the seller,
then emits `Paid`. The edge checks that log. It never holds a signing key,
and nothing here can be upgraded.

The fee may be at most half of the payment, which is the fixed split of a
machine order. The edge checks the exact fee of each order, and before it
trusts a log it reads `usdc()` and `treasury()` back from the contract and
refuses one deployed over any other token or treasury.

Deploy once per chain (Base for live, Base Sepolia for test) with Circle's
USDC and the treasury address. The two addresses go in the edge config, not
in a secret:

```nix
losos.edge.market.usdc = {
  enable = true;
  chainId = 84532; # 8453 for Base
  pay = "0x…";       # this contract
  treasury = "0x…";  # platform fee
};
```

The HTTPS RPC URL is a secret (it usually carries an API key). Seal it with
systemd-creds under the name `usdc-rpc-url`; see
[Market](../handbook/docs/in-depth/market.md). The registrar is the only unit
that reads it, and the browser never does: chain reads are proxied through
the box.

Circle USDC:

| Chain         | chain id | USDC |
| ------------- | -------- | ---- |
| Base          | 8453     | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` |
| Base Sepolia  | 84532    | `0x036CbD53842c5426634e7929541eC2318f3dCF7e` |

The edge uses the Sepolia address when `chainId` is 84532 and the Base
address otherwise.

## Wallet

Buyers and sellers use a BIP-39 phrase on MetaMask's path
`m/44'/60'/0'/0/0` (the same path Rabby, Rainbow and Trust Wallet use).
Create one in the Market pane, import an existing phrase, or connect the
wallet already in the browser. The phrase stays in the browser, encrypted
with a password. The box never sees it.

## Tests

```sh
# Local transactions, then a real pay() on anvil, checked by the registrar.
# If https://sepolia.base.org answers, the same payment runs against Circle's
# Base Sepolia USDC on a fork.
./test-payment.sh
```

The Base Sepolia fork uses forge-std (`forge install foundry-rs/forge-std`
from this directory). The script installs it if it is missing. It deals
Circle's test USDC at `0x036CbD53842c5426634e7929541eC2318f3dCF7e` and pays
through `LososPay` on a fork of `https://sepolia.base.org`.


