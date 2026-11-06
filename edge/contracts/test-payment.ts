#!/usr/bin/env bun
// Pay a real USDC deposit into LososEscrow on a local chain (anvil), etc.
// Shim target for test-payment.sh. Preserves behavior.
import { existsSync, mkdtempSync, openSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import { dirname, join } from "node:path";
import { spawn, spawnSync } from "node:child_process";

function sh(cmd: string, opts: { allowFail?: boolean; capture?: boolean } = {}) {
  const p = spawnSync("bash", ["-euo", "pipefail", "-c", cmd], { stdio: opts.capture ? ["ignore","pipe","pipe"] : "inherit", encoding: "utf8", env: process.env, maxBuffer: 64*1024*1024 });
  if (p.status !== 0 && !opts.allowFail) { if (opts.capture) process.stderr.write((p.stderr||"")+(p.stdout||"")); process.exit(p.status ?? 1); }
  return p;
}
const here = dirname(import.meta.path);
process.chdir(here);
process.env.PATH = `${homedir()}/.foundry/bin:${process.env.PATH||""}`;
if (!existsSync("lib/forge-std")) sh("forge install foundry-rs/forge-std@v1.17.0");
console.log("== forge: local transactions ==");
sh("forge test --no-match-path 'test/*Fork.t.sol' -vv");
const RPC="http://127.0.0.1:8545", KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
 BUYER="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", SELLER="0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
 SELLER_KEY="0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d", TREASURY="0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC",
 RELEASER="0x90F79bf6EB2c4f870365E785982E1f101E93b906", RELEASER_KEY="7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6",
 BUYER_BOX="0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc", SELLER_BOX="0x976EA74026E726554dB657fA54763abd0C3a0aa9",
 SELLER_BOX_KEY="0x92db14e403b83dfe3df233f83dfa3a0d7096f21ca9b0d6d6b8d88b2b4ec1564e", PORT="8545";
if (spawnSync("curl",["-sf","-o","/dev/null","--max-time","1",RPC]).status===0) { console.error(`something is already on ${PORT}`); process.exit(1); }
const anvilLog = openSync("/tmp/losos-anvil.log", "w");
const anvil = spawn("anvil", ["--port", PORT, "--silent"], { stdio: ["ignore", "ignore", anvilLog] });
process.on("exit", ()=>{ try{anvil.kill();}catch{} });
for(let i=0;i<50;i++){ if(spawnSync("curl",["-sf","-o","/dev/null","--max-time","1","-X","POST","-H","content-type: application/json","--data",'{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}',RPC]).status===0) break; await Bun.sleep(100); }
const jf = (json:string,field:string)=>JSON.parse(json.split('\n').filter(l=>l.trim().startsWith('{')).join('\n')||json)[field];
const USDC = jf(sh(`forge create src/MockUSDC.sol:MockUSDC --rpc-url ${RPC} --private-key ${KEY} --broadcast --json`,{capture:true}).stdout,"deployedTo");
const PAY = jf(sh(`forge create src/LososEscrow.sol:LososEscrow --rpc-url ${RPC} --private-key ${KEY} --broadcast --json --constructor-args ${USDC} ${TREASURY} ${RELEASER}`,{capture:true}).stdout,"deployedTo");
sh(`cast send ${USDC} "mint(address,uint256)" ${BUYER} 2000000 --rpc-url ${RPC} --private-key ${KEY} >/dev/null`);
sh(`cast send ${USDC} "approve(address,uint256)" ${PAY} 2000000 --rpc-url ${RPC} --private-key ${KEY} >/dev/null`);
const ORDER_ID="ord_anvil"; const HASH=sh(`cast keccak ${ORDER_ID}`,{capture:true}).stdout.trim();
const NOW=Number(sh(`cast block latest --field timestamp --rpc-url ${RPC}`,{capture:true}).stdout.trim()); const DEADLINE=NOW+86400;
console.log("== anvil: deploy, mint, approve, pay into the escrow ==");
const TX=jf(sh(`cast send ${PAY} "pay(bytes32,address,uint256,uint256,uint64,address,address)" ${HASH} ${SELLER} 2000000 80000 ${DEADLINE} ${BUYER_BOX} ${SELLER_BOX} --rpc-url ${RPC} --private-key ${KEY} --json`,{capture:true}).stdout,"transactionHash");
const bal=(addr:string)=>sh(`cast call ${USDC} "balanceOf(address)(uint256)" ${addr} --rpc-url ${RPC}`,{capture:true}).stdout.trim().split(/\s+/)[0];
console.log(`escrow ${bal(PAY)} seller ${bal(SELLER)} tx ${TX}`);
if(bal(PAY)!=="2000000"||bal(SELLER)!=="0") process.exit(1);
const receipt = sh(`curl -sf -X POST -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":1,"method":"eth_getTransactionReceipt","params":["${TX}"]}' ${RPC} | jq .result`,{capture:true}).stdout;
console.log("== wallet: MetaMask derivation, then a real payment ==");
if(!existsSync("wallet/node_modules/viem")) sh("npm install --prefix wallet --silent");
sh("bun contracts/wallet/check.ts 84532 > /tmp/losos-payout.json");
writeFileSync("/tmp/losos-anvil-payment.json", JSON.stringify({pay:PAY,seller:SELLER,order_id_hash:HASH,gross:"2000000",fee:"80000",deadline:DEADLINE,buyer_box:BUYER_BOX,seller_box:SELLER_BOX,deposit:1,receipt:JSON.parse(receipt),payout_address:JSON.parse(readFileSync("/tmp/losos-payout.json","utf8")).address,payout_message:JSON.parse(readFileSync("/tmp/losos-payout.json","utf8")).message,payout_signature:JSON.parse(readFileSync("/tmp/losos-payout.json","utf8")).signature}));
console.log("wrote /tmp/losos-anvil-payment.json");
writeFileSync("/tmp/losos-anvil-escrow.json", JSON.stringify({rpc:RPC,escrow:PAY,deposit:1,releaser_key:RELEASER_KEY,seller_box_key:SELLER_BOX_KEY}));
console.log("== registrar: the receipt, the payout signature, the release ==");
sh("( cd ../registrar && LOSOS_ANVIL_RECEIPT=/tmp/losos-anvil-payment.json cargo test --lib an_anvil_receipt_pays_the_order_it_names -- --nocapture )");
sh("( cd ../registrar && LOSOS_ANVIL_ESCROW=/tmp/losos-anvil-escrow.json cargo test --lib an_anvil_deposit_is_released_by_the_edge_key -- --nocapture )");
console.log(`escrow ${bal(PAY)} treasury ${bal(TREASURY)} seller ${bal(SELLER)}`);
if(bal(PAY)!=="0"||bal(TREASURY)!=="80000"||bal(SELLER)!=="1920000") process.exit(1);
console.log("== registrar: a seller pays platform fees on coin sales to the treasury ==");
const FEE_TX=jf(sh(`cast send ${USDC} "transfer(address,uint256)" ${TREASURY} 500000 --rpc-url ${RPC} --private-key ${SELLER_KEY} --json`,{capture:true}).stdout,"transactionHash");
const feeReceipt=sh(`curl -sf -X POST -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":1,"method":"eth_getTransactionReceipt","params":["${FEE_TX}"]}' ${RPC} | jq .result`,{capture:true}).stdout;
writeFileSync("/tmp/losos-anvil-fee.json", JSON.stringify({receipt:JSON.parse(feeReceipt),usdc:USDC,treasury:TREASURY,from:SELLER,amount:"500000"}));
sh("( cd ../registrar && LOSOS_ANVIL_FEE_RECEIPT=/tmp/losos-anvil-fee.json cargo test --lib an_anvil_fee_transfer_pays_the_treasury -- --nocapture )");
if(bal(TREASURY)!=="580000") process.exit(1);
console.log("== taps: a wallet tops up and opens a tap; the edge key settles and closes it ==");
const RATE=3600000;
sh(`cast send ${USDC} "mint(address,uint256)" ${BUYER} 10000000 --rpc-url ${RPC} --private-key ${KEY} >/dev/null`);
sh(`cast send ${USDC} "approve(address,uint256)" ${PAY} 10000000 --rpc-url ${RPC} --private-key ${KEY} >/dev/null`);
sh(`cast send ${PAY} "topUp(uint256)" 10000000 --rpc-url ${RPC} --private-key ${KEY} >/dev/null`);
const TAP_HASH=sh("cast keccak tap_anvil_1",{capture:true}).stdout.trim();
sh(`cast send ${PAY} "openTaps(bytes32[],address[],uint128[],uint16)" "[${TAP_HASH}]" "[${SELLER}]" "[${RATE}]" 400 --rpc-url ${RPC} --private-key ${SELLER_KEY} >/dev/null`);
sh(`cast send ${PAY} "openTaps(bytes32[],address[],uint128[],uint16)" "[${TAP_HASH}]" "[${SELLER}]" "[${RATE}]" 400 --rpc-url ${RPC} --private-key ${KEY} >/dev/null`);
const TAP_KEY=sh(`cast call ${PAY} "tapKey(address,bytes32)(bytes32)" ${BUYER} ${TAP_HASH} --rpc-url ${RPC}`,{capture:true}).stdout.trim();
const SELLER_BEFORE=Number(bal(SELLER)), TREASURY_BEFORE=Number(bal(TREASURY));
writeFileSync("/tmp/losos-anvil-taps.json", JSON.stringify({rpc:RPC,escrow:PAY,tap:"tap_anvil_1",tap_key:TAP_KEY,buyer:BUYER,seller:SELLER,rate:RATE,topped_up:10000000,releaser_key:RELEASER_KEY}));
sh("( cd ../registrar && LOSOS_ANVIL_TAPS=/tmp/losos-anvil-taps.json cargo test --lib an_anvil_tap_is_settled_within_its_cap_and_closed -- --nocapture )");
if(bal(SELLER)!==String(SELLER_BEFORE+3456000)||bal(TREASURY)!==String(TREASURY_BEFORE+144000)||bal(PAY)!=="6400000") process.exit(1);
console.log("== Base Sepolia fork, if the public RPC answers ==");
const fork=spawnSync("curl",["-sf","--max-time","8","-X","POST","-H","content-type: application/json","--data",'{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}',"https://sepolia.base.org"],{encoding:"utf8"});
if((fork.stdout||"").includes("0x14a34")) sh("BASE_SEPOLIA_RPC=https://sepolia.base.org forge test --match-path 'test/*Fork.t.sol' -vv");
else console.log("Base Sepolia did not answer; the fork test was not run");
console.log("payment run ok");
try{anvil.kill();}catch{}
