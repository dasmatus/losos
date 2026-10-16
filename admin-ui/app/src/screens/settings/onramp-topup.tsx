import * as React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { UsdcChain } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { paneHref } from "@/lib/routes";
import { balanceOn, formatUnits, shortAddress } from "@/wallet/chains";
import {
  acceptTerms,
  baseTransfers,
  onrampLink,
  ONRAMP_MAX_EUR,
  ONRAMP_MIN_EUR,
  ONRAMP_TERMS,
  parseEuros,
  readTopUp,
  returnedOrder,
  termsAccepted,
  writeTopUp,
  type PendingTopUp,
} from "@/wallet/onramp";
import { Row } from "./rows";

/** How often the page reads the Base balance while a purchase is on its way. */
const POLL_MS = 20_000;

const link = "underline underline-offset-2 hover:text-ink";

/** Buy USDC on Base with euros at Onramp.money, sent to `address`. The row
 *  names the provider and its terms before anything opens, and then waits
 *  for the USDC on chain. */
export function OnrampTopUp({
  address,
  base,
  before,
  disabled,
  onArrived,
}: {
  address: string;
  /** The edge's Base mainnet chain. */
  base: UsdcChain;
  /** The Base USDC balance the wallet last read, in base units. */
  before: bigint | null;
  disabled: boolean;
  onArrived: () => void;
}) {
  const t = useT();
  const [open, setOpen] = React.useState(false);
  const [euros, setEuros] = React.useState("");
  const [accepted, setAccepted] = React.useState(termsAccepted);
  const [pending, setPending] = React.useState<PendingTopUp | null>(readTopUp);
  const [arrived, setArrived] = React.useState<string | null>(null);
  const arrivedRef = React.useRef(onArrived);
  arrivedRef.current = onArrived;

  // Onramp.money sends the buyer back here with its order id and status.
  // Keep them with the purchase, then take them off the address bar.
  React.useEffect(() => {
    const back = returnedOrder(window.location.search);
    if (back === null) return;
    const current = readTopUp();
    if (current !== null) {
      const next = { ...current, orderId: back.orderId, status: back.status };
      writeTopUp(next);
      setPending(next);
    }
    window.history.replaceState(window.history.state, "", window.location.pathname);
  }, []);

  // Another tab (the one Onramp.money sent back) may have changed it.
  React.useEffect(() => {
    const onStorage = () => setPending(readTopUp());
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const mine = pending !== null && pending.address.toLowerCase() === address.toLowerCase() ? pending : null;

  React.useEffect(() => {
    if (mine === null) return;
    let stopped = false;
    const check = async () => {
      const read = await balanceOn(base, address);
      if (stopped || read === null) return;
      const current = readTopUp();
      if (current === null) return;
      if (current.before === null) {
        const next = { ...current, before: read.usdc.toString() };
        writeTopUp(next);
        setPending(next);
        return;
      }
      const gained = read.usdc - BigInt(current.before);
      if (gained > 0n) {
        writeTopUp(null);
        setPending(null);
        setArrived(formatUnits(gained, 6, 2));
        arrivedRef.current();
      }
    };
    void check();
    const timer = window.setInterval(() => void check(), POLL_MS);
    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a new purchase (its time) restarts the watch, not its status
  }, [mine?.at, base, address]);

  const amount = parseEuros(euros);

  const start = () => {
    if (amount === null || !accepted) return;
    acceptTerms();
    const redirect = `${window.location.origin}${paneHref("market")}`;
    window.open(onrampLink(address, amount, redirect), "_blank", "noopener,noreferrer");
    const next: PendingTopUp = { address, euros: amount, before: before === null ? null : before.toString(), at: Date.now() };
    writeTopUp(next);
    setPending(next);
    setArrived(null);
    setOpen(false);
    setEuros("");
  };

  const stopWaiting = () => {
    writeTopUp(null);
    setPending(null);
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
          {!open && mine === null && (
            <Button size="sm" variant="secondary" disabled={disabled} onClick={() => setOpen(true)}>
              {t("panes.market.onramp.buy")}
            </Button>
          )}
        </div>
        {open && (
          <div className="grid gap-2" role="group" aria-label={t("panes.market.onramp.title")}>
            <p className="text-[13px] text-muted">{t("panes.market.onramp.notice")}</p>
            <p className="text-[13px] text-muted">
              <a className={link} href={ONRAMP_TERMS.terms} target="_blank" rel="noopener noreferrer">
                {t("panes.market.onramp.terms")}
              </a>
              {" · "}
              <a className={link} href={ONRAMP_TERMS.privacy} target="_blank" rel="noopener noreferrer">
                {t("panes.market.onramp.privacy")}
              </a>
              {" · "}
              <a className={link} href={ONRAMP_TERMS.risk} target="_blank" rel="noopener noreferrer">
                {t("panes.market.onramp.risk")}
              </a>
            </p>
            <label className="flex items-start gap-2 text-[13px]">
              <input
                type="checkbox"
                className="mt-0.5"
                checked={accepted}
                onChange={(e) => setAccepted(e.target.checked)}
              />
              {t("panes.market.onramp.accept")}
            </label>
            <div className="flex flex-wrap items-center gap-2">
              <Input
                className="w-32"
                inputMode="decimal"
                aria-label={t("panes.market.onramp.amount")}
                placeholder={t("panes.market.onramp.amountPlaceholder")}
                value={euros}
                onChange={(e) => setEuros(e.target.value)}
              />
              <Button size="sm" disabled={disabled || amount === null || !accepted} onClick={start}>
                {t("panes.market.onramp.continue")}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => setOpen(false)}>
                {t("panes.market.onramp.cancel")}
              </Button>
            </div>
            {euros.trim() !== "" && amount === null && (
              <p className="text-[13px] text-muted" role="status">
                {t("panes.market.onramp.range", { min: ONRAMP_MIN_EUR, max: ONRAMP_MAX_EUR })}
              </p>
            )}
          </div>
        )}
        {mine !== null && (
          <div className="flex flex-wrap items-center justify-between gap-2" role="status">
            <p className="text-[13px] text-muted">
              {mine.orderId
                ? t(mine.status === "success" ? "panes.market.onramp.paidOrder" : "panes.market.onramp.waitingOrder", {
                    euros: mine.euros,
                    order: mine.orderId,
                  })
                : t("panes.market.onramp.waiting", { euros: mine.euros })}
            </p>
            <Button size="sm" variant="secondary" onClick={stopWaiting}>
              {t("panes.market.onramp.stop")}
            </Button>
          </div>
        )}
        {arrived !== null && (
          <p className="text-[13px] text-muted" role="status">
            {t("panes.market.onramp.arrived", { amount: arrived })}
            {" · "}
            <a className={link} href={baseTransfers(address)} target="_blank" rel="noopener noreferrer">
              {t("panes.market.onramp.viewTransfers")}
            </a>
          </p>
        )}
      </div>
    </Row>
  );
}
