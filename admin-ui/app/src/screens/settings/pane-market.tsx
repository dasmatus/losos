import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ShoppingBag02Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
} from "@/components/ui/empty";
import { FieldError, Input } from "@/components/ui/input";
import { NativeSelect } from "@/components/ui/native-select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Spinner } from "@/components/ui/progress";
import { Switch } from "@/components/ui/switch";
import { toast } from "@/components/ui/toast";
import type { MarketKind, MarketOrder, MarketShelfListing } from "@/lib/api";
import { t as translate, type MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow } from "./rows";
import {
  formatDay,
  formatMoney,
  isStripePage,
  minorDigits,
  toMinorUnits,
  useMarket,
  type MarketData,
  ORDER_STATUS,
} from "./market";
import { useEdge } from "./use-edge";
import type { SettingsForm } from "./use-settings-form";

/* Sharing this box's disk, and buying and selling what the mesh shares.
 *
 * The first group is the disk-sharing switch (`sharingMyStorage`), moved here
 * from the Storage pane: lending disk to other boxes and being paid for it
 * are one decision, so they sit on one pane, and while the pane is planned
 * (panes.ts) the switch is out of reach with it. It renders in every state
 * below, before the market has answered and whether or not it is available,
 * because it is a setting of this box, not something the edge serves.
 *
 * The rest is a way to be paid for the storage and compute a box contributes,
 * not a separate product, so the sell side only offers what the owner is
 * already sharing (the edge enforces it; this pane greys out the rest and
 * says why). Payment happens on Stripe's own page in a new tab — nothing on
 * this page ever sees a card — so after a purchase a toast tells the owner
 * to come back and refresh rather than pretending to know the outcome, and
 * the refresh that finds the order paid raises the "payment received" one
 * (market.ts). Every refusal is a toast too; only a field the owner can fix
 * keeps its error under the field.
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
  const sharing = <SharingSection form={form} />;

  if (state.kind === "loading") {
    return (
      <>
        {sharing}
        <PaneSection>
          <Group>
            <Row last>
              <RowText title={t("panes.market.loading")} />
              <Spinner size={16} />
            </Row>
          </Group>
        </PaneSection>
      </>
    );
  }
  if (state.kind === "failed") {
    return (
      <>
        {sharing}
        <PaneSection>
          <Group>
            <Row last>
              <RowText title={t("panes.market.failed")} detail={state.message} />
              <Button size="sm" variant="secondary" disabled={form.locked} onClick={market.refresh}>
                {t("panes.market.refresh")}
              </Button>
            </Row>
          </Group>
        </PaneSection>
      </>
    );
  }
  if (state.kind === "unavailable") {
    return (
      <>
        {sharing}
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
      </>
    );
  }

  const { account, listings } = state;
  const disabled = form.locked || market.busy;
  return (
    <>
      {sharing}

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
            {/* A shadcn Table: every order has the same four facts, and the
                amount and the date want a column each so they line up. */}
            <Table data-testid="market-purchases">
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead>{t("panes.market.col.item")}</TableHead>
                  <TableHead>{t("panes.market.col.quantity")}</TableHead>
                  <TableHead>{t("panes.market.col.until")}</TableHead>
                  <TableHead className="text-right">{t("panes.market.col.status")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {account.purchases.map((order) => (
                  <PurchaseRow key={order.id} order={order} />
                ))}
              </TableBody>
            </Table>
          </Group>
        )}
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.market.buy")}</GroupTitle>
        <Group>
          {listings.length === 0 ? (
            <Empty className="border-0 p-5 md:p-8">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <HugeiconsIcon icon={ShoppingBag02Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                </EmptyMedia>
                <EmptyDescription>{t("panes.market.shelfEmpty")}</EmptyDescription>
              </EmptyHeader>
              <EmptyContent>
                <Button size="sm" variant="secondary" disabled={disabled} onClick={market.refresh}>
                  {t("panes.market.refresh")}
                </Button>
              </EmptyContent>
            </Empty>
          ) : (
            listings.map((listing, i) => (
              <ShelfRow
                key={listing.id}
                listing={listing}
                market={market}
                disabled={disabled}
                last={i === listings.length - 1}
              />
            ))
          )}
        </Group>
        <GroupCaption>{t("panes.market.payCaption")}</GroupCaption>
      </PaneSection>

      <SellSection market={market} disabled={disabled} />
    </>
  );
}

/* The disk-sharing switch and what it buys the owner. Disabled on `locked`
 * and until the form has loaded, like every other control on the form; the
 * value is the draft's, and Apply writes it with the rest. */
function SharingSection({ form }: { form: SettingsForm }) {
  const t = useT();
  const edge = useEdge();
  const shareId = React.useId();
  const sharing = form.draft?.sharingMyStorage ?? false;
  /* Same rule as the Join switch on the Mesh pane: no edge in reach means
   * the daemon refuses to turn this on, so it is greyed and says why, unless
   * it is already on, in which case turning it off must stay possible. */
  const refused = edge.blocked && !(form.saved?.sharingMyStorage ?? false);
  return (
    <PaneSection>
      <GroupTitle>{t("panes.market.share.title")}</GroupTitle>
      <Group>
        <Row>
          <RowText
            htmlFor={shareId}
            title={t("panes.market.share.switch")}
            detail={refused && !form.locked ? t("panes.mesh.edge.needed") : undefined}
          />
          <Switch
            id={shareId}
            checked={sharing}
            disabled={form.locked || !form.ready || refused}
            onCheckedChange={(next) => form.set("sharingMyStorage", next)}
          />
        </Row>
        <Row last>
          <RowText title={t("panes.market.share.ifDies")} />
          <RowValue className={sharing ? "text-ok" : undefined}>
            {sharing ? t("panes.market.share.rebuilds") : t("panes.market.share.notCopied")}
          </RowValue>
        </Row>
      </Group>
      <GroupCaption>{t("panes.market.share.caption")}</GroupCaption>
    </PaneSection>
  );
}

function PurchaseRow({ order }: { order: MarketOrder }) {
  const t = useT();
  const status =
    order.status === "paid" && order.expired
      ? t("panes.market.status.lapsed")
      : t(`panes.market.status.${order.status}` as MessageKey);
  const until =
    order.status === "paid" && order.expires_at !== null && !order.expired
      ? formatDay(order.expires_at)
      : "–";
  return (
    <TableRow>
      <TableCell>
        <p>{`${t(KIND_LABEL[order.kind])} · ${formatMoney(order.amount, order.currency)}`}</p>
        {order.volume !== null && (
          <p className="numeric mt-0.5 text-[12px] text-faint">
            {t("panes.market.volume", { volume: order.volume })}
          </p>
        )}
      </TableCell>
      <TableCell className="numeric">{`${order.quantity} ${unitName(order.unit)}`}</TableCell>
      <TableCell className="numeric text-muted">{until}</TableCell>
      <TableCell className="text-right text-muted">{status}</TableCell>
    </TableRow>
  );
}

function ShelfRow({
  listing,
  market,
  disabled,
  last,
}: {
  listing: MarketShelfListing;
  market: MarketData;
  /** The pane's lock: a request in flight, or the admin token gone. */
  disabled: boolean;
  last: boolean;
}) {
  const t = useT();
  const qtyId = React.useId();
  const qtyErrorId = React.useId();
  const [quantity, setQuantity] = React.useState("1");
  const [quantityProblem, setQuantityProblem] = React.useState<string | null>(null);

  const buy = async () => {
    const n = Number(quantity);
    if (!Number.isInteger(n) || n < 1 || n > listing.available) {
      setQuantityProblem(t("panes.market.quantityRange", { max: listing.available }));
      return;
    }
    setQuantityProblem(null);
    const checkoutTab = window.open("about:blank", "_blank");
    if (checkoutTab === null) {
      toast.error(t("panes.market.actionFailedTitle"), t("panes.market.popupBlocked"));
      return;
    }
    checkoutTab.opener = null;
    const reply = await market.order(listing.id, n);
    if (reply !== null) {
      if (isStripePage(reply.checkout_url)) {
        checkoutTab.location.href = reply.checkout_url;
        // Longer than a plain note: it is the one instruction the owner needs
        // when they come back from the other tab.
        toast.status(t("panes.market.orderPlaced"), t("panes.market.paying"), {
          id: ORDER_STATUS,
          duration: Infinity,
        });
      } else {
        checkoutTab.close();
        toast.error(t("panes.market.actionFailedTitle"), t("panes.market.noCheckout"));
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
        {/* A shadcn Button Group: the quantity and the Order button are one
            control, so the number reads as the button's argument. */}
        <ButtonGroup>
          <label htmlFor={qtyId} className="sr-only">
            {t("panes.market.quantity")}
          </label>
          <Input
            id={qtyId}
            inputMode="numeric"
            className="numeric w-20"
            value={quantity}
            disabled={disabled}
            aria-invalid={quantityProblem !== null}
            aria-describedby={quantityProblem === null ? undefined : qtyErrorId}
            onChange={(e) => setQuantity(e.target.value)}
          />
          <Button disabled={disabled} onClick={() => void buy()}>
            {t("panes.market.buyButton")}
          </Button>
        </ButtonGroup>
      </div>
      <FieldError id={qtyErrorId}>{quantityProblem}</FieldError>
    </StackRow>
  );
}

function SellSection({ market, disabled }: { market: MarketData; disabled: boolean }) {
  const t = useT();
  const kindId = React.useId();
  const priceId = React.useId();
  const capId = React.useId();
  const priceErrorId = React.useId();
  const capErrorId = React.useId();
  const state = market.state;
  const account = state.kind === "ready" ? state.account : null;
  const canStorage = account?.can_sell_storage ?? false;
  const canCompute = account?.can_sell_compute ?? false;
  const [kind, setKind] = React.useState<MarketKind>("storage");
  const [price, setPrice] = React.useState("");
  const [capacity, setCapacity] = React.useState("");
  const [problem, setProblem] = React.useState<{
    field: "price" | "capacity";
    text: string;
  } | null>(null);

  if (account === null) return null;

  const effectiveKind: MarketKind =
    kind === "storage" && !canStorage && canCompute ? "compute" : kind;
  const sellable = effectiveKind === "storage" ? canStorage : canCompute;
  const feePercent = account.fee_bps / 100;

  const onboard = async () => {
    const onboardingTab = window.open("about:blank", "_blank");
    if (onboardingTab === null) {
      toast.error(t("panes.market.actionFailedTitle"), t("panes.market.popupBlocked"));
      return;
    }
    onboardingTab.opener = null;
    const reply = await market.onboard();
    if (reply !== null && isStripePage(reply.url)) {
      onboardingTab.location.href = reply.url;
      toast.status(t("panes.market.payoutsOpened"), t("panes.market.payoutsOpenedBody"), { duration: 12000 });
    } else onboardingTab.close();
  };

  const submit = async () => {
    const minor = toMinorUnits(price, account.currency);
    const cap = Number(capacity);
    if (minor === null) {
      const digits = minorDigits(account.currency);
      const example = (5 / 10 ** digits).toFixed(digits);
      return setProblem({ field: "price", text: t("panes.market.priceInvalid", { example }) });
    }
    if (!Number.isInteger(cap) || cap < 1) {
      return setProblem({ field: "capacity", text: t("panes.market.capacityInvalid") });
    }
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
            <Button size="sm" disabled={disabled} onClick={() => void onboard()}>
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
                  <NativeSelect
                    id={kindId}
                    value={effectiveKind}
                    disabled={disabled}
                    onChange={(e) => setKind(e.target.value === "compute" ? "compute" : "storage")}
                  >
                    <option value="storage" disabled={!canStorage}>
                      {t("panes.market.kind.storage")}
                    </option>
                    <option value="compute" disabled={!canCompute}>
                      {t("panes.market.kind.compute")}
                    </option>
                  </NativeSelect>
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
                    disabled={disabled}
                    aria-invalid={problem?.field === "price"}
                    aria-describedby={problem?.field === "price" ? priceErrorId : undefined}
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
                    disabled={disabled}
                    aria-invalid={problem?.field === "capacity"}
                    aria-describedby={problem?.field === "capacity" ? capErrorId : undefined}
                    onChange={(e) => setCapacity(e.target.value)}
                  />
                </div>
                <Button disabled={disabled || !sellable} onClick={() => void submit()}>
                  {t("panes.market.listButton")}
                </Button>
              </div>
            )}
            <FieldError id={problem?.field === "price" ? priceErrorId : capErrorId}>
              {problem?.text}
            </FieldError>
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
                disabled={disabled}
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
