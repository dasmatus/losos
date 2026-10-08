/* The order dialog: the boxes and gateways on the canvas as a cart, priced
 * by the official edge's catalogue, checked out on Stripe's page. Drawn only
 * when lososd says ordering is on (order.ts). */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon, InformationCircleIcon, MinusSignIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { intlTag } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { actions } from "./actions";
import { type CatalogueItem, checkout, money, SKU_OF_TYPE } from "./order";
import { useLab } from "./store";

const MAX_QUANTITY = 20;

/** How many of each sku the setup on the canvas has. */
function counted(types: string[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const type of types) {
    const sku = SKU_OF_TYPE[type];
    if (sku) out[sku] = (out[sku] ?? 0) + 1;
  }
  return out;
}

function Stepper({ item, value, onChange, disabled }: { item: CatalogueItem; value: number; onChange: (n: number) => void; disabled: boolean }) {
  const t = useT();
  return (
    <div className="flex items-center gap-1.5">
      <Button
        variant="secondary"
        size="icon-sm"
        aria-label={t("lab.order.fewer", { name: item.name })}
        disabled={disabled || value <= 0}
        onClick={() => onChange(value - 1)}
      >
        <HugeiconsIcon icon={MinusSignIcon} size={15} strokeWidth={1.8} color="currentColor" />
      </Button>
      <output
        className="w-7 text-center text-[14px] font-semibold tabular-nums"
        aria-label={t("lab.order.quantity", { name: item.name })}
        aria-live="polite"
        data-testid={`order-qty-${item.sku}`}
      >
        {value}
      </output>
      <Button
        variant="secondary"
        size="icon-sm"
        aria-label={t("lab.order.more", { name: item.name })}
        disabled={disabled || value >= MAX_QUANTITY}
        onClick={() => onChange(value + 1)}
      >
        <HugeiconsIcon icon={Add01Icon} size={15} strokeWidth={1.8} color="currentColor" />
      </Button>
    </div>
  );
}

export function OrderDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const s = useLab();
  const t = useT();
  const titleId = React.useId();
  const descId = React.useId();
  const ordering = s.ordering;
  const [quantities, setQuantities] = React.useState<Record<string, number>>({});
  const [busy, setBusy] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);

  const types = s.snap.world.devices.map((d) => d.type);
  const onCanvas = counted(types);
  // Every opening starts again from the canvas.
  const typesKey = types.join(",");
  React.useEffect(() => {
    if (!open) return;
    setQuantities(counted(typesKey ? typesKey.split(",") : []));
    setProblem(null);
    setBusy(false);
  }, [open, typesKey]);

  if (!ordering?.enabled) return null;
  const locale = intlTag();
  const catalogue = ordering.available ? ordering.catalogue : null;
  const lines = catalogue ? catalogue.items.map((item) => ({ item, quantity: quantities[item.sku] ?? 0 })) : [];
  const total = lines.reduce((sum, l) => sum + l.item.unit_amount * l.quantity, 0);
  const countries = catalogue ? new Intl.DisplayNames([locale], { type: "region" }) : null;

  const pay = async () => {
    setBusy(true);
    setProblem(null);
    const result = await checkout(lines.filter((l) => l.quantity > 0).map((l) => ({ sku: l.item.sku, quantity: l.quantity })));
    if ("url" in result) {
      // The setup is kept in this browser, so coming back finds it.
      actions().keepLast();
      window.location.assign(result.url);
      return;
    }
    setBusy(false);
    setProblem(result.error);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !busy && onOpenChange(next)}
      dismissible={!busy}
      labelledBy={titleId}
      describedBy={descId}
      dialogClassName="w-[min(34rem,calc(100vw-2rem))]"
      data-testid="order-dialog"
    >
      <DialogHeader>
        <DialogTitle id={titleId}>{t("lab.order.title")}</DialogTitle>
        <DialogDescription id={descId}>
          {t("lab.order.counted", {
            boxes: t("lab.order.boxes", { count: onCanvas["box"] ?? 0 }),
            gateways: t("lab.order.gateways", { count: onCanvas["gateway"] ?? 0 }),
          })}
        </DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        {!ordering.available ? (
          <Alert variant="warn" data-testid="order-unavailable">
            <HugeiconsIcon icon={InformationCircleIcon} size={17} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <AlertDescription>{t(ordering.reason === "noOfficialEdge" ? "lab.order.noOfficialEdge" : "lab.order.notSold")}</AlertDescription>
          </Alert>
        ) : (
          <>
            <ul className="flex flex-col divide-y divide-hair rounded-control border border-line">
              {lines.map(({ item, quantity }) => (
                <li key={item.sku} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-4 gap-y-2 px-3.5 py-3 min-[520px]:grid-cols-[minmax(0,1fr)_auto_6.5rem]">
                  <div className="min-w-0">
                    <div className="text-[14px] font-semibold">{item.name}</div>
                    {item.detail && <div className="text-[12.5px] leading-snug text-muted">{item.detail}</div>}
                    <div className="text-[12.5px] text-muted tabular-nums">{money(item.unit_amount, catalogue!.currency, locale)}</div>
                  </div>
                  <Stepper
                    item={item}
                    value={quantity}
                    disabled={busy}
                    onChange={(n) => setQuantities((q) => ({ ...q, [item.sku]: Math.max(0, Math.min(MAX_QUANTITY, n)) }))}
                  />
                  <div className="col-span-2 text-right text-[14px] tabular-nums min-[520px]:col-span-1">
                    {money(item.unit_amount * quantity, catalogue!.currency, locale)}
                  </div>
                </li>
              ))}
            </ul>
            <div className="flex items-baseline justify-between px-1">
              <span className="text-[14px] font-semibold">{t("lab.order.total")}</span>
              <span className="text-[17px] font-semibold tabular-nums" data-testid="order-total">
                {money(total, catalogue!.currency, locale)}
              </span>
            </div>
            <p className="px-1 text-[12.5px] leading-snug text-muted">
              {t("lab.order.ships", { countries: catalogue!.countries.map((c) => countries?.of(c) ?? c).join(", ") })}
            </p>
            {problem && (
              <Alert variant="crit" role="alert">
                <HugeiconsIcon icon={InformationCircleIcon} size={17} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                <AlertDescription>{t("lab.order.failed", { detail: problem })}</AlertDescription>
              </Alert>
            )}
          </>
        )}
      </DialogBody>
      <DialogFooter>
        <Button variant="secondary" disabled={busy} onClick={() => onOpenChange(false)}>
          {t("lab.order.cancel")}
        </Button>
        {ordering.available && (
          <Button disabled={busy || total === 0} onClick={() => void pay()} data-testid="order-pay">
            {busy && <Spinner size={15} />}
            {busy ? t("lab.order.paying") : t("lab.order.pay")}
          </Button>
        )}
      </DialogFooter>
    </Dialog>
  );
}
