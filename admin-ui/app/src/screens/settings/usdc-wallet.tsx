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
  postMarketFeesPay,
  postMarketPayout,
  type FeePayment,
  type StablecoinInvoice,
  type UsdcPublic,
  type Coin,
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
import { cheapest, estimateFee, formatFeeUsd, lowFeeFields, type FeeEstimate } from "@/wallet/fees";
import {
  payEscrow,
  sendUsdc,
  solanaInjected,
  waitConfirmed,
  type SolanaInjected,
  type SolanaSigner,
} from "@/wallet/solana-pay";
import { Group, GroupCaption, GroupTitle, Row, RowText } from "./rows";
import { COIN_NAMES, CoinKeyRow, CoinPayPanel } from "./coin-pay";
import { formatDay, formatMoney } from "./market";
import { OnrampTopUp } from "./onramp-topup";

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
      { name: "deadline", type: "uint64" },
      { name: "buyerBox", type: "address" },
      { name: "sellerBox", type: "address" },
    ],
    outputs: [{ name: "depositId", type: "uint256" }],
  },
] as const;

type Mode = "none" | "local" | "injected";

/** A box's escrow key from the invoice. An escrowed payment always names
 *  both; one without them would be a deposit the edge alone could move,
 *  which the contract refuses anyway. */
function boxKey(key: string | undefined): string {
  if (!key) throw new Error("the invoice names no escrow key");
  return key;
}

/** A paid invoice's transaction and the network it is on, kept until the
 *  edge has counted it, so a failed confirmation is retried with the same
 *  transaction instead of paying twice. */
type PendingInvoice = StablecoinInvoice & { tx?: string; network?: string };

/** Start no payment this close to the end of the order's hold: the approve
 *  and the payment each need a block, and a payment after the hold is only
 *  honoured while its units are still free. */
const HOLD_MARGIN_SECS = 120;

/** The edge's answer when it holds a payment it will not keep: the seller's
 *  address cannot receive it, or the order was paid or sold meanwhile. The
 *  escrow sends it back (edge/registrar's `RefundReason::message`). */
const REFUNDING = /being refunded/;

/** How long a payment waits for its block, and then for the edge to count
 *  it, before the page stops and offers to check again. */
const CONFIRM_WAIT_MS = 60_000;

/** The edge's "not yet" for a fee transfer: not mined, or short of blocks.
 *  Anything else it refuses is final. */
const FEE_NOT_YET = /not on chain yet|another block/;
/** A transfer the edge already counted: an earlier try got through. */
const FEE_COUNTED = /already counted/;

/** Platform fees owed on coin sales, from the account. Minor units. */
export interface CoinFees {
  owed: number;
  /** Unix seconds: when the debt began. */
  since: number | null;
  /** Kept from a USDC sale that took more than was owed. */
  credit: number;
  /** Above this, coin payments stop. */
  limit: number | null;
  /** Owed this long, any amount stops them. */
  graceSecs: number | null;
  paused: boolean;
  currency: string;
}

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
  coins = [],
  coinKeys = {},
  coinFees = null,
  disabled,
  onPaid,
}: {
  usdc: UsdcPublic;
  payout?: string | null;
  payoutSolana?: string | null;
  /** Coins other than USDC the edge takes, and this box's keys for them. */
  coins?: Coin[];
  coinKeys?: Partial<Record<Coin, string>>;
  /** Platform fees owed on this box's coin sales; null when the edge takes
   *  no coins. */
  coinFees?: CoinFees | null;
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
  // USDC, or a coin the seller takes. A seller with no USDC address leaves
  // only the coins.
  const [payWith, setPayWith] = React.useState<"usdc" | Coin>("usdc");
  const [busy, setBusy] = React.useState(false);
  const [balances, setBalances] = React.useState<Balance[] | null>(null);
  const [names, setNames] = React.useState<Record<string, string>>({});
  const [sendTo, setSendTo] = React.useState("");
  const [sendAmount, setSendAmount] = React.useState("");
  // The last send, kept on the row with a link to its transaction.
  const [lastSent, setLastSent] = React.useState<{ text: string; href: string | null } | null>(null);
  const [sendOn, setSendOn] = React.useState(usdc.chains[0]?.network ?? "");
  const [resolved, setResolved] = React.useState<string | null>(null);
  // Paying the platform fees owed on coin sales: the network, and a sent
  // transfer the edge has not counted yet, kept so it is never sent twice.
  const [feeOn, setFeeOn] = React.useState(usdc.chains.find((c) => c.family === "evm")?.network ?? "");
  const [feeTx, setFeeTx] = React.useState<PendingFee | null>(readPendingFee);

  const evmChains = usdc.chains.filter((c) => c.family === "evm");
  const solChains = usdc.chains.filter((c) => c.family === "solana");
  const allNames = listNames(
    usdc.chains.map((c) => c.name),
    t("panes.market.usdc.or"),
  );
  const ens = usdc.ens_chain_id !== null;
  // Euros buy USDC at Binance through the edge, which offers it only while
  // it settles on Base mainnet: Binance delivers there and nowhere else here.
  const onramp = usdc.onramp === "binance" && !usdc.test && evmChains.some((c) => c.network === "base");

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

  // What paying the invoice costs in network fees on each network it can be
  // paid on, at the lowest fee that still confirms.
  const [payFees, setPayFees] = React.useState<Record<string, FeeEstimate | null>>({});
  const payPicked = React.useRef(false);
  React.useEffect(() => {
    if (!invoice || invoice.tx || payable.length === 0) return;
    let live = true;
    const amount = BigInt(invoice.gross);
    void Promise.all(
      payable.map((c) =>
        estimateFee(
          c,
          {
            kind: "pay",
            amount,
            owner: (c.family === "evm" ? address : solAddress) ?? undefined,
            recipients: [c.seller],
          },
          usdc.prices,
        ),
      ),
    ).then((found) => {
      if (live) setPayFees(Object.fromEntries(payable.map((c, i) => [c.network, found[i] ?? null])));
    });
    return () => {
      live = false;
    };
  }, [invoice, payable, address, solAddress, usdc.prices]);

  // Offer first USDC on Solana through the escrow, where it settles in
  // seconds for a fraction of a cent, unless the wallet holds the USDC only
  // elsewhere; then the network where the wallet holds the USDC and the fee
  // and the fee is lowest; else the first holding the USDC; else the first.
  // A network the owner picked stays picked.
  React.useEffect(() => {
    if (!invoice || payable.length === 0) return;
    if (payPicked.current && payable.some((c) => c.network === payOn)) return;
    const gross = BigInt(invoice.gross);
    const low = cheapest(
      payable.map((c) => payFees[c.network] ?? null),
      balances,
      gross,
    );
    const holds = (c: { network: string }) =>
      (balances ?? []).some((b) => b.network === c.network && b.usdc >= gross);
    const funded = payable.find(holds);
    const escrow = payable.find((c) => c.family === "solana" && c.pay);
    const preferred = escrow && (holds(escrow) || funded === undefined) ? escrow.network : null;
    const pick = preferred ?? low ?? funded?.network ?? payable[0]?.network;
    if (pick && pick !== payOn) setPayOn(pick);
  }, [invoice, payable, payOn, balances, payFees]);

  // A new invoice starts on the cheapest network again, and on USDC unless
  // the seller takes only coins.
  React.useEffect(() => {
    payPicked.current = false;
    const onlyCoins = invoice !== null && invoice.chains.length === 0 ? invoice.coins?.[0] : undefined;
    setPayWith(onlyCoins ?? "usdc");
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a new order, not each poll of the same one
  }, [invoice?.order_id]);

  /** The edge counted a coin payment: the order is paid. */
  const coinPaid = React.useCallback(() => {
    sessionStorage.removeItem(PENDING_INVOICE);
    setInvoice(null);
    if (onPaid) onPaid();
    else toast.success(t("panes.market.usdc.paid"));
  }, [onPaid, t]);

  // The same for a plain send: the fee of a USDC transfer on each network,
  // and the cheapest network holding the amount is offered first.
  const [sendFees, setSendFees] = React.useState<Record<string, FeeEstimate | null>>({});
  const sendPicked = React.useRef(false);
  React.useEffect(() => {
    if (!address && !solAddress) return;
    let live = true;
    void Promise.all(usdc.chains.map((c) => estimateFee(c, { kind: "send", amount: 0n }, usdc.prices))).then(
      (found) => {
        if (live) setSendFees(Object.fromEntries(usdc.chains.map((c, i) => [c.network, found[i] ?? null])));
      },
    );
    return () => {
      live = false;
    };
  }, [usdc, address, solAddress]);
  React.useEffect(() => {
    if (sendPicked.current || balances === null) return;
    const amount = parseUsdc(sendAmount) ?? 1n;
    const low = cheapest(
      usdc.chains.map((c) => sendFees[c.network] ?? null),
      balances,
      amount > 0n ? amount : 1n,
    );
    if (low && low !== sendOn) setSendOn(low);
  }, [usdc, balances, sendFees, sendAmount, sendOn]);

  /** A network's name with what its fee costs, when that is known. */
  const withFee = (name: string, fee: FeeEstimate | null | undefined) =>
    fee?.usdMicros != null ? t("panes.market.usdc.withFee", { chain: name, fee: formatFeeUsd(fee.usdMicros) }) : name;

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
   *  tries: a slow chain endpoint makes each try take seconds. A payment
   *  the edge is sending back is final: it is not retried, and the invoice
   *  is closed so nothing is paid twice. */
  const confirm = async (pending: PendingInvoice & { tx: string; network: string }) => {
    const giveUpAt = Date.now() + CONFIRM_WAIT_MS;
    for (;;) {
      try {
        await postMarketConfirm(pending.order_id, pending.tx, pending.network);
        break;
      } catch (error) {
        if (error instanceof ApiError && error.status === 409 && REFUNDING.test(error.message)) {
          sessionStorage.removeItem(PENDING_INVOICE);
          setInvoice(null);
          void refreshBalances();
          toast.error(t("panes.market.usdc.refundedTitle"), error.message, { help: "index" });
          return;
        }
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

  /** Point a browser extension's wallet at `network`, adding it when the
   *  extension does not know it. A wallet on this box needs nothing. */
  const switchInjected = async (network: string) => {
    if (mode !== "injected") return;
    const eth = injected();
    const viemChain = evmChain(network);
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
            rpcUrls: [publicRpc(network)],
          },
        ],
      });
    }
  };

  /** Ask the edge to count a sent fee transfer. A "not yet" is retried for
   *  up to a minute like an order's payment; a final refusal drops the
   *  transfer, which the edge will never count. */
  const countFees = async (pending: PendingFee) => {
    const giveUpAt = Date.now() + CONFIRM_WAIT_MS;
    let reply: FeePayment | null = null;
    for (;;) {
      try {
        reply = await postMarketFeesPay(pending.tx, pending.network);
        break;
      } catch (error) {
        if (error instanceof ApiError && error.status === 409 && FEE_COUNTED.test(error.message)) break;
        const notYet = error instanceof ApiError && error.status === 409 && FEE_NOT_YET.test(error.message);
        if (notYet && Date.now() < giveUpAt) {
          await sleep(2_000);
          continue;
        }
        if (error instanceof ApiError && (error.status === 400 || (error.status === 409 && !notYet))) {
          sessionStorage.removeItem(PENDING_FEE);
          setFeeTx(null);
        }
        throw error;
      }
    }
    sessionStorage.removeItem(PENDING_FEE);
    setFeeTx(null);
    void refreshBalances();
    const currency = coinFees?.currency ?? "usd";
    toast.success(
      t("panes.market.coinFees.paidTitle"),
      reply === null
        ? pending.tx
        : t("panes.market.coinFees.paidDetail", {
            credited: formatMoney(reply.credited, currency),
            owed: formatMoney(reply.owed, currency),
          }),
    );
    onPaid?.();
  };

  /** Pay the platform fees owed on coin sales: a plain USDC transfer from
   *  the payout address to the edge's treasury, then the edge counts it. */
  const settleFees = async () => {
    if (feeTx !== null) {
      setBusy(true);
      try {
        await countFees(feeTx);
      } catch (error) {
        fail(error);
      } finally {
        setBusy(false);
      }
      return;
    }
    const chain = evmChains.find((c) => c.network === feeOn) ?? evmChains[0];
    // The market counts cents and USDC has six decimals.
    const amount = BigInt(coinFees?.owed ?? 0) * 10_000n;
    if (!chain || !address || amount === 0n) return;
    setBusy(true);
    try {
      let words: string | null = null;
      if (mode === "local") {
        words = await unlock();
        if (words === null) return;
      }
      const account = words !== null ? accountFromPhrase(words) : undefined;
      const eth = injected();
      const viemChain = evmChain(chain.network);
      const client = createWalletClient({
        account: account ?? (address as Address),
        chain: viemChain,
        transport: mode === "injected" && eth ? custom(eth) : relay(chain.network),
      });
      await switchInjected(chain.network);
      const tx = await client.writeContract({
        address: chain.usdc as Address,
        abi: ERC20,
        functionName: "transfer",
        args: [chain.treasury as Address, amount],
        account: account ?? (address as Address),
        chain: viemChain,
        ...(await lowFeeFields(chain.network)),
      });
      const pending = { tx, network: chain.network };
      sessionStorage.setItem(PENDING_FEE, JSON.stringify(pending));
      setFeeTx(pending);
      const mined = await evmClient(chain.network).waitForTransactionReceipt({ hash: tx, timeout: CONFIRM_WAIT_MS });
      if (mined.status !== "success") {
        sessionStorage.removeItem(PENDING_FEE);
        setFeeTx(null);
        throw new Error(t("panes.market.usdc.failed"));
      }
      await countFees(pending);
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  /** Pay on an EVM chain: approve USDC for LososEscrow when short, then
   *  pay into it. The edge checks the seller's address and releases the
   *  deposit, or sends it back. */
  const payEvm = async (chain: StablecoinInvoice["chains"][number], words: string | null) => {
    if (!invoice || !address) return;
    const viemChain = evmChain(chain.network);
    const account = words !== null ? accountFromPhrase(words) : undefined;
    const eth = injected();
    const client = createWalletClient({
      account: account ?? (address as Address),
      chain: viemChain,
      transport: mode === "injected" && eth ? custom(eth) : relay(chain.network),
    });
    await switchInjected(chain.network);
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
        ...(await lowFeeFields(chain.network)),
      });
      // The payment's gas estimate runs against the chain as it is, so
      // the approval has to be in a block before the payment is built.
      await pub.waitForTransactionReceipt({ hash: approval });
    }
    const hash = await client.writeContract({
      address: chain.pay as Address,
      abi: PAY,
      functionName: "pay",
      args: [
        invoice.order_id_hash as `0x${string}`,
        chain.seller as Address,
        gross,
        BigInt(invoice.fee),
        BigInt(invoice.deadline),
        boxKey(chain.buyer_box) as Address,
        boxKey(chain.seller_box) as Address,
      ],
      account: account ?? owner,
      chain: viemChain,
      ...(await lowFeeFields(chain.network)),
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

  /** Pay on Solana: into the escrow program when the edge has one, which
   *  holds the payment until the slot is over; else the seller's share and
   *  the fee in one transaction, with the order in a memo. */
  const paySolana = async (chain: StablecoinInvoice["chains"][number], words: string | null) => {
    if (!invoice) return;
    const signer: SolanaSigner =
      words !== null
        ? { kind: "local", key: solanaKeyFromPhrase(words) }
        : { kind: "injected", address: solAddress!, provider: solProvider! };
    const gross = BigInt(invoice.gross);
    const fee = BigInt(invoice.fee);
    const signature = chain.pay
      ? await payEscrow(chain.network, signer, {
          program: chain.pay,
          mint: chain.usdc,
          treasury: chain.treasury,
          seller: chain.seller,
          orderIdHash: invoice.order_id_hash,
          gross,
          fee,
          deadline: BigInt(invoice.deadline),
          buyerBox: boxKey(chain.buyer_box),
          sellerBox: boxKey(chain.seller_box),
        })
      : await sendUsdc(chain.network, signer, {
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
          ...(await lowFeeFields(chain.network)),
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
  // The edge counts a fee transfer only from the signed payout address.
  const fromPayout =
    mode !== "none" && !!address && !!payout && address.toLowerCase() === payout.toLowerCase();
  const showFees =
    coinFees !== null && (coinFees.owed > 0 || coinFees.credit > 0 || feeTx !== null);

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
          <RowText
            title={t("panes.market.usdc.howTitle")}
            detail={
              t("panes.market.usdc.how", { chain: allNames }) +
              (coins.length > 0
                ? ` ${t("panes.market.coins.also", {
                    coins: listNames(
                      coins.map((c) => COIN_NAMES[c]),
                      t("panes.market.usdc.or"),
                    ),
                  })}`
                : "")
            }
          />
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
        {address && onramp && (
          <OnrampTopUp address={address} disabled={disabled} onArrived={() => void refreshBalances()} />
        )}
        <Row last={invoice === null && !haveWallet && !otherOpen && coins.length === 0 && !showFees}>
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
          <Row last={invoice === null && !haveWallet && coins.length === 0 && !showFees}>
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
        {coins.map((c, i) => (
          <CoinKeyRow
            key={c}
            coin={c}
            saved={coinKeys[c]}
            disabled={disabled}
            last={invoice === null && !haveWallet && !showFees && i === coins.length - 1}
            onSaved={() => onPaid?.()}
          />
        ))}
        {showFees && coinFees && (
          <Row last={invoice === null && !haveWallet}>
            <div className="grid w-full gap-2" data-testid="coin-fees">
              <p className="text-[13px] font-medium text-ink">{t("panes.market.coinFees.title")}</p>
              {coinFees.owed > 0 && (
                <p className="text-[13px] text-muted">
                  {t("panes.market.coinFees.owed", { amount: formatMoney(coinFees.owed, coinFees.currency) })}
                </p>
              )}
              {coinFees.paused ? (
                <p className="text-[13px] text-ink" role="status">
                  {t("panes.market.coinFees.paused")}
                </p>
              ) : (
                coinFees.owed > 0 &&
                coinFees.limit !== null &&
                coinFees.since !== null &&
                coinFees.graceSecs !== null && (
                  <p className="text-[13px] text-muted">
                    {t("panes.market.coinFees.limits", {
                      limit: formatMoney(coinFees.limit, coinFees.currency),
                      date: formatDay(coinFees.since + coinFees.graceSecs),
                    })}
                  </p>
                )
              )}
              {coinFees.credit > 0 && (
                <p className="text-[13px] text-muted">
                  {t("panes.market.coinFees.credit", { amount: formatMoney(coinFees.credit, coinFees.currency) })}
                </p>
              )}
              {(coinFees.owed > 0 || feeTx !== null) && evmChains.length > 0 && (
                <>
                  <p className="text-[13px] text-muted">
                    {!payout
                      ? t("panes.market.coinFees.noPayout")
                      : fromPayout || feeTx !== null
                        ? t("panes.market.coinFees.payFrom")
                        : t("panes.market.coinFees.notPayout", { address: named(payout) })}
                  </p>
                  {feeTx !== null && <p className="text-[13px] text-muted">{t("panes.market.coinFees.counting")}</p>}
                  <div className="flex flex-wrap items-center gap-2">
                    {feeTx === null && evmChains.length > 1 && (
                      <Select
                        label={t("panes.market.usdc.network")}
                        value={feeOn}
                        onChange={setFeeOn}
                        disabled={busy}
                        options={evmChains.map((c) => ({ value: c.network, label: withFee(c.name, sendFees[c.network]) }))}
                      />
                    )}
                    <Button
                      size="sm"
                      disabled={disabled || busy || (feeTx === null && (!fromPayout || needsPassword))}
                      onClick={() => void settleFees()}
                    >
                      {feeTx !== null
                        ? t("panes.market.usdc.checkAgain")
                        : t("panes.market.coinFees.pay", { amount: formatMoney(coinFees.owed, coinFees.currency) })}
                    </Button>
                  </div>
                </>
              )}
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
                  onChange={(v) => {
                    sendPicked.current = true;
                    setSendOn(v);
                  }}
                  options={usdc.chains.map((c) => ({ value: c.network, label: withFee(c.name, sendFees[c.network]) }))}
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
                    : payWith !== "usdc"
                      ? t("panes.market.coins.payDetail", {
                          gross: formatMoney(Number(BigInt(invoice.gross) / 10_000n), "usd"),
                          coin: COIN_NAMES[payWith],
                        })
                      : invoicePayDetail(t, invoice, invoiceChain?.name ?? allNames)
                }
              />
              {!invoice.tx && (invoice.coins?.length ?? 0) > 0 && (
                <label className="flex items-center gap-2 text-[13px] text-ink">
                  {t("panes.market.coins.payWith")}
                  <Select
                    label={t("panes.market.coins.payWith")}
                    value={payWith}
                    onChange={(v) => setPayWith(v as "usdc" | Coin)}
                    disabled={busy}
                    options={[
                      ...(invoice.chains.length > 0 ? [{ value: "usdc", label: "USDC" }] : []),
                      ...(invoice.coins ?? []).map((c) => ({ value: c, label: COIN_NAMES[c] })),
                    ]}
                  />
                </label>
              )}
              {!invoice.tx && payWith !== "usdc" && (
                <CoinPayPanel orderId={invoice.order_id} coin={payWith} onPaid={coinPaid} />
              )}
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
              {!invoice.tx && payWith === "usdc" && payable.length > 1 && (
                <label className="flex items-center gap-2 text-[13px] text-ink">
                  {t("panes.market.usdc.payOn")}
                  <Select
                    label={t("panes.market.usdc.payOn")}
                    value={payOn}
                    onChange={(v) => {
                      payPicked.current = true;
                      setPayOn(v);
                    }}
                    disabled={busy}
                    options={payable.map((c) => ({ value: c.network, label: withFee(c.name, payFees[c.network]) }))}
                  />
                </label>
              )}
            </div>
            {(payWith === "usdc" || invoice.tx) && (
              <Button
                size="sm"
                disabled={disabled || busy || payable.length === 0 || (needsPassword && !invoice.tx)}
                onClick={() => void payInvoice()}
              >
                {invoice.tx ? t("panes.market.usdc.checkAgain") : t("panes.market.usdc.pay")}
              </Button>
            )}
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
const PENDING_FEE = "losos-coin-fee-tx";

/** A fee transfer sent to the treasury that the edge has not counted yet. */
type PendingFee = { tx: `0x${string}`; network: string };

function readPendingFee(): PendingFee | null {
  try {
    const raw = sessionStorage.getItem(PENDING_FEE);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PendingFee;
    return typeof parsed?.tx === "string" && typeof parsed.network === "string" ? parsed : null;
  } catch {
    return null;
  }
}

/** What the buyer pays and where it goes. When part of the fee settles the
 *  seller's platform fees on coin sales, the buyer still pays `gross` and
 *  the line says how the fee splits. */
export function invoicePayDetail(t: ReturnType<typeof useT>, invoice: StablecoinInvoice, chain: string): string {
  const offset = BigInt(invoice.fee_offset ?? "0");
  return offset > 0n
    ? t("panes.market.usdc.payDetailOffset", {
        gross: formatUsdc(invoice.gross),
        fee: formatUsdc(invoice.fee),
        offset: formatUsdc(offset.toString()),
        chain,
      })
    : t("panes.market.usdc.payDetail", {
        gross: formatUsdc(invoice.gross),
        fee: formatUsdc(invoice.fee),
        chain,
      });
}

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
