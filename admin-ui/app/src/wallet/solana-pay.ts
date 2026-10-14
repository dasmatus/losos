// Sending USDC on Solana through the box's relay: a recent blockhash, the
// transaction solana.ts builds, the send, and the wait for `confirmed`, the
// commitment the edge reads a payment back at.
import { base58 } from "@scure/base";
import { solanaRpc } from "./chains";
import { signTransaction, usdcTransferMessage, type SolanaKey, type UsdcTransfer } from "./solana";

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Phantom, Solflare and Backpack inject this shape. */
export interface SolanaInjected {
  connect(): Promise<{ publicKey: { toString(): string } }>;
  signMessage(message: Uint8Array, display?: "utf8"): Promise<{ signature: Uint8Array }>;
  request(args: { method: string; params?: unknown }): Promise<unknown>;
}

export function solanaInjected(): SolanaInjected | null {
  const w = window as unknown as { phantom?: { solana?: SolanaInjected }; solana?: SolanaInjected };
  return w.phantom?.solana ?? w.solana ?? null;
}

/** Who signs: the browser wallet's key, or an extension. */
export type SolanaSigner = { kind: "local"; key: SolanaKey } | { kind: "injected"; address: string; provider: SolanaInjected };

export function signerAddress(signer: SolanaSigner): string {
  return signer.kind === "local" ? signer.key.address : signer.address;
}

/** Build, sign and send one USDC transfer; the signature is its id. */
export async function sendUsdc(
  network: string,
  signer: SolanaSigner,
  transfer: Omit<UsdcTransfer, "payer" | "recentBlockhash">,
): Promise<string> {
  const { value } = await solanaRpc<{ value: { blockhash: string } }>(network, "getLatestBlockhash", [
    { commitment: "confirmed" },
  ]);
  const message = usdcTransferMessage({
    ...transfer,
    payer: signerAddress(signer),
    recentBlockhash: value.blockhash,
  });
  if (signer.kind === "injected") {
    // The extension sends it on its own cluster.
    const reply = (await signer.provider.request({
      method: "signAndSendTransaction",
      params: { message: base58.encode(message) },
    })) as { signature: string };
    return reply.signature;
  }
  const { wire, signature } = signTransaction(signer.key, message);
  await solanaRpc(network, "sendTransaction", [wire, { encoding: "base64", preflightCommitment: "confirmed" }]);
  return signature;
}

/** Wait until the cluster has the transaction at `confirmed`. Throws when it
 *  failed, or when it never shows up (its blockhash expires in about a
 *  minute, after which it can no longer land). */
export async function waitConfirmed(network: string, signature: string, timeoutMs = 90_000): Promise<void> {
  const until = Date.now() + timeoutMs;
  while (Date.now() < until) {
    const { value } = await solanaRpc<{
      value: ({ err: unknown; confirmationStatus?: string } | null)[];
    }>(network, "getSignatureStatuses", [[signature], { searchTransactionHistory: true }]);
    const status = value[0];
    if (status?.err) throw new Error("the transaction failed");
    if (status && (status.confirmationStatus === "confirmed" || status.confirmationStatus === "finalized")) return;
    await sleep(1_000);
  }
  throw new Error("the transaction did not land");
}
