import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { LinkSquare02Icon, SquareLock01Icon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { FieldError } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import {
  ApiError,
  postAccountCode,
  twoFactorSettingsUrl,
  type Account,
  type LoginReason,
} from "@/lib/api";
import { intlTag } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { MIN_PASSWORD_CHARS, passwordRules } from "@/screens/wizard/api";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow } from "./rows";
import { useAccount, type AccountData } from "./use-account";

/* Your account: a person's own name, email address, password and second
 * factors on LosOS Login.
 *
 * LosOS Login (Authelia) draws its own page for adding an authenticator app
 * or a passkey, and asks for a one-time code first. This box sends no email:
 * Authelia writes the code to a file only lososd reads, and the One-time
 * code row hands it over behind the person's password
 * (`POST /api/account/code`).
 *
 * Someone a rule holds (backend/src/login.rs) sees only this pane, from
 * every address, with a notice saying what to do (App.tsx). */

function when(seconds: number): string {
  return new Intl.DateTimeFormat(intlTag(), { dateStyle: "long" }).format(new Date(seconds * 1000));
}

export function AccountPane({ locked }: { locked: boolean }) {
  const t = useT();
  const account = useAccount(!locked);
  const { state } = account;

  if (state.kind === "loading") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("account.loading")} />
            <Spinner size={16} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "noAccount") {
    return (
      <PaneSection data-testid="account-none">
        <Group>
          <Row last>
            <RowText title={t("account.noAccount")} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "failed") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("account.failed")} detail={state.message} />
            <Button size="sm" variant="secondary" disabled={locked} onClick={account.refresh}>
              {t("account.retry")}
            </Button>
          </Row>
        </Group>
      </PaneSection>
    );
  }

  const disabled = locked || account.busy;
  return (
    <>
      {state.account.locked.length > 0 && <HeldNotice reasons={state.account.locked} />}
      <DetailsSection account={state.account} data={account} disabled={disabled} />
      <PasswordSection account={state.account} data={account} disabled={disabled} />
      <FactorsSection account={state.account} disabled={disabled} />
    </>
  );
}

// ── Held by a rule ──────────────────────────────────────────────────────

function HeldNotice({ reasons }: { reasons: LoginReason[] }) {
  const t = useT();
  return (
    <div
      role="status"
      data-testid="account-held"
      className="animate-fade-in mb-6 flex items-start gap-2.5 rounded-card border border-warn/35 bg-warn/8 p-3 text-[13px] leading-snug text-ink"
    >
      <HugeiconsIcon
        icon={SquareLock01Icon}
        size={17}
        strokeWidth={1.5}
        color="currentColor"
        className="mt-px shrink-0 text-warn"
        aria-hidden="true"
      />
      <div className="min-w-0 flex-1 space-y-1">
        <p className="font-medium">{t("account.held.title")}</p>
        {reasons.includes("password") && <p data-reason="password">{t("account.held.password")}</p>}
        {reasons.includes("twoFactor") && <p data-reason="twoFactor">{t("account.held.twoFactor")}</p>}
        <p className="text-muted">{t("account.held.after")}</p>
      </div>
    </div>
  );
}

// ── Details ─────────────────────────────────────────────────────────────

function DetailsSection({
  account,
  data,
  disabled,
}: {
  account: Account;
  data: AccountData;
  disabled: boolean;
}) {
  const t = useT();
  const ids = { name: React.useId(), email: React.useId() };
  const [name, setName] = React.useState(account.displayName);
  const [email, setEmail] = React.useState(account.email);
  const [error, setError] = React.useState<string | null>(null);

  // A fresh answer (after a save, or the tab's focus coming back) is the new
  // starting point, unless the person is in the middle of typing.
  const saved = React.useRef({ name: account.displayName, email: account.email });
  React.useEffect(() => {
    setName((typed) => (typed === saved.current.name ? account.displayName : typed));
    setEmail((typed) => (typed === saved.current.email ? account.email : typed));
    saved.current = { name: account.displayName, email: account.email };
  }, [account.displayName, account.email]);

  const changed = name.trim() !== account.displayName || email.trim() !== account.email;
  const ready = changed && name.trim().length > 0 && email.includes("@");

  const submit = async () => {
    if (!ready) return;
    setError(null);
    const outcome = await data.saveDetails(name.trim(), email.trim());
    if (!outcome.ok && outcome.message.length > 0) setError(outcome.message);
  };

  return (
    <PaneSection data-testid="account-details">
      <GroupTitle>{t("account.details.title")}</GroupTitle>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <Group>
          <Row>
            <RowText title={t("account.details.signInName")} />
            <RowValue data-testid="account-sign-in-name">{account.name}</RowValue>
          </Row>
          <FieldRow id={ids.name} title={t("account.details.name")}>
            <Input
              id={ids.name}
              data-testid="account-name"
              value={name}
              maxLength={100}
              autoComplete="name"
              disabled={disabled}
              onChange={(event) => setName(event.target.value)}
            />
          </FieldRow>
          <FieldRow id={ids.email} title={t("account.details.email")} last>
            <Input
              id={ids.email}
              data-testid="account-email"
              type="email"
              value={email}
              maxLength={254}
              autoComplete="email"
              disabled={disabled}
              aria-invalid={error !== null ? true : undefined}
              onChange={(event) => setEmail(event.target.value)}
            />
          </FieldRow>
        </Group>
        <div className="flex flex-wrap items-center justify-end gap-3 pt-3">
          {error !== null && <FieldError className="mr-auto">{error}</FieldError>}
          <Button type="submit" size="sm" data-testid="account-details-save" disabled={disabled || !ready}>
            {t("account.details.save")}
          </Button>
        </div>
      </form>
      <GroupCaption>{t("account.details.caption")}</GroupCaption>
    </PaneSection>
  );
}

// ── Password ────────────────────────────────────────────────────────────

function PasswordSection({
  account,
  data,
  disabled,
}: {
  account: Account;
  data: AccountData;
  disabled: boolean;
}) {
  const t = useT();
  const id = React.useId();
  const [password, setPassword] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const ready = passwordRules(password).every((rule) => rule.met);
  const expired = account.locked.includes("password");

  const submit = async () => {
    if (!ready) return;
    setError(null);
    const outcome = await data.changePassword(password);
    if (outcome.ok) setPassword("");
    else if (outcome.message.length > 0) setError(outcome.message);
  };

  return (
    <PaneSection data-testid="account-password">
      <GroupTitle>{t("account.password.title")}</GroupTitle>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <Group>
          <Row>
            <RowText title={t("account.password.changed")} />
            <RowValue>{when(account.passwordSetAt)}</RowValue>
          </Row>
          <Row>
            <RowText title={t("account.password.expires")} />
            <RowValue data-testid="account-password-expires" className={expired ? "text-warn" : undefined}>
              {account.passwordExpiresAt === null ? t("account.password.never") : when(account.passwordExpiresAt)}
            </RowValue>
          </Row>
          <FieldRow
            id={id}
            title={t("account.password.new")}
            detail={t("account.password.detail", { count: MIN_PASSWORD_CHARS })}
            last
          >
            <Input
              id={id}
              data-testid="account-new-password"
              type="password"
              autoComplete="new-password"
              value={password}
              disabled={disabled}
              aria-invalid={error !== null ? true : undefined}
              onChange={(event) => setPassword(event.target.value)}
            />
          </FieldRow>
        </Group>
        <div className="flex flex-wrap items-center justify-end gap-3 pt-3">
          {error !== null && <FieldError className="mr-auto">{error}</FieldError>}
          <Button type="submit" size="sm" data-testid="account-password-save" disabled={disabled || !ready}>
            {t("account.password.save")}
          </Button>
        </div>
      </form>
      <GroupCaption>{t("account.password.caption")}</GroupCaption>
    </PaneSection>
  );
}

// ── Two-factor sign-in ──────────────────────────────────────────────────

function FactorsSection({ account, disabled }: { account: Account; disabled: boolean }) {
  const t = useT();
  const { app, passkeys } = account.factors;
  return (
    <PaneSection data-testid="account-factors">
      <GroupTitle>{t("account.factors.title")}</GroupTitle>
      <Group>
        <Row>
          <RowText title={t("account.factors.app")} />
          <Badge variant={app ? "ok" : "neutral"} data-testid="account-app">
            {app ? t("account.factors.added") : t("account.factors.notAdded")}
          </Badge>
        </Row>
        <Row>
          <RowText
            title={t("account.factors.passkeys")}
            detail={passkeys.length > 0 ? passkeys.join(", ") : undefined}
          />
          <RowValue data-testid="account-passkeys">
            {passkeys.length > 0 ? passkeys.length : t("account.factors.none")}
          </RowValue>
        </Row>
        <Row>
          <RowText title={t("account.factors.open")} detail={t("account.factors.openDetail")} />
          <a
            href={twoFactorSettingsUrl()}
            target="_blank"
            rel="noopener"
            data-testid="account-open-login"
            aria-disabled={disabled ? true : undefined}
            className={cn(buttonVariants({ variant: "secondary", size: "sm" }), disabled && "pointer-events-none opacity-50")}
          >
            {t("account.factors.open")}
            <HugeiconsIcon icon={LinkSquare02Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          </a>
        </Row>
        <CodeRow disabled={disabled} />
      </Group>
      <GroupCaption>
        {t("account.factors.caption")}
        {account.rules.twoFactor && ` ${t("account.factors.required")}`}
      </GroupCaption>
    </PaneSection>
  );
}

function CodeRow({ disabled }: { disabled: boolean }) {
  const t = useT();
  const id = React.useId();
  const [password, setPassword] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [code, setCode] = React.useState<string | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  const ask = async () => {
    if (password.length === 0) return;
    setBusy(true);
    setError(null);
    setCode(null);
    try {
      const answer = await postAccountCode(password);
      setCode(answer.code);
      setPassword("");
    } catch (failure) {
      const status = failure instanceof ApiError ? failure.status : 0;
      setError(
        status === 403
          ? t("account.code.wrongPassword")
          : status === 404
            ? t("account.code.none")
            : t("account.code.failed"),
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <StackRow last data-testid="account-code">
      <RowText title={t("account.code.title")} detail={t("account.code.detail")} />
      <form
        className="flex flex-wrap items-center gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          void ask();
        }}
      >
        <label htmlFor={id} className="sr-only">
          {t("account.code.password")}
        </label>
        <Input
          id={id}
          data-testid="account-code-password"
          type="password"
          autoComplete="current-password"
          placeholder={t("account.code.password")}
          className="w-[16rem] max-sm:w-full"
          value={password}
          disabled={disabled || busy}
          aria-invalid={error !== null ? true : undefined}
          onChange={(event) => setPassword(event.target.value)}
        />
        <Button
          type="submit"
          size="sm"
          variant="secondary"
          data-testid="account-code-show"
          disabled={disabled || busy || password.length === 0}
        >
          {t("account.code.show")}
        </Button>
      </form>
      {error !== null && <FieldError data-testid="account-code-error">{error}</FieldError>}
      {code !== null && (
        <p className="text-[13px] leading-snug text-ink" aria-live="polite">
          {t("account.code.value")}{" "}
          <span data-testid="account-code-value" className="numeric font-medium tracking-wider select-all">
            {code}
          </span>
        </p>
      )}
    </StackRow>
  );
}

function FieldRow({
  id,
  title,
  detail,
  last,
  children,
}: {
  id: string;
  title: string;
  detail?: string | undefined;
  last?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Row last={last}>
      <RowText htmlFor={id} title={title} detail={detail} />
      <div className="w-[16rem] max-sm:w-[11rem]">{children}</div>
    </Row>
  );
}
