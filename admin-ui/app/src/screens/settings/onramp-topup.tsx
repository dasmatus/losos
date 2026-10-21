import * as React from "react";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ApiError, postMarketOnramp, postMarketOnrampStatus, type OnrampOrder } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { shortAddress } from "@/wallet/chains";
import {
  baseTx,
  BINANCE_TERMS,
  finished,
  ONRAMP_MAX_EUR,
  ONRAMP_MIN_EUR,
  parseEuros,
  readOrder,
  recordAcceptance,
  writeOrder,
  type TermsDocument,
} from "@/wallet/onramp";
import { Row } from "./rows";

/** How often the row asks the edge where an order is. */
const POLL_MS = 15_000;

const link = "underline underline-offset-2 hover:text-ink";

/** Buy USDC on Base with euros at Binance, sent to `address`. Binance's
 *  documents come first: the accept box stays locked until each one was
 *  opened. The edge makes the order; the row follows it until Binance has
 *  sent the USDC. */
export function OnrampTopUp({
  address,
  disabled,
  onArrived,
}: {
  address: string;
  disabled: boolean;
  onArrived: () => void;
}) {
  const t = useT();
  const [open, setOpen] = React.useState(false);
  const [euros, setEuros] = React.useState("");
  const [opened, setOpened] = React.useState<ReadonlySet<TermsDocument>>(new Set());
  const [accepted, setAccepted] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [failed, setFailed] = React.useState<string | null>(null);
  const [order, setOrder] = React.useState<OnrampOrder | null>(null);
  const [following, setFollowing] = React.useState(() => {
    const pending = readOrder();
    return pending !== null && pending.address.toLowerCase() === address.toLowerCase() ? pending.orderId : null;
  });
  const arrivedRef = React.useRef(onArrived);
  arrivedRef.current = onArrived;

  // Another tab may have started or dropped an order.
  React.useEffect(() => {
    const onStorage = () => {
      const pending = readOrder();
      setFollowing(pending !== null && pending.address.toLowerCase() === address.toLowerCase() ? pending.orderId : null);
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, [address]);

  React.useEffect(() => {
    if (following === null) return;
    let stopped = false;
    let timer = 0;
    const check = async () => {
      try {
        const next = await postMarketOnrampStatus(following);
        if (stopped) return;
        setOrder(next);
        if (finished(next.stage)) {
          if (next.stage === "done") arrivedRef.current();
          return;
        }
      } catch (error) {
        if (stopped) return;
        // An order the edge no longer knows is not worth following.
        if (error instanceof ApiError && error.status === 400) {
          writeOrder(null);
          setFollowing(null);
          return;
        }
      }
      timer = window.setTimeout(() => void check(), POLL_MS);
    };
    void check();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  }, [following]);

  const amount = parseEuros(euros);
  const allOpened = BINANCE_TERMS.documents.every((d) => opened.has(d.key));

  const markOpened = (key: TermsDocument) =>
    setOpened((prev) => (prev.has(key) ? prev : new Set([...prev, key])));

  const start = async () => {
    if (amount === null || !accepted || !allOpened) return;
    setBusy(true);
    setFailed(null);
    try {
      const made = await postMarketOnramp(address, amount);
      recordAcceptance();
      writeOrder({ address, orderId: made.order_id, at: Date.now() });
      setOrder(made);
      setFollowing(made.order_id);
      setOpen(false);
      setEuros("");
      setOpened(new Set());
      setAccepted(false);
    } catch (error) {
      setFailed(error instanceof ApiError ? error.message : t("panes.market.coins.unreachable"));
    } finally {
      setBusy(false);
    }
  };

  const stopFollowing = () => {
    writeOrder(null);
    setFollowing(null);
    setOrder(null);
  };

  const current = following !== null && order?.order_id === following ? order : null;

  const stageText = (o: OnrampOrder): string => {
    switch (o.stage) {
      case "open":
        return t("panes.market.onramp.stage.open", { euros: o.euros });
      case "paying":
        return t("panes.market.onramp.stage.paying", { euros: o.euros });
      case "sending":
        return t("panes.market.onramp.stage.sending", { usdc: o.usdc ?? "…" });
      case "done":
        return t("panes.market.onramp.stage.done", { usdc: o.usdc ?? "…" });
      case "kept":
        return t("panes.market.onramp.stage.kept");
      case "failed":
        return t("panes.market.onramp.stage.failed");
    }
  };

  return (
    <Row data-testid="onramp-topup">
      <div className="grid w-full gap-2">
        <div className="flex items-start justify-between gap-4">
          <div className="grid gap-0.5">
            <p className="text-[13px] font-medium text-ink">{t("panes.market.onramp.title")}</p>
            <p className="text-[13px] text-muted">
              {t("panes.market.onramp.detail", { address: shortAddress(address) })}
            </p>
          </div>
          {!open && following === null && (
            <Button size="sm" variant="secondary" disabled={disabled} onClick={() => setOpen(true)}>
              {t("panes.market.onramp.buy")}
            </Button>
          )}
        </div>
        {open && (
          <div className="grid gap-3" role="group" aria-label={t("panes.market.onramp.title")}>
            <p className="text-[13px] text-muted">{t("panes.market.onramp.notice")}</p>
            <p className="text-[13px] text-muted">{t("panes.market.onramp.region")}</p>
            <div className="grid gap-1.5">
              <p className="text-[13px] text-ink">{t("panes.market.onramp.readFirst")}</p>
              <ul className="grid gap-1" aria-label={t("panes.market.onramp.documents")}>
                {BINANCE_TERMS.documents.map((d) => (
                  <li key={d.key} className="flex items-center justify-between gap-3 text-[13px]">
                    <a
                      className={link}
                      href={d.href}
                      target="_blank"
                      rel="noopener noreferrer"
                      onClick={() => markOpened(d.key)}
                      onAuxClick={() => markOpened(d.key)}
                    >
                      {t(`panes.market.onramp.doc.${d.key}`)}
                    </a>
                    {opened.has(d.key) ? (
                      <Badge variant="ok">{t("panes.market.onramp.docOpened")}</Badge>
                    ) : (
                      <Badge variant="neutral">{t("panes.market.onramp.docNotOpened")}</Badge>
                    )}
                  </li>
                ))}
              </ul>
            </div>
            <label className={cn("flex items-start gap-2 text-[13px]", !allOpened && "text-muted")}>
              <input
                type="checkbox"
                className="mt-0.5"
                checked={accepted}
                disabled={!allOpened}
                onChange={(e) => setAccepted(e.target.checked)}
              />
              {t("panes.market.onramp.accept")}
            </label>
            {!allOpened && (
              <p className="text-[13px] text-muted" role="status">
                {t("panes.market.onramp.openEach")}
              </p>
            )}
            <div className="flex flex-wrap items-center gap-2">
              <Input
                className="w-32"
                inputMode="decimal"
                aria-label={t("panes.market.onramp.amount")}
                placeholder={t("panes.market.onramp.amountPlaceholder")}
                value={euros}
                onChange={(e) => setEuros(e.target.value)}
              />
              <Button
                size="sm"
                disabled={disabled || busy || amount === null || !accepted || !allOpened}
                onClick={() => void start()}
              >
                {t("panes.market.onramp.continue")}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => setOpen(false)}>
                {t("panes.market.onramp.cancel")}
              </Button>
            </div>
            {euros.trim() !== "" && amount === null && (
              <p className="text-[13px] text-muted" role="status">
                {t("panes.market.onramp.range", { min: ONRAMP_MIN_EUR, max: ONRAMP_MAX_EUR.toLocaleString("en") })}
              </p>
            )}
          </div>
        )}
        {failed !== null && (
          <p className="text-[13px] text-crit" role="alert">
            {failed}
          </p>
        )}
        {following !== null && (
          <div className="grid gap-2" role="status" data-testid="onramp-order">
            <p className="text-[13px] text-muted">
              {current === null ? t("panes.market.onramp.checking") : stageText(current)}
            </p>
            <div className="flex flex-wrap items-center gap-2">
              {current !== null && (current.stage === "open" || current.stage === "paying") && (
                <a
                  className={cn(buttonVariants({ size: "sm" }), "no-underline")}
                  href={current.link}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {t("panes.market.onramp.openBinance")}
                </a>
              )}
              {current?.tx && (
                <a className={cn(link, "text-[13px]")} href={baseTx(current.tx)} target="_blank" rel="noopener noreferrer">
                  {t("panes.market.onramp.viewTransaction")}
                </a>
              )}
              <Button size="sm" variant="secondary" onClick={stopFollowing}>
                {current !== null && finished(current.stage)
                  ? t("panes.market.onramp.dismiss")
                  : t("panes.market.onramp.stop")}
              </Button>
            </div>
          </div>
        )}
      </div>
    </Row>
  );
}
