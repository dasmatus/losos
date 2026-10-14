import * as React from "react";
import { base58 } from "@scure/base";
import { createWalletClient, custom, type Address } from "viem";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { NativeSelect } from "@/components/ui/native-select";
import { toast } from "@/components/ui/toast";
import {
  ApiError,
  postMarketConfirm,
  postMarketPayout,
  type StablecoinInvoice,
  type UsdcChain,
  type UsdcPublic,
} from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import {
  balanceOn,
  ensAddress,
  ensName,
  ERC20,
  evmChain,
  evmClient,
  explorerTx,
  formatUnits,
  listNames,
  looksLikeName,
  parseUsdc,
  publicRpc,
  relay,
  shortAddress,
  type Balance,
} from "@/wallet/chains";
import {
  accountFromPhrase,
  createPhrase,
  forgetPhrase,
  hasPhrase,
  openPhrase,
  payoutMessage,
  savedAddresses,
  sealPhrase,
} from "@/wallet/hot";
import { isSolanaAddress, signMessage, solanaKeyFromPhrase } from "@/wallet/solana";
import { sendUsdc, solanaInjected, waitConfirmed, type SolanaInjected, type SolanaSigner } from "@/wallet/solana-pay";
import { Group, GroupCaption, GroupTitle, Row, RowText } from "./rows";

const APPROVE = [
  {
    type: "function",
    name: "allowance",
    stateMutability: "view",
    inputs: [
      { name: "owner", type: "address" },
      { name: "spender", type: "address" },
    ],
    outputs: [{ type: "uint256" }],
  },
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

/** A paid invoice's transaction and the network it is on, kept until the
 *  edge has counted it, so a failed confirmation is retried with the same
 *  transaction instead of paying twice. */
type PendingInvoice = StablecoinInvoice & { tx?: string; network?: string };

/** Start no payment this close to the end of the order's hold: the approve
 *  and the payment each need a block, and a payment after the hold is only
 *  honoured while its units are still free. */
const HOLD_MARGIN_SECS = 120;

/** How long a payment waits for its block, and then for the edge to count
 *  it, before the page stops and offers to check again. */
const CONFIRM_WAIT_MS = 60_000;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

interface Injected {
  request: (args: { method: string; params?: unknown[] }) => Promise<unknown>;
}

function injected(): Injected | null {
  const eth = (window as unknown as { ethereum?: Injected }).ethereum;
  return eth ?? null;
}

function Select({
  label,
  value,
  onChange,
  options,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
  disabled?: boolean;
}) {
  return (
    <NativeSelect aria-label={label} value={value} disabled={disabled} onChange={(e) => onChange(e.target.value)}>
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </NativeSelect>
  );
}

export function UsdcWallet({
  usdc,
  payout,
  payoutSolana,
  disabled,
  onPaid,
}: {
  usdc: UsdcPublic;
  payout?: string | null;
  payoutSolana?: string | null;
  disabled: boolean;
  /** Reload the market once the edge has counted a payment, so the order
   *  reads paid and the refresh announces it. */
  onPaid?: () => void;
}) {
  const t = useT();
  const [mode, setMode] = React.useState<Mode>("none");
  const [address, setAddress] = React.useState<string | null>(payout ?? null);
  const [solAddress, setSolAddress] = React.useState<string | null>(null);
  const [solProvider, setSolProvider] = React.useState<SolanaInjected | null>(null);
  const [phrase, setPhrase] = React.useState<string | null>(null);
  // A wallet made on this box lives only in this browser: a weak place to
  // collect sales, so the pane asks where they go and offers another wallet.
  const [madeHere, setMadeHere] = React.useState(false);
  const [otherOpen, setOtherOpen] = React.useState(false);
  const [otherAddress, setOtherAddress] = React.useState("");
  const [otherSignature, setOtherSignature] = React.useState("");
  const [importText, setImportText] = React.useState("");
  const [password, setPassword] = React.useState("");
  const [wroteItDown, setWroteItDown] = React.useState(false);
  const [invoice, setInvoice] = React.useState<PendingInvoice | null>(readPending);
  const [payOn, setPayOn] = React.useState<string>("");
  const [busy, setBusy] = React.useState(false);
  const [balances, setBalances] = React.useState<Balance[] | null>(null);
  const [names, setNames] = React.useState<Record<string, string>>({});
  const [sendTo, setSendTo] = React.useState("");
  const [sendAmount, setSendAmount] = React.useState("");
  // The last send, kept on the row with a link to its transaction.
  const [lastSent, setLastSent] = React.useState<{ text: string; href: string | null } | null>(null);
  const [sendOn, setSendOn] = React.useState(usdc.chains[0]?.network ?? "");
  const [resolved, setResolved] = React.useState<string | null>(null);

  const evmChains = usdc.chains.filter((c) => c.family === "evm");
  const solChains = usdc.chains.filter((c) => c.family === "solana");
  const allNames = listNames(
    usdc.chains.map((c) => c.name),
    t("panes.market.usdc.or"),
  );
  const ens = usdc.ens_chain_id !== null;

  React.useEffect(() => {
    const onPay = (ev: Event) => {
      const detail = (ev as CustomEvent<StablecoinInvoice>).detail;
      if (detail) setInvoice(detail);
    };
    window.addEventListener("losos-usdc-pay", onPay);
    return () => window.removeEventListener("losos-usdc-pay", onPay);
  }, []);

  React.useEffect(() => {
    void hasPhrase().then(async (yes) => {
      if (!yes) return;
      setMode("local");
      const saved = await savedAddresses();
      if (saved?.evm) setAddress(saved.evm);
      if (saved?.sol) setSolAddress(saved.sol);
      setMadeHere(saved?.made === true);
    });
  }, []);

  /** The networks this wallet can pay an invoice on. */
  const payable = React.useMemo(
    () =>
      (invoice?.chains ?? []).filter((c) =>
        c.family === "evm" ? mode !== "none" : mode === "local" || solProvider !== null,
      ),
    [invoice, mode, solProvider],
  );

  // Offer first the network holding enough USDC to pay, else the first.
  React.useEffect(() => {
    if (!invoice || payable.length === 0) return;
    if (payable.some((c) => c.network === payOn)) return;
    const gross = BigInt(invoice.gross);
    const funded = payable.find((c) => (balances ?? []).some((b) => b.network === c.network && b.usdc >= gross));
    const pick = funded ?? payable[0];
    if (pick) setPayOn(pick.network);
  }, [invoice, payable, payOn, balances]);

  const refreshBalances = React.useCallback(async () => {
    const reads = usdc.chains.map((c) => {
      const owner = c.family === "evm" ? address : solAddress;
      return owner ? balanceOn(c, owner) : Promise.resolve(null);
    });
    const found = (await Promise.all(reads)).filter((b): b is Balance => b !== null);
    setBalances(found);
  }, [usdc, address, solAddress]);

  React.useEffect(() => {
    if (address || solAddress) void refreshBalances();
  }, [address, solAddress, refreshBalances]);

  // ENS names for the wallet's address and the payout address.
  React.useEffect(() => {
    if (!ens) return;
    for (const a of [address, payout]) {
      if (!a || names[a.toLowerCase()] !== undefined) continue;
      void ensName(usdc.test, a).then((name) =>
        setNames((n) => ({ ...n, [a.toLowerCase()]: name ?? "" })),
      );
    }
  }, [ens, address, payout, usdc.test, names]);

  // Resolve a name typed into the send field as it is typed.
  React.useEffect(() => {
    setResolved(null);
    const chain = usdc.chains.find((c) => c.network === sendOn);
    if (!ens || chain?.family !== "evm" || !looksLikeName(sendTo)) return;
    const name = sendTo.trim();
    const timer = setTimeout(() => {
      void ensAddress(usdc.test, name).then((a) => setResolved(a ?? ""));
    }, 400);
    return () => clearTimeout(timer);
  }, [ens, sendTo, sendOn, usdc]);

  const named = (a: string) => {
    const name = names[a.toLowerCase()];
    return name ? `${name} (${shortAddress(a)})` : a;
  };

  const fail = (error: unknown) => {
    const text = error instanceof Error ? error.message : t("panes.market.usdc.failed");
    toast.error(t("panes.market.usdc.failedTitle"), text, { help: "index" });
  };

  /** The saved phrase, or a toast naming the password as the problem. */
  const unlock = async (): Promise<string | null> => {
    try {
      return await openPhrase(password);
    } catch {
      toast.error(t("panes.market.usdc.failedTitle"), t("panes.market.usdc.wrongPassword"), {
        help: "index",
      });
      return null;
    }
  };

  /** Tell the edge which transaction paid the order. It counts a payment
   *  once it is settled on its chain, so a "not yet" is retried for up to a
   *  minute before the owner is asked to check again. The bound is time, not
   *  tries: a slow chain endpoint makes each try take seconds. */
  const confirm = async (pending: PendingInvoice & { tx: string; network: string }) => {
    const giveUpAt = Date.now() + CONFIRM_WAIT_MS;
    for (;;) {
      try {
        await postMarketConfirm(pending.order_id, pending.tx, pending.network);
        break;
      } catch (error) {
        const retry = error instanceof ApiError && error.status === 409 && Date.now() < giveUpAt;
        if (!retry) throw error;
        await sleep(2_000);
      }
    }
    sessionStorage.removeItem(PENDING_INVOICE);
    setInvoice(null);
    void refreshBalances();
    if (onPaid) onPaid();
    else toast.success(t("panes.market.usdc.paid"), pending.tx);
  };

  const useLocal = async (words: string, made = false) => {
    const account = accountFromPhrase(words);
    const sol = solanaKeyFromPhrase(words);
    await sealPhrase(password, words, made);
    setMadeHere(made);
    setAddress(account.address);
    setSolAddress(sol.address);
    setSolProvider(null);
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
      // The extension holds its own key; a password typed for the browser
      // wallet has no use here and would sit in the import row.
      setPassword("");
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  const connectSolana = async () => {
    const provider = solanaInjected();
    if (provider === null) {
      toast.error(t("panes.market.usdc.failedTitle"), t("panes.market.usdc.noSolanaExtension"), { help: "index" });
      return;
    }
    setBusy(true);
    try {
      const { publicKey } = await provider.connect();
      setSolAddress(publicKey.toString());
      setSolProvider(provider);
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  /** Prove control of the payout addresses: the EVM one for every EVM
   *  network, and the Solana one when the edge settles there too. */
  const signPayout = async () => {
    if (!address && !solAddress) return;
    setBusy(true);
    try {
      let words: string | null = null;
      if (mode === "local") {
        words = await unlock();
        if (words === null) return;
      }
      if (address && usdc.chain_id !== null && mode !== "none") {
        const message = payoutMessage(usdc.chain_id, address);
        let signature: string;
        if (mode === "injected") {
          const eth = injected();
          if (eth === null) throw new Error("no wallet");
          signature = (await eth.request({ method: "personal_sign", params: [message, address] })) as string;
        } else {
          signature = await accountFromPhrase(words!).signMessage({ message });
        }
        await postMarketPayout(address.toLowerCase(), signature, "evm");
      }
      if (solAddress && usdc.solana !== null && (words !== null || solProvider !== null)) {
        const message = payoutMessage(usdc.solana, solAddress);
        let signature: string;
        if (words !== null) {
          signature = signMessage(solanaKeyFromPhrase(words), message);
        } else {
          const { signature: raw } = await solProvider!.signMessage(new TextEncoder().encode(message), "utf8");
          signature = base58.encode(raw);
        }
        await postMarketPayout(solAddress, signature, "solana");
      }
      toast.success(t("panes.market.usdc.payoutSet"), address ?? solAddress ?? "");
      if (onPaid) onPaid();
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  /** Set an address from a wallet kept elsewhere as the EVM payout: the
   *  owner signs the payout message there and pastes the signature. */
  const otherMessage =
    usdc.chain_id !== null && /^0x[0-9a-fA-F]{40}$/.test(otherAddress.trim())
      ? payoutMessage(usdc.chain_id, otherAddress.trim())
      : null;
  const saveOtherPayout = async () => {
    const signature = otherSignature.trim();
    if (otherMessage === null || !/^0x[0-9a-fA-F]{130}$/.test(signature)) {
      toast.error(t("panes.market.usdc.failedTitle"), t("panes.market.usdc.otherBad"), { help: "index" });
      return;
    }
    setBusy(true);
    try {
      await postMarketPayout(otherAddress.trim().toLowerCase(), signature, "evm");
      toast.success(t("panes.market.usdc.payoutSet"), otherAddress.trim());
      setOtherOpen(false);
      setOtherAddress("");
      setOtherSignature("");
      if (onPaid) onPaid();
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  /** Pay on an EVM chain: approve USDC for LososPay when short, then pay. */
  const payEvm = async (chain: UsdcChain & { seller: string }, words: string | null) => {
    if (!invoice || !address) return;
    const viemChain = evmChain(chain.network);
    const account = words !== null ? accountFromPhrase(words) : undefined;
    const eth = injected();
    const client = createWalletClient({
      account: account ?? (address as Address),
      chain: viemChain,
      transport: mode === "injected" && eth ? custom(eth) : relay(chain.network),
    });
    if (mode === "injected") {
      const hexId = `0x${viemChain.id.toString(16)}`;
      try {
        await eth?.request({ method: "wallet_switchEthereumChain", params: [{ chainId: hexId }] });
      } catch {
        await eth?.request({
          method: "wallet_addEthereumChain",
          params: [
            {
              chainId: hexId,
              chainName: viemChain.name,
              nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
              rpcUrls: [publicRpc(chain.network)],
            },
          ],
        });
      }
    }
    const owner = (account?.address ?? address) as Address;
    const pub = evmClient(chain.network);
    const gross = BigInt(invoice.gross);
    const allowed = await pub.readContract({
      address: chain.usdc as Address,
      abi: APPROVE,
      functionName: "allowance",
      args: [owner, chain.pay as Address],
    });
    if (allowed < gross) {
      const approval = await client.writeContract({
        address: chain.usdc as Address,
        abi: APPROVE,
        functionName: "approve",
        args: [chain.pay as Address, gross],
        account: account ?? owner,
        chain: viemChain,
      });
      // The payment's gas estimate runs against the chain as it is, so
      // the approval has to be in a block before the payment is built.
      await pub.waitForTransactionReceipt({ hash: approval });
    }
    const hash = await client.writeContract({
      address: chain.pay as Address,
      abi: PAY,
      functionName: "pay",
      args: [invoice.order_id_hash as `0x${string}`, chain.seller as Address, gross, BigInt(invoice.fee)],
      account: account ?? owner,
      chain: viemChain,
    });
    const paid = { ...invoice, tx: hash, network: chain.network };
    sessionStorage.setItem(PENDING_INVOICE, JSON.stringify(paid));
    setInvoice(paid);
    const mined = await pub.waitForTransactionReceipt({ hash, timeout: CONFIRM_WAIT_MS });
    if (mined.status !== "success") {
      sessionStorage.removeItem(PENDING_INVOICE);
      setInvoice(invoice);
      throw new Error(t("panes.market.usdc.failed"));
    }
    await confirm(paid);
  };

  /** Pay on Solana: the seller's share and the fee in one transaction, with
   *  the order in a memo. */
  const paySolana = async (chain: UsdcChain & { seller: string }, words: string | null) => {
    if (!invoice) return;
    const signer: SolanaSigner =
      words !== null
        ? { kind: "local", key: solanaKeyFromPhrase(words) }
        : { kind: "injected", address: solAddress!, provider: solProvider! };
    const gross = BigInt(invoice.gross);
    const fee = BigInt(invoice.fee);
    const signature = await sendUsdc(chain.network, signer, {
      mint: chain.usdc,
      seller: chain.seller,
      treasury: chain.treasury,
      sellerAmount: gross - fee,
      feeAmount: fee,
      memo: invoice.order_id_hash,
    });
    const paid = { ...invoice, tx: signature, network: chain.network };
    sessionStorage.setItem(PENDING_INVOICE, JSON.stringify(paid));
    setInvoice(paid);
    try {
      await waitConfirmed(chain.network, signature);
    } catch (error) {
      if (error instanceof Error && error.message === "the transaction failed") {
        sessionStorage.removeItem(PENDING_INVOICE);
        setInvoice(invoice);
      }
      throw error;
    }
    await confirm(paid);
  };

  const payInvoice = async () => {
    if (!invoice) return;
    if (payable.length === 0) {
      toast.error(t("panes.market.usdc.failedTitle"), t("panes.market.usdc.needWallet"), { help: "index" });
      return;
    }
    setBusy(true);
    try {
      // Already paid: only the edge's count is missing. Never pay again.
      if (invoice.tx && invoice.network) {
        await confirm({ ...invoice, tx: invoice.tx, network: invoice.network });
        return;
      }
      if (Date.now() / 1000 > invoice.pay_before - HOLD_MARGIN_SECS) {
        sessionStorage.removeItem(PENDING_INVOICE);
        setInvoice(null);
        toast.error(t("panes.market.usdc.failedTitle"), t("panes.market.usdc.lapsed"), { help: "index" });
        return;
      }
      const chain = payable.find((c) => c.network === payOn) ?? payable[0];
      if (!chain) return;
      let words: string | null = null;
      if (mode === "local" && (chain.family === "evm" || solProvider === null)) {
        words = await unlock();
        if (words === null) return;
      }
      if (chain.family === "evm") await payEvm(chain, words);
      else await paySolana(chain, words);
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  /** Send USDC out of the wallet: to an address, or to an ENS name on an
   *  EVM network. */
  const send = async () => {
    const chain = usdc.chains.find((c) => c.network === sendOn);
    const amount = parseUsdc(sendAmount);
    if (!chain || amount === null || amount === 0n) {
      toast.error(t("panes.market.usdc.failedTitle"), t("panes.market.usdc.badAmount"), { help: "index" });
      return;
    }
    const to = chain.family === "evm" && looksLikeName(sendTo) ? resolved : sendTo.trim();
    const valid =
      chain.family === "evm" ? !!to && /^0x[0-9a-fA-F]{40}$/.test(to) : !!to && isSolanaAddress(to);
    if (!to || !valid) {
      toast.error(t("panes.market.usdc.failedTitle"), t("panes.market.usdc.badRecipient"), { help: "index" });
      return;
    }
    setBusy(true);
    try {
      let words: string | null = null;
      if (mode === "local" && (chain.family === "evm" || solProvider === null)) {
        words = await unlock();
        if (words === null) return;
      }
      let tx: string;
      if (chain.family === "evm") {
        const account = words !== null ? accountFromPhrase(words) : undefined;
        const eth = injected();
        const viemChain = evmChain(chain.network);
        const client = createWalletClient({
          account: account ?? (address as Address),
          chain: viemChain,
          transport: mode === "injected" && eth ? custom(eth) : relay(chain.network),
        });
        tx = await client.writeContract({
          address: chain.usdc as Address,
          abi: ERC20,
          functionName: "transfer",
          args: [to as Address, amount],
          account: account ?? (address as Address),
          chain: viemChain,
        });
        await evmClient(chain.network).waitForTransactionReceipt({ hash: tx as `0x${string}` });
      } else {
        const signer: SolanaSigner =
          words !== null
            ? { kind: "local", key: solanaKeyFromPhrase(words) }
            : { kind: "injected", address: solAddress!, provider: solProvider! };
        tx = await sendUsdc(chain.network, signer, {
          mint: chain.usdc,
          seller: to,
          treasury: to,
          sellerAmount: amount,
          feeAmount: 0n,
          memo: "",
        });
        await waitConfirmed(chain.network, tx);
      }
      const text = t("panes.market.usdc.sent", {
        amount: formatUsdc(amount.toString()),
        to: looksLikeName(sendTo) ? sendTo.trim() : shortAddress(to),
      });
      toast.success(text, tx);
      setLastSent({ text, href: explorerTx(chain.network, tx) });
      setSendAmount("");
      setSendTo("");
      void refreshBalances();
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  const haveWallet = mode !== "none" || solProvider !== null;
  const needsPassword = mode === "local" && password === "";
  const invoiceChain = invoice?.chains.find((c) => c.network === (invoice.network ?? payOn));
  const sendChain = usdc.chains.find((c) => c.network === sendOn);

  const payoutDetail = () => {
    const parts: string[] = [];
    if (payout && evmChains.length > 0)
      parts.push(
        t("panes.market.usdc.payoutOn", { address: named(payout), chains: listNames(evmChains.map((c) => c.name), t("panes.market.usdc.and")) }),
      );
    if (payoutSolana && solChains.length > 0)
      parts.push(
        t("panes.market.usdc.payoutOn", { address: payoutSolana, chains: listNames(solChains.map((c) => c.name), t("panes.market.usdc.and")) }),
      );
    return parts.length > 0 ? parts.join(" ") : t("panes.market.usdc.receive", { chain: allNames });
  };

  return (
    <>
      <GroupTitle>{t("panes.market.usdc.title")}</GroupTitle>
      <Group>
        <Row last={false}>
          <RowText title={t("panes.market.usdc.howTitle")} detail={t("panes.market.usdc.how", { chain: allNames })} />
        </Row>
        {phrase !== null && (
          <Row>
            <div className="grid w-full gap-2">
              <p className="text-[13px] text-ink">{t("panes.market.usdc.writeDown")}</p>
              <p className="font-mono text-[13px] break-all">{phrase}</p>
              <label className="flex items-center gap-2 text-[13px]">
                <input type="checkbox" checked={wroteItDown} onChange={(e) => setWroteItDown(e.target.checked)} />
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
                onClick={() => void useLocal(phrase, true).catch(fail)}
              >
                {t("panes.market.usdc.saveWallet")}
              </Button>
            </div>
          </Row>
        )}
        <Row>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" disabled={disabled || busy} onClick={create}>
              {t("panes.market.usdc.create")}
            </Button>
            {evmChains.length > 0 && (
              <Button size="sm" variant="secondary" disabled={disabled || busy} onClick={() => void connectInjected()}>
                {t("panes.market.usdc.connect")}
              </Button>
            )}
            {solChains.length > 0 && (
              <Button size="sm" variant="secondary" disabled={disabled || busy} onClick={() => void connectSolana()}>
                {t("panes.market.usdc.connectSolana")}
              </Button>
            )}
            <Button
              size="sm"
              variant="secondary"
              disabled={disabled || busy || mode !== "local"}
              onClick={() =>
                void forgetPhrase().then(() => {
                  setMode("none");
                  setMadeHere(false);
                  setSolAddress(null);
                  setBalances(null);
                })
              }
            >
              {t("panes.market.usdc.forget")}
            </Button>
          </div>
        </Row>
        <Row>
          <div className="grid w-full gap-2">
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
        {mode === "local" && phrase === null && (
          <Row>
            <Input
              type="password"
              autoComplete="current-password"
              aria-label={t("panes.market.usdc.unlock")}
              placeholder={t("panes.market.usdc.unlock")}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Row>
        )}
        {(address || solAddress) && haveWallet && (
          <Row>
            <div className="grid w-full gap-1">
              <p className="text-[13px] font-medium text-ink">{t("panes.market.usdc.balancesTitle")}</p>
              {address && evmChains.length > 0 && (
                <p className="text-[13px] text-muted break-all">
                  {t("panes.market.usdc.evmAddress", { address: named(address) })}
                </p>
              )}
              {solAddress && solChains.length > 0 && (
                <p className="text-[13px] text-muted break-all">
                  {t("panes.market.usdc.solanaAddress", { address: solAddress })}
                </p>
              )}
              <ul className="grid gap-0.5 text-[13px] tabular-nums" aria-label={t("panes.market.usdc.balancesTitle")}>
                {(balances ?? []).map((b) => (
                  <li key={b.network} className="flex justify-between gap-4">
                    <span>{b.name}</span>
                    <span>
                      {formatUnits(b.usdc, 6, 2)} USDC · {formatUnits(b.gas, b.gasDecimals)} {b.gasSymbol}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
            <Button size="sm" variant="secondary" disabled={busy} onClick={() => void refreshBalances()}>
              {t("panes.market.usdc.refresh")}
            </Button>
          </Row>
        )}
        <Row last={invoice === null && !haveWallet && !otherOpen}>
          <RowText
            title={t("panes.market.usdc.receiveTitle")}
            detail={
              madeHere && mode === "local" && !payout && !payoutSolana
                ? t("panes.market.usdc.madeHere")
                : payoutDetail()
            }
          />
          <div className="flex shrink-0 flex-col items-end gap-2">
            <Button
              size="sm"
              disabled={disabled || busy || !haveWallet || (!address && !solAddress) || needsPassword}
              onClick={() => void signPayout()}
            >
              {t("panes.market.usdc.setPayout")}
            </Button>
            {usdc.chain_id !== null && (
              <Button size="sm" variant="secondary" disabled={disabled || busy} onClick={() => setOtherOpen((o) => !o)}>
                {t("panes.market.usdc.otherWallet")}
              </Button>
            )}
          </div>
        </Row>
        {otherOpen && usdc.chain_id !== null && (
          <Row last={invoice === null && !haveWallet}>
            <div className="grid w-full gap-2">
              <Input
                aria-label={t("panes.market.usdc.otherAddress")}
                placeholder={t("panes.market.usdc.otherAddress")}
                value={otherAddress}
                onChange={(e) => setOtherAddress(e.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
              {otherMessage !== null && (
                <>
                  <p className="text-[13px] text-muted">{t("panes.market.usdc.otherSign")}</p>
                  <pre className="rounded-md bg-sunk p-2 font-mono text-[12px] whitespace-pre-wrap break-all select-all">
                    {otherMessage}
                  </pre>
                  <Input
                    aria-label={t("panes.market.usdc.otherSignature")}
                    placeholder={t("panes.market.usdc.otherSignature")}
                    value={otherSignature}
                    onChange={(e) => setOtherSignature(e.target.value)}
                    autoComplete="off"
                    spellCheck={false}
                  />
                </>
              )}
              <Button
                size="sm"
                disabled={disabled || busy || otherMessage === null || otherSignature.trim() === ""}
                onClick={() => void saveOtherPayout()}
              >
                {t("panes.market.usdc.otherSave")}
              </Button>
            </div>
          </Row>
        )}
        {haveWallet && (
          <Row last={invoice === null}>
            <div className="grid w-full gap-2">
              <p className="text-[13px] font-medium text-ink">{t("panes.market.usdc.sendTitle")}</p>
              <div className="flex flex-wrap gap-2">
                <Input
                  className="min-w-[14rem] flex-1"
                  aria-label={t("panes.market.usdc.sendTo")}
                  placeholder={
                    sendChain?.family === "evm" && ens
                      ? t("panes.market.usdc.sendToName")
                      : t("panes.market.usdc.sendTo")
                  }
                  value={sendTo}
                  onChange={(e) => setSendTo(e.target.value)}
                  autoComplete="off"
                  spellCheck={false}
                />
                <Input
                  className="w-28"
                  inputMode="decimal"
                  aria-label={t("panes.market.usdc.sendAmount")}
                  placeholder="0.00"
                  value={sendAmount}
                  onChange={(e) => setSendAmount(e.target.value)}
                />
                <Select
                  label={t("panes.market.usdc.network")}
                  value={sendOn}
                  onChange={setSendOn}
                  options={usdc.chains.map((c) => ({ value: c.network, label: c.name }))}
                />
                <Button
                  size="sm"
                  disabled={disabled || busy || sendTo.trim() === "" || sendAmount.trim() === "" || needsPassword}
                  onClick={() => void send()}
                >
                  {t("panes.market.usdc.send")}
                </Button>
              </div>
              {lastSent !== null && (
                <p className="text-[13px] text-muted break-all">
                  {lastSent.text}
                  {lastSent.href !== null && (
                    <>
                      {" · "}
                      <a className="underline underline-offset-2 hover:text-ink" href={lastSent.href} target="_blank" rel="noopener noreferrer">
                        {t("panes.market.viewTx")}
                      </a>
                    </>
                  )}
                </p>
              )}
              {looksLikeName(sendTo) && sendChain?.family === "evm" && ens && (
                <p className="text-[13px] text-muted break-all" aria-live="polite">
                  {resolved === null
                    ? t("panes.market.usdc.resolving")
                    : resolved === ""
                      ? t("panes.market.usdc.noName", { name: sendTo.trim() })
                      : t("panes.market.usdc.resolved", { name: sendTo.trim(), address: resolved })}
                </p>
              )}
            </div>
          </Row>
        )}
        {invoice !== null && (
          <Row last>
            <div className="grid w-full gap-2">
              <RowText
                title={t("panes.market.usdc.payTitle")}
                detail={
                  invoice.tx
                    ? t("panes.market.usdc.counting")
                    : t("panes.market.usdc.payDetail", {
                        gross: formatUsdc(invoice.gross),
                        fee: formatUsdc(invoice.fee),
                        chain: invoiceChain?.name ?? allNames,
                      })
                }
              />
              {invoice.tx && invoiceChain && explorerTx(invoiceChain.network, invoice.tx) !== null && (
                <a
                  className="text-[13px] text-muted underline underline-offset-2 hover:text-ink"
                  href={explorerTx(invoiceChain.network, invoice.tx)!}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {t("panes.market.viewTx")}
                </a>
              )}
              {!invoice.tx && payable.length > 1 && (
                <label className="flex items-center gap-2 text-[13px] text-ink">
                  {t("panes.market.usdc.payOn")}
                  <Select
                    label={t("panes.market.usdc.payOn")}
                    value={payOn}
                    onChange={setPayOn}
                    disabled={busy}
                    options={payable.map((c) => ({ value: c.network, label: c.name }))}
                  />
                </label>
              )}
            </div>
            <Button
              size="sm"
              disabled={disabled || busy || payable.length === 0 || (needsPassword && !invoice.tx)}
              onClick={() => void payInvoice()}
            >
              {invoice.tx ? t("panes.market.usdc.checkAgain") : t("panes.market.usdc.pay")}
            </Button>
          </Row>
        )}
      </Group>
      <GroupCaption>
        {t("panes.market.usdc.gas", {
          chain: allNames,
        })}
      </GroupCaption>
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

function readPending(): PendingInvoice | null {
  try {
    const raw = sessionStorage.getItem(PENDING_INVOICE);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PendingInvoice;
    // An invoice from before the edge named its networks cannot be paid.
    return typeof parsed?.order_id === "string" && Array.isArray(parsed.chains) ? parsed : null;
  } catch {
    return null;
  }
}

/** Keep a USDC invoice until the Market pane pays it, including one started
 *  from the Machines pane. */
export function rememberInvoice(invoice: StablecoinInvoice): void {
  // A new order replaces an unpaid one, but never one whose payment the
  // edge has not counted yet: that money has already moved.
  const current = readPending();
  if (current?.tx) return;
  sessionStorage.setItem(PENDING_INVOICE, JSON.stringify(invoice));
  window.dispatchEvent(new CustomEvent("losos-usdc-pay", { detail: invoice }));
}
