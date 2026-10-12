#!/bin/bash
# Pay a real USDC transfer on a local chain (anvil), check the split, and
# hand the receipt to the registrar's matcher. Then, if Base Sepolia answers,
# run the same payment against Circle's testnet USDC on a fork.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="${HOME}/.foundry/bin:${PATH}"

echo "== forge: local transactions =="
forge test --match-path test/LososPay.t.sol -vv

RPC=http://127.0.0.1:8545
KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
BUYER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
SELLER=0x70997970C51812dc3A010C7d01b50e0d17dc79C8
TREASURY=0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
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

echo "== anvil: deploy, mint, approve, pay =="
USDC=$(forge create src/MockUSDC.sol:MockUSDC --rpc-url "${RPC}" --private-key "${KEY}" --broadcast --json | json_field deployedTo)
PAY=$(forge create src/LososPay.sol:LososPay --rpc-url "${RPC}" --private-key "${KEY}" --broadcast --json --constructor-args "${USDC}" "${TREASURY}" | json_field deployedTo)
cast send "${USDC}" "mint(address,uint256)" "${BUYER}" 2000000 --rpc-url "${RPC}" --private-key "${KEY}" >/dev/null
cast send "${USDC}" "approve(address,uint256)" "${PAY}" 2000000 --rpc-url "${RPC}" --private-key "${KEY}" >/dev/null
ORDER_ID=ord_anvil
HASH=$(cast keccak "${ORDER_ID}")
TX=$(cast send "${PAY}" "pay(bytes32,address,uint256,uint256)" "${HASH}" "${SELLER}" 2000000 80000 --rpc-url "${RPC}" --private-key "${KEY}" --json | json_field transactionHash)
TREASURY_BAL=$(cast call "${USDC}" "balanceOf(address)(uint256)" "${TREASURY}" --rpc-url "${RPC}" | awk '{print $1}')
SELLER_BAL=$(cast call "${USDC}" "balanceOf(address)(uint256)" "${SELLER}" --rpc-url "${RPC}" | awk '{print $1}')
echo "treasury ${TREASURY_BAL} seller ${SELLER_BAL} tx ${TX}"
test "${TREASURY_BAL}" = "80000"
test "${SELLER_BAL}" = "1920000"

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
  '{pay:$pay, seller:$seller, order_id_hash:$hash, gross:$gross, fee:$fee, receipt:$receipt[0], payout_address:$payout[0].address, payout_message:$payout[0].message, payout_signature:$payout[0].signature}' \
  > /tmp/losos-anvil-payment.json
echo "wrote /tmp/losos-anvil-payment.json"

echo "== registrar: the receipt and the payout signature =="
( cd ../backend-registrar && LOSOS_ANVIL_RECEIPT=/tmp/losos-anvil-payment.json cargo test --lib an_anvil_receipt_pays_the_order_it_names -- --nocapture )

echo "== Base Sepolia fork, if the public RPC answers =="
if [ ! -d lib/forge-std ]; then
  forge install foundry-rs/forge-std --no-commit
fi
if curl -sf --max-time 8 -X POST -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
  https://sepolia.base.org | grep -q 0x14a34; then
  BASE_SEPOLIA_RPC=https://sepolia.base.org forge test --match-path test/LososPayFork.t.sol -vv
else
  echo "Base Sepolia did not answer; the fork test was not run"
fi

echo "payment run ok"
