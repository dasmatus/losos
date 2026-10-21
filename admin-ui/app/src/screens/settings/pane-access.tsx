import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { CheckmarkCircle02Icon, Copy01Icon, Key01Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { FieldError } from "@/components/ui/field";
import { HelpLink } from "@/components/ui/help-link";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/progress";
import { toast } from "@/components/ui/toast";
import type { AccessExpiry, AccessKey, AccessResponse, AccessScope, AccessToken } from "@/lib/api";
import { intlTag, type MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { copyText } from "@/screens/wizard/copy";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";
import { useAccess, type AccessData } from "./use-access";

/* Tokens for `losos`, the terminal client (docs/terminal-access.md).
 *
 * Two things are needed to reach the box from a terminal: a token made here
 * and the secret half of a GPG key the installer seeded. This pane shows the
 * keys and cannot add one, on purpose: a stolen web session can make a
 * token, but with no way to seed a key it has nothing to sign with.
 *
 * The box's own key comes first because it is the thing to compare. The
 * client pins it on first contact the way SSH pins a host key, and this page
 * is where the owner reads what it should be.
 *
 * A new token's secret is in the reply that made it and nowhere else. It
 * stays in this component's state until the owner presses Done or leaves
 * the pane, and is never written to storage. */

/** Hex in groups of four: "ABCD 1234 …", the way gpg prints it. */
export function groupHex(fingerprint: string): string {
  const hex = fingerprint.toUpperCase().replace(/[^0-9A-F]/g, "");
  return hex.match(/.{1,4}/g)?.join(" ") ?? "";
}

/** The long key ID: the last 16 hex characters, as `gpg -K` shows it. */
export function shortFingerprint(fingerprint: string): string {
  return fingerprint.toUpperCase().replace(/[^0-9A-F]/g, "").slice(-16);
}

/** The most tokens lososd keeps; a 33rd is a 409. */
export const MAX_TOKENS = 32;

const SCOPES: { id: AccessScope; label: MessageKey; detail: MessageKey }[] = [
  { id: "read", label: "access.scope.read", detail: "access.form.readDetail" },
  { id: "full", label: "access.scope.full", detail: "access.form.fullDetail" },
];

const EXPIRIES: { id: string; days: AccessExpiry; label: MessageKey }[] = [
  { id: "30", days: 30, label: "access.form.days30" },
  { id: "90", days: 90, label: "access.form.days90" },
  { id: "365", days: 365, label: "access.form.days365" },
  { id: "never", days: null, label: "access.form.never" },
];

export function AccessPane({ locked }: { locked: boolean }) {
  const t = useT();
  const access = useAccess(!locked);
  const { state } = access;

  if (state.kind === "loading") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("access.loading")} />
            <Spinner size={16} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "unavailable") {
    return (
      <PaneSection data-testid="access-unavailable">
        <Group>
          <Row last>
            <RowText title={t("access.unavailable")} />
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
            <RowText title={t("access.failed")} detail={state.message} />
            <Button size="sm" variant="secondary" disabled={locked} onClick={access.refresh}>
              {t("access.retry")}
            </Button>
          </Row>
        </Group>
      </PaneSection>
    );
  }

  const { view } = state;
  return (
    <>
      <HostKeySection view={view} />
      <KeysSection keys={view.keys} />
      <TokensSection view={view} access={access} disabled={locked || access.busy} />
      <NewTokenSection view={view} access={access} locked={locked} />
    </>
  );
}

// ── The box's own key ───────────────────────────────────────────────────

function HostKeySection({ view }: { view: AccessResponse }) {
  const t = useT();
  const fingerprint = view.hostKey?.fingerprint ?? "";
  return (
    <PaneSection data-testid="access-host-key">
      <GroupTitle>{t("access.host.title")}</GroupTitle>
      <Group>
        <Row last className="max-sm:flex-col max-sm:items-start max-sm:gap-1">
          <RowText title={t("access.host.fingerprint")} />
          <span
            data-testid="access-host-fingerprint"
            className="numeric min-w-0 text-[13px] break-words text-ink tabular-nums select-all sm:text-right"
          >
            {fingerprint.length > 0 ? groupHex(fingerprint) : "—"}
          </span>
        </Row>
      </Group>
      <GroupCaption>{t("access.host.caption")}</GroupCaption>
    </PaneSection>
  );
}

// ── The seeded keys ─────────────────────────────────────────────────────

/* Reads the language on screen, so a switch of language re-renders it. */
function useDay(): (seconds: number) => string {
  useLocale();
  return (seconds) =>
    new Intl.DateTimeFormat(intlTag(), { dateStyle: "medium" }).format(new Date(seconds * 1000));
}

function KeysSection({ keys }: { keys: AccessKey[] }) {
  const t = useT();
  const day = useDay();
  return (
    <PaneSection data-testid="access-keys">
      <GroupTitle>{t("access.keys.title")}</GroupTitle>
      {keys.length === 0 ? (
        <Empty className="bg-surface" data-testid="access-keys-empty">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={Key01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("access.keys.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("access.keys.emptyBody")}</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <HelpLink entry="terminal" label={t("access.keys.emptyLink")} />
          </EmptyContent>
        </Empty>
      ) : (
        <Group>
          {keys.map((key) => (
            <StackRow key={key.fingerprint} data-testid="access-key" className="gap-1">
              <div className="flex items-baseline justify-between gap-3">
                <div className="min-w-0">
                  {key.userIds.length === 0 ? (
                    <p className="text-sm text-muted">{t("access.keys.noUserId")}</p>
                  ) : (
                    key.userIds.map((uid) => (
                      <p key={uid} className="truncate text-sm leading-snug text-ink">
                        {uid}
                      </p>
                    ))
                  )}
                </div>
                {key.created !== null && (
                  <span className="shrink-0 text-[12.5px] text-muted">
                    {t("access.keys.created", { date: day(key.created) })}
                  </span>
                )}
              </div>
              <p className="numeric text-[12.5px] break-words text-muted tabular-nums select-all">
                {groupHex(key.fingerprint)}
              </p>
            </StackRow>
          ))}
        </Group>
      )}
      {keys.length > 0 && <GroupCaption>{t("access.keys.caption")}</GroupCaption>}
    </PaneSection>
  );
}

// ── The tokens ──────────────────────────────────────────────────────────

function TokensSection({
  view,
  access,
  disabled,
}: {
  view: AccessResponse;
  access: AccessData;
  disabled: boolean;
}) {
  const t = useT();
  const day = useDay();
  const [revoking, setRevoking] = React.useState<AccessToken | null>(null);
  const titleId = React.useId();
  const bodyId = React.useId();
  const now = Date.now() / 1000;

  return (
    <PaneSection data-testid="access-tokens">
      <GroupTitle>{t("access.tokens.title")}</GroupTitle>
      <Group>
        {view.tokens.length === 0 ? (
          <Row last>
            <RowText title={t("access.tokens.none")} />
          </Row>
        ) : (
          view.tokens.map((token) => {
            const expired = token.expires !== null && token.expires <= now;
            return (
              <StackRow key={token.id} data-testid="access-token" data-token-id={token.id}>
                <div className="flex items-center gap-3">
                  <p className="min-w-0 flex-1 truncate text-sm text-ink">{token.name}</p>
                  {expired && <Badge variant="warn">{t("access.tokens.expired")}</Badge>}
                  <Badge variant={token.scope === "full" ? "local" : "neutral"}>
                    {t(token.scope === "full" ? "access.scope.full" : "access.scope.read")}
                  </Badge>
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={disabled}
                    onClick={() => setRevoking(token)}
                  >
                    {t("access.tokens.revoke")}
                  </Button>
                </div>
                <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-[12.5px] leading-snug sm:grid-cols-4">
                  <Fact label={t("access.tokens.key")}>
                    <span className="numeric tabular-nums">{shortFingerprint(token.fingerprint)}</span>
                  </Fact>
                  <Fact label={t("access.tokens.created")}>{day(token.created)}</Fact>
                  <Fact label={t("access.tokens.expires")}>
                    {token.expires === null ? t("access.tokens.never") : day(token.expires)}
                  </Fact>
                  <Fact label={t("access.tokens.lastUsed")}>
                    {token.lastUsed === null ? t("access.tokens.neverUsed") : day(token.lastUsed)}
                  </Fact>
                </dl>
              </StackRow>
            );
          })
        )}
      </Group>
      <GroupCaption>{t("access.tokens.caption")}</GroupCaption>

      <Dialog
        open={revoking !== null}
        onOpenChange={(open) => {
          if (!open) setRevoking(null);
        }}
        labelledBy={titleId}
        describedBy={bodyId}
      >
        <DialogHeader>
          <DialogTitle id={titleId}>
            {t("access.tokens.dialogTitle", { name: revoking?.name ?? "" })}
          </DialogTitle>
          <DialogDescription id={bodyId}>{t("access.tokens.dialogBody")}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setRevoking(null)}>
            {t("access.tokens.keep")}
          </Button>
          <Button
            onClick={() => {
              const token = revoking;
              setRevoking(null);
              if (token !== null) void access.revoke(token.id, token.name);
            }}
          >
            {t("access.tokens.confirm")}
          </Button>
        </DialogFooter>
      </Dialog>
    </PaneSection>
  );
}

function Fact({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="min-w-0">
      <dt className="text-muted">{label}</dt>
      <dd className="truncate text-ink">{children}</dd>
    </div>
  );
}

// ── Making a token ──────────────────────────────────────────────────────

interface Draft {
  name: string;
  fingerprint: string;
  scope: AccessScope;
  expiry: string;
}

function NewTokenSection({
  view,
  access,
  locked,
}: {
  view: AccessResponse;
  access: AccessData;
  locked: boolean;
}) {
  const t = useT();
  const ids = {
    name: React.useId(),
    key: React.useId(),
    scope: React.useId(),
    expiry: React.useId(),
  };
  const [draft, setDraft] = React.useState<Draft>({
    name: "",
    fingerprint: "",
    scope: "read",
    expiry: "90",
  });
  const [error, setError] = React.useState<string | null>(null);
  const [created, setCreated] = React.useState<string | null>(null);

  const noKeys = view.keys.length === 0;
  const full = view.tokens.length >= MAX_TOKENS;
  const disabled = locked || access.busy || noKeys || full;
  // A key that is no longer listed (or none picked yet) falls back to the first.
  const fingerprint = view.keys.some((k) => k.fingerprint === draft.fingerprint)
    ? draft.fingerprint
    : (view.keys[0]?.fingerprint ?? "");
  const name = draft.name.trim();
  const ready = name.length >= 1 && name.length <= 64 && fingerprint.length > 0;
  const scopeDetail = SCOPES.find((s) => s.id === draft.scope)?.detail ?? "access.form.readDetail";

  const submit = async () => {
    if (!ready) return;
    setError(null);
    // `null` is "never", so a missing choice must not fall back through `??`.
    const expiry = EXPIRIES.find((e) => e.id === draft.expiry);
    const outcome = await access.create({
      name,
      fingerprint,
      scope: draft.scope,
      expiresInDays: expiry === undefined ? 90 : expiry.days,
    });
    if (outcome.ok) {
      setCreated(outcome.token);
      setDraft((d) => ({ ...d, name: "" }));
    } else if (outcome.message.length > 0) {
      setError(outcome.message);
    }
  };

  const reason = noKeys
    ? t("access.form.needsKey")
    : full
      ? t("access.form.full", { count: MAX_TOKENS })
      : undefined;

  return (
    <PaneSection data-testid="access-form">
      <GroupTitle>{t("access.form.title")}</GroupTitle>
      {created !== null && <CreatedToken token={created} onDone={() => setCreated(null)} />}
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <Group>
          <FieldRow id={ids.name} title={t("access.form.name")}>
            <Input
              id={ids.name}
              value={draft.name}
              placeholder={t("access.form.namePlaceholder")}
              maxLength={64}
              autoComplete="off"
              spellCheck={false}
              disabled={disabled}
              aria-invalid={error !== null ? true : undefined}
              onChange={(event) => setDraft((d) => ({ ...d, name: event.target.value }))}
            />
          </FieldRow>
          <FieldRow id={ids.key} title={t("access.form.key")}>
            <NativeSelect
              id={ids.key}
              className="w-full"
              value={fingerprint}
              disabled={disabled}
              onChange={(event) => setDraft((d) => ({ ...d, fingerprint: event.target.value }))}
            >
              {view.keys.map((key) => (
                <NativeSelectOption key={key.fingerprint} value={key.fingerprint}>
                  {keyLabel(key)}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </FieldRow>
          <FieldRow id={ids.scope} title={t("access.form.access")} detail={t(scopeDetail)}>
            <NativeSelect
              id={ids.scope}
              className="w-full"
              value={draft.scope}
              disabled={disabled}
              onChange={(event) =>
                setDraft((d) => ({ ...d, scope: event.target.value as AccessScope }))
              }
            >
              {SCOPES.map((scope) => (
                <NativeSelectOption key={scope.id} value={scope.id}>
                  {t(scope.label)}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </FieldRow>
          <FieldRow id={ids.expiry} title={t("access.form.expires")}>
            <NativeSelect
              id={ids.expiry}
              className="w-full"
              value={draft.expiry}
              disabled={disabled}
              onChange={(event) => setDraft((d) => ({ ...d, expiry: event.target.value }))}
            >
              {EXPIRIES.map((expiry) => (
                <NativeSelectOption key={expiry.id} value={expiry.id}>
                  {t(expiry.label)}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </FieldRow>
          <Row last className="flex-wrap justify-end">
            {reason !== undefined && (
              <p className="mr-auto text-[12.5px] leading-snug text-muted">{reason}</p>
            )}
            {error !== null && (
              <FieldError className="mr-auto" data-testid="access-error">
                {error}
              </FieldError>
            )}
            <Button type="submit" size="sm" disabled={disabled || !ready}>
              {t("access.form.create")}
            </Button>
          </Row>
        </Group>
      </form>
      <GroupCaption>{t("access.form.caption")}</GroupCaption>
    </PaneSection>
  );
}

/* "Matus <matus@example.org> · 89ABCDEF01234567": the first user ID and the
 * long key ID, which tell two keys of the same person apart. */
function keyLabel(key: AccessKey): string {
  const id = shortFingerprint(key.fingerprint);
  const uid = key.userIds[0];
  return uid === undefined ? id : `${uid} · ${id}`;
}

function FieldRow({
  id,
  title,
  detail,
  children,
}: {
  id: string;
  title: string;
  detail?: string | undefined;
  children: React.ReactNode;
}) {
  return (
    <Row>
      <RowText htmlFor={id} title={title} detail={detail} />
      <div className="w-[16rem] max-sm:w-[11rem]">{children}</div>
    </Row>
  );
}

/* The one time the secret is on screen. */
function CreatedToken({ token, onDone }: { token: string; onDone: () => void }) {
  const t = useT();
  const tokenRef = React.useRef<HTMLElement>(null);
  const command = `losos login ${pageOrigin()}`;

  const copy = async () => {
    if (await copyText(token, tokenRef.current)) toast.done(t("access.created.copied"));
    else toast.error(t("access.created.copyFailed"), undefined, { help: "terminal" });
  };

  return (
    <Alert variant="ok" className="mb-3" data-testid="access-created">
      <HugeiconsIcon icon={CheckmarkCircle02Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <AlertTitle>{t("access.created.title")}</AlertTitle>
      <AlertDescription className="gap-2.5">
        <p>{t("access.created.once")}</p>
        <div className="flex flex-wrap items-center gap-2">
          <code
            ref={tokenRef}
            data-testid="access-created-token"
            className="numeric min-w-0 flex-1 rounded-control border border-line bg-surface px-2.5 py-1.5 text-[13px] break-all text-ink select-all"
          >
            {token}
          </code>
          <Button size="sm" variant="secondary" onClick={() => void copy()}>
            <HugeiconsIcon icon={Copy01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("access.created.copy")}
          </Button>
        </div>
        <p>{t("access.created.use")}</p>
        <code
          data-testid="access-created-command"
          className="numeric rounded-control border border-line bg-surface px-2.5 py-1.5 text-[13px] break-all text-ink select-all"
        >
          {command}
        </code>
        <div className="flex justify-end">
          <Button size="sm" onClick={onDone}>
            {t("access.created.done")}
          </Button>
        </div>
      </AlertDescription>
    </Alert>
  );
}

/* The origin in the address bar: the address this browser reached the box
 * on, which is the one the client can reach it on too. */
function pageOrigin(): string {
  try {
    const origin = window.location.origin;
    return origin === "null" ? "http://<box>" : origin;
  } catch {
    return "http://<box>";
  }
}
