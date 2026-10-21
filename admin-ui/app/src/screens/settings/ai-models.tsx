import * as React from "react";
import { mnemonicToAccount } from "viem/accounts";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { FieldError, Input } from "@/components/ui/input";
import { NativeSelect } from "@/components/ui/native-select";
import { Progress, Spinner } from "@/components/ui/progress";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import {
  ApiError,
  isAbort,
  isUnauthorized,
  postLlmChat,
  postLlmModels,
  postTapConfirm,
  postTapQuote,
  postTapStop,
  type ChatAnswer,
  type ChatMessage,
  type LlmLending,
  type LlmMeter,
  type LlmModel,
  type LlmResponse,
  type LlmWallet,
  type OptionDoc,
  type UsdcChain,
  type UsdcPublic,
} from "@/lib/api";
import { intlTag, t as translate } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { toLiteral } from "@/lib/option-value";
import { cn } from "@/lib/utils";
import { parseUsdc, shortAddress } from "@/wallet/chains";
import { openPhrase, savedAddresses } from "@/wallet/hot";
import { solanaKeyFromPhrase } from "@/wallet/solana";
import { solanaInjected } from "@/wallet/solana-pay";
import { askBack, escrowBalance, injectedEvm, signerAddress, startTaps, takeBack, type TapSigner } from "@/wallet/taps";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow, SwitchRow } from "./rows";
import type { SettingsForm } from "./use-settings-form";

/* AI models on the Market pane: large models that run split across the
 * spare memory several boxes lend, paid per token through the edge
 * (edge/registrar/src/server/llm.rs, relayed by lososd).
 *
 * Buying is a meter: the edge quotes one tap per lending box, the owner's
 * wallet tops up a balance it keeps in the escrow and opens the taps, and
 * the edge reads them back from the chain before it serves a request. Each
 * answer is billed by its tokens, never more in an hour than the owner
 * allowed. Stopping needs no wallet: the edge closes the taps. What is left
 * in the balance comes back to the wallet after the escrow's notice.
 *
 * Lending is two lines of overrides.nix (`losos.inference.lend.*`), edited
 * through form.extra like the Apps pane's site settings, so it goes out
 * with Apply and the Advanced pane links here (lib/option-value.ts,
 * ON_PANE). */

const LEND_ENABLE = "inference.lend.enable";
const LEND_MEMORY = "inference.lend.memoryMiB";
const MIB_PER_GIB = 1024;

/** The meters still billing, or about to. */
const LIVE: ReadonlySet<LlmMeter["state"]> = new Set(["quoted", "open", "ending"]);

/** A quote whose taps the wallet sent, kept until the edge has read them
 *  back, so a confirmation that failed is retried rather than paid again. */
const PENDING = "losos-llm-meter";

/** How long a confirmation waits for the chain to show every tap. */
const CONFIRM_WAIT_MS = 90_000;
const NOT_YET = /every tap yet/;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// ── Money and time ────────────────────────────────────────────────────────

/** Dollars from USDC base units, with the cents a small price needs. */
export function formatUsd(baseUnits: bigint | string | number): string {
  const units = typeof baseUnits === "bigint" ? Number(baseUnits) : Number(baseUnits);
  const dollars = units / 1_000_000;
  const places = dollars === 0 || dollars >= 1 ? 2 : dollars >= 0.01 ? 4 : 6;
  return new Intl.NumberFormat(intlTag(), {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: places,
  }).format(dollars);
}

/** Dollars from millionths of a base unit (`billedE6`, `costE6`). */
function formatE6(e6: string): string {
  return formatUsd(Number(BigInt(e6) / 1_000_000n));
}

function formatSpan(secs: number): string {
  if (secs < 3600) return translate("panes.market.ai.time.minutes", { count: Math.max(1, Math.round(secs / 60)) });
  if (secs < 2 * 86400) return translate("panes.market.ai.time.hours", { count: Math.round(secs / 3600) });
  return translate("panes.market.ai.time.days", { count: Math.round(secs / 86400) });
}

function formatWhen(epochSeconds: number): string {
  return new Intl.DateTimeFormat(intlTag(), { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(epochSeconds * 1000),
  );
}

function gib(mib: number): string {
  return translate("panes.market.ai.gib", { count: Math.round((mib / MIB_PER_GIB) * 10) / 10 });
}

/** Base units as a USDC amount for a field: "2.5". */
function usdcText(units: bigint): string {
  const whole = units / 1_000_000n;
  const frac = (units % 1_000_000n).toString().padStart(6, "0").replace(/0+$/, "");
  return frac === "" ? whole.toString() : `${whole}.${frac}`;
}

// ── The data ──────────────────────────────────────────────────────────────

export type LlmState =
  | { kind: "off" }
  | { kind: "loading" }
  | { kind: "unavailable" }
  | { kind: "failed"; message: string }
  | ({ kind: "ready" } & Extract<LlmResponse, { available: true }>);

export interface LlmData {
  state: LlmState;
  /** The network asked for; null lets the edge pick its first. */
  network: string | null;
  setNetwork: (network: string) => void;
  refresh: () => void;
}

/** The edge's models and this box's meters, balances and lending, while
 *  `enabled`. Read again every half minute while something is changing: a
 *  model loading, a meter waiting or stopping. */
export function useLlm(enabled: boolean): LlmData {
  const [state, setState] = React.useState<LlmState>({ kind: enabled ? "loading" : "off" });
  const [network, setNetwork] = React.useState<string | null>(null);
  const [tick, setTick] = React.useState(0);
  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  React.useEffect(() => {
    if (!enabled) {
      setState({ kind: "off" });
      return;
    }
    const controller = new AbortController();
    void (async () => {
      try {
        const reply = await postLlmModels(network, { signal: controller.signal });
        if (controller.signal.aborted) return;
        setState(reply.available ? { kind: "ready", ...reply } : { kind: "unavailable" });
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        setState({ kind: "failed", message: error instanceof Error ? error.message : "" });
      }
    })();
    return () => controller.abort();
  }, [enabled, network, tick]);

  const changing =
    state.kind === "ready" &&
    (state.models.some((m) => m.group === "loading" || m.group === "down") ||
      state.meters.some((m) => m.state === "quoted" || m.state === "ending"));
  React.useEffect(() => {
    if (!changing) return;
    const timer = setInterval(refresh, 30_000);
    return () => clearInterval(timer);
  }, [changing, refresh]);

  return { state, network, setNetwork, refresh };
}

function said(error: unknown): string {
  const text = error instanceof ApiError || error instanceof Error ? error.message : "";
  return text.length > 0 ? text : translate("panes.market.ai.failedBody");
}

function fail(error: unknown): void {
  if (isUnauthorized(error)) return;
  toast.error(translate("panes.market.ai.failedTitle"), said(error), { help: "index" });
}

// ── The wallet that pays ──────────────────────────────────────────────────

type PayFrom = "local" | "injected";

interface Payer {
  choices: PayFrom[];
  from: PayFrom | null;
  setFrom: (f: PayFrom) => void;
  password: string;
  setPassword: (p: string) => void;
  /** The paying address, when it is known before signing. */
  address: string | null;
  /** Unlock or connect the wallet; null after a toast saying why not. */
  sign: () => Promise<TapSigner | null>;
}

function usePayer(family: UsdcChain["family"] | null): Payer {
  const [saved, setSaved] = React.useState<{ evm?: string; sol?: string } | null>(null);
  const [from, setFrom] = React.useState<PayFrom | null>(null);
  const [password, setPassword] = React.useState("");
  const [connected, setConnected] = React.useState<{ evm?: string; sol?: string }>({});

  React.useEffect(() => {
    void savedAddresses()
      .then(setSaved)
      .catch(() => setSaved(null));
  }, []);

  const local = saved !== null && (family === "solana" ? saved.sol !== undefined : saved.evm !== undefined);
  const extension = family === "solana" ? solanaInjected() !== null : family === "evm" ? injectedEvm() !== null : false;
  const choices: PayFrom[] = [...(local ? (["local"] as const) : []), ...(extension ? (["injected"] as const) : [])];
  const chosen = from !== null && choices.includes(from) ? from : (choices[0] ?? null);
  const address =
    chosen === "local"
      ? ((family === "solana" ? saved?.sol : saved?.evm) ?? null)
      : chosen === "injected"
        ? ((family === "solana" ? connected.sol : connected.evm) ?? null)
        : null;

  const sign = async (): Promise<TapSigner | null> => {
    if (chosen === null || family === null) {
      toast.error(translate("panes.market.ai.failedTitle"), translate("panes.market.ai.noWallet"), { help: "index" });
      return null;
    }
    if (chosen === "local") {
      let words: string;
      try {
        words = await openPhrase(password);
      } catch {
        toast.error(translate("panes.market.ai.failedTitle"), translate("panes.market.usdc.wrongPassword"), {
          help: "index",
        });
        return null;
      }
      return family === "evm"
        ? { family: "evm", kind: "local", account: mnemonicToAccount(words) }
        : { family: "solana", signer: { kind: "local", key: solanaKeyFromPhrase(words) } };
    }
    if (family === "evm") {
      const eth = injectedEvm();
      if (eth === null) return null;
      const accounts = (await eth.request({ method: "eth_requestAccounts" })) as string[];
      const first = accounts[0];
      if (!first) return null;
      setConnected((c) => ({ ...c, evm: first }));
      return { family: "evm", kind: "injected", address: first, eth };
    }
    const provider = solanaInjected();
    if (provider === null) return null;
    const { publicKey } = await provider.connect();
    const address = publicKey.toString();
    setConnected((c) => ({ ...c, sol: address }));
    return { family: "solana", signer: { kind: "injected", address, provider } };
  };

  return { choices, from: chosen, setFrom, password, setPassword, address, sign };
}

// ── The section ───────────────────────────────────────────────────────────

export function AiModelsSection({
  usdc,
  llm,
  disabled,
}: {
  usdc: UsdcPublic;
  llm: LlmData;
  disabled: boolean;
}) {
  const t = useT();
  const { state } = llm;
  const networks = usdc.chains.filter((c) => c.pay !== null);
  const chain =
    state.kind === "ready"
      ? (usdc.chains.find((c) => c.network === state.network) ?? null)
      : (networks.find((c) => c.network === llm.network) ?? networks[0] ?? null);
  const payer = usePayer(chain?.family ?? null);
  const [busy, setBusy] = React.useState<string | null>(null);
  const [held, setHeld] = React.useState<bigint | null>(null);
  const locked = disabled || busy !== null;

  // What the paying wallet already holds in the escrow, when its address is
  // known before it signs.
  const heldKey = chain !== null && payer.address !== null ? `${chain.network}:${payer.address}` : null;
  React.useEffect(() => {
    setHeld(null);
    if (chain === null || payer.address === null) return;
    let live = true;
    void escrowBalance(chain, payer.address)
      .then((b) => live && setHeld(b))
      .catch(() => undefined);
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the wallet and network, not the objects
  }, [heldKey, state.kind === "ready" ? state.meters.length : 0]);

  if (networks.length === 0) return null;

  const run = async (name: string, fn: () => Promise<void>) => {
    setBusy(name);
    try {
      await fn();
    } catch (error) {
      fail(error);
    } finally {
      setBusy(null);
      llm.refresh();
    }
  };

  let body: React.ReactNode;
  if (state.kind === "loading" || state.kind === "off") {
    body = (
      <Group>
        <Row last>
          <RowText title={t("panes.market.ai.loading")} />
          <Spinner size={16} />
        </Row>
      </Group>
    );
  } else if (state.kind === "failed") {
    body = (
      <Group>
        <Row last>
          <RowText title={t("panes.market.ai.failed")} detail={state.message || undefined} />
          <Button size="sm" variant="secondary" disabled={disabled} onClick={llm.refresh}>
            {t("panes.market.refresh")}
          </Button>
        </Row>
      </Group>
    );
  } else if (state.kind === "unavailable") {
    body = (
      <Group>
        <Row last>
          <RowText title={t("panes.market.ai.none")} />
        </Row>
      </Group>
    );
  } else {
    body = (
      <ReadyModels
        data={state}
        chain={chain}
        networks={networks}
        llm={llm}
        payer={payer}
        held={held}
        locked={locked}
        busy={busy}
        run={run}
      />
    );
  }

  return (
    <PaneSection data-testid="ai-models">
      <GroupTitle>{t("panes.market.ai.title")}</GroupTitle>
      {body}
    </PaneSection>
  );
}

type Ready = Extract<LlmState, { kind: "ready" }>;

function ReadyModels({
  data,
  chain,
  networks,
  llm,
  payer,
  held,
  locked,
  busy,
  run,
}: {
  data: Ready;
  chain: UsdcChain | null;
  networks: UsdcChain[];
  llm: LlmData;
  payer: Payer;
  held: bigint | null;
  locked: boolean;
  busy: string | null;
  run: (name: string, fn: () => Promise<void>) => Promise<void>;
}) {
  const t = useT();
  const networkId = React.useId();
  const fromId = React.useId();
  const passwordId = React.useId();
  const [starting, setStarting] = React.useState<string | null>(null);
  const live = data.meters.filter((m) => LIVE.has(m.state));
  const recent = data.meters.filter((m) => !LIVE.has(m.state)).slice(0, 3);
  const nameOf = (id: string) => data.models.find((m) => m.id === id)?.name ?? id;
  const wallets = data.wallets.filter((w) => w.network === data.network);

  /** Read the taps back until the edge has seen every one. */
  const confirm = async (meter: string) => {
    const giveUpAt = Date.now() + CONFIRM_WAIT_MS;
    for (;;) {
      try {
        const reply = await postTapConfirm(meter);
        sessionStorage.removeItem(PENDING);
        return reply.state;
      } catch (error) {
        const retry = error instanceof ApiError && error.status === 409 && NOT_YET.test(error.message);
        if (!retry || Date.now() > giveUpAt) throw error;
        await sleep(3_000);
      }
    }
  };

  const start = (model: LlmModel, limit: bigint, topUp: bigint) =>
    run(`start:${model.id}`, async () => {
      if (chain === null) return;
      const signer = await payer.sign();
      if (signer === null) return;
      toast.status(t("panes.market.ai.step.quote"), undefined, { id: "llm-start" });
      const quote = await postTapQuote(model.id, data.network, limit.toString());
      let sent = false;
      try {
        toast.status(t("panes.market.ai.step.wallet"), undefined, { id: "llm-start", duration: Infinity });
        const quoted: UsdcChain = { ...quote.chain };
        await startTaps(quoted, signer, quote, topUp).then(() => {
          sent = true;
        });
      } catch (error) {
        // Nothing reached the escrow: let the quote go, so the model can be
        // started again at once rather than after the quote lapses.
        if (!sent) await postTapStop(quote.meter).catch(() => undefined);
        toast.dismiss("llm-start");
        throw error;
      }
      sessionStorage.setItem(PENDING, quote.meter);
      toast.status(t("panes.market.ai.step.confirm"), undefined, { id: "llm-start", duration: Infinity });
      try {
        await confirm(quote.meter);
      } finally {
        toast.dismiss("llm-start");
      }
      setStarting(null);
      toast.success(
        t("panes.market.ai.startedTitle", { model: model.name }),
        t("panes.market.ai.startedBody", { limit: formatUsd(limit) }),
      );
    });

  const stop = (meter: LlmMeter) =>
    run(`stop:${meter.id}`, async () => {
      await postTapStop(meter.id);
      toast.done(t("panes.market.ai.stoppedTitle"), t("panes.market.ai.stoppedBody"));
    });

  const checkAgain = (meter: LlmMeter) =>
    run(`check:${meter.id}`, async () => {
      if ((await confirm(meter.id)) === "open") {
        toast.success(t("panes.market.ai.startedTitle", { model: nameOf(meter.model) }), t("panes.market.ai.startedBody", { limit: formatUsd(meter.spendPerHour) }));
      }
    });

  /** The paying wallet, checked against the one that holds `wallet`'s
   *  balance: only that one can ask for it back. */
  const signFor = async (wallet: LlmWallet): Promise<TapSigner | null> => {
    const signer = await payer.sign();
    if (signer === null) return null;
    const address = signerAddress(signer);
    const same = chain?.family === "evm" ? address.toLowerCase() === wallet.wallet.toLowerCase() : address === wallet.wallet;
    if (!same) {
      toast.error(t("panes.market.ai.failedTitle"), t("panes.market.ai.withdraw.wrongWallet", { wallet: shortAddress(wallet.wallet) }), {
        help: "index",
      });
      return null;
    }
    return signer;
  };

  const ask = (wallet: LlmWallet) =>
    run(`ask:${wallet.wallet}`, async () => {
      if (chain === null) return;
      const signer = await signFor(wallet);
      if (signer === null) return;
      await askBack(chain, signer, BigInt(wallet.balance));
      const when = formatWhen(Math.floor(Date.now() / 1000) + data.noticeSecs);
      toast.success(t("panes.market.ai.withdraw.askedTitle"), t("panes.market.ai.withdraw.askedBody", { when }));
    });

  const take = (wallet: LlmWallet) =>
    run(`take:${wallet.wallet}`, async () => {
      if (chain === null) return;
      const signer = await signFor(wallet);
      if (signer === null) return;
      await takeBack(chain, signer);
      toast.success(t("panes.market.ai.withdraw.takenTitle"), formatUsd(Math.min(wallet.asked, wallet.balance)));
    });

  const showNetworks = networks.length > 1;
  return (
    <>
      <Group>
        {showNetworks && (
          <Row>
            <RowText htmlFor={networkId} title={t("panes.market.ai.network")} />
            <NativeSelect
              id={networkId}
              className="w-auto"
              value={data.network}
              disabled={locked}
              onChange={(e) => llm.setNetwork(e.target.value)}
            >
              {networks.map((c) => (
                <option key={c.network} value={c.network}>
                  {c.name}
                </option>
              ))}
            </NativeSelect>
          </Row>
        )}
        <Row last={payer.from !== "local"}>
          <RowText
            htmlFor={payer.choices.length > 1 ? fromId : undefined}
            title={t("panes.market.ai.payFrom")}
            detail={
              payer.choices.length === 0
                ? t("panes.market.ai.noWallet")
                : held !== null
                  ? t("panes.market.ai.held", { amount: formatUsd(held) })
                  : payer.address !== null
                    ? shortAddress(payer.address)
                    : undefined
            }
          />
          {payer.choices.length > 1 ? (
            <NativeSelect
              id={fromId}
              className="w-auto"
              value={payer.from ?? ""}
              disabled={locked}
              onChange={(e) => payer.setFrom(e.target.value === "injected" ? "injected" : "local")}
            >
              {payer.choices.map((c) => (
                <option key={c} value={c}>
                  {t(c === "local" ? "panes.market.ai.payFrom.local" : "panes.market.ai.payFrom.injected")}
                </option>
              ))}
            </NativeSelect>
          ) : payer.from !== null ? (
            <RowValue>
              {t(payer.from === "local" ? "panes.market.ai.payFrom.local" : "panes.market.ai.payFrom.injected")}
            </RowValue>
          ) : null}
        </Row>
        {payer.from === "local" && (
          <Row last>
            <RowText htmlFor={passwordId} title={t("panes.market.ai.password")} />
            <Input
              id={passwordId}
              type="password"
              autoComplete="current-password"
              className="w-48"
              value={payer.password}
              disabled={locked}
              onChange={(e) => payer.setPassword(e.target.value)}
            />
          </Row>
        )}
      </Group>

      <Group className="mt-3" data-testid="ai-catalogue">
        {data.models.length === 0 ? (
          <Row last>
            <RowText title={t("panes.market.ai.none")} />
          </Row>
        ) : (
          data.models.map((model, i) => (
            <ModelRow
              key={model.id}
              model={model}
              running={live.some((m) => m.model === model.id)}
              open={starting === model.id}
              onOpen={() => setStarting(starting === model.id ? null : model.id)}
              held={held}
              locked={locked}
              busy={busy === `start:${model.id}`}
              minSpend={BigInt(data.minSpendPerHour)}
              maxSpend={BigInt(data.maxSpendPerHour)}
              onStart={(limit, topUp) => void start(model, limit, topUp)}
              last={i === data.models.length - 1}
            />
          ))
        )}
      </Group>
      <GroupCaption>{t("panes.market.ai.caption", { percent: data.feeBps / 100 })}</GroupCaption>

      {live.length + recent.length > 0 && (
        <>
          <GroupTitle className="mt-5">{t("panes.market.ai.meters")}</GroupTitle>
          <Group data-testid="ai-meters">
            {[...live, ...recent].map((meter, i, all) => (
              <Row key={meter.id} last={i === all.length - 1}>
                <RowText
                  title={`${nameOf(meter.model)} · ${t(`panes.market.ai.meter.${meter.state}`)}`}
                  detail={
                    meter.reason && !LIVE.has(meter.state)
                      ? meter.reason
                      : t("panes.market.ai.meter.detail", {
                          spent: formatE6(meter.billedE6),
                          tokens: meter.tokensIn + meter.tokensOut,
                          limit: formatUsd(meter.spendPerHour),
                        })
                  }
                />
                <div className="flex shrink-0 gap-2">
                  {meter.state === "quoted" && (
                    <Button size="sm" variant="secondary" disabled={locked} onClick={() => void checkAgain(meter)}>
                      {t("panes.market.ai.checkAgain")}
                    </Button>
                  )}
                  {(meter.state === "open" || meter.state === "quoted") && (
                    <Button size="sm" variant="secondary" disabled={locked} onClick={() => void stop(meter)}>
                      {busy === `stop:${meter.id}` ? <Spinner size={14} /> : t("panes.market.ai.stop")}
                    </Button>
                  )}
                </div>
              </Row>
            ))}
          </Group>
        </>
      )}

      {wallets.length > 0 && (
        <>
          <GroupTitle className="mt-5">{t("panes.market.ai.balance")}</GroupTitle>
          <Group data-testid="ai-balance">
            {wallets.map((wallet, i) => (
              <BalanceRow
                key={wallet.wallet}
                wallet={wallet}
                chainName={chain?.name ?? data.network}
                locked={locked}
                onAsk={() => void ask(wallet)}
                onTake={() => void take(wallet)}
                last={i === wallets.length - 1}
              />
            ))}
          </Group>
          <GroupCaption>
            {t("panes.market.ai.withdraw.caption", { hours: Math.round(data.noticeSecs / 3600) })}
          </GroupCaption>
        </>
      )}

      {live.some((m) => m.state === "open") && (
        <ChatBox meters={live.filter((m) => m.state === "open")} nameOf={nameOf} disabled={locked} onAnswered={llm.refresh} />
      )}
    </>
  );
}

function ModelRow({
  model,
  running,
  open,
  onOpen,
  held,
  locked,
  busy,
  minSpend,
  maxSpend,
  onStart,
  last,
}: {
  model: LlmModel;
  running: boolean;
  open: boolean;
  onOpen: () => void;
  held: bigint | null;
  locked: boolean;
  busy: boolean;
  minSpend: bigint;
  maxSpend: bigint;
  onStart: (limit: bigint, topUp: bigint) => void;
  last: boolean;
}) {
  const t = useT();
  const limitId = React.useId();
  const topUpId = React.useId();
  const errorId = React.useId();
  const [limit, setLimit] = React.useState("0.50");
  const [topUp, setTopUp] = React.useState<string | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  const enough = model.lentMib >= model.needMib;
  const fill = model.needMib > 0 ? (model.lentMib / model.needMib) * 100 : 0;

  /* Enough for about eight hours at the limit, less what the balance
   * already holds, in whole dollars: a starting point, not a rule. */
  const limitUnits = parseUsdc(limit);
  const suggested =
    limitUnits === null ? null : (() => {
      const want = limitUnits * 8n - (held ?? 0n);
      if (want <= 0n) return 0n;
      return ((want + 999_999n) / 1_000_000n) * 1_000_000n;
    })();
  const topUpText = topUp ?? (suggested === null ? "" : usdcText(suggested));

  const submit = () => {
    const l = parseUsdc(limit);
    if (l === null || l < minSpend || l > maxSpend) return setProblem(t("panes.market.ai.limitInvalid"));
    const up = topUpText.trim() === "" ? 0n : parseUsdc(topUpText);
    if (up === null) return setProblem(t("panes.market.ai.topUpInvalid"));
    if (up === 0n && held === 0n) return setProblem(t("panes.market.ai.topUpNeeded"));
    setProblem(null);
    onStart(l, up);
  };

  const status =
    model.group === "ready" ? (
      <Badge variant="ok">{t("panes.market.ai.state.ready", { count: model.boxes ?? 1 })}</Badge>
    ) : model.group === "loading" ? (
      <Badge variant="warn">{t("panes.market.ai.state.loading")}</Badge>
    ) : model.group === "down" ? (
      <Badge variant="warn">{t("panes.market.ai.state.down")}</Badge>
    ) : (
      <Badge variant="neutral">{t(enough ? "panes.market.ai.state.idle" : "panes.market.ai.state.short")}</Badge>
    );

  return (
    <StackRow last={last} data-model={model.id}>
      <div className="flex items-center justify-between gap-4">
        <RowText
          title={model.name}
          detail={`${t("panes.market.ai.price", { in: formatUsd(model.priceIn), out: formatUsd(model.priceOut) })} · ${model.licence}`}
        />
        <div className="flex shrink-0 items-center gap-2">
          {status}
          {!running && (
            <Button size="sm" variant={open ? "secondary" : "primary"} disabled={locked || (!enough && model.group === undefined)} onClick={onOpen}>
              {open ? t("panes.market.ai.cancel") : t("panes.market.ai.start")}
            </Button>
          )}
        </div>
      </div>
      <div className="flex items-center gap-3">
        <Progress value={fill} label={t("panes.market.ai.memoryLabel")} className={cn("max-w-48", !enough && "opacity-70")} />
        <span className="numeric text-[12px] text-muted">
          {t("panes.market.ai.memory", { need: gib(model.needMib), lent: gib(model.lentMib) })}
        </span>
      </div>
      {open && !running && (
        <div className="grid gap-2 sm:grid-cols-[1fr_1fr_auto] sm:items-end">
          <div>
            <label htmlFor={limitId} className="mb-1 block text-[12.5px] text-muted">
              {t("panes.market.ai.limit")}
            </label>
            <Input
              id={limitId}
              inputMode="decimal"
              className="numeric"
              value={limit}
              disabled={locked}
              aria-invalid={problem !== null}
              aria-describedby={problem === null ? undefined : errorId}
              onChange={(e) => setLimit(e.target.value)}
            />
          </div>
          <div>
            <label htmlFor={topUpId} className="mb-1 block text-[12.5px] text-muted">
              {t("panes.market.ai.topUp")}
            </label>
            <Input
              id={topUpId}
              inputMode="decimal"
              className="numeric"
              value={topUpText}
              disabled={locked}
              aria-invalid={problem !== null}
              aria-describedby={problem === null ? undefined : errorId}
              onChange={(e) => setTopUp(e.target.value)}
            />
          </div>
          <Button disabled={locked} onClick={submit}>
            {busy ? <Spinner size={14} /> : t("panes.market.ai.start")}
          </Button>
          <FieldError id={errorId} className="sm:col-span-3">
            {problem}
          </FieldError>
        </div>
      )}
    </StackRow>
  );
}

function BalanceRow({
  wallet,
  chainName,
  locked,
  onAsk,
  onTake,
  last,
}: {
  wallet: LlmWallet;
  chainName: string;
  locked: boolean;
  onAsk: () => void;
  onTake: () => void;
  last: boolean;
}) {
  const t = useT();
  const now = Math.floor(Date.now() / 1000);
  const asked = wallet.asked > 0 && wallet.notBefore > 0;
  const due = asked && now >= wallet.notBefore;
  const amount = formatUsd(Math.min(wallet.asked, wallet.balance));
  const detail = asked
    ? due
      ? t("panes.market.ai.withdraw.ready", { amount })
      : t("panes.market.ai.withdraw.asked", { amount, when: formatWhen(wallet.notBefore) })
    : wallet.runwaySecs === undefined
      ? t("panes.market.ai.balance.idle")
      : t(wallet.low ? "panes.market.ai.balance.low" : "panes.market.ai.balance.runway", { time: formatSpan(wallet.runwaySecs) });
  return (
    <Row last={last}>
      <RowText
        title={`${t("panes.market.ai.balance.on", { chain: chainName })} · ${shortAddress(wallet.wallet)}`}
        detail={<span className={cn(wallet.low && !asked && "text-warn")}>{detail}</span>}
      />
      <RowValue className="text-ink">{formatUsd(wallet.available)}</RowValue>
      {due ? (
        <Button size="sm" disabled={locked} onClick={onTake}>
          {t("panes.market.ai.withdraw.take")}
        </Button>
      ) : !asked && wallet.balance > 0 ? (
        <Button size="sm" variant="secondary" disabled={locked} onClick={onAsk}>
          {t("panes.market.ai.withdraw.ask")}
        </Button>
      ) : null}
    </Row>
  );
}

/* A small chat to try a running model: one conversation per meter, kept on
 * this page only. Each answer says what it cost. */
function ChatBox({
  meters,
  nameOf,
  disabled,
  onAnswered,
}: {
  meters: LlmMeter[];
  nameOf: (id: string) => string;
  disabled: boolean;
  onAnswered: () => void;
}) {
  const t = useT();
  const meterId = React.useId();
  const promptId = React.useId();
  const maxId = React.useId();
  const [picked, setPicked] = React.useState<string | null>(null);
  const meter = meters.find((m) => m.id === picked) ?? meters[0];
  const [turns, setTurns] = React.useState<{ meter: string; ask: string; answer?: ChatAnswer }[]>([]);
  const [prompt, setPrompt] = React.useState("");
  const [maxTokens, setMaxTokens] = React.useState("256");
  const [waiting, setWaiting] = React.useState(false);
  if (meter === undefined) return null;
  const shown = turns.filter((turn) => turn.meter === meter.id);

  const send = async () => {
    const ask = prompt.trim();
    const max = Number(maxTokens);
    if (ask === "" || !Number.isInteger(max) || max < 1) return;
    const history: ChatMessage[] = shown.flatMap((turn) =>
      turn.answer
        ? [
            { role: "user" as const, content: turn.ask },
            { role: "assistant" as const, content: turn.answer.content },
          ]
        : [],
    );
    setWaiting(true);
    setTurns((all) => [...all, { meter: meter.id, ask }]);
    setPrompt("");
    try {
      const answer = await postLlmChat(meter.id, [...history, { role: "user", content: ask }], max);
      setTurns((all) => all.map((turn) => (turn.meter === meter.id && turn.ask === ask && !turn.answer ? { ...turn, answer } : turn)));
      onAnswered();
    } catch (error) {
      setTurns((all) => all.filter((turn) => !(turn.meter === meter.id && turn.ask === ask && !turn.answer)));
      setPrompt(ask);
      fail(error);
    } finally {
      setWaiting(false);
    }
  };

  return (
    <>
      <GroupTitle className="mt-5">{t("panes.market.ai.chat.title")}</GroupTitle>
      <Group data-testid="ai-chat">
        {meters.length > 1 && (
          <Row>
            <RowText htmlFor={meterId} title={t("panes.market.ai.chat.model")} />
            <NativeSelect id={meterId} className="w-auto" value={meter.id} disabled={waiting} onChange={(e) => setPicked(e.target.value)}>
              {meters.map((m) => (
                <option key={m.id} value={m.id}>
                  {nameOf(m.model)}
                </option>
              ))}
            </NativeSelect>
          </Row>
        )}
        {shown.map((turn, i) => (
          <StackRow key={`${turn.meter}-${i}`}>
            <p className="text-[13px] text-muted">{turn.ask}</p>
            {turn.answer ? (
              <>
                <p className="text-sm whitespace-pre-wrap text-ink">{turn.answer.content}</p>
                <p className="numeric text-[12px] text-faint">
                  {t("panes.market.ai.chat.cost", {
                    cost: formatE6(turn.answer.costE6),
                    in: turn.answer.tokensIn,
                    out: turn.answer.tokensOut,
                  })}
                </p>
              </>
            ) : (
              <p className="flex items-center gap-2 text-[13px] text-muted">
                <Spinner size={14} />
                {t("panes.market.ai.chat.waiting")}
              </p>
            )}
          </StackRow>
        ))}
        <StackRow last>
          <label htmlFor={promptId} className="sr-only">
            {t("panes.market.ai.chat.prompt")}
          </label>
          <Textarea
            id={promptId}
            placeholder={t("panes.market.ai.chat.prompt")}
            value={prompt}
            disabled={disabled || waiting}
            onChange={(e) => setPrompt(e.target.value)}
          />
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2">
              <label htmlFor={maxId} className="text-[12.5px] text-muted">
                {t("panes.market.ai.chat.maxTokens")}
              </label>
              <Input
                id={maxId}
                inputMode="numeric"
                className="numeric w-20"
                value={maxTokens}
                disabled={disabled || waiting}
                onChange={(e) => setMaxTokens(e.target.value)}
              />
            </div>
            <Button size="sm" disabled={disabled || waiting || prompt.trim() === ""} onClick={() => void send()}>
              {t("panes.market.ai.chat.send")}
            </Button>
          </div>
          {meter.group !== undefined && meter.group !== "ready" && (
            <p className="text-[12.5px] text-muted">{t("panes.market.ai.chat.notReady")}</p>
          )}
        </StackRow>
      </Group>
    </>
  );
}

// ── Lending this box's memory ─────────────────────────────────────────────

/** One extra line's option and literal, or null when the box does not
 *  declare it (an older box would refuse the line at Apply). */
function extraSlot(form: SettingsForm, name: string): { option: OptionDoc; raw: string | null; defaultRaw: string | null } | null {
  const option = form.options?.options.find((candidate) => candidate.name === name);
  if (option === undefined || option.readOnly) return null;
  const defaultRaw = toLiteral(option.editor, option.default);
  return { option, raw: form.extra[name] ?? defaultRaw, defaultRaw };
}

export function LendSection({
  form,
  lending,
  models,
}: {
  form: SettingsForm;
  lending: LlmLending | null;
  /** The edge's catalogue, to name the model this box serves. */
  models: LlmModel[];
}) {
  const t = useT();
  const switchId = React.useId();
  const amountId = React.useId();
  const amountErrorId = React.useId();
  const enable = extraSlot(form, LEND_ENABLE);
  const memory = extraSlot(form, LEND_MEMORY);
  const savedMib = Number(memory?.raw ?? "4096");
  const [amount, setAmount] = React.useState<string | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  if (enable === null || memory === null) return null;

  const on = enable.raw === "true";
  const disabled = form.locked || !form.ready;
  // Lending goes through the edge's tunnel: with the proxy off nothing could
  // reach the box. Turning it off stays possible.
  const needsProxy = !on && !(form.draft?.proxyEnable ?? false);
  const shownAmount = amount ?? String(Math.round((savedMib / MIB_PER_GIB) * 10) / 10);

  const setLine = (name: string, defaultRaw: string | null, next: string) =>
    form.setExtra(name, next === defaultRaw ? null : next);

  const onAmount = (text: string) => {
    setAmount(text);
    const n = Number(text.trim());
    if (!/^\d{1,3}$/.test(text.trim()) || n < 1 || n > 256) {
      setProblem(t("panes.market.lend.amountInvalid"));
      return;
    }
    setProblem(null);
    setLine(LEND_MEMORY, memory.defaultRaw, String(n * MIB_PER_GIB));
  };

  const groupState =
    lending?.group === "ready"
      ? null
      : lending?.group === "loading"
        ? t("panes.market.ai.state.loading")
        : lending?.group === "down"
          ? t("panes.market.ai.state.down")
          : null;

  return (
    <PaneSection data-testid="lend-memory">
      <GroupTitle>{t("panes.market.lend.title")}</GroupTitle>
      <Group>
        <SwitchRow
          id={switchId}
          option={LEND_ENABLE}
          title={t("panes.market.lend.switch")}
          description={needsProxy && !form.locked ? t("panes.mesh.needsProxy") : undefined}
          checked={on}
          disabled={disabled || needsProxy}
          onCheckedChange={(next) => setLine(LEND_ENABLE, enable.defaultRaw, next ? "true" : "false")}
          last={!on && lending === null}
        />
        {on && (
          <StackRow last={lending === null}>
            <div className="flex items-center justify-between gap-4">
              <RowText htmlFor={amountId} title={t("panes.market.lend.amount")} />
              <Input
                id={amountId}
                inputMode="numeric"
                className="numeric w-24"
                data-option={LEND_MEMORY}
                value={shownAmount}
                disabled={disabled}
                aria-invalid={problem !== null}
                aria-describedby={problem === null ? undefined : amountErrorId}
                onChange={(e) => onAmount(e.target.value)}
              />
            </div>
            <FieldError id={amountErrorId}>{problem}</FieldError>
          </StackRow>
        )}
        {lending !== null && (
          <>
            <Row>
              <RowText title={t("panes.market.lend.serving")} />
              <RowValue>
                {lending.model === undefined
                  ? t("panes.market.lend.servingNone")
                  : `${models.find((m) => m.id === lending.model)?.name ?? lending.model}${lending.lentMib !== undefined ? ` · ${gib(lending.lentMib)}` : ""}${groupState ? ` · ${groupState}` : ""}`}
              </RowValue>
            </Row>
            <Row>
              <RowText title={t("panes.market.lend.paid")} />
              <RowValue className="text-ink">{formatUsd(lending.paid)}</RowValue>
            </Row>
            <Row last>
              <RowText title={t("panes.market.lend.owed")} />
              <RowValue>{formatUsd(lending.owed)}</RowValue>
            </Row>
          </>
        )}
      </Group>
      <GroupCaption>{t("panes.market.lend.caption")}</GroupCaption>
    </PaneSection>
  );
}
