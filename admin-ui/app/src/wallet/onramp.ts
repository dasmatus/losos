// Buying USDC with euros at Binance. The box asks its edge for an order
// (`POST /api/market/onramp`); the edge signs it with the partner key and
// Binance answers with its checkout. The buyer signs in to Binance there,
// passes Binance's identity checks and pays, and Binance withdraws the USDC
// on Base to this wallet. LosOS sees the order's progress, never the buyer's
// identity or payment.

import type { OnrampStage } from "@/lib/api";

/** Binance's legal documents, and the day this page was written against
 *  them. Buyers open each one before they can accept; acceptance is kept per
 *  version, so new documents are agreed to again. */
export const BINANCE_TERMS = {
  provider: "binance",
  version: "2026-10-21",
  documents: [
    { key: "terms", href: "https://www.binance.com/en/terms" },
    { key: "product", href: "https://www.binance.com/en/about-legal/product-terms" },
    { key: "privacy", href: "https://www.binance.com/en/about-legal/privacy-portal" },
    { key: "risk", href: "https://www.binance.com/en/risk-warning" },
  ],
} as const;

export type TermsDocument = (typeof BINANCE_TERMS.documents)[number]["key"];

/** The euro amounts the edge passes on. Binance shows its own limits for
 *  the payment method on its page. */
export const ONRAMP_MIN_EUR = 1;
export const ONRAMP_MAX_EUR = 100_000;

/** "25", "25.5" or "25,50" euros as the edge's decimal text, or null. */
export function parseEuros(raw: string): string | null {
  const m = /^\s*(\d{1,6})(?:[.,](\d{1,2}))?\s*$/.exec(raw);
  if (!m) return null;
  const value = Number(`${m[1]}.${m[2] ?? "0"}`);
  if (value < ONRAMP_MIN_EUR || value > ONRAMP_MAX_EUR) return null;
  return m[2] === undefined ? String(Number(m[1])) : `${Number(m[1])}.${m[2]}`;
}

/** Whether Binance is done with the order, one way or another. */
export function finished(stage: OnrampStage): boolean {
  return stage === "done" || stage === "kept" || stage === "failed";
}

/** Where the withdrawal can be looked at. */
export function baseTx(tx: string): string {
  return `https://basescan.org/tx/${tx}`;
}

/** The order this browser is following, so a reload or another tab picks
 *  it up. */
export interface PendingOrder {
  address: string;
  orderId: string;
  /** Unix milliseconds. */
  at: number;
}

const PENDING = "losos-binance-order";
const ACCEPTED = "losos-binance-terms";

/** A purchase can wait days for a bank transfer; past that the page stops
 *  following it. */
export const ORDER_WAIT_MS = 7 * 24 * 3600 * 1000;

export function readOrder(): PendingOrder | null {
  try {
    const raw = window.localStorage.getItem(PENDING);
    if (raw === null) return null;
    const pending = JSON.parse(raw) as PendingOrder;
    if (typeof pending.address !== "string" || typeof pending.orderId !== "string" || typeof pending.at !== "number") {
      return null;
    }
    if (Date.now() - pending.at > ORDER_WAIT_MS) {
      window.localStorage.removeItem(PENDING);
      return null;
    }
    return pending;
  } catch {
    return null;
  }
}

export function writeOrder(pending: PendingOrder | null): void {
  try {
    if (pending === null) window.localStorage.removeItem(PENDING);
    else window.localStorage.setItem(PENDING, JSON.stringify(pending));
  } catch {
    // Private windows may refuse storage; the row still follows the order
    // while the page is open.
  }
}

/** Keep that this browser's owner accepted this version of the terms, and
 *  when. */
export function recordAcceptance(): void {
  try {
    window.localStorage.setItem(
      ACCEPTED,
      JSON.stringify({ terms: `${BINANCE_TERMS.provider}@${BINANCE_TERMS.version}`, at: new Date().toISOString() }),
    );
  } catch {
    // Nothing else depends on it.
  }
}
