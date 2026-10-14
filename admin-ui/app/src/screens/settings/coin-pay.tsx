import * as React from "react";
import encodeQR from "qr";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { toast } from "@/components/ui/toast";
import { ApiError, postMarketCoinKey, postMarketCoinPay, type Coin, type CoinPayment } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { copyText } from "../wizard/copy";
import { Row, RowText } from "./rows";

/* Orders paid in a coin other than USDC: Bitcoin and Monero. The edge gives
 * each order a fresh address of the seller's own wallet (derived from a
 * Bitcoin public key, or a Monero subaddress seen through the view key), so
 * the coins go straight to the seller's wallet and the edge only watches.
 * The buyer pays from any wallet, by the QR code or the payment link, and
 * nothing is reported back: the edge's node sees the payment arrive. */

export const COIN_NAMES: Record<Coin, string> = { btc: "Bitcoin", xmr: "Monero" };
const TICKERS: Record<Coin, string> = { btc: "BTC", xmr: "XMR" };

/** How often the panel asks the edge what reached the address. */
const POLL_MS = 15_000;

/** The payment link as a QR code: one path over the dark modules, drawn as
 *  React elements, so nothing is injected as markup. */
export function QrCode({ text, label }: { text: string; label: string }) {
  const d = React.useMemo(() => {
    const grid = encodeQR(text, "raw", { ecc: "medium", border: 2 });
    const parts: string[] = [];
    grid.forEach((row, y) =>
      row.forEach((dark, x) => {
        if (dark) parts.push(`M${x} ${y}h1v1h-1z`);
      }),
    );
    return { path: parts.join(""), size: grid.length };
  }, [text]);
  return (
    <svg
      role="img"
      aria-label={label}
      viewBox={`0 0 ${d.size} ${d.size}`}
      className="size-44 shrink-0 rounded-md bg-white"
      shapeRendering="crispEdges"
    >
      <path d={d.path} fill="#000" />
    </svg>
  );
}

function clock(unix: number): string {
  return new Date(unix * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/** Where to pay `orderId` in `coin`, and how far the payment has got. Calls
 *  `onPaid` once the edge counts it. */
export function CoinPayPanel({
  orderId,
  coin,
  onPaid,
}: {
  orderId: string;
  coin: Coin;
  onPaid: (payment: CoinPayment) => void;
}) {
  const t = useT();
  const [payment, setPayment] = React.useState<CoinPayment | null>(null);
  const [failed, setFailed] = React.useState<string | null>(null);
  const addressRef = React.useRef<HTMLElement>(null);
  const amountRef = React.useRef<HTMLElement>(null);
  const paidOnce = React.useRef(false);

  const check = React.useCallback(async () => {
    try {
      const found = await postMarketCoinPay(orderId, coin);
      setPayment(found);
      setFailed(null);
      if (found.state === "paid" && !paidOnce.current) {
        paidOnce.current = true;
        onPaid(found);
      }
    } catch (error) {
      setFailed(error instanceof ApiError ? error.message : t("panes.market.coins.unreachable"));
    }
  }, [orderId, coin, onPaid, t]);

  React.useEffect(() => {
    void check();
    const timer = window.setInterval(() => void check(), POLL_MS);
    return () => window.clearInterval(timer);
  }, [check]);

  const copy = async (text: string, element: HTMLElement | null) => {
    if (await copyText(text, element)) toast.success(t("panes.market.coins.copied"));
  };

  if (payment === null) {
    return <p className="text-[13px] text-muted">{failed ?? t("panes.market.coins.loading")}</p>;
  }
  const ticker = TICKERS[coin];
  const state = t(`panes.market.coins.state.${payment.state}`, {
    paid: payment.paid,
    due: payment.due ?? payment.amount,
    ticker,
    n: String(payment.confirmations ?? 0),
    required: String(payment.required),
  });
  return (
    <div className="flex flex-wrap items-start gap-4">
      {payment.state !== "paid" && <QrCode text={payment.uri} label={t("panes.market.coins.qr")} />}
      <div className="grid min-w-[14rem] flex-1 gap-2 text-[13px] text-ink">
        {payment.state !== "paid" && (
          <p>
            {t("panes.market.coins.send", { ticker, time: clock(payment.pay_before) })}
          </p>
        )}
        <p className="flex flex-wrap items-center gap-2">
          <code ref={amountRef} className="font-mono text-[15px] font-medium select-all">
            {payment.amount} {ticker}
          </code>
          <Button size="sm" variant="secondary" onClick={() => void copy(payment.amount, amountRef.current)}>
            {t("panes.market.coins.copyAmount")}
          </Button>
        </p>
        <p className="flex flex-wrap items-center gap-2">
          <code ref={addressRef} className="font-mono text-[12px] break-all select-all">
            {payment.address}
          </code>
          <Button size="sm" variant="secondary" onClick={() => void copy(payment.address, addressRef.current)}>
            {t("panes.market.coins.copyAddress")}
          </Button>
        </p>
        <p className="text-muted" aria-live="polite">
          {state}
        </p>
        {failed !== null && <p className="text-crit">{failed}</p>}
        <p className="flex flex-wrap gap-3">
          {payment.state !== "paid" && (
            <a className="underline underline-offset-2 hover:text-ink" href={payment.uri}>
              {t("panes.market.coins.openWallet")}
            </a>
          )}
        </p>
      </div>
    </div>
  );
}

/** The seller's watch-only key for `coin`, and what coin sales owe. */
export function CoinKeyRow({
  coin,
  saved,
  feesOwed,
  disabled,
  last,
  onSaved,
}: {
  coin: Coin;
  saved: string | undefined;
  /** Already formatted in the market's currency, or null when none. */
  feesOwed: string | null;
  disabled: boolean;
  last: boolean;
  onSaved: () => void;
}) {
  const t = useT();
  const [key, setKey] = React.useState("");
  const [viewKey, setViewKey] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  // Monero is watched through the wallet's view key, beside its address.
  const monero = coin === "xmr";
  const save = async () => {
    setBusy(true);
    try {
      await postMarketCoinKey(coin, key.trim(), monero ? viewKey.trim() : undefined);
      setKey("");
      setViewKey("");
      toast.success(t("panes.market.coins.keySaved", { coin: COIN_NAMES[coin] }));
      onSaved();
    } catch (error) {
      toast.error(
        t("panes.market.coins.keyFailed"),
        error instanceof ApiError ? error.message : t("panes.market.coins.unreachable"),
        { help: "index" },
      );
    } finally {
      setBusy(false);
    }
  };
  const short = saved ? `${saved.slice(0, 20)}…${saved.slice(-14)}` : null;
  return (
    <Row last={last}>
      <div className="grid w-full gap-2">
        <RowText
          title={t("panes.market.coins.keyTitle", { coin: COIN_NAMES[coin] })}
          detail={t(monero ? "panes.market.coins.keyDetailXmr" : "panes.market.coins.keyDetail")}
        />
        {short !== null && (
          <p className="text-[13px] text-muted break-all">
            {t("panes.market.coins.keyCurrent")} <code className="font-mono text-[12px]">{short}</code>
          </p>
        )}
        {feesOwed !== null && <p className="text-[13px] text-muted">{t("panes.market.coins.feesOwed", { amount: feesOwed })}</p>}
        <div className="flex flex-wrap gap-2">
          <Input
            className="min-w-[14rem] flex-1"
            aria-label={t(monero ? "panes.market.coins.addressPlaceholder" : "panes.market.coins.keyPlaceholder")}
            placeholder={t(monero ? "panes.market.coins.addressPlaceholder" : "panes.market.coins.keyPlaceholder")}
            value={key}
            onChange={(e) => setKey(e.target.value)}
            autoComplete="off"
            spellCheck={false}
          />
          {monero && (
            <Input
              className="min-w-[14rem] flex-1"
              aria-label={t("panes.market.coins.viewKeyPlaceholder")}
              placeholder={t("panes.market.coins.viewKeyPlaceholder")}
              value={viewKey}
              onChange={(e) => setViewKey(e.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
          )}
          <Button
            size="sm"
            disabled={disabled || busy || key.trim() === "" || (monero && viewKey.trim() === "")}
            onClick={() => void save()}
          >
            {short === null ? t("panes.market.coins.keySave") : t("panes.market.coins.keyReplace")}
          </Button>
        </div>
      </div>
    </Row>
  );
}
