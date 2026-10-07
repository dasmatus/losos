import * as React from "react";
import {
  ApiError,
  getMarket,
  isAbort,
  isUnauthorized,
  postMarketClose,
  postMarketListing,
  postMarketOnboard,
  postMarketOrder,
  type MarketAccount,
  type MarketActionResponse,
  type MarketKind,
  type MarketOrder,
  type MarketShelfListing,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { intlTag, t, type MessageKey } from "@/lib/i18n";

/* The status a purchase raises while Stripe is open in the other tab;
 * the "payment received" confirmation settles it (pane-market.tsx raises it). */
export const ORDER_STATUS = "market-order";

/* The market pane's data.
 *
 * The market is optional at three levels (the edge, this box's tenant entry,
 * the seller's Stripe account), so most boxes will meet `unavailable`. That is
 * a quiet state with a sentence, not an error: lososd answers it as a 200 for
 * exactly this reason, because the shell latches a 404 as "this box does not
 * serve the route" for the whole session.
 *
 * Money is whole minor units on the wire (cents) and decimal text in the
 * field; the two conversions are here so the pane never does arithmetic on a
 * float it will then send. */

export type MarketState =
  | { kind: "loading" }
  | { kind: "unavailable"; reason?: "noOfficialEdge" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; listings: MarketShelfListing[]; account: MarketAccount };

export interface MarketData {
  state: MarketState;
  /** True while an action is in flight. */
  busy: boolean;
  refresh: () => void;
  onboard: () => Promise<MarketActionResponse | null>;
  list: (kind: MarketKind, priceMinor: number, capacity: number) => Promise<boolean>;
  close: (listingId: string) => Promise<boolean>;
  order: (listingId: string, quantity: number) => Promise<MarketActionResponse | null>;
}

const KIND_NAME: Record<MarketKind, MessageKey> = {
  storage: "panes.market.kind.storage",
  compute: "panes.market.kind.compute",
};

const UNIT_NAME: Record<string, MessageKey> = {
  "GiB-month": "panes.market.unit.storage",
  "vCPU-hour": "panes.market.unit.compute",
};

/* Payment happens on Stripe's page in another tab, and the box learns of it
 * from the edge, so this pane only ever sees a purchase go from pending to
 * paid on a refresh. That moment is the "purchase done" the owner came back
 * for: compare the statuses the previous answer carried and say so once per
 * order. The first answer of a session seeds the map and announces nothing;
 * a purchase paid last week is not news. */
function announcePaid(previous: Map<string, MarketOrder["status"]>, account: MarketAccount): void {
  for (const order of account.purchases) {
    const before = previous.get(order.id);
    if (before !== undefined && before !== "paid" && order.status === "paid") {
      const unitKey = UNIT_NAME[order.unit];
      toast.success(
        t("panes.market.paidTitle"),
        t("panes.market.paidBody", {
          kind: t(KIND_NAME[order.kind]),
          quantity: order.quantity,
          unit: unitKey === undefined ? order.unit : t(unitKey),
        }),
        // Settles the "order placed, finish paying" status the buy raised.
        { settles: ORDER_STATUS },
      );
    }
  }
}

function statusesOf(account: MarketAccount): Map<string, MarketOrder["status"]> {
  return new Map(account.purchases.map((order) => [order.id, order.status]));
}

export function useMarket(enabled: boolean): MarketData {
  const [state, setState] = React.useState<MarketState>({ kind: "loading" });
  const [busy, setBusy] = React.useState(false);
  const [tick, setTick] = React.useState(0);
  const seen = React.useRef<Map<string, MarketOrder["status"]> | null>(null);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const reply = await getMarket({ signal: controller.signal });
        if (controller.signal.aborted) return;
        if (reply.available) {
          if (seen.current !== null) announcePaid(seen.current, reply.account);
          seen.current = statusesOf(reply.account);
        }
        setState(
          reply.available
            ? { kind: "ready", listings: reply.listings, account: reply.account }
            : { kind: "unavailable", reason: reply.reason },
        );
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        setState({ kind: "failed", message: error instanceof Error ? error.message : "" });
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  /* One in-flight action at a time: each one changes money or what is for
   * sale, and a double click must not become two orders. A refusal or a
   * failure is a toast carrying the edge's own sentence when it sent one. */
  const act = React.useCallback(
    async <T,>(fn: () => Promise<T>, done?: () => void): Promise<T | null> => {
      setBusy(true);
      try {
        const out = await fn();
        done?.();
        refresh();
        return out;
      } catch (error) {
        if (!isUnauthorized(error)) {
          const said = error instanceof ApiError || error instanceof Error ? error.message : "";
          toast.error(
            t("panes.market.actionFailedTitle"),
            said.length > 0 ? said : t("panes.market.actionFailed"),
            /* The market answers 503 while its edge is out of reach; anything
             * else has no page of its own yet. */
            { help: error instanceof ApiError && error.status === 503 ? "edge-not-found" : "index" },
          );
        }
        return null;
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  return {
    state,
    busy,
    refresh,
    onboard: () => act(() => postMarketOnboard()),
    list: async (kind, priceMinor, capacity) =>
      (await act(
        () => postMarketListing({ kind, unit_price: priceMinor, capacity }),
        () => toast.success(t("panes.market.listedTitle"), t("panes.market.listedBody")),
      )) !== null,
    close: async (id) =>
      (await act(
        () => postMarketClose(id),
        () => toast.done(t("panes.market.closedTitle")),
      )) !== null,
    order: (id, quantity) => act(() => postMarketOrder(id, quantity)),
  };
}

// ── Money ─────────────────────────────────────────────────────────────────

/** Decimal places in `currency`'s minor unit (2 for EUR, 0 for JPY, 3 for
 *  KWD), from the browser's ISO 4217 table. Stripe and the edge both count
 *  prices in that minor unit, so this is the one place the scale is decided.
 *  The edge only accepts two-decimal currencies today (`SUPPORTED_CURRENCIES`
 *  in backend-registrar/src/market.rs); this keeps the pane right if that
 *  list grows rather than relying on it. */
export function minorDigits(currency: string): number {
  try {
    return (
      new Intl.NumberFormat("en", {
        style: "currency",
        currency: currency.toUpperCase(),
      }).resolvedOptions().maximumFractionDigits ?? 2
    );
  } catch {
    return 2;
  }
}

/** "12", "12.5", "12,50" in EUR → 1200, 1250, 1250; "1200" in JPY → 1200.
 *  Null for anything else, including more decimals than the currency has,
 *  and zero. */
export function toMinorUnits(text: string, currency: string): number | null {
  const digits = minorDigits(currency);
  const fraction = digits > 0 ? `(?:[.,](\\d{1,${digits}}))?` : "";
  const match = new RegExp(`^(\\d{1,7})${fraction}$`).exec(text.trim());
  if (match === null) return null;
  const whole = Number(match[1]);
  const part = Number((match[2] ?? "").padEnd(digits, "0") || "0");
  const total = whole * 10 ** digits + part;
  return total > 0 ? total : null;
}

export function formatMoney(minor: number, currency: string): string {
  const digits = minorDigits(currency);
  const major = minor / 10 ** digits;
  try {
    return new Intl.NumberFormat(intlTag(), {
      style: "currency",
      currency: currency.toUpperCase(),
    }).format(major);
  } catch {
    return `${major.toFixed(digits)} ${currency.toUpperCase()}`;
  }
}

export function formatDay(epochSeconds: number): string {
  return new Intl.DateTimeFormat(intlTag(), { dateStyle: "medium" }).format(
    new Date(epochSeconds * 1000),
  );
}

/** The hosts a payment or onboarding tab may be sent to. Mirrors
 *  `STRIPE_HOSTS` in backend/src/market.rs, which already refuses anything
 *  else; this is the second check, so a `javascript:` link or a look-alike
 *  card form never gets as far as a window. */
const STRIPE_HOSTS: ReadonlySet<string> = new Set(["checkout.stripe.com", "connect.stripe.com"]);

export function isStripePage(url: unknown): url is string {
  if (typeof url !== "string") return false;
  try {
    const parsed = new URL(url);
    return (
      parsed.protocol === "https:" &&
      parsed.username === "" &&
      parsed.password === "" &&
      parsed.port === "" &&
      STRIPE_HOSTS.has(parsed.hostname)
    );
  } catch {
    return false;
  }
}
