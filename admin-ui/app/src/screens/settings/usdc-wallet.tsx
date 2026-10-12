import * as React from "react";
import { createPublicClient, createWalletClient, custom, type Address, type Chain } from "viem";
import { base, baseSepolia } from "viem/chains";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { toast } from "@/components/ui/toast";
import {
  postMarketConfirm,
  postMarketPayout,
  postMarketRpc,
  type StablecoinInvoice,
  type UsdcPublic,
} from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import {
  accountFromPhrase,
  createPhrase,
  forgetPhrase,
  hasPhrase,
  openPhrase,
  payoutMessage,
  sealPhrase,
} from "@/wallet/hot";
import { Group, GroupCaption, GroupTitle, Row, RowText } from "./rows";

const APPROVE = [
  {
    type: "function",
    name: "approve",
    stateMutability: "nonpayable",
    inputs: [
      { name: "spender", type: "address" },
      { name: "amount", type: "uint256" },
    ],
    outputs: [{ type: "bool" }],
  },
] as const;

const PAY = [
  {
    type: "function",
    name: "pay",
    stateMutability: "nonpayable",
    inputs: [
      { name: "orderId", type: "bytes32" },
      { name: "seller", type: "address" },
      { name: "gross", type: "uint256" },
      { name: "fee", type: "uint256" },
    ],
    outputs: [],
  },
] as const;

type Mode = "none" | "local" | "injected";

interface Injected {
  request: (args: { method: string; params?: unknown[] }) => Promise<unknown>;
}

function injected(): Injected | null {
  const eth = (window as unknown as { ethereum?: Injected }).ethereum;
  return eth ?? null;
}

function chainOf(id: number): Chain {
  return id === base.id ? base : baseSepolia;
}

function transport() {
  return custom({
    async request({ method, params }) {
      const body = await postMarketRpc(method, params ?? []);
      if (body.error) throw new Error(body.error.message ?? "chain");
      return body.result;
    },
  });
}

export function UsdcWallet({
  usdc,
  payout,
  disabled,
}: {
  usdc: UsdcPublic;
  payout?: string | null;
  disabled: boolean;
}) {
  const t = useT();
  const [mode, setMode] = React.useState<Mode>("none");
  const [address, setAddress] = React.useState<string | null>(payout ?? null);
  const [phrase, setPhrase] = React.useState<string | null>(null);
  const [importText, setImportText] = React.useState("");
  const [password, setPassword] = React.useState("");
  const [wroteItDown, setWroteItDown] = React.useState(false);
  const [invoice, setInvoice] = React.useState<StablecoinInvoice | null>(readPending);
  const [busy, setBusy] = React.useState(false);
  const chainName = usdc.test ? "Base Sepolia" : "Base";

  React.useEffect(() => {
    const onPay = (ev: Event) => {
      const detail = (ev as CustomEvent<StablecoinInvoice>).detail;
      if (detail) setInvoice(detail);
    };
    window.addEventListener("losos-usdc-pay", onPay);
    return () => window.removeEventListener("losos-usdc-pay", onPay);
  }, []);

  React.useEffect(() => {
    void hasPhrase().then((yes) => {
      if (yes) setMode("local");
    });
  }, []);

  const fail = (error: unknown) => {
    const text = error instanceof Error ? error.message : t("panes.market.usdc.failed");
    toast.error(t("panes.market.usdc.failedTitle"), text, { help: "index" });
  };

  const useLocal = async (words: string) => {
    const account = accountFromPhrase(words);
    await sealPhrase(password, words);
    setAddress(account.address);
    setMode("local");
    setPhrase(null);
    setPassword("");
    setWroteItDown(false);
    toast.success(t("panes.market.usdc.saved"), account.address);
  };

  const create = () => {
    setPhrase(createPhrase());
    setWroteItDown(false);
  };

  const importPhrase = async () => {
    setBusy(true);
    try {
      await useLocal(importText);
      setImportText("");
    } catch {
      toast.error(t("panes.market.usdc.failedTitle"), t("panes.market.usdc.badPhrase"), { help: "index" });
    } finally {
      setBusy(false);
    }
  };

  const connectInjected = async () => {
    const eth = injected();
    if (eth === null) {
      toast.error(t("panes.market.usdc.failedTitle"), t("panes.market.usdc.noExtension"), { help: "index" });
      return;
    }
    setBusy(true);
    try {
      const accounts = (await eth.request({ method: "eth_requestAccounts" })) as string[];
      const first = accounts[0];
      if (!first) throw new Error("no account");
      setAddress(first);
      setMode("injected");
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  const signPayout = async () => {
    if (!address) return;
    setBusy(true);
    try {
      const message = payoutMessage(usdc.chain_id, address);
      let signature: string;
      if (mode === "injected") {
        const eth = injected();
        if (eth === null) throw new Error("no wallet");
        signature = (await eth.request({
          method: "personal_sign",
          params: [message, address],
        })) as string;
      } else {
        const words = await openPhrase(password);
        const account = accountFromPhrase(words);
        signature = await account.signMessage({ message });
        setAddress(account.address);
      }
      await postMarketPayout(address.toLowerCase(), signature);
      toast.success(t("panes.market.usdc.payoutSet"), address);
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  const payInvoice = async () => {
    if (!invoice || !address) return;
    setBusy(true);
    try {
      const chain = chainOf(invoice.chain_id);
      const account =
        mode === "local" ? accountFromPhrase(await openPhrase(password)) : undefined;
      const client = createWalletClient({
        account: account ?? (address as Address),
        chain,
        transport: mode === "injected" && injected() ? custom(injected()!) : transport(),
      });
      if (mode === "injected") {
        const eth = injected();
        const hexId = `0x${invoice.chain_id.toString(16)}`;
        try {
          await eth?.request({
            method: "wallet_switchEthereumChain",
            params: [{ chainId: hexId }],
          });
        } catch {
          await eth?.request({
            method: "wallet_addEthereumChain",
            params: [
              {
                chainId: hexId,
                chainName: chain.name,
                nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
                rpcUrls: [usdc.test ? "https://sepolia.base.org" : "https://mainnet.base.org"],
              },
            ],
          });
        }
      }
      const gross = BigInt(invoice.gross);
      await client.writeContract({
        address: invoice.usdc as Address,
        abi: APPROVE,
        functionName: "approve",
        args: [invoice.pay as Address, gross],
        account: account ?? (address as Address),
        chain,
      });
      const hash = await client.writeContract({
        address: invoice.pay as Address,
        abi: PAY,
        functionName: "pay",
        args: [invoice.order_id_hash as `0x${string}`, invoice.seller as Address, gross, BigInt(invoice.fee)],
        account: account ?? (address as Address),
        chain,
      });
      const pub = createPublicClient({ chain, transport: transport() });
      await pub.waitForTransactionReceipt({ hash });
      await postMarketConfirm(invoice.order_id, hash);
      sessionStorage.removeItem(PENDING_INVOICE);
      setInvoice(null);
      toast.success(t("panes.market.usdc.paid"), hash);
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <GroupTitle>{t("panes.market.usdc.title")}</GroupTitle>
      <Group>
        <Row last={invoice === null && phrase === null}>
          <RowText
            title={t("panes.market.usdc.howTitle")}
            detail={t("panes.market.usdc.how", { chain: chainName })}
          />
        </Row>
        {phrase !== null && (
          <Row>
            <div className="grid gap-2">
              <p className="text-[13px] text-ink">{t("panes.market.usdc.writeDown")}</p>
              <p className="font-mono text-[13px] break-all">{phrase}</p>
              <label className="flex items-center gap-2 text-[13px]">
                <input
                  type="checkbox"
                  checked={wroteItDown}
                  onChange={(e) => setWroteItDown(e.target.checked)}
                />
                {t("panes.market.usdc.wroteItDown")}
              </label>
              <Input
                type="password"
                autoComplete="new-password"
                placeholder={t("panes.market.usdc.password")}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
              <Button
                size="sm"
                disabled={disabled || busy || !wroteItDown || password.length < 8}
                onClick={() => void useLocal(phrase).catch(fail)}
              >
                {t("panes.market.usdc.saveWallet")}
              </Button>
            </div>
          </Row>
        )}
        <Row last={invoice === null}>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" disabled={disabled || busy} onClick={create}>
              {t("panes.market.usdc.create")}
            </Button>
            <Button size="sm" variant="secondary" disabled={disabled || busy} onClick={() => void connectInjected()}>
              {t("panes.market.usdc.connect")}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              disabled={disabled || busy || mode !== "local"}
              onClick={() => void forgetPhrase().then(() => setMode("none"))}
            >
              {t("panes.market.usdc.forget")}
            </Button>
          </div>
        </Row>
        <Row last={invoice === null}>
          <div className="grid gap-2">
            <Input
              placeholder={t("panes.market.usdc.importPlaceholder")}
              value={importText}
              onChange={(e) => setImportText(e.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
            {mode !== "local" && (
              <Input
                type="password"
                autoComplete="new-password"
                placeholder={t("panes.market.usdc.password")}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            )}
            <Button
              size="sm"
              variant="secondary"
              disabled={disabled || busy || importText.trim() === "" || password.length < 8}
              onClick={() => void importPhrase()}
            >
              {t("panes.market.usdc.import")}
            </Button>
          </div>
        </Row>
        <Row last={invoice === null}>
          <RowText
            title={t("panes.market.usdc.receiveTitle")}
            detail={
              payout
                ? t("panes.market.usdc.payoutReady", { address: payout })
                : t("panes.market.usdc.receive", { chain: chainName })
            }
          />
          <Button size="sm" disabled={disabled || busy || !address} onClick={() => void signPayout()}>
            {t("panes.market.usdc.setPayout")}
          </Button>
        </Row>
        {invoice !== null && (
          <Row last>
            <RowText
              title={t("panes.market.usdc.payTitle")}
              detail={t("panes.market.usdc.payDetail", {
                gross: formatUsdc(invoice.gross),
                fee: formatUsdc(invoice.fee),
                chain: chainName,
              })}
            />
            <Button size="sm" disabled={disabled || busy || !address} onClick={() => void payInvoice()}>
              {t("panes.market.usdc.pay")}
            </Button>
          </Row>
        )}
      </Group>
      <GroupCaption>{t("panes.market.usdc.gas", { chain: chainName })}</GroupCaption>
    </>
  );
}

const PENDING_INVOICE = "losos-usdc-invoice";

export function formatUsdc(baseUnits: string): string {
  const n = BigInt(baseUnits);
  const whole = n / 1_000_000n;
  const frac = (n % 1_000_000n).toString().padStart(6, "0").replace(/0+$/, "");
  return frac === "" ? `${whole} USDC` : `${whole}.${frac} USDC`;
}

function readPending(): StablecoinInvoice | null {
  try {
    const raw = sessionStorage.getItem(PENDING_INVOICE);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as StablecoinInvoice;
    return typeof parsed?.order_id === "string" ? parsed : null;
  } catch {
    return null;
  }
}

/** Keep a USDC invoice until the Market pane pays it, including one started
 *  from the Machines pane. */
export function rememberInvoice(invoice: StablecoinInvoice): void {
  sessionStorage.setItem(PENDING_INVOICE, JSON.stringify(invoice));
  window.dispatchEvent(new CustomEvent("losos-usdc-pay", { detail: invoice }));
}
