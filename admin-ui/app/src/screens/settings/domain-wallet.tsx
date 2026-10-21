import * as React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { toast } from "@/components/ui/toast";
import type { BoundWallet } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { shortAddress } from "@/wallet/chains";
import { accountFromPhrase, openPhrase, savedAddresses, walletMessage } from "@/wallet/hot";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow } from "./rows";

/* The box's wallet, bound to its names on the edge.
 *
 * The edge serves the box's names in a DNSSEC-signed zone. Once the owner's
 * wallet signs the binding message, the edge puts an ENS record (`ENS1 …`)
 * at each of them, and a wallet that sends to ENS names (MetaMask, Rainbow,
 * Coinbase Wallet, viem) turns `mattbox.losos.cfd` into this address on
 * Ethereum, Base, Arbitrum and OP. The edge publishes only an address whose
 * signature it checked, so nobody can point the box's name at another
 * wallet, the edge included.
 *
 * The signature comes from the wallet made on this box, opened with its
 * password the way the Market pane opens it, or else from a browser
 * extension. Like the domains above it, nothing here is a setting of this
 * box: each button is its own request to the edge. */

interface Injected {
  request: (args: { method: string; params?: unknown[] }) => Promise<unknown>;
}

function injected(): Injected | null {
  return (window as unknown as { ethereum?: Injected }).ethereum ?? null;
}

export function DomainWallet({
  box,
  zone,
  names,
  wallet,
  disabled,
  bind,
}: {
  box: string;
  zone: string;
  names: string[];
  wallet: BoundWallet | null;
  disabled: boolean;
  /** Ask the edge to bind (or, with null, unbind); false when it refused. */
  bind: (binding: { address: string; signature: string } | null) => Promise<boolean>;
}) {
  const t = useT();
  const passwordId = React.useId();
  // The wallet made on this box, from the address saved beside the sealed
  // phrase, so it shows before the password is typed.
  const [local, setLocal] = React.useState<string | null>(null);
  const [asking, setAsking] = React.useState(false);
  const [password, setPassword] = React.useState("");
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    void savedAddresses().then((saved) => setLocal(saved?.evm?.toLowerCase() ?? null));
  }, []);

  const signLocal = async () => {
    let phrase: string;
    try {
      phrase = await openPhrase(password);
    } catch {
      toast.error(t("panes.market.usdc.failedTitle"), t("panes.market.usdc.wrongPassword"), { help: "index" });
      return;
    }
    const account = accountFromPhrase(phrase);
    const address = account.address.toLowerCase();
    const signature = await account.signMessage({ message: walletMessage(zone, box, address) });
    if (await bind({ address, signature })) {
      setAsking(false);
      setPassword("");
    }
  };

  const signInjected = async (eth: Injected) => {
    const accounts = (await eth.request({ method: "eth_requestAccounts" })) as string[];
    const address = accounts[0]?.toLowerCase();
    if (!address) return;
    const signature = (await eth.request({
      method: "personal_sign",
      params: [walletMessage(zone, box, address), address],
    })) as string;
    await bind({ address, signature });
  };

  const run = (fn: () => Promise<void>) => async () => {
    setBusy(true);
    try {
      await fn();
    } catch (error) {
      toast.error(
        t("panes.network.wallet.failedTitle"),
        error instanceof Error && error.message.length > 0 ? error.message : t("panes.network.wallet.failed"),
        { help: "custom-domain" },
      );
    } finally {
      setBusy(false);
    }
  };

  const start = () => {
    if (local !== null) {
      setAsking(true);
      return;
    }
    const eth = injected();
    if (eth === null) {
      toast.error(t("panes.network.wallet.failedTitle"), t("panes.network.wallet.noWallet"), { help: "index" });
      return;
    }
    void run(() => signInjected(eth))();
  };

  const off = disabled || busy;
  let body: React.ReactNode;
  if (names.length === 0) {
    body = (
      <Row last>
        <RowText title={t("panes.network.wallet.noNames")} detail={t("panes.network.wallet.noNamesDetail")} />
      </Row>
    );
  } else if (wallet !== null) {
    body = (
      <>
        {names.map((name) => (
          <Row key={name} data-testid="wallet-name">
            <RowText title={name} detail={t("panes.network.wallet.paysTo")} />
            <RowValue className="numeric select-all" title={wallet.address}>
              {shortAddress(wallet.address)}
            </RowValue>
          </Row>
        ))}
        <Row last>
          <RowText
            title={t("panes.network.wallet.bound")}
            detail={t("panes.network.wallet.boundDetail", { address: wallet.address })}
          />
          <Button size="sm" variant="secondary" disabled={off} onClick={run(async () => void (await bind(null)))}>
            {t("panes.network.wallet.unbind")}
          </Button>
        </Row>
      </>
    );
  } else {
    body = (
      <>
        <Row last={!asking}>
          <RowText
            title={t("panes.network.wallet.bind")}
            detail={t("panes.network.wallet.bindDetail", { names: names.join(", ") })}
          />
          <Button size="sm" disabled={off || asking} onClick={start} data-testid="wallet-bind">
            {t("panes.network.wallet.bindButton")}
          </Button>
        </Row>
        {asking && (
          <StackRow last>
            <RowText
              htmlFor={passwordId}
              title={t("panes.network.wallet.password")}
              detail={local === null ? undefined : t("panes.network.wallet.passwordDetail", { address: local })}
            />
            <form
              className="flex items-center gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                void run(signLocal)();
              }}
            >
              <Input
                id={passwordId}
                type="password"
                autoComplete="current-password"
                placeholder={t("panes.market.usdc.password")}
                value={password}
                disabled={off}
                className="w-[13rem]"
                onChange={(event) => setPassword(event.target.value)}
              />
              <Button type="submit" size="sm" disabled={off || password.length === 0}>
                {t("panes.network.wallet.sign")}
              </Button>
              <Button type="button" size="sm" variant="secondary" disabled={busy} onClick={() => setAsking(false)}>
                {t("panes.network.wallet.cancel")}
              </Button>
            </form>
          </StackRow>
        )}
      </>
    );
  }

  return (
    <PaneSection data-testid="domain-wallet">
      <GroupTitle>{t("panes.network.wallet.title")}</GroupTitle>
      <Group>{body}</Group>
      <GroupCaption>{t("panes.network.wallet.caption")}</GroupCaption>
    </PaneSection>
  );
}
