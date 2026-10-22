import * as React from "react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { HelpLink } from "@/components/ui/help-link";
import { Spinner } from "@/components/ui/progress";
import type { Credential, CredentialKey } from "@/lib/api";
import { intlTag } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";
import {
  APP_ORDER,
  appTitle,
  credentialDetail,
  credentialRotateEffect,
  credentialTitle,
} from "./pane-credentials-words";
import { useCredentials, type CredentialsData } from "./use-credentials";

/* Every secret the box keeps, and nothing of any of them.
 *
 * Each one is a systemd credential sealed to this box (modules/credentials.nix):
 * to its TPM, or to a key only root can read on a box without one. The page
 * shows what each is for, what reads it, what it is sealed to and when it
 * last changed, and lets an administrator make a new value for the ones a
 * new value cannot break. A value is never on this page, in either
 * direction: the box makes the new one and seals it itself, and the one
 * secret an owner types (the backup bucket's key) is typed on the pane that
 * uses it, which this page links.
 *
 * Grouped by what the secret belongs to, so "LosOS Login" reads as one
 * thing with three keys rather than three unrelated rows. */

export function CredentialsPane({ locked }: { locked: boolean }) {
  const t = useT();
  const data = useCredentials(!locked);
  const { state } = data;

  if (state.kind === "loading") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("credentials.loading")} />
            <Spinner size={16} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "unavailable" || (state.kind === "ready" && state.view.key === null)) {
    return (
      <PaneSection data-testid="credentials-unavailable">
        <Group>
          <Row last>
            <RowText title={t("credentials.unavailable")} />
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
            <RowText title={t("credentials.failed")} detail={state.message} />
            <Button size="sm" variant="secondary" disabled={locked} onClick={data.refresh}>
              {t("credentials.retry")}
            </Button>
          </Row>
        </Group>
      </PaneSection>
    );
  }

  const { view } = state;
  const groups = groupByApp(view.credentials);
  return (
    <>
      <SealSection sealedWith={view.key ?? "host"} count={view.credentials.length} />
      {groups.map(([app, list]) => (
        <AppSection key={app} app={app} list={list} data={data} locked={locked} />
      ))}
    </>
  );
}

function groupByApp(list: Credential[]): [string, Credential[]][] {
  const byApp = new Map<string, Credential[]>();
  for (const c of list) byApp.set(c.app, [...(byApp.get(c.app) ?? []), c]);
  const rank = (app: string) => {
    const i = APP_ORDER.indexOf(app);
    return i === -1 ? APP_ORDER.length : i;
  };
  return [...byApp.entries()].sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b));
}

// ── What they are sealed to ─────────────────────────────────────────────

function SealSection({ sealedWith, count }: { sealedWith: CredentialKey; count: number }) {
  const t = useT();
  return (
    <PaneSection data-testid="credentials-seal">
      <GroupTitle>{t("credentials.seal.title")}</GroupTitle>
      <Group>
        <Row last>
          <RowText
            title={t(sealedWith === "tpm2" ? "credentials.seal.tpm" : "credentials.seal.host")}
            detail={t(
              sealedWith === "tpm2" ? "credentials.seal.tpmDetail" : "credentials.seal.hostDetail",
              { count },
            )}
          />
          <Badge variant={sealedWith === "tpm2" ? "ok" : "warn"} className="shrink-0">
            {t(sealedWith === "tpm2" ? "credentials.badge.tpm" : "credentials.badge.host")}
          </Badge>
        </Row>
      </Group>
      <GroupCaption>
        {t("credentials.seal.caption")}{" "}
        <HelpLink entry="credentials" label={t("credentials.seal.more")} />
      </GroupCaption>
    </PaneSection>
  );
}

// ── One group ───────────────────────────────────────────────────────────

/* Reads the language on screen, so a switch of language re-renders it. */
function useDay(): (seconds: number) => string {
  useLocale();
  return (seconds) =>
    new Intl.DateTimeFormat(intlTag(), { dateStyle: "medium" }).format(new Date(seconds * 1000));
}

function AppSection({
  app,
  list,
  data,
  locked,
}: {
  app: string;
  list: Credential[];
  data: CredentialsData;
  locked: boolean;
}) {
  const t = useT();
  const day = useDay();
  const [asking, setAsking] = React.useState<Credential | null>(null);
  const titleId = React.useId();
  const bodyId = React.useId();

  return (
    <PaneSection data-testid="credentials-app" data-app={app}>
      <GroupTitle>{appTitle(app)}</GroupTitle>
      <Group>
        {list.map((c, i) => (
          <StackRow
            key={c.name}
            last={i === list.length - 1}
            data-testid="credential"
            data-name={c.name}
            className="gap-1.5"
          >
            <div className="flex items-start gap-3">
              <div className="min-w-0 flex-1">
                <p className="text-sm leading-snug text-ink">{credentialTitle(c)}</p>
                <p className="text-[12.5px] leading-snug text-muted">{credentialDetail(c)}</p>
              </div>
              <Action credential={c} data={data} locked={locked} onAsk={setAsking} />
            </div>
            <div className="flex flex-wrap items-center gap-1.5 text-[12.5px] leading-snug text-muted">
              {c.set ? (
                <Badge variant={c.source === "box" ? "local" : "neutral"}>
                  {t(c.source === "box" ? "credentials.badge.box" : "credentials.badge.owner")}
                </Badge>
              ) : (
                <Badge variant={c.source === "box" ? "crit" : "neutral"}>
                  {t("credentials.badge.notSet")}
                </Badge>
              )}
              {c.adopted && <Badge variant="ok">{t("credentials.badge.adopted")}</Badge>}
              {c.remade && <Badge variant="warn">{t("credentials.badge.remade")}</Badge>}
              {c.set && c.updated !== null && (
                <span>{t("credentials.changed", { date: day(c.updated) })}</span>
              )}
              {c.services.length > 0 && (
                <span className="truncate">
                  {t("credentials.readBy", { units: c.services.join(", ") })}
                </span>
              )}
            </div>
          </StackRow>
        ))}
      </Group>

      <Dialog
        open={asking !== null}
        onOpenChange={(open) => {
          if (!open) setAsking(null);
        }}
        labelledBy={titleId}
        describedBy={bodyId}
      >
        <DialogHeader>
          <DialogTitle id={titleId}>
            {t("credentials.rotate.title", { name: asking === null ? "" : credentialTitle(asking) })}
          </DialogTitle>
          <DialogDescription id={bodyId}>
            {asking === null ? "" : credentialRotateEffect(asking)}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setAsking(null)}>
            {t("credentials.rotate.keep")}
          </Button>
          <Button
            onClick={() => {
              const c = asking;
              setAsking(null);
              if (c !== null) void data.rotate(c);
            }}
          >
            {t("credentials.rotate.confirm")}
          </Button>
        </DialogFooter>
      </Dialog>
    </PaneSection>
  );
}

/* What a row offers: a new value for one the box makes and can change, the
 * pane that asks for one the owner types, or nothing. */
function Action({
  credential: c,
  data,
  locked,
  onAsk,
}: {
  credential: Credential;
  data: CredentialsData;
  locked: boolean;
  onAsk: (c: Credential) => void;
}) {
  const t = useT();
  if (c.rotate) {
    const busy = data.rotating === c.name;
    return (
      <Button
        size="sm"
        variant="secondary"
        className="shrink-0"
        disabled={locked || data.rotating !== null}
        onClick={() => onAsk(c)}
      >
        {busy && <Spinner size={14} />}
        {t("credentials.rotate.button")}
      </Button>
    );
  }
  if (c.source === "owner" && c.pane !== null) {
    return (
      <Link to={c.pane} className={buttonVariants({ variant: "secondary", size: "sm", className: "shrink-0" })}>
        {t(c.set ? "credentials.change" : "credentials.setUp")}
      </Link>
    );
  }
  return null;
}
