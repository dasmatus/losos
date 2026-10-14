// The networks an edge settles USDC on, as viem sees them, and the reads the
// wallet makes on each. Every call goes through the box's relay with the
// network named, so the page talks to its own origin only (the admin CSP
// allows nothing else) and never sees an RPC URL.
import {
  createPublicClient,
  custom,
  type Address,
  type Chain,
  type PublicClient,
} from "viem";
import {
  arbitrum,
  arbitrumSepolia,
  base,
  baseSepolia,
  mainnet,
  optimism,
  optimismSepolia,
  sepolia,
} from "viem/chains";
import { normalize } from "viem/ens";
import { postMarketRpc, type UsdcChain } from "@/lib/api";
import { associatedTokenAddress } from "./solana";

const EVM: Record<string, Chain> = {
  base,
  "base-sepolia": baseSepolia,
  arbitrum,
  "arbitrum-sepolia": arbitrumSepolia,
  optimism,
  "optimism-sepolia": optimismSepolia,
};

/** Public RPCs an extension wallet may add the chain with. The page itself
 *  never calls them. */
const PUBLIC_RPC: Record<string, string> = {
  base: "https://mainnet.base.org",
  "base-sepolia": "https://sepolia.base.org",
  arbitrum: "https://arb1.arbitrum.io/rpc",
  "arbitrum-sepolia": "https://sepolia-rollup.arbitrum.io/rpc",
  optimism: "https://mainnet.optimism.io",
  "optimism-sepolia": "https://sepolia.optimism.io",
};

export function evmChain(network: string): Chain {
  const chain = EVM[network];
  if (!chain) throw new Error(`unknown network ${network}`);
  return chain;
}

export function publicRpc(network: string): string {
  return PUBLIC_RPC[network] ?? "";
}

/** A viem transport over the box's relay for one network, or `ens`. */
export function relay(network: string) {
  return custom({
    async request({ method, params }) {
      // viem asks the node to fill a transaction before it fills one itself.
      // The edge forwards no such method, so answer here the way a node
      // without it does, and viem falls back without a round trip.
      if (method === "eth_fillTransaction")
        throw Object.assign(new Error("the method eth_fillTransaction does not exist"), { code: -32601 });
      const body = await postMarketRpc(network, method, (params as unknown[] | undefined) ?? []);
      if (body.error) throw Object.assign(new Error(body.error.message ?? "chain"), { code: body.error.code });
      return body.result;
    },
  });
}

export function evmClient(network: string): PublicClient {
  return createPublicClient({ chain: evmChain(network), transport: relay(network) }) as PublicClient;
}

/** One Solana JSON-RPC call through the relay. */
export async function solanaRpc<T>(network: string, method: string, params: unknown[] = []): Promise<T> {
  const body = await postMarketRpc(network, method, params);
  if (body.error) throw new Error(body.error.message ?? "chain");
  return body.result as T;
}

const ERC20 = [
  {
    type: "function",
    name: "balanceOf",
    stateMutability: "view",
    inputs: [{ name: "owner", type: "address" }],
    outputs: [{ type: "uint256" }],
  },
  {
    type: "function",
    name: "transfer",
    stateMutability: "nonpayable",
    inputs: [
      { name: "to", type: "address" },
      { name: "amount", type: "uint256" },
    ],
    outputs: [{ type: "bool" }],
  },
] as const;

export { ERC20 };

export interface Balance {
  network: string;
  name: string;
  /** USDC base units. */
  usdc: bigint;
  /** The network fee token, in its smallest unit (wei or lamports). */
  gas: bigint;
  gasSymbol: "ETH" | "SOL";
  gasDecimals: number;
}

/** USDC and the fee token held on `chain` by `owner`, or null when the
 *  chain did not answer. A Solana owner with no USDC account holds zero. */
export async function balanceOn(chain: UsdcChain, owner: string): Promise<Balance | null> {
  try {
    if (chain.family === "evm") {
      const client = evmClient(chain.network);
      const [usdc, gas] = await Promise.all([
        client.readContract({
          address: chain.usdc as Address,
          abi: ERC20,
          functionName: "balanceOf",
          args: [owner as Address],
        }),
        client.getBalance({ address: owner as Address }),
      ]);
      return { network: chain.network, name: chain.name, usdc, gas, gasSymbol: "ETH", gasDecimals: 18 };
    }
    const account = associatedTokenAddress(owner, chain.usdc);
    const [token, lamports] = await Promise.all([
      solanaRpc<{ value: { amount: string } }>(chain.network, "getTokenAccountBalance", [
        account,
        { commitment: "confirmed" },
      ]).catch(() => ({ value: { amount: "0" } })),
      solanaRpc<{ value: number }>(chain.network, "getBalance", [owner, { commitment: "confirmed" }]),
    ]);
    return {
      network: chain.network,
      name: chain.name,
      usdc: BigInt(token.value.amount),
      gas: BigInt(lamports.value),
      gasSymbol: "SOL",
      gasDecimals: 9,
    };
  } catch {
    return null;
  }
}

function ensClient(test: boolean): PublicClient {
  return createPublicClient({
    chain: test ? sepolia : mainnet,
    transport: relay("ens"),
    // Offchain names would send the page to a gateway the admin CSP refuses;
    // names held on chain resolve without one.
    ccipRead: false,
  }) as PublicClient;
}

/** The primary ENS name of `address`, or null. */
export async function ensName(test: boolean, address: string): Promise<string | null> {
  try {
    return await ensClient(test).getEnsName({ address: address as Address });
  } catch {
    return null;
  }
}

/** The address an ENS name points at, or null when it points nowhere. */
export async function ensAddress(test: boolean, name: string): Promise<string | null> {
  try {
    return await ensClient(test).getEnsAddress({ name: normalize(name) });
  } catch {
    return null;
  }
}

/** Whether `raw` reads as an ENS name rather than an address. */
export function looksLikeName(raw: string): boolean {
  const s = raw.trim();
  return s.includes(".") && !s.startsWith("0x") && !/\s/.test(s);
}

export function formatUnits(value: bigint, decimals: number, places = 4): string {
  const scale = 10n ** BigInt(decimals);
  const whole = value / scale;
  const frac = (value % scale).toString().padStart(decimals, "0").slice(0, places).replace(/0+$/, "");
  return frac === "" ? whole.toString() : `${whole}.${frac}`;
}

/** "12.5" or "0.25" USDC as base units, or null for anything else. */
export function parseUsdc(raw: string): bigint | null {
  const m = /^\s*(\d{1,12})(?:\.(\d{1,6}))?\s*$/.exec(raw);
  if (!m) return null;
  return BigInt(m[1] ?? "0") * 1_000_000n + BigInt((m[2] ?? "").padEnd(6, "0"));
}

export function shortAddress(address: string): string {
  return address.length > 14 ? `${address.slice(0, 6)}…${address.slice(-4)}` : address;
}

/** The network names joined for a sentence: "A, B or C". */
export function listNames(names: string[], or: string): string {
  if (names.length <= 1) return names.join("");
  return `${names.slice(0, -1).join(", ")} ${or} ${names[names.length - 1]}`;
}
