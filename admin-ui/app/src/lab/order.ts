/* Ordering the hardware of a setup. Only in the copy a box serves at /lab/,
 * and only when the box has `losos.lab.ordering.enable`: lososd answers
 * GET /api/lab/order with `enabled: false` otherwise, and the Lab draws
 * nothing. The catalogue and the checkout come from the official edge
 * through lososd; payment and the shipping address are on Stripe's page. */

const TOKEN_KEY = "losos-token";

export interface CatalogueItem {
  sku: string;
  name: string;
  detail: string;
  /** Minor units of `currency`. */
  unit_amount: number;
}

export interface Catalogue {
  currency: string;
  countries: string[];
  items: CatalogueItem[];
}

export type Ordering =
  | { enabled: false }
  | { enabled: true; available: false; reason: "noOfficialEdge" | "notSold" }
  | { enabled: true; available: true; catalogue: Catalogue };

/** The device types the Lab counts for each sku the edge sells. */
export const SKU_OF_TYPE: Record<string, string> = { box: "box", "edge-local": "gateway" };

function token(): string | null {
  try {
    return window.sessionStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

/** Never throws: a box that does not answer reads as "off". */
export async function readOrdering(): Promise<Ordering> {
  const key = token();
  if (!key) return { enabled: false };
  try {
    const r = await fetch("/api/lab/order", { headers: { Authorization: "Bearer " + key }, cache: "no-store" });
    if (!r.ok) return { enabled: false };
    const body = (await r.json()) as Partial<Record<string, unknown>>;
    if (body["enabled"] !== true) return { enabled: false };
    const catalogue = body["catalogue"] as Catalogue | undefined;
    if (body["available"] === true && catalogue && Array.isArray(catalogue.items)) {
      return { enabled: true, available: true, catalogue };
    }
    const reason = body["reason"] === "noOfficialEdge" ? "noOfficialEdge" : "notSold";
    return { enabled: true, available: false, reason };
  } catch {
    return { enabled: false };
  }
}

export type CheckoutResult = { url: string } | { error: string };

/** Ask the edge for a Stripe Checkout. The URL is lososd's, already checked
 * to be a Stripe-hosted page; this checks it once more before leaving. */
export async function checkout(items: { sku: string; quantity: number }[]): Promise<CheckoutResult> {
  const key = token();
  if (!key) return { error: "signedOut" };
  try {
    const r = await fetch("/api/lab/order", {
      method: "POST",
      headers: { Authorization: "Bearer " + key, "Content-Type": "application/json" },
      body: JSON.stringify({ items }),
    });
    const body = (await r.json().catch(() => ({}))) as Partial<Record<string, unknown>>;
    const url = body["checkout_url"];
    if (r.ok && typeof url === "string" && url.startsWith("https://checkout.stripe.com/")) return { url };
    const message = body["error"];
    return { error: typeof message === "string" && message ? message : `HTTP ${r.status}` };
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
}

/** A price in minor units, in the page's language. */
export function money(minor: number, currency: string, locale: string): string {
  return new Intl.NumberFormat(locale, { style: "currency", currency: currency.toUpperCase() }).format(minor / 100);
}
