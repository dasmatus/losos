import * as React from "react";
import { Button } from "@/components/ui/button";
import { FieldError, Input, Select } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import type { MarketKind, MarketOrder, MarketShelfListing } from "@/lib/api";
import { t as translate, type MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";
import { formatDay, formatMoney, isStripePage, toMinorUnits, useMarket, type MarketData } from "./market";
import type { SettingsForm } from "./use-settings-form";

/* Buying and selling what the mesh already shares.
 *
 * This is a way to be paid for the storage and compute a box contributes, not
 * a separate product, so the sell side only offers what the owner is already
 * sharing (the edge enforces it; this pane greys out the rest and says why).
 * Payment happens on Stripe's own page in a new tab — nothing on this page
 * ever sees a card — so after a purchase the pane tells the owner to come back
 * and refresh rather than pretending to know the outcome.
 */

const KIND_LABEL: Record<MarketKind, MessageKey> = {
  storage: "panes.market.kind.storage",
  compute: "panes.market.kind.compute",
};

const UNIT_LABEL: Record<string, MessageKey> = {
  "GiB-month": "panes.market.unit.storage",
  "vCPU-hour": "panes.market.unit.compute",
};

function unitName(unit: string): string {
  const key = UNIT_LABEL[unit];
  return key === undefined ? unit : translate(key);
}

export function MarketPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const market = useMarket(!form.locked);
  const { state } = market;

  if (state.kind === "loading") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("panes.market.loading")} />
            <Spinner size={16} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "failed") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("panes.market.failed")} detail={state.message} />
            <Button size="sm" variant="secondary" onClick={market.refresh}>
              {t("panes.market.refresh")}
            </Button>
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "unavailable") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText
              title={t("panes.market.unavailable.title")}
              detail={t("panes.market.unavailable.detail")}
            />
          </Row>
        </Group>
      </PaneSection>
    );
  }

  const { account, listings } = state;
  const disabled = form.locked || market.busy;
  return (
    <>
      {market.actionError !== null && (
        <p role="alert" className="mb-3 text-[13px] text-crit">
          {market.actionError.length > 0 ? market.actionError : t("panes.market.actionFailed")}
        </p>
      )}

      <PaneSection>
        <GroupTitle>{t("panes.market.yours")}</GroupTitle>
        <Group>
          <Row>
            <RowText
              title={t("panes.market.storageBought")}
              detail={t("panes.market.storageBoughtDetail")}
            />
            <span className="text-sm text-ink">
              {t("panes.market.gib", { count: account.entitlements.storage_gib })}
            </span>
          </Row>
          <Row last>
            <RowText
              title={t("panes.market.computeBought")}
              detail={t("panes.market.computeBoughtDetail")}
            />
            <span className="text-sm text-ink">
              {t("panes.market.hours", { count: account.entitlements.compute_vcpu_hours })}
            </span>
          </Row>
        </Group>
        {account.purchases.length > 0 && (
          <Group className="mt-3">
            {account.purchases.map((order, i) => (
              <PurchaseRow key={order.id} order={order} last={i === account.purchases.length - 1} />
            ))}
          </Group>
        )}
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.market.buy")}</GroupTitle>
        <Group>
          {listings.length === 0 ? (
            <Row last>
              <RowText title={t("panes.market.shelfEmpty")} />
              <Button size="sm" variant="secondary" disabled={disabled} onClick={market.refresh}>
                {t("panes.market.refresh")}
              </Button>
            </Row>
          ) : (
            listings.map((listing, i) => (
              <ShelfRow
                key={listing.id}
                listing={listing}
                market={market}
                last={i === listings.length - 1}
              />
            ))
          )}
        </Group>
        <GroupCaption>{t("panes.market.payCaption")}</GroupCaption>
      </PaneSection>

      <SellSection market={market} />
    </>
  );
}

function PurchaseRow({ order, last }: { order: MarketOrder; last: boolean }) {
  const t = useT();
  const status =
    order.status === "paid" && order.expired
      ? t("panes.market.status.lapsed")
      : t(`panes.market.status.${order.status}` as MessageKey);
  const bits = [
    `${order.quantity} ${unitName(order.unit)}`,
    order.status === "paid" && order.expires_at !== null && !order.expired
      ? t("panes.market.until", { date: formatDay(order.expires_at) })
      : null,
    order.volume !== null ? t("panes.market.volume", { volume: order.volume }) : null,
  ].filter((x): x is string => x !== null);
  return (
    <Row last={last}>
      <RowText
        title={`${t(KIND_LABEL[order.kind])} · ${formatMoney(order.amount, order.currency)}`}
        detail={bits.join(" · ")}
      />
      <span className="text-[13px] text-muted">{status}</span>
    </Row>
  );
}

function ShelfRow({
  listing,
  market,
  last,
}: {
  listing: MarketShelfListing;
  market: MarketData;
  last: boolean;
}) {
  const t = useT();
  const qtyId = React.useId();
  const [quantity, setQuantity] = React.useState("1");
  const [problem, setProblem] = React.useState<string | null>(null);
  const [opened, setOpened] = React.useState(false);

  const buy = async () => {
    const n = Number(quantity);
    if (!Number.isInteger(n) || n < 1 || n > listing.available) {
      setProblem(t("panes.market.quantityRange", { max: listing.available }));
      return;
    }
    setProblem(null);
    const checkoutTab = window.open("about:blank", "_blank");
    if (checkoutTab === null) {
      setProblem(t("panes.market.popupBlocked"));
      return;
    }
    checkoutTab.opener = null;
    const reply = await market.order(listing.id, n);
    if (reply !== null) {
      if (isStripePage(reply.checkout_url)) {
        checkoutTab.location.href = reply.checkout_url;
        setOpened(true);
      } else {
        checkoutTab.close();
        setProblem(t("panes.market.noCheckout"));
      }
    } else {
      checkoutTab.close();
    }
  };

  return (
    <StackRow last={last}>
      <div className="flex items-center justify-between gap-4">
        <RowText
          title={`${t(KIND_LABEL[listing.kind])} · ${formatMoney(listing.unit_price, listing.currency)} / ${unitName(listing.unit)}`}
          detail={t("panes.market.available", { count: listing.available })}
        />
        <div className="flex items-center gap-2">
          <label htmlFor={qtyId} className="sr-only">
            {t("panes.market.quantity")}
          </label>
          <Input
            id={qtyId}
            inputMode="numeric"
            className="w-20"
            value={quantity}
            disabled={market.busy}
            onChange={(e) => setQuantity(e.target.value)}
          />
          <Button size="sm" disabled={market.busy} onClick={() => void buy()}>
            {t("panes.market.buyButton")}
          </Button>
        </div>
      </div>
      {problem !== null && <FieldError>{problem}</FieldError>}
      {opened && <p className="text-[12.5px] text-muted">{t("panes.market.paying")}</p>}
    </StackRow>
  );
}

function SellSection({ market }: { market: MarketData }) {
  const t = useT();
  const kindId = React.useId();
  const priceId = React.useId();
  const capId = React.useId();
  const state = market.state;
  const account = state.kind === "ready" ? state.account : null;
  const canStorage = account?.can_sell_storage ?? false;
  const canCompute = account?.can_sell_compute ?? false;
  const [kind, setKind] = React.useState<MarketKind>("storage");
  const [price, setPrice] = React.useState("");
  const [capacity, setCapacity] = React.useState("");
  const [problem, setProblem] = React.useState<string | null>(null);

  if (account === null) return null;

  const effectiveKind: MarketKind =
    kind === "storage" && !canStorage && canCompute ? "compute" : kind;
  const sellable = effectiveKind === "storage" ? canStorage : canCompute;
  const feePercent = account.fee_bps / 100;

  const onboard = async () => {
    const onboardingTab = window.open("about:blank", "_blank");
    if (onboardingTab === null) {
      setProblem(t("panes.market.popupBlocked"));
      return;
    }
    onboardingTab.opener = null;
    const reply = await market.onboard();
    if (reply !== null && isStripePage(reply.url)) onboardingTab.location.href = reply.url;
    else onboardingTab.close();
  };

  const submit = async () => {
    const minor = toMinorUnits(price);
    const cap = Number(capacity);
    if (minor === null) return setProblem(t("panes.market.priceInvalid"));
    if (!Number.isInteger(cap) || cap < 1) return setProblem(t("panes.market.capacityInvalid"));
    setProblem(null);
    if (await market.list(effectiveKind, minor, cap)) {
      setPrice("");
      setCapacity("");
    }
  };

  return (
    <PaneSection>
      <GroupTitle>{t("panes.market.sell")}</GroupTitle>
      <Group>
        <Row last={!account.seller_ready}>
          <RowText
            title={t("panes.market.payouts")}
            detail={
              account.seller_ready
                ? t("panes.market.payoutsReady")
                : account.seller_onboarded
                  ? t("panes.market.payoutsPending")
                  : t("panes.market.payoutsNone")
            }
          />
          {!account.seller_ready && (
            <Button size="sm" disabled={market.busy} onClick={() => void onboard()}>
              {account.seller_onboarded
                ? t("panes.market.payoutsContinue")
                : t("panes.market.payoutsStart")}
            </Button>
          )}
        </Row>

        {account.seller_ready && (
          <StackRow last={account.listings.length === 0}>
            {!canStorage && !canCompute ? (
              <p className="text-[13px] text-muted">{t("panes.market.nothingShared")}</p>
            ) : (
              <div className="grid gap-2 sm:grid-cols-4">
                <div>
                  <label htmlFor={kindId} className="sr-only">
                    {t("panes.market.kindLabel")}
                  </label>
                  <Select
                    id={kindId}
                    value={effectiveKind}
                    disabled={market.busy}
                    onChange={(e) => setKind(e.target.value === "compute" ? "compute" : "storage")}
                  >
                    <option value="storage" disabled={!canStorage}>
                      {t("panes.market.kind.storage")}
                    </option>
                    <option value="compute" disabled={!canCompute}>
                      {t("panes.market.kind.compute")}
                    </option>
                  </Select>
                </div>
                <div>
                  <label htmlFor={priceId} className="sr-only">
                    {t("panes.market.priceLabel", { currency: account.currency.toUpperCase() })}
                  </label>
                  <Input
                    id={priceId}
                    inputMode="decimal"
                    placeholder={t("panes.market.priceLabel", {
                      currency: account.currency.toUpperCase(),
                    })}
                    value={price}
                    disabled={market.busy}
                    onChange={(e) => setPrice(e.target.value)}
                  />
                </div>
                <div>
                  <label htmlFor={capId} className="sr-only">
                    {t("panes.market.capacityLabel")}
                  </label>
                  <Input
                    id={capId}
                    inputMode="numeric"
                    placeholder={t("panes.market.capacityLabel")}
                    value={capacity}
                    disabled={market.busy}
                    onChange={(e) => setCapacity(e.target.value)}
                  />
                </div>
                <Button disabled={market.busy || !sellable} onClick={() => void submit()}>
                  {t("panes.market.listButton")}
                </Button>
              </div>
            )}
            {problem !== null && <FieldError>{problem}</FieldError>}
          </StackRow>
        )}

        {account.listings.map((l, i) => (
          <Row key={l.id} last={i === account.listings.length - 1}>
            <RowText
              title={`${t(KIND_LABEL[l.kind])} · ${formatMoney(l.unit_price, account.currency)} / ${unitName(l.unit)}`}
              detail={`${t("panes.market.available", { count: l.available })} · ${
                l.active ? t("panes.market.open") : t("panes.market.closed")
              }`}
            />
            {l.active && (
              <Button
                size="sm"
                variant="secondary"
                disabled={market.busy}
                onClick={() => void market.close(l.id)}
              >
                {t("panes.market.closeButton")}
              </Button>
            )}
          </Row>
        ))}
      </Group>
      <GroupCaption>{t("panes.market.sellCaption", { percent: feePercent })}</GroupCaption>
      {account.sales.length > 0 && (
        <Group className="mt-3">
          {account.sales.map((o, i) => (
            <Row key={o.id} last={i === account.sales.length - 1}>
              <RowText
                title={`${t(KIND_LABEL[o.kind])} · ${o.quantity} ${unitName(o.unit)}`}
                detail={t("panes.market.youReceive", { amount: formatMoney(o.seller_net, o.currency) })}
              />
              <span className="text-[13px] text-muted">
                {t(`panes.market.status.${o.status}` as MessageKey)}
              </span>
            </Row>
          ))}
        </Group>
      )}
    </PaneSection>
  );
}
