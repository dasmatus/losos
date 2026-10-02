/* Step 2 — Choose how you sign in.
 *
 * This is the step the whole wizard exists for. modules/nextcloud-common.nix
 * generates the first admin password from /dev/urandom, writes it 0600 and
 * shows it to nobody; the appliance has no SSH and no shell login, so without
 * this step a fresh box cannot be signed into at all.
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
  FingerPrintIcon,
  InformationCircleIcon,
  LockPasswordIcon,
  SecurityLockIcon,
  UserCircleIcon,
  ViewIcon,
  ViewOffSlashIcon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { FieldError, Input } from "@/components/ui/input";
import { Label, LabelHint } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/progress";
import { isUnauthorized } from "@/lib/api";
import { t } from "@/lib/i18n";
import { Rich, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { isMissingRoute, passwordProblem, postSetPassword, MIN_PASSWORD_CHARS } from "./api";
import { createPasskey, probePasskeySupport, type PasskeySupport } from "./passkey";
import { Callout, ReadoutRow, StepText } from "./parts";

export interface StepSignInProps {
  /** The box's own name, for the label the passkey is saved under. */
  boxName: string;
  /** The account lososd reported after the password was set, or null. */
  account: string | null;
  onPasswordSet: (user: string) => void;
}

export function StepSignIn({ boxName, account, onPasswordSet }: StepSignInProps) {
  const t = useT();
  return (
    <div className="flex flex-col gap-5">
      <StepText>{t("wizard.signin.intro")}</StepText>

      <PasswordForm account={account} onPasswordSet={onPasswordSet} />

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
  onPasswordSet: (user: string) => void;
}) {
  const t = useT();
  const [password, setPassword] = React.useState("");
  const [confirm, setConfirm] = React.useState("");
  const [visible, setVisible] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);
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
      const result = await postSetPassword(password);
      setPassword("");
      setConfirm("");
      setVisible(false);
      onPasswordSet(result.user);
    } catch (error) {
      setProblem(describeSetPassword(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} noValidate className="flex flex-col gap-4">
      <div className="flex flex-col gap-2">
        <Label htmlFor={passwordId}>{t("wizard.signin.newPassword")}</Label>
        <div className="flex items-center gap-2">
          <Input
            id={passwordId}
            name="new-password"
            type={visible ? "text" : "password"}
            autoComplete="new-password"
            value={password}
            disabled={busy}
            aria-invalid={problem !== null}
            aria-describedby={problem !== null ? problemId : undefined}
            onChange={(event) => {
              setPassword(event.target.value);
              setProblem(null);
            }}
          />
          <Button
            variant="secondary"
            size="icon"
            aria-pressed={visible}
            aria-label={visible ? t("wizard.signin.hide") : t("wizard.signin.show")}
            onClick={() => setVisible((shown) => !shown)}
          >
            <HugeiconsIcon
              icon={visible ? ViewOffSlashIcon : ViewIcon}
              size={18}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
          </Button>
        </div>
        <LabelHint>
          {t("wizard.signin.hint", { count: MIN_PASSWORD_CHARS })}
        </LabelHint>
      </div>

      <div className="flex flex-col gap-2">
        <Label htmlFor={confirmId}>{t("wizard.signin.again")}</Label>
        <Input
          id={confirmId}
          name="confirm-password"
          type={visible ? "text" : "password"}
          autoComplete="new-password"
          value={confirm}
          disabled={busy}
          aria-invalid={problem !== null}
          onChange={(event) => {
            setConfirm(event.target.value);
            setProblem(null);
          }}
        />
        <LabelHint>{t("wizard.signin.againHint")}</LabelHint>
      </div>

      <FieldError id={problemId}>{problem}</FieldError>

      <div className="flex items-center gap-3">
        <Button type="submit" disabled={busy || password.length === 0 || confirm.length === 0}>
          <HugeiconsIcon
            icon={LockPasswordIcon}
            size={18}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
          {account === null ? t("wizard.signin.set") : t("wizard.signin.setDifferent")}
        </Button>
        {busy && <Spinner label={t("wizard.signin.setting")} className="text-muted" />}
      </div>

      {account !== null && (
        <Callout tone="ok" icon={CheckmarkCircle02Icon} title={t("wizard.signin.done.title")}>
          <ReadoutRow label={t("wizard.signin.done.name")} className="mt-2">
            <code className="numeric text-[13px] text-ink select-all">{account}</code>
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
        </Callout>
      )}
    </form>
  );
}

function describeSetPassword(error: unknown): string {
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
        <Callout tone="info" icon={SecurityLockIcon} title={t("wizard.passkey.unavailable.title")}>
          <p className="mt-1">{support.reason}</p>
        </Callout>
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
              <Spinner label={t("wizard.passkey.waiting")} className="text-muted" />
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
        <Callout tone="ok" icon={CheckmarkCircle02Icon} title={t("wizard.passkey.created.title")}>
          <p className="mt-1">
            <Rich
              k="wizard.passkey.created.body"
              vars={{ label: <span className="numeric text-ink">{attempt.label}</span> }}
            />
          </p>
        </Callout>
      )}

      {/* The expected outcome today: the routes in ./passkey.ts are not
          served. Say so plainly rather than showing a transport error — the
          owner has done nothing wrong and there is nothing for them to fix. */}
      {attempt.kind === "not-implemented" && (
        <Callout tone="info" icon={InformationCircleIcon} title={t("wizard.passkey.notYet.title")}>
          <p className="mt-1">{t("wizard.passkey.notYet.body")}</p>
        </Callout>
      )}

      {attempt.kind === "problem" && (
        <Callout tone="warn" icon={Alert02Icon} title={t("wizard.passkey.problem.title")}>
          <p className={cn("mt-1 break-words")}>{attempt.message}</p>
        </Callout>
      )}
    </div>
  );
}
