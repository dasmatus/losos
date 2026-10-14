// What a USDC payment or send costs in network fees on each network, so the
// wallet can offer the cheapest one first. The fee is paid in the network's
// own coin (ETH, or SOL on Solana) on top of the USDC; the seller's share and
// the platform fee are the same on every network.
//
// The estimates are the gas a payment typically uses at the network's gas
// price now, plus the Ethereum data fee an OP Stack or Arbitrum chain adds,
// and on Solana the base fee plus the rent for any USDC account the payment
// has to open. They are close enough to rank networks, not a promise.
import { decodeFunctionResult, encodeFunctionData, type Address, type Hex } from "viem";
import type { UsdcChain } from "@/lib/api";
import { evmClient, solanaRpc } from "./chains";
import { associatedTokenAddress } from "./solana";

/** Gas a USDC approval, a payment through the pay contract, and a plain
 *  USDC transfer use, measured on Base Sepolia and rounded up. */
const GAS = { approve: 46_000n, pay: 100_000n, send: 65_000n } as const;

/** Serialized size of each transaction, for the L1 data fee. */
const BYTES = { approve: 120n, pay: 260n, send: 120n } as const;

/** Solana's fee per signature; a payment carries one. */
const LAMPORTS_PER_SIGNATURE = 5_000n;
/** Size of an SPL token account, for its rent-exempt minimum. */
const TOKEN_ACCOUNT_BYTES = 165;

const OP_STACK = new Set(["base", "base-sepolia", "optimism", "optimism-sepolia"]);
const ARBITRUM = new Set(["arbitrum", "arbitrum-sepolia"]);

const ALLOWANCE = [
  {
    type: "function",
    name: "allowance",
    stateMutability: "view",
    inputs: [
      { name: "owner", type: "address" },
      { name: "spender", type: "address" },
    ],
    outputs: [{ type: "uint256" }],
  },
] as const;

/** OP Stack's predeploy that prices the L1 data of a transaction. */
const GAS_PRICE_ORACLE = "0x420000000000000000000000000000000000000F" as const;
const ORACLE_ABI = [
  {
    type: "function",
    name: "getL1FeeUpperBound",
    stateMutability: "view",
    inputs: [{ name: "_unsignedTxSize", type: "uint256" }],
    outputs: [{ type: "uint256" }],
  },
] as const;

/** Arbitrum's virtual contract that says how much L2 gas the L1 data costs. */
const NODE_INTERFACE = "0x00000000000000000000000000000000000000C8" as const;
const NODE_INTERFACE_ABI = [
  {
    type: "function",
    name: "gasEstimateL1Component",
    stateMutability: "payable",
    inputs: [
      { name: "to", type: "address" },
      { name: "contractCreation", type: "bool" },
      { name: "data", type: "bytes" },
    ],
    outputs: [
      { name: "gasEstimateForL1", type: "uint64" },
      { name: "baseFee", type: "uint256" },
      { name: "l1BaseFeeEstimate", type: "uint256" },
    ],
  },
] as const;

/** The lowest EIP-1559 fee that still gets into a block, like a wallet's
 *  "slow" setting: the tip the cheapest tenth of recent transactions paid,
 *  and room for the base fee to rise for two blocks before the payment
 *  waits. What the payment pays is the base fee plus that tip; the cap only
 *  bounds it. */
export interface LowFees {
  maxFeePerGas: bigint;
  maxPriorityFeePerGas: bigint;
  /** What one unit of gas costs now: the base fee plus the tip. */
  perGas: bigint;
}

/** Recent blocks the tip is read from, and the percentile of their tips. */
const FEE_BLOCKS = 10;
const TIP_PERCENTILE = 10;

export async function lowestFees(network: string): Promise<LowFees> {
  const client = evmClient(network);
  const history = await client.getFeeHistory({
    blockCount: FEE_BLOCKS,
    rewardPercentiles: [TIP_PERCENTILE],
  });
  const base = history.baseFeePerGas[history.baseFeePerGas.length - 1] ?? (await client.getGasPrice());
  const tips = (history.reward ?? [])
    .map((r) => r[0] ?? 0n)
    .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  // The median of the low tips, so one empty block's zero does not set it.
  const tip = tips.length > 0 ? tips[Math.floor(tips.length / 2)]! : 0n;
  // EIP-1559 lets the base fee rise 12.5% a block: two blocks of headroom.
  const maxFeePerGas = (base * 81n) / 64n + tip;
  return { maxFeePerGas, maxPriorityFeePerGas: tip, perGas: base + tip };
}

/** The fee fields for a transaction at the lowest fee, or none (the wallet
 *  then picks its own) when the network will not say. */
export async function lowFeeFields(
  network: string,
): Promise<{ maxFeePerGas: bigint; maxPriorityFeePerGas: bigint } | Record<string, never>> {
  try {
    const { maxFeePerGas, maxPriorityFeePerGas } = await lowestFees(network);
    return { maxFeePerGas, maxPriorityFeePerGas };
  } catch {
    return {};
  }
}

export interface FeeEstimate {
  network: string;
  /** In the fee coin's smallest unit: wei or lamports. */
  native: bigint;
  symbol: "ETH" | "SOL";
  decimals: number;
  /** In micro-dollars, when the edge had a price for the fee coin. */
  usdMicros: bigint | null;
}

/** Fee-coin prices from the edge, micro-dollars per whole coin as text. */
export type Prices = Partial<Record<"eth" | "sol", string>>;

export interface FeeQuestion {
  /** A payment through the pay contract, or a plain USDC send. */
  kind: "pay" | "send";
  /** USDC base units moved. */
  amount: bigint;
  /** The wallet paying, to see whether it still has to approve USDC. */
  owner?: string;
  /** Who receives on Solana, to see whether their USDC account exists. The
   *  seller for a payment, the recipient for a send. */
  recipients?: string[];
}

function usd(native: bigint, decimals: number, price: string | undefined): bigint | null {
  if (!price || !/^\d+$/.test(price)) return null;
  return (native * BigInt(price)) / 10n ** BigInt(decimals);
}

/** The fee estimate for `question` on `chain`, or null when the network did
 *  not answer. */
export async function estimateFee(
  chain: UsdcChain,
  question: FeeQuestion,
  prices: Prices = {},
): Promise<FeeEstimate | null> {
  try {
    if (chain.family === "solana") {
      const owners = [...(question.recipients ?? [])];
      if (question.kind === "pay") owners.push(chain.treasury);
      const missing = await Promise.all(
        owners.map(async (owner) => {
          const info = await solanaRpc<{ value: unknown }>(chain.network, "getAccountInfo", [
            associatedTokenAddress(owner, chain.usdc),
            { encoding: "base64", commitment: "confirmed" },
          ]);
          return info.value === null;
        }),
      );
      const opens = BigInt(missing.filter(Boolean).length);
      const rent =
        opens === 0n
          ? 0n
          : BigInt(
              await solanaRpc<number>(chain.network, "getMinimumBalanceForRentExemption", [TOKEN_ACCOUNT_BYTES]),
            );
      const native = LAMPORTS_PER_SIGNATURE + opens * rent;
      return { network: chain.network, native, symbol: "SOL", decimals: 9, usdMicros: usd(native, 9, prices.sol) };
    }

    const client = evmClient(chain.network);
    let approve = false;
    if (question.kind === "pay" && chain.pay && question.owner) {
      const allowance = await client.readContract({
        address: chain.usdc as Address,
        abi: ALLOWANCE,
        functionName: "allowance",
        args: [question.owner as Address, chain.pay as Address],
      });
      approve = allowance < question.amount;
    }
    const gas = question.kind === "send" ? GAS.send : GAS.pay + (approve ? GAS.approve : 0n);
    const bytes = question.kind === "send" ? BYTES.send : BYTES.pay + (approve ? BYTES.approve : 0n);
    const gasPrice = await lowestFees(chain.network)
      .then((f) => f.perGas)
      .catch(() => client.getGasPrice());
    let native = gas * gasPrice;
    native += await dataFee(chain, bytes, gasPrice, approve ? 2n : 1n).catch(() => 0n);
    return { network: chain.network, native, symbol: "ETH", decimals: 18, usdMicros: usd(native, 18, prices.eth) };
  } catch {
    return null;
  }
}

/** The Ethereum data fee an L2 adds for `bytes` of transactions, in wei. A
 *  chain without the predeploy (a local fork) answers nothing and adds 0. */
async function dataFee(chain: UsdcChain, bytes: bigint, gasPrice: bigint, txs: bigint): Promise<bigint> {
  const client = evmClient(chain.network);
  if (OP_STACK.has(chain.network)) {
    return await client.readContract({
      address: GAS_PRICE_ORACLE,
      abi: ORACLE_ABI,
      functionName: "getL1FeeUpperBound",
      args: [bytes],
    });
  }
  if (ARBITRUM.has(chain.network)) {
    // A transaction of the same length as the real one: the L1 component
    // depends on the size of the data, not on what it says.
    const filler = `0x${"ab".repeat(Number(bytes / txs))}` as Hex;
    const data = encodeFunctionData({
      abi: NODE_INTERFACE_ABI,
      functionName: "gasEstimateL1Component",
      args: [chain.usdc as Address, false, filler],
    });
    const reply = await client.call({ to: NODE_INTERFACE, data });
    if (!reply.data) return 0n;
    const [l1Gas] = decodeFunctionResult({
      abi: NODE_INTERFACE_ABI,
      functionName: "gasEstimateL1Component",
      data: reply.data,
    });
    return BigInt(l1Gas) * gasPrice * txs;
  }
  return 0n;
}

/** What a wallet holds on one network, enough to know whether it can pay. */
export interface Holding {
  network: string;
  usdc: bigint;
  gas: bigint;
}

/** The network to offer first: among those where the wallet holds the USDC
 *  and the fee, the one whose fee is lowest in dollars. Without prices, ETH
 *  fees compare among themselves; a network with no estimate comes last.
 *  Null when none can pay. */
export function cheapest(
  estimates: (FeeEstimate | null)[],
  holdings: Holding[] | null,
  amount: bigint,
): string | null {
  const fundable = estimates.filter((e): e is FeeEstimate => {
    if (e === null) return false;
    if (holdings === null) return true;
    const held = holdings.find((h) => h.network === e.network);
    return held !== undefined && held.usdc >= amount && held.gas >= e.native;
  });
  if (fundable.length === 0) return null;
  const priced = fundable.filter((e) => e.usdMicros !== null);
  if (priced.length > 0) {
    return priced.reduce((a, b) => (b.usdMicros! < a.usdMicros! ? b : a)).network;
  }
  const eth = fundable.filter((e) => e.symbol === "ETH");
  if (eth.length > 0) return eth.reduce((a, b) => (b.native < a.native ? b : a)).network;
  return fundable[0]!.network;
}

/** "about $0.003" for a fee in micro-dollars; "<$0.001" below that. */
export function formatFeeUsd(micros: bigint): string {
  if (micros < 1_000n) return "<$0.001";
  const cents = Number(micros) / 1_000_000;
  const digits = cents < 0.01 ? 3 : 2;
  return `$${cents.toFixed(digits)}`;
}
