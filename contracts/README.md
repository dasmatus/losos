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
Mainnet live, their Sepolia networks for test) with Circle's USDC on that
chain and the treasury address. The two addresses go in the edge config, not
in a secret:

```nix
losos.edge.market.usdc = {
  enable = true;
  networks.base-sepolia = {
    pay = "0x…";       # this contract
    treasury = "0x…";  # platform fee
  };
};
```

Each network's HTTPS RPC URL is a secret (it usually carries an API key),
sealed with systemd-creds as `usdc-rpc-<network>`; see
[CI and releases](../docs/ci-and-releases.md#usdc-networks). The registrar is
the only unit that reads it, and the browser never does: chain reads are
proxied through the box. Solana needs no contract: the buyer's transaction
splits the payment itself.

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
# Local transactions, then a real pay() on anvil, checked by the registrar.
# If https://sepolia.base.org answers, the same payment runs against Circle's
# Base Sepolia USDC on a fork.
./test-payment.sh
```

The Base Sepolia fork uses forge-std (`forge install foundry-rs/forge-std`
from this directory). The script installs it if it is missing. It deals
Circle's test USDC at `0x036CbD53842c5426634e7929541eC2318f3dCF7e` and pays
through `LososPay` on a fork of `https://sepolia.base.org`.


