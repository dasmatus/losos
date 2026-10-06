/* Step 2 — Choose how you sign in.
 *
 * This is the step the whole wizard exists for. modules/nextcloud-common.nix
 * generates the first admin password from /dev/urandom, writes it 0600 and
 * shows it to nobody; the appliance has no SSH and no shell login, so without
 * this step a fresh box cannot be signed into at all.
 *
 * On a fresh box this tab holds no admin token, and every /api route except
 * the claim refuses without one. So the first password goes through
 * `POST /api/setup/claim` (@/lib/api `claimBox`): public while the box is
 * unowned, never after, and it hands back the admin token so the rest of the
 * wizard — the recovery code, the sign-in frame — runs authenticated. The
 * token-gated `/api/set-password` is only for a second attempt from the same
 * tab, once the claim has closed the window. The old code posted to
 * set-password first, got a 401 on every fresh box, and the wizard could not
 * get past this step; the browser test walks it now.
 *
 * The step also waits before it asks. On a fresh box the Nextcloud pod spends
 * its first minutes in `occ maintenance:install`, and a claim sent in that
 * window fails: it is occ that sets the password. `GET /api/setup/claim` now
 * carries `ready` and `waitingFor`, so the form stays disabled behind a
 * waiting panel that polls every few seconds and opens on its own, and a 503
 * from the claim itself (the race between the last poll and the submit) goes
 * back to waiting rather than reading as an error. The recorded install demo
 * showed what this replaces: an owner typing a good password into a bare
 * "command failed; see the lososd journal".
 *
 * A password is collected whatever else happens. The passkey is an addition,
 * never a substitute: the desktop and phone sync clients authenticate with a
 * name and a password, and no passkey helps them. See ./passkey.ts.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert02Icon,
  CheckmarkCircle02Icon,
  CircleIcon,
  Copy01Icon,
  FingerPrintIcon,
  Key01Icon,
  InformationCircleIcon,
  Loading03Icon,
  LockPasswordIcon,
  PrinterIcon,
  SecurityLockIcon,
  UserCircleIcon,
  ViewIcon,
  ViewOffSlashIcon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import {
  ApiError,
  claimBox,
  getClaimState,
  hasToken,
  isAbort,
  isNotReady,
  isUnauthorized,
  saveToken,
  type ClaimResponse,
} from "@/lib/api";
import { intlTag, t } from "@/lib/i18n";
import { Rich, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import {
  isMissingRoute,
  passwordProblem,
  passwordRules,
  postSetPassword,
  MIN_PASSWORD_CHARS,
  type PasswordRuleId,
} from "./api";
import { copyText } from "./copy";
import {
  createPasskey,
  probePasskeySupport,
  type PasskeySupport,
} from "./passkey";
import { ReadoutRow, StepText } from "./parts";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

export interface StepSignInProps {
  /** The box's own name, for the label the passkey is saved under. */
  boxName: string;
  /** The account lososd reported after the password was set, or null. */
  account: string | null;
  /** The admin key the claim released to this tab, or null when the password
   *  was set some other way. Held by the wizard so Back does not lose it. */
  adminKey: string | null;
  onPasswordSet: (user: string, adminKey: string | null) => void;
}

export function StepSignIn({
  boxName,
  account,
  adminKey,
  onPasswordSet,
}: StepSignInProps) {
  const t = useT();
  return (
    <div className="flex flex-col gap-5">
      <StepText>{t("wizard.signin.intro")}</StepText>

      <PasswordForm account={account} onPasswordSet={onPasswordSet} />

      {adminKey !== null && <AdminKey value={adminKey} boxName={boxName} />}

      <Separator />

      <PasskeyPanel boxName={boxName} />
    </div>
  );
}

// ── Password ──────────────────────────────────────────────────────────────

function PasswordForm({
  account,
  onPasswordSet,
}: {
  account: string | null;
  onPasswordSet: (user: string, adminKey: string | null) => void;
}) {
  const t = useT();
  const [password, setPassword] = React.useState("");
  const [confirm, setConfirm] = React.useState("");
  const [visible, setVisible] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);
  // Only a box nobody has claimed has anything to wait for: once this tab
  // holds the token the password goes through the gated route, and lososd
  // answers readiness only on the public one.
  const readiness = useReadiness(account === null && !hasToken());
  const passwordId = React.useId();
  const confirmId = React.useId();
  const problemId = React.useId();

  const submit = async (event: React.FormEvent): Promise<void> => {
    event.preventDefault();

    // The server checks the same things; this only means the owner hears
    // about a short password before the request rather than as a 400 after.
    const local = passwordProblem(password);
    if (local !== null) {
      setProblem(local);
      return;
    }
    // Not a server rule — the server never sees the second field. It is here
    // because a typo in the only password on a box with no shell login is not
    // recoverable by anything short of a factory reset.
    if (password !== confirm) {
      setProblem(t("wizard.signin.mismatch"));
      return;
    }

    setBusy(true);
    setProblem(null);
    try {
      const result = await setFirstPassword(password);
      setPassword("");
      setConfirm("");
      setVisible(false);
      onPasswordSet(result.user, result.adminKey);
    } catch (error) {
      // Lost the race between the last poll and the submit: lososd says not
      // yet, and changed nothing. Back to waiting, with the reason on screen.
      if (isNotReady(error))
        readiness.notYet(error instanceof Error ? error.message : null);
      setProblem(describeSetPassword(error));
    } finally {
      setBusy(false);
    }
  };

  // The form is open only once lososd has said the box is ready. Before the
  // first answer (`unknown`) nothing is shown, so a ready box never flashes a
  // waiting panel, but the fields are already disabled: on take 9 of the
  // recorded install demo (2026-10-06) that first answer took twelve seconds
  // on a busy box, the form was open meanwhile, and a password typed into it
  // sat greyed out behind the panel that then appeared.
  const waiting = readiness.state.kind !== "ready";

  return (
    <form onSubmit={submit} noValidate className="flex flex-col gap-4">
      {readiness.state.kind !== "ready" && (
        <WaitingPanel state={readiness.state} />
      )}
      {readiness.state.kind === "ready" && readiness.state.waited && (
        <p
          role="status"
          className="flex items-center gap-1.5 text-[13px] text-ok"
        >
          <HugeiconsIcon
            icon={CheckmarkCircle02Icon}
            size={16}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
          {t("wizard.signin.ready")}
        </p>
      )}
      {/* Two shadcn Fields in a FieldGroup. Each field carries its own
          invalid and disabled state, the eye is an Input Group addon inside
          the password box, and the four rules are the field's description. */}
      <FieldGroup>
        <Field invalid={problem !== null} disabled={busy || waiting}>
          <FieldLabel htmlFor={passwordId}>{t("wizard.signin.newPassword")}</FieldLabel>
          <InputGroup>
            <InputGroupInput
              id={passwordId}
              name="new-password"
              type={visible ? "text" : "password"}
              autoComplete="new-password"
              value={password}
              disabled={busy || waiting}
              aria-invalid={problem !== null}
              aria-describedby={problem !== null ? problemId : undefined}
              onChange={(event) => {
                setPassword(event.target.value);
                setProblem(null);
              }}
            />
            <InputGroupAddon align="inline-end">
              <InputGroupButton
                size="icon-xs"
                aria-pressed={visible}
                aria-label={
                  visible ? t("wizard.signin.hide") : t("wizard.signin.show")
                }
                onClick={() => setVisible((shown) => !shown)}
              >
                <HugeiconsIcon
                  icon={visible ? ViewOffSlashIcon : ViewIcon}
                  size={16}
                  strokeWidth={1.5}
                  color="currentColor"
                  aria-hidden="true"
                />
              </InputGroupButton>
            </InputGroupAddon>
          </InputGroup>
          <FieldDescription>{t("wizard.signin.hint")}</FieldDescription>
          <PasswordRules password={password} />
        </Field>

        <Field invalid={problem !== null} disabled={busy || waiting}>
          <FieldLabel htmlFor={confirmId}>{t("wizard.signin.again")}</FieldLabel>
          <Input
            id={confirmId}
            name="confirm-password"
            type={visible ? "text" : "password"}
            autoComplete="new-password"
            value={confirm}
            disabled={busy || waiting}
            aria-invalid={problem !== null}
            onChange={(event) => {
              setConfirm(event.target.value);
              setProblem(null);
            }}
          />
          <FieldDescription>{t("wizard.signin.againHint")}</FieldDescription>
        </Field>

        <FieldError id={problemId}>{problem}</FieldError>
      </FieldGroup>

      <div className="flex items-center gap-3">
        <Button
          type="submit"
          disabled={
            busy || waiting || password.length === 0 || confirm.length === 0
          }
        >
          <HugeiconsIcon
            icon={LockPasswordIcon}
            size={18}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
          {account === null
            ? t("wizard.signin.set")
            : t("wizard.signin.setDifferent")}
        </Button>
        {busy && (
          <Spinner label={t("wizard.signin.setting")} className="text-muted" />
        )}
      </div>

      {account !== null && (
        <Alert variant="ok">
          <HugeiconsIcon icon={CheckmarkCircle02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.signin.done.title")}</AlertTitle>
          <AlertDescription>
            <ReadoutRow label={t("wizard.signin.done.name")} className="mt-2">
              <code className="numeric text-[13px] text-ink select-all">
                {account}
              </code>
            </ReadoutRow>
            <p className="mt-2 flex items-start gap-1.5">
              <HugeiconsIcon
                icon={UserCircleIcon}
                size={16}
                strokeWidth={1.5}
                color="currentColor"
                className="mt-px shrink-0"
                aria-hidden="true"
              />
              {t("wizard.signin.done.body")}
            </p>
          </AlertDescription>
        </Alert>
      )}
    </form>
  );
}

/* The four rules, each with a tick that fills in as the owner types. On
 * screen before the first keystroke, so a refusal never cites a rule the
 * owner has not seen; and only four, so the list is read rather than
 * skipped. The server checks the same four (backend/src/setup.rs). */
const RULE_KEYS: Record<
  PasswordRuleId,
  | "wizard.signin.rule.length"
  | "wizard.signin.rule.cases"
  | "wizard.signin.rule.digit"
  | "wizard.signin.rule.symbol"
> = {
  length: "wizard.signin.rule.length",
  cases: "wizard.signin.rule.cases",
  digit: "wizard.signin.rule.digit",
  symbol: "wizard.signin.rule.symbol",
};

function PasswordRules({ password }: { password: string }) {
  const t = useT();
  return (
    <ul
      aria-label={t("wizard.signin.rules")}
      data-testid="password-rules"
      className="mt-1 flex flex-col gap-1 text-[13px]"
    >
      {passwordRules(password).map((rule) => (
        <li
          key={rule.id}
          data-rule={rule.id}
          data-met={rule.met ? "true" : "false"}
          className={cn("flex items-center gap-1.5", rule.met ? "text-ok" : "text-muted")}
        >
          <HugeiconsIcon
            icon={rule.met ? CheckmarkCircle02Icon : CircleIcon}
            size={16}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
          {t(RULE_KEYS[rule.id], { count: MIN_PASSWORD_CHARS })}
          <span className="sr-only">
            {rule.met ? t("wizard.signin.rule.met") : t("wizard.signin.rule.unmet")}
          </span>
        </li>
      ))}
    </ul>
  );
}

/* Claim the box when this tab holds no token, which on a fresh box is always;
 * change the password through the gated route when it does. The claim's reply
 * carries the admin token and it is stored before anything else happens, so a
 * failure after this point (the name missing from the reply, say) still leaves
 * the tab signed in rather than locked out of a box it just claimed. */
async function setFirstPassword(
  password: string,
): Promise<{ user: string; adminKey: string | null }> {
  if (hasToken()) {
    const result = await postSetPassword(password);
    return { user: result.user, adminKey: null };
  }
  const claimed = await claimAgainIfTheReplyWasLost(password);
  saveToken(claimed.token);
  // lososd always names the account it changed; the type allows null because
  // the wire contract does. An unnamed account is still a set password.
  return { user: claimed.user ?? "", adminKey: claimed.token };
}

/* How many times step 2 asks again when the claim's reply never arrived, and
 * how long it waits between asks. Six tries five seconds apart is about half
 * a minute of asking on top of whatever each request itself took. */
const LOST_REPLY_TRIES = 6;
const LOST_REPLY_PAUSE_MS = 5000;

/* A claim whose answer was lost on the way, as opposed to one lososd refused.
 *
 * Take 6 of the recorded install demo (2026-10-05): on a box still warming
 * up, setting the password took longer than the proxy in front of lososd
 * waited, the browser got "HTTP 504", and lososd finished anyway — password
 * set, box claimed, and the admin key, which that reply carries exactly once,
 * delivered to nobody. The step showed an error over a box that was in fact
 * owned by the person reading it. lososd now answers the same reply again to
 * the same password for a few minutes after a claim (backend/src/receipt.rs),
 * so the right move on a lost reply is to ask again, with the same password,
 * rather than to report a failure the owner cannot act on.
 *
 * 504 and 502 are the proxy speaking for a lososd that did not answer in
 * time or is restarting; 408 is the proxy giving up on the request; a fetch
 * that throws a TypeError never reached a server at all. A 4xx from lososd
 * itself (a short password, a box someone else claimed) is an answer, and is
 * not retried. */
function isLostReply(error: unknown): boolean {
  if (error instanceof ApiError) {
    return error.status === 408 || error.status === 502 || error.status === 504;
  }
  return error instanceof TypeError;
}

async function claimAgainIfTheReplyWasLost(
  password: string,
): Promise<ClaimResponse> {
  for (let attempt = 1; ; attempt += 1) {
    try {
      return await claimBox(password);
    } catch (error) {
      if (!isLostReply(error) || attempt >= LOST_REPLY_TRIES) throw error;
      await new Promise((resolve) => setTimeout(resolve, LOST_REPLY_PAUSE_MS));
    }
  }
}

// ── Waiting for the box to be ready ───────────────────────────────────────

/** How often the step asks lososd again while it is not ready. Each ask is a
 *  `crictl exec` on the box, so this is seconds, not milliseconds; the first
 *  boot's install takes minutes anyway. */
const READINESS_POLL_MS = 5000;

type ReadinessState =
  /* Not asked yet, or the answer is on its way: nothing is shown until the
   * first reply so a box that is ready never flashes a waiting panel, but
   * the form is disabled, as it is while waiting. */
  | { kind: "unknown" }
  /* lososd said not yet; `reason` is its sentence, `since` when waiting
   * began, `unreachable` whether the last poll got no answer at all. */
  | {
      kind: "waiting";
      reason: string | null;
      since: number;
      unreachable: boolean;
    }
  /* `waited` says whether a waiting panel was ever shown, so the step can
   * say "ready now" to someone who sat through it and nothing to anyone
   * else. */
  | { kind: "ready"; waited: boolean };

interface Readiness {
  state: ReadinessState;
  /** The submit got a 503: go back to waiting with lososd's reason. */
  notYet: (reason: string | null) => void;
}

/* Poll `GET /api/setup/claim` until `ready` is true. A lososd from before the
 * field answers without it, which reads as ready: an old box must not be
 * waited on forever for an answer it cannot give. A poll that fails outright
 * (the box rebooting, say) keeps waiting and says the box did not answer. */
function useReadiness(enabled: boolean): Readiness {
  const [state, setState] = React.useState<ReadinessState>(
    enabled ? { kind: "unknown" } : { kind: "ready", waited: false },
  );
  // Bumped by `notYet` so the effect below starts a fresh poll loop after a
  // 503 from the submit; `enabled` alone would not change.
  const [restarts, setRestarts] = React.useState(0);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    let live = true;

    const ask = async (): Promise<void> => {
      try {
        const answer = await getClaimState({ signal: controller.signal });
        if (!live) return;
        if (answer.ready !== false) {
          setState((prev) => ({
            kind: "ready",
            waited: prev.kind === "waiting",
          }));
          return;
        }
        setState((prev) => ({
          kind: "waiting",
          reason: answer.waitingFor ?? null,
          since: prev.kind === "waiting" ? prev.since : Date.now(),
          unreachable: false,
        }));
      } catch (error) {
        if (!live || isAbort(error)) return;
        setState((prev) =>
          prev.kind === "waiting"
            ? { ...prev, unreachable: true }
            : {
                kind: "waiting",
                reason: null,
                since: Date.now(),
                unreachable: true,
              },
        );
      }
      timer = setTimeout(() => void ask(), READINESS_POLL_MS);
    };
    // A restart after a 503 already knows the answer is "not yet", so it
    // waits one interval before asking; the first run asks at once so a
    // ready box never shows the panel.
    if (restarts === 0) void ask();
    else timer = setTimeout(() => void ask(), READINESS_POLL_MS);

    return () => {
      live = false;
      controller.abort();
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [enabled, restarts]);

  const notYet = React.useCallback((reason: string | null) => {
    setState((prev) => ({
      kind: "waiting",
      reason,
      since: prev.kind === "waiting" ? prev.since : Date.now(),
      unreachable: false,
    }));
    setRestarts((n) => n + 1);
  }, []);

  return { state, notYet };
}

/* What the owner sees instead of a disabled form with no explanation. The
 * reason is lososd's own sentence when it gave one ("Nextcloud is still
 * installing itself…"), and the elapsed count ticks so a long wait visibly
 * is one rather than a hung page. */
function WaitingPanel({ state }: { state: ReadinessState }) {
  const t = useT();
  const [now, setNow] = React.useState(() => Date.now());
  React.useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  if (state.kind !== "waiting") return null;
  const seconds = Math.max(0, Math.floor((now - state.since) / 1000));
  return (
    <Alert variant="default">
      <HugeiconsIcon icon={Loading03Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <AlertTitle>{t("wizard.signin.waiting.title")}</AlertTitle>
      <AlertDescription>
        <div
          role="status"
          aria-live="polite"
          className="flex flex-col gap-2"
        >
          <p>{t("wizard.signin.waiting.body")}</p>
          {state.reason !== null && <p className="text-ink">{state.reason}</p>}
          <p className="flex items-center gap-2 text-faint">
            <Spinner
              label={t("wizard.signin.waiting.title")}
              className="text-muted"
            />
            {t("wizard.signin.waiting.since", { count: seconds })}
            {state.unreachable && (
              <span>{t("wizard.signin.waiting.unreachable")}</span>
            )}
          </p>
        </div>
      </AlertDescription>
    </Alert>
  );
}

// ── The admin key ─────────────────────────────────────────────────────────

/* The spare. The password the owner just set is what unlocks the admin
 * pages from now on (the shell's dialog, `POST /api/sign-in`), and it is
 * LosOS cloud that checks it — so while LosOS cloud is not running the
 * password cannot be checked, and this key is the way in. lososd mints it
 * into a 0600 file on a box with no shell and releases it exactly once, in
 * the claim reply, which is why it is shown here, copyable and printable,
 * and nowhere else. (It used to print on the recovery-code step's sheet;
 * that step is hidden, see ./steps.ts, so the sheet lives here now.) */
function AdminKey({ value, boxName }: { value: string; boxName: string }) {
  const t = useT();
  const [copied, setCopied] = React.useState<boolean | null>(null);
  const keyRef = React.useRef<HTMLElement>(null);

  const copy = async (): Promise<void> => {
    setCopied(await copyText(value, keyRef.current));
  };

  // Prints the document; the rules in ./wizard.css narrow that to the sheet.
  const print = (): void => window.print();

  return (
    <Alert variant="default">
      <HugeiconsIcon icon={Key01Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <AlertTitle>{t("wizard.signin.key.title")}</AlertTitle>
      <AlertDescription>
        <p>{t("wizard.signin.key.body")}</p>
        <div className="mt-3 flex flex-col gap-3 rounded-card border border-line bg-surface px-3.5 py-3">
          <span className="text-[12px] font-medium tracking-wide text-faint uppercase">
            {t("wizard.signin.key.label")}
          </span>
          {/* select-all: one click takes the whole key, so a manual Ctrl+C works
              even where the clipboard API is unavailable. */}
          <code
            ref={keyRef}
            className="numeric text-[13px] leading-snug break-all text-ink select-all"
          >
            {value}
          </code>
        </div>
        <div className="mt-3 flex flex-wrap items-center gap-2.5">
          <Button variant="secondary" size="sm" onClick={() => void copy()}>
            <HugeiconsIcon
              icon={Copy01Icon}
              size={16}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
            {t("wizard.signin.key.copy")}
          </Button>
          <Button variant="secondary" size="sm" onClick={print}>
            <HugeiconsIcon
              icon={PrinterIcon}
              size={16}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
            {t("wizard.signin.key.print")}
          </Button>
          {copied === true && (
            <span
              role="status"
              className="inline-flex items-center gap-1.5 text-[13px] text-ok"
            >
              <HugeiconsIcon
                icon={CheckmarkCircle02Icon}
                size={16}
                strokeWidth={1.5}
                color="currentColor"
                aria-hidden="true"
              />
              {t("wizard.signin.key.copied")}
            </span>
          )}
          {copied === false && (
            <span role="status" className="text-[13px] text-muted">
              {t("wizard.signin.key.copyFailed")}
            </span>
          )}
        </div>
        {/* Present in the document at all times, shown only on paper: the
            print rules in ./wizard.css hide everything else on the page and
            force this subtree to black on white. */}
        <KeySheet boxName={boxName} value={value} />
      </AlertDescription>
    </Alert>
  );
}

/* What comes out of the printer: the key, what it is for, and the date, in
 * the interface language, so the sheet still makes sense in a drawer in two
 * years with no wizard around it. */
function KeySheet({ boxName, value }: { boxName: string; value: string }) {
  const t = useT();
  return (
    <section className="wizard-print-sheet hidden print:block">
      <h1 className="text-lg font-semibold">
        {t("wizard.signin.key.sheetTitle", { name: boxName })}
      </h1>
      <p className="mt-1 text-sm">
        {t("wizard.signin.key.sheetPrinted", {
          date: new Date().toLocaleDateString(intlTag(), { dateStyle: "long" }),
        })}
      </p>
      <p className="numeric mt-6 text-xl break-all">{value}</p>
      <p className="mt-6 max-w-prose text-sm leading-relaxed">
        {t("wizard.signin.key.sheetBody")}
      </p>
      <p className="mt-4 flex items-center gap-2 text-sm">
        <HugeiconsIcon
          icon={Key01Icon}
          size={16}
          strokeWidth={1.5}
          color="currentColor"
          aria-hidden="true"
        />
        {t("wizard.signin.key.sheetKey")}
      </p>
    </section>
  );
}

function describeSetPassword(error: unknown): string {
  if (isNotReady(error)) {
    return t("wizard.signin.err.notReady");
  }
  if (isUnauthorized(error)) {
    return t("wizard.signin.err.unauthorized");
  }
  if (isMissingRoute(error)) {
    return t("wizard.signin.err.missing");
  }
  if (error instanceof Error && error.message.length > 0) return error.message;
  return t("wizard.noAnswer");
}

// ── Passkey ───────────────────────────────────────────────────────────────

type Attempt =
  | { kind: "idle" }
  | { kind: "working" }
  | { kind: "registered"; label: string }
  | { kind: "not-implemented" }
  | { kind: "problem"; message: string };

function PasskeyPanel({ boxName }: { boxName: string }) {
  const t = useT();
  const [support, setSupport] = React.useState<PasskeySupport | null>(null);
  const [attempt, setAttempt] = React.useState<Attempt>({ kind: "idle" });

  React.useEffect(() => {
    let live = true;
    void probePasskeySupport().then((result) => {
      if (live) setSupport(result);
    });
    return () => {
      live = false;
    };
  }, []);

  const label = t("wizard.passkey.label", { name: boxName });

  const register = async (): Promise<void> => {
    setAttempt({ kind: "working" });
    const outcome = await createPasskey({ label });
    switch (outcome.kind) {
      case "registered":
        setAttempt({ kind: "registered", label: outcome.label });
        return;
      case "cancelled":
        setAttempt({ kind: "idle" });
        return;
      case "not-implemented":
        setAttempt({ kind: "not-implemented" });
        return;
      case "unsupported-here":
        setSupport({ kind: "unavailable", reason: outcome.message });
        setAttempt({ kind: "idle" });
        return;
      case "failed":
        setAttempt({ kind: "problem", message: outcome.message });
        return;
      default: {
        const unreachable: never = outcome;
        return unreachable;
      }
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start gap-3">
        <HugeiconsIcon
          icon={FingerPrintIcon}
          size={21}
          strokeWidth={1.5}
          color="currentColor"
          className="mt-0.5 shrink-0 text-accent"
          aria-hidden="true"
        />
        <div className="min-w-0 flex-1">
          <h3 className="text-[14px] leading-tight font-semibold text-ink">
            {t("wizard.passkey.heading")}
          </h3>
          <StepText className="mt-1">{t("wizard.passkey.body")}</StepText>
        </div>
      </div>

      {support === null && (
        <p className="text-[13px] text-faint" aria-busy="true">
          {t("wizard.passkey.checking")}
        </p>
      )}

      {/* Visibly unavailable with a reason, never a button that fails. */}
      {support?.kind === "unavailable" && (
        <Alert variant="default">
          <HugeiconsIcon icon={SecurityLockIcon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.passkey.unavailable.title")}</AlertTitle>
          <AlertDescription>
            <p>{support.reason}</p>
          </AlertDescription>
        </Alert>
      )}

      {support?.kind === "available" && attempt.kind !== "registered" && (
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-3">
            <Button
              variant="secondary"
              disabled={attempt.kind === "working"}
              onClick={() => void register()}
            >
              <HugeiconsIcon
                icon={FingerPrintIcon}
                size={18}
                strokeWidth={1.5}
                color="currentColor"
                aria-hidden="true"
              />
              {t("wizard.passkey.create")}
            </Button>
            {attempt.kind === "working" && (
              <Spinner
                label={t("wizard.passkey.waiting")}
                className="text-muted"
              />
            )}
          </div>
          {!support.platformAuthenticator && (
            <p className="text-[12.5px] leading-snug text-faint">
              {t("wizard.passkey.securityKey")}
            </p>
          )}
        </div>
      )}

      {attempt.kind === "registered" && (
        <Alert variant="ok">
          <HugeiconsIcon icon={CheckmarkCircle02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.passkey.created.title")}</AlertTitle>
          <AlertDescription>
            <p>
              <Rich
                k="wizard.passkey.created.body"
                vars={{
                  label: (
                    <span className="numeric text-ink">{attempt.label}</span>
                  ),
                }}
              />
            </p>
          </AlertDescription>
        </Alert>
      )}

      {/* The expected outcome today: the routes in ./passkey.ts are not
          served. Say so plainly rather than showing a transport error — the
          owner has done nothing wrong and there is nothing for them to fix. */}
      {attempt.kind === "not-implemented" && (
        <Alert variant="default">
          <HugeiconsIcon icon={InformationCircleIcon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.passkey.notYet.title")}</AlertTitle>
          <AlertDescription>
            <p>{t("wizard.passkey.notYet.body")}</p>
          </AlertDescription>
        </Alert>
      )}

      {attempt.kind === "problem" && (
        <Alert variant="warn">
          <HugeiconsIcon icon={Alert02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("wizard.passkey.problem.title")}</AlertTitle>
          <AlertDescription>
            <p className={cn("mt-1 break-words")}>{attempt.message}</p>
          </AlertDescription>
        </Alert>
      )}
    </div>
  );
}
