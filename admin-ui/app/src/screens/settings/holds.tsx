import { Button } from "@/components/ui/button";
import type { MarketOrder, MarketOwnListing, MarketPayouts, RefundReason } from "@/lib/api";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { Group, GroupTitle, PaneSection, Row, RowText, RowValue } from "./rows";
import { formatDay, formatMoney } from "./market";

/* Market payments are held until the buyer's period ends (the edge's
 * `slots.rs`): a card payment on the platform's Stripe balance, a USDC one in
 * the escrow. These are the pieces both the Market and the Machines panes
 * show of that: a listing someone has paid for cannot be closed, every order
 * says where its money stands, and the seller sees what is held and pays out
 * what is ready. */

const REASONS: readonly RefundReason[] = [
  "sellerCannotReceive",
  "alreadyPaid",
  "lapsed",
  "buyerReclaimed",
  "notDelivered",
  "sellerOffline",
];

/** " · paid for until 12 Nov" on a listing someone has paid for. */
export function lockedSuffix(t: ReturnType<typeof useT>, listing: MarketOwnListing): string {
  return listing.locked_until ? ` · ${t("panes.market.lockedUntil", { date: formatDay(listing.locked_until) })}` : "";
}

/** Stop selling, greyed while a paid period runs on the listing. */
export function StopSelling({
  listing,
  disabled,
  onClose,
}: {
  listing: MarketOwnListing;
  disabled: boolean;
  onClose: () => void;
}) {
  const t = useT();
  const locked = listing.locked_until != null;
  return (
    <Button
      size="sm"
      variant="secondary"
      disabled={disabled || locked}
      title={locked ? t("panes.market.lockedHint", { date: formatDay(listing.locked_until ?? 0) }) : undefined}
      onClick={onClose}
    >
      {t("panes.market.closeButton")}
    </Button>
  );
}

/** Where an order's held money stands, from the card hold or the escrow.
 *  Nothing for an order with neither (a coin payment, or not paid). */
export function HoldNote({ order }: { order: MarketOrder }) {
  const t = useT();
  const hold = order.card ?? order.escrow ?? null;
  if (hold === null) return null;
  const refund = formatMoney(hold.refund ?? 0, order.currency);
  // The edge keeps a payment `held` until it is paid out, also after the
  // period is over; say which of the two it is.
  const over = hold.step === "held" && order.expires_at !== null && order.expires_at * 1000 <= Date.now();
  const step = over
    ? t("panes.market.escrow.over")
    : t(`panes.market.escrow.${hold.step}` as MessageKey, { refund });
  const reason =
    hold.reason != null && REASONS.includes(hold.reason) && hold.step !== "release" && hold.step !== "released"
      ? t(`panes.market.escrow.${hold.reason}` as MessageKey)
      : null;
  return (
    <p className="mt-0.5 text-[12px] whitespace-normal text-faint" data-testid="market-hold">
      {step}
      {reason !== null && ` · ${reason}`}
    </p>
  );
}

/** What this box's sales hold for it, and Pay out for what is ready. Shown
 *  once anything has been held. */
export function EarningsSection({
  payouts,
  currency,
  offlineSecs,
  disabled,
  onPayOut,
}: {
  payouts: MarketPayouts;
  currency: string;
  offlineSecs: number;
  disabled: boolean;
  onPayOut: () => void;
}) {
  const t = useT();
  const hours = Math.max(1, Math.round(offlineSecs / 3600));
  return (
    <PaneSection>
      <GroupTitle>{t("panes.market.earnings.title")}</GroupTitle>
      <Group data-testid="market-earnings">
        <Row>
          <RowText title={t("panes.market.earnings.held")} detail={t("panes.market.earnings.heldDetail", { hours })} />
          <RowValue className="numeric">{formatMoney(payouts.held, currency)}</RowValue>
        </Row>
        <Row last={payouts.paying === 0}>
          <RowText
            title={t("panes.market.earnings.ready")}
            detail={
              payouts.ready === 0 && payouts.next_at
                ? t("panes.market.earnings.waitDetail", { date: formatDay(payouts.next_at) })
                : t("panes.market.earnings.readyDetail")
            }
          />
          <div className="flex items-center gap-3">
            <RowValue className="numeric">{formatMoney(payouts.ready, currency)}</RowValue>
            <Button size="sm" disabled={disabled || payouts.ready === 0} onClick={onPayOut}>
              {t("panes.market.earnings.payOut")}
            </Button>
          </div>
        </Row>
        {payouts.paying > 0 && (
          <Row last>
            <RowText title={t("panes.market.earnings.paying")} />
            <RowValue className="numeric">{formatMoney(payouts.paying, currency)}</RowValue>
          </Row>
        )}
      </Group>
    </PaneSection>
  );
}
