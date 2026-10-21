// Paying for an AI model by the token: the buyer's side of the escrow's
// prepaid balances and taps (LososEscrow on an EVM chain, the losos-escrow
// program on Solana). The wallet tops up a balance of its own, opens one tap
// per lending box from the edge's quote, and later asks for what is left
// back, which it can take after the escrow's 48-hour notice. The edge bills
// the taps from the balance and closes them when the owner stops; the
// wallet signs nothing for that.
import { createWalletClient, custom, type Address, type HDAccount } from "viem";
import type { TapQuote, UsdcChain } from "@/lib/api";
import { evmChain, evmClient, publicRpc, relay, solanaRpc } from "./chains";
import { lowFeeFields } from "./fees";
import { escrowBalanceVault } from "./solana";
import { askWithdrawal, openTaps as openSolanaTaps, waitConfirmed, withdraw as withdrawSolana, type SolanaSigner } from "./solana-pay";

const ESCROW = [
  {
    type: "function",
    name: "topUp",
    stateMutability: "nonpayable",
    inputs: [{ name: "amount", type: "uint256" }],
    outputs: [],
  },
  {
    type: "function",
    name: "openTaps",
    stateMutability: "nonpayable",
    inputs: [
      { name: "ids", type: "bytes32[]" },
      { name: "sellers", type: "address[]" },
      { name: "rates", type: "uint128[]" },
      { name: "feeBps", type: "uint16" },
    ],
    outputs: [],
  },
  {
    type: "function",
    name: "askWithdrawal",
    stateMutability: "nonpayable",
    inputs: [{ name: "amount", type: "uint256" }],
    outputs: [],
  },
  {
    type: "function",
    name: "withdraw",
    stateMutability: "nonpayable",
    inputs: [],
    outputs: [],
  },
  {
    type: "function",
    name: "balances",
    stateMutability: "view",
    inputs: [{ name: "buyer", type: "address" }],
    outputs: [{ type: "uint256" }],
  },
] as const;

const USDC = [
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
  {
    type: "function",
    name: "approve",
    stateMutability: "nonpayable",
    inputs: [
      { name: "spender", type: "address" },
      { name: "amount", type: "uint256" },
    ],
    outputs: [{ type: "bool" }],
  },
] as const;

/** A browser extension's EVM provider. */
export interface Injected {
  request: (args: { method: string; params?: unknown[] }) => Promise<unknown>;
}

export function injectedEvm(): Injected | null {
  return (window as unknown as { ethereum?: Injected }).ethereum ?? null;
}

/** Who signs: the wallet saved on this box, or a browser extension. */
export type TapSigner =
  | { family: "evm"; kind: "local"; account: HDAccount }
  | { family: "evm"; kind: "injected"; address: string; eth: Injected }
  | { family: "solana"; signer: SolanaSigner };

export function signerAddress(s: TapSigner): string {
  if (s.family === "evm") return s.kind === "local" ? s.account.address : s.address;
  return s.signer.kind === "local" ? s.signer.key.address : s.signer.address;
}

/** The escrow the chain's payments go through, or null without one. */
function escrowOf(chain: UsdcChain): string {
  if (!chain.pay) throw new Error("this network has no escrow on this edge");
  return chain.pay;
}

/** What `owner` holds in the escrow on `chain`, base units. Zero for a
 *  wallet that never topped up. */
export async function escrowBalance(chain: UsdcChain, owner: string): Promise<bigint> {
  const escrow = escrowOf(chain);
  if (chain.family === "evm") {
    return evmClient(chain.network).readContract({
      address: escrow as Address,
      abi: ESCROW,
      functionName: "balances",
      args: [owner as Address],
    });
  }
  try {
    const vault = escrowBalanceVault(escrow, chain.treasury, chain.usdc, owner);
    const { value } = await solanaRpc<{ value: { amount: string } }>(chain.network, "getTokenAccountBalance", [
      vault,
      { commitment: "confirmed" },
    ]);
    return BigInt(value.amount);
  } catch {
    // No vault yet: nothing topped up.
    return 0n;
  }
}

/** Point an extension at `network`, adding it when the extension does not
 *  know it. */
async function switchTo(eth: Injected, network: string): Promise<void> {
  const viemChain = evmChain(network);
  const hexId = `0x${viemChain.id.toString(16)}`;
  try {
    await eth.request({ method: "wallet_switchEthereumChain", params: [{ chainId: hexId }] });
  } catch {
    await eth.request({
      method: "wallet_addEthereumChain",
      params: [
        {
          chainId: hexId,
          chainName: viemChain.name,
          nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
          rpcUrls: [publicRpc(network)],
        },
      ],
    });
  }
}

/** A viem wallet client for one EVM signer on `network`, through the box's
 *  relay unless an extension sends it itself. */
async function evmWallet(s: Extract<TapSigner, { family: "evm" }>, network: string) {
  const chain = evmChain(network);
  if (s.kind === "injected") await switchTo(s.eth, network);
  const account = s.kind === "local" ? s.account : (s.address as Address);
  const client = createWalletClient({
    account,
    chain,
    transport: s.kind === "injected" ? custom(s.eth) : relay(network),
  });
  return { client, account, chain };
}

async function mined(network: string, hash: `0x${string}`): Promise<void> {
  const receipt = await evmClient(network).waitForTransactionReceipt({ hash, timeout: 120_000 });
  if (receipt.status !== "success") throw new Error("the transaction failed");
}

/** Top up the balance by `topUp` (approving USDC first when the allowance
 *  is short) and open every tap of `quote`. Waits for each transaction to
 *  land, so the edge can read the taps back next. The transactions, in
 *  order. */
export async function startTaps(chain: UsdcChain, s: TapSigner, quote: TapQuote, topUp: bigint): Promise<string[]> {
  const escrow = escrowOf(chain);
  if (s.family === "solana") {
    const sent = await openSolanaTaps(chain.network, s.signer, {
      program: escrow,
      mint: chain.usdc,
      treasury: chain.treasury,
      topUp,
      feeBps: quote.feeBps,
      taps: quote.taps.map((t) => ({ key: t.key, seller: t.seller, rate: BigInt(t.rate) })),
    });
    for (const signature of sent) await waitConfirmed(chain.network, signature);
    return sent;
  }
  const { client, account, chain: viemChain } = await evmWallet(s, chain.network);
  const owner = signerAddress(s) as Address;
  const pub = evmClient(chain.network);
  const sent: string[] = [];
  if (topUp > 0n) {
    const allowed = await pub.readContract({
      address: chain.usdc as Address,
      abi: USDC,
      functionName: "allowance",
      args: [owner, escrow as Address],
    });
    if (allowed < topUp) {
      const approval = await client.writeContract({
        address: chain.usdc as Address,
        abi: USDC,
        functionName: "approve",
        args: [escrow as Address, topUp],
        account,
        chain: viemChain,
        ...(await lowFeeFields(chain.network)),
      });
      // The top-up's gas estimate runs against the chain as it is.
      await mined(chain.network, approval);
      sent.push(approval);
    }
    const hash = await client.writeContract({
      address: escrow as Address,
      abi: ESCROW,
      functionName: "topUp",
      args: [topUp],
      account,
      chain: viemChain,
      ...(await lowFeeFields(chain.network)),
    });
    await mined(chain.network, hash);
    sent.push(hash);
  }
  const hash = await client.writeContract({
    address: escrow as Address,
    abi: ESCROW,
    functionName: "openTaps",
    args: [
      quote.taps.map((t) => t.key as `0x${string}`),
      quote.taps.map((t) => t.seller as Address),
      quote.taps.map((t) => BigInt(t.rate)),
      quote.feeBps,
    ],
    account,
    chain: viemChain,
    ...(await lowFeeFields(chain.network)),
  });
  await mined(chain.network, hash);
  sent.push(hash);
  return sent;
}

/** Ask to take `amount` out of the balance once the notice is over. */
export async function askBack(chain: UsdcChain, s: TapSigner, amount: bigint): Promise<string> {
  const escrow = escrowOf(chain);
  if (s.family === "solana") {
    const signature = await askWithdrawal(chain.network, s.signer, { program: escrow, mint: chain.usdc, treasury: chain.treasury }, amount);
    await waitConfirmed(chain.network, signature);
    return signature;
  }
  const { client, account, chain: viemChain } = await evmWallet(s, chain.network);
  const hash = await client.writeContract({
    address: escrow as Address,
    abi: ESCROW,
    functionName: "askWithdrawal",
    args: [amount],
    account,
    chain: viemChain,
    ...(await lowFeeFields(chain.network)),
  });
  await mined(chain.network, hash);
  return hash;
}

/** Take what was asked for back into the wallet. */
export async function takeBack(chain: UsdcChain, s: TapSigner): Promise<string> {
  const escrow = escrowOf(chain);
  if (s.family === "solana") {
    const signature = await withdrawSolana(chain.network, s.signer, { program: escrow, mint: chain.usdc, treasury: chain.treasury });
    await waitConfirmed(chain.network, signature);
    return signature;
  }
  const { client, account, chain: viemChain } = await evmWallet(s, chain.network);
  const hash = await client.writeContract({
    address: escrow as Address,
    abi: ESCROW,
    functionName: "withdraw",
    args: [],
    account,
    chain: viemChain,
    ...(await lowFeeFields(chain.network)),
  });
  await mined(chain.network, hash);
  return hash;
}
