// Buying USDC with euros through Onramp.money's hosted page. The box builds
// the link itself: no key, no edge, nothing signed. Onramp.money takes the
// payment and the buyer's identity on its own site and sends USDC on Base to
// the address in the link; the wallet sees it arrive by reading its balance.
// Someone who edits the link can only send their own purchase elsewhere.

/** The page that takes the order. */
export const ONRAMP_BUY = "https://onramp.money/main/buy/";

/** Onramp.money's partner id for LosOS. 1 is the id their documentation
 *  uses for links that name no partner. */
export const ONRAMP_APP_ID = "1";

/** Their terms, and the date they took effect. A notice accepted under
 *  older terms is shown again. */
export const ONRAMP_TERMS = {
  provider: "onramp.money",
  version: "2025-11-21",
  terms: "https://onramp.money/legal/terms-and-conditions/",
  privacy: "https://onramp.money/legal/privacy-policy/",
  risk: "https://onramp.money/legal/risk-disclosure/",
} as const;

/** The euro amounts Onramp.money takes in one order. */
export const ONRAMP_MIN_EUR = 5;
export const ONRAMP_MAX_EUR = 100_000;

/** Onramp.money's codes: Base is `base`, the euro is fiat type 12. */
const NETWORK = "base";
const FIAT_EUR = "12";

/** "25" or "25.50" euros as a number in range, or null. */
export function parseEuros(raw: string): number | null {
  const m = /^\s*(\d{1,6})(?:[.,](\d{1,2}))?\s*$/.exec(raw);
  if (!m) return null;
  const value = Number(`${m[1]}.${m[2] ?? "0"}`);
  return value >= ONRAMP_MIN_EUR && value <= ONRAMP_MAX_EUR ? value : null;
}

/** The hosted page for `euros` worth of USDC on Base, sent to `address`.
 *  After the purchase Onramp.money sends the buyer to `redirect` with
 *  `orderId` and `status` added to its query. */
export function onrampLink(address: string, euros: number, redirect: string): string {
  const url = new URL(ONRAMP_BUY);
  url.searchParams.set("appId", ONRAMP_APP_ID);
  url.searchParams.set("walletAddress", address);
  url.searchParams.set("coinCode", "usdc");
  url.searchParams.set("network", NETWORK);
  url.searchParams.set("fiatType", FIAT_EUR);
  url.searchParams.set("fiatAmount", String(euros));
  url.searchParams.set("redirectUrl", redirect);
  return url.toString();
}

/** Where a USDC transfer to `address` on Base can be looked at. */
export function baseTransfers(address: string): string {
  return `https://basescan.org/address/${address}#tokentxns`;
}

/** A purchase handed to Onramp.money and not seen on chain yet. Kept in
 *  this browser's local storage so the tab Onramp.money sends back to and
 *  the one that opened it agree. */
export interface PendingTopUp {
  address: string;
  euros: number;
  /** USDC base units held on Base when the page was opened, as text; null
   *  when no reading was at hand, and the first one becomes the baseline. */
  before: string | null;
  /** Unix milliseconds. */
  at: number;
  orderId?: string;
  status?: string;
}

const PENDING = "losos-onramp-topup";
const ACCEPTED = "losos-onramp-terms";

/** A bank transfer can take a few working days; past that the page stops
 *  waiting. */
export const TOPUP_WAIT_MS = 5 * 24 * 3600 * 1000;

export function readTopUp(): PendingTopUp | null {
  try {
    const raw = window.localStorage.getItem(PENDING);
    if (raw === null) return null;
    const pending = JSON.parse(raw) as PendingTopUp;
    if (typeof pending.address !== "string" || typeof pending.at !== "number") return null;
    if (Date.now() - pending.at > TOPUP_WAIT_MS) {
      window.localStorage.removeItem(PENDING);
      return null;
    }
    return pending;
  } catch {
    return null;
  }
}

export function writeTopUp(pending: PendingTopUp | null): void {
  try {
    if (pending === null) window.localStorage.removeItem(PENDING);
    else window.localStorage.setItem(PENDING, JSON.stringify(pending));
  } catch {
    // Private windows may refuse storage; the page still opens the link.
  }
}

/** Whether this browser accepted the current terms. */
export function termsAccepted(): boolean {
  try {
    return window.localStorage.getItem(ACCEPTED) === `${ONRAMP_TERMS.provider}@${ONRAMP_TERMS.version}`;
  } catch {
    return false;
  }
}

export function acceptTerms(): void {
  try {
    window.localStorage.setItem(ACCEPTED, `${ONRAMP_TERMS.provider}@${ONRAMP_TERMS.version}`);
  } catch {
    // Asked again next time.
  }
}

/** The `orderId` and `status` Onramp.money adds when it sends the buyer
 *  back, or null when the address carries neither. */
export function returnedOrder(search: string): { orderId: string; status: string } | null {
  const q = new URLSearchParams(search);
  const orderId = q.get("orderId");
  if (orderId === null || !/^[A-Za-z0-9_-]{1,64}$/.test(orderId)) return null;
  const status = q.get("status") ?? "";
  return { orderId, status: /^[a-z]{1,16}$/.test(status) ? status : "" };
}
