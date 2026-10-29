#!/bin/bash
# Pay a real USDC deposit into LososEscrow on a local chain (anvil), hand the
# receipt to the registrar's matcher, and have the registrar's own signer
# release it with the seller box's agreement; then check the split. Then, if Base Sepolia answers, run the
# same payments against Circle's testnet USDC on a fork.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="${HOME}/.foundry/bin:${PATH}"

# forge compiles every test file, the fork test's forge-std import included,
# so the library has to be there before the first run.
if [ ! -d lib/forge-std ]; then
  forge install foundry-rs/forge-std@v1.17.0
fi

echo "== forge: local transactions =="
forge test --no-match-path 'test/*Fork.t.sol' -vv

RPC=http://127.0.0.1:8545
KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
BUYER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
SELLER=0x70997970C51812dc3A010C7d01b50e0d17dc79C8
# anvil's second account, so the seller can pay its coin fees itself.
SELLER_KEY=0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
TREASURY=0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
# anvil's fourth account, standing in for the edge's releaser key.
RELEASER=0x90F79bf6EB2c4f870365E785982E1f101E93b906
RELEASER_KEY=7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
# anvil's sixth and seventh accounts, standing in for the two boxes' escrow
# keys: with the edge's, any two of the three move a deposit.
BUYER_BOX=0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc
SELLER_BOX=0x976EA74026E726554dB657fA54763abd0C3a0aa9
SELLER_BOX_KEY=0x92db14e403b83dfe3df233f83dfa3a0d7096f21ca9b0d6d6b8d88b2b4ec1564e
PORT=8545

if curl -sf -o /dev/null --max-time 1 "${RPC}"; then
  echo "something is already on ${PORT}" >&2
  exit 1
fi
anvil --port "${PORT}" --silent >/tmp/losos-anvil.log 2>&1 &
ANVIL=$!
cleanup() { kill "${ANVIL}" 2>/dev/null || true; }
trap cleanup EXIT
for _ in $(seq 1 50); do
  curl -sf -o /dev/null --max-time 1 -X POST -H 'content-type: application/json' \
    --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' "${RPC}" && break
  sleep 0.1
done

# forge prints "Compiling…" on stdout when a pipe is the compiler's first
# look at a file. Keep the JSON object and nothing before it.
json_field() {
  sed -n '/^{/,$p' | jq -er ".$1"
}

balance() {
  cast call "${USDC}" "balanceOf(address)(uint256)" "$1" --rpc-url "${RPC}" | awk '{print $1}'
}

echo "== anvil: deploy, mint, approve, pay into the escrow =="
USDC=$(forge create src/MockUSDC.sol:MockUSDC --rpc-url "${RPC}" --private-key "${KEY}" --broadcast --json | json_field deployedTo)
PAY=$(forge create src/LososEscrow.sol:LososEscrow --rpc-url "${RPC}" --private-key "${KEY}" --broadcast --json --constructor-args "${USDC}" "${TREASURY}" "${RELEASER}" | json_field deployedTo)
cast send "${USDC}" "mint(address,uint256)" "${BUYER}" 2000000 --rpc-url "${RPC}" --private-key "${KEY}" >/dev/null
cast send "${USDC}" "approve(address,uint256)" "${PAY}" 2000000 --rpc-url "${RPC}" --private-key "${KEY}" >/dev/null
ORDER_ID=ord_anvil
HASH=$(cast keccak "${ORDER_ID}")
NOW=$(cast block latest --field timestamp --rpc-url "${RPC}")
DEADLINE=$((NOW + 86400))
TX=$(cast send "${PAY}" "pay(bytes32,address,uint256,uint256,uint64,address,address)" "${HASH}" "${SELLER}" 2000000 80000 "${DEADLINE}" "${BUYER_BOX}" "${SELLER_BOX}" --rpc-url "${RPC}" --private-key "${KEY}" --json | json_field transactionHash)
echo "escrow $(balance "${PAY}") seller $(balance "${SELLER}") tx ${TX}"
test "$(balance "${PAY}")" = "2000000"
test "$(balance "${SELLER}")" = "0"

RECEIPT=$(mktemp)
cast receipt "${TX}" --json >"${RECEIPT}"
META=$(mktemp)
# cast may number the status. The registrar wants 0x-hex, which anvil's
# eth_getTransactionReceipt returns. Ask anvil the same way the registrar will.
curl -sf -X POST -H 'content-type: application/json' \
  --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"eth_getTransactionReceipt\",\"params\":[\"${TX}\"]}" \
  "${RPC}" | jq .result >"${RECEIPT}"

echo "== wallet: MetaMask derivation, then a real payment =="
if [ ! -d wallet/node_modules/viem ]; then
  npm install --prefix wallet --silent
fi
node wallet/check.mjs 84532 > /tmp/losos-payout.json

jq -n \
  --slurpfile receipt "${RECEIPT}" \
  --slurpfile payout /tmp/losos-payout.json \
  --arg pay "${PAY}" \
  --arg seller "${SELLER}" \
  --arg hash "${HASH}" \
  --arg gross "2000000" \
  --arg fee "80000" \
  --argjson deadline "${DEADLINE}" \
  --arg buyer_box "${BUYER_BOX}" \
  --arg seller_box "${SELLER_BOX}" \
  '{pay:$pay, seller:$seller, order_id_hash:$hash, gross:$gross, fee:$fee, deadline:$deadline, buyer_box:$buyer_box, seller_box:$seller_box, deposit:1, receipt:$receipt[0], payout_address:$payout[0].address, payout_message:$payout[0].message, payout_signature:$payout[0].signature}' \
  > /tmp/losos-anvil-payment.json
echo "wrote /tmp/losos-anvil-payment.json"
jq -n --arg rpc "${RPC}" --arg escrow "${PAY}" --arg key "${RELEASER_KEY}" --arg box "${SELLER_BOX_KEY}" \
  '{rpc:$rpc, escrow:$escrow, deposit:1, releaser_key:$key, seller_box_key:$box}' > /tmp/losos-anvil-escrow.json

echo "== registrar: the receipt, the payout signature, the release =="
( cd ../registrar && LOSOS_ANVIL_RECEIPT=/tmp/losos-anvil-payment.json cargo test --lib an_anvil_receipt_pays_the_order_it_names -- --nocapture )
( cd ../registrar && LOSOS_ANVIL_ESCROW=/tmp/losos-anvil-escrow.json cargo test --lib an_anvil_deposit_is_released_by_the_edge_key -- --nocapture )
echo "escrow $(balance "${PAY}") treasury $(balance "${TREASURY}") seller $(balance "${SELLER}")"
test "$(balance "${PAY}")" = "0"
test "$(balance "${TREASURY}")" = "80000"
test "$(balance "${SELLER}")" = "1920000"

echo "== registrar: a seller pays platform fees on coin sales to the treasury =="
FEE_TX=$(cast send "${USDC}" "transfer(address,uint256)" "${TREASURY}" 500000 --rpc-url "${RPC}" --private-key "${SELLER_KEY}" --json | json_field transactionHash)
FEE_RECEIPT=$(mktemp)
curl -sf -X POST -H 'content-type: application/json' \
  --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"eth_getTransactionReceipt\",\"params\":[\"${FEE_TX}\"]}" \
  "${RPC}" | jq .result >"${FEE_RECEIPT}"
jq -n --slurpfile receipt "${FEE_RECEIPT}" --arg usdc "${USDC}" --arg treasury "${TREASURY}" --arg from "${SELLER}" \
  '{receipt:$receipt[0], usdc:$usdc, treasury:$treasury, from:$from, amount:"500000"}' > /tmp/losos-anvil-fee.json
( cd ../registrar && LOSOS_ANVIL_FEE_RECEIPT=/tmp/losos-anvil-fee.json cargo test --lib an_anvil_fee_transfer_pays_the_treasury -- --nocapture )
test "$(balance "${TREASURY}")" = "580000"

echo "== taps: a wallet tops up and opens a tap; the edge key settles and closes it =="
RATE=3600000
cast send "${USDC}" "mint(address,uint256)" "${BUYER}" 10000000 --rpc-url "${RPC}" --private-key "${KEY}" >/dev/null
cast send "${USDC}" "approve(address,uint256)" "${PAY}" 10000000 --rpc-url "${RPC}" --private-key "${KEY}" >/dev/null
cast send "${PAY}" "topUp(uint256)" 10000000 --rpc-url "${RPC}" --private-key "${KEY}" >/dev/null
TAP_HASH=$(cast keccak tap_anvil_1)
# Another wallet copies the quoted id first; it gets a tap of its own and
# the buyer's opens all the same.
cast send "${PAY}" "openTaps(bytes32[],address[],uint128[],uint16)" "[${TAP_HASH}]" "[${SELLER}]" "[${RATE}]" 400 --rpc-url "${RPC}" --private-key "${SELLER_KEY}" >/dev/null
cast send "${PAY}" "openTaps(bytes32[],address[],uint128[],uint16)" "[${TAP_HASH}]" "[${SELLER}]" "[${RATE}]" 400 --rpc-url "${RPC}" --private-key "${KEY}" >/dev/null
TAP_KEY=$(cast call "${PAY}" "tapKey(address,bytes32)(bytes32)" "${BUYER}" "${TAP_HASH}" --rpc-url "${RPC}")
SELLER_BEFORE=$(balance "${SELLER}")
TREASURY_BEFORE=$(balance "${TREASURY}")
jq -n --arg rpc "${RPC}" --arg escrow "${PAY}" --arg key "${RELEASER_KEY}" \
  --arg buyer "${BUYER}" --arg seller "${SELLER}" --argjson rate "${RATE}" --arg tapkey "${TAP_KEY}" \
  '{rpc:$rpc, escrow:$escrow, tap:"tap_anvil_1", tap_key:$tapkey, buyer:$buyer, seller:$seller, rate:$rate, topped_up:10000000, releaser_key:$key}' \
  > /tmp/losos-anvil-taps.json
( cd ../registrar && LOSOS_ANVIL_TAPS=/tmp/losos-anvil-taps.json cargo test --lib an_anvil_tap_is_settled_within_its_cap_and_closed -- --nocapture )
# An hour at the rate: 4% to the treasury, the rest to the seller.
test "$(balance "${SELLER}")" = "$((SELLER_BEFORE + 3456000))"
test "$(balance "${TREASURY}")" = "$((TREASURY_BEFORE + 144000))"
test "$(balance "${PAY}")" = "6400000"

echo "== Base Sepolia fork, if the public RPC answers =="
if curl -sf --max-time 8 -X POST -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
  https://sepolia.base.org | grep -q 0x14a34; then
  BASE_SEPOLIA_RPC=https://sepolia.base.org forge test --match-path 'test/*Fork.t.sol' -vv
else
  echo "Base Sepolia did not answer; the fork test was not run"
fi

echo "payment run ok"
