import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge, type BadgeProps } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  getConfig,
  isAbort,
  isUnauthorized,
  postConfigSync,
  type ConfigResponse,
  type ConfigSyncState,
} from "@/lib/api";
import { intlTag } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow } from "./rows";

/* The configuration repository: where it is on LosOS Git, how the last sync
 * went, and the commits lososd made.
 *
 * Every Apply, storage change and factory reset is one commit in /etc/nixos
 * (backend/src/losos.rs, commit_settings), and lososd's reconciler pushes
 * the branch to a private repository on LosOS Git owned by the admin's
 * account, then takes a push made there (a clone, edited and pushed) and
 * rebuilds from it after the same per-line check Apply gets. This pane is
 * the read of that: GET /api/config, and POST /api/config/sync for a sync
 * now rather than at the reconciler's next tick. No draft and no Apply bar.
 *
 * The clone address shown is the box's own origin plus Forgejo's path, not
 * the loopback URL lososd pushes to: the owner clones from where they are.
 */

export function HistoryPane({ locked }: { locked: boolean }) {
  const t = useT();
  const locale = useLocale();
  const [config, setConfig] = React.useState<ConfigResponse | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [syncing, setSyncing] = React.useState(false);

  const load = React.useCallback(async (signal?: AbortSignal) => {
    try {
      setConfig(normalize(await getConfig(signal === undefined ? {} : { signal })));
      setError(null);
    } catch (fault) {
      if (isAbort(fault) || isUnauthorized(fault)) return;
      setError(fault instanceof Error && fault.message.length > 0 ? fault.message : "?");
    }
  }, []);

  React.useEffect(() => {
    if (locked) {
      setConfig(null);
      setError(null);
      return;
    }
    const controller = new AbortController();
    void load(controller.signal);
    return () => controller.abort();
  }, [locked, load]);

  const syncNow = async () => {
    setSyncing(true);
    try {
      setConfig(normalize(await postConfigSync()));
      setError(null);
    } catch (fault) {
      if (!isUnauthorized(fault)) {
        setError(fault instanceof Error && fault.message.length > 0 ? fault.message : "?");
      }
    } finally {
      setSyncing(false);
    }
  };

  if (error !== null) {
    return (
      <>
        <Alert variant="crit">
          <HugeiconsIcon icon={Alert02Icon} size={17} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertDescription>{t("history.loadError", { message: error })}</AlertDescription>
        </Alert>
        <div className="mt-3">
          <Button variant="secondary" size="sm" onClick={() => void load()}>
            {t("history.refresh")}
          </Button>
        </div>
      </>
    );
  }

  if (config === null) {
    return (
      <PaneSection>
        <Group>
          {[0, 1, 2, 3].map((row) => (
            <Row key={row} last={row === 3}>
              <Skeleton className="h-4 w-48" />
              <Skeleton className="h-4 w-28" />
            </Row>
          ))}
        </Group>
      </PaneSection>
    );
  }

  const repo = config.repository;
  const cloneAddress = repo === null ? null : `${browserOrigin()}${repo.url}.git`;
  const when = (iso: string): string => formatWhen(iso, locale);

  return (
    <>
      <PaneSection>
        <GroupTitle>{t("history.repo.title")}</GroupTitle>
        <Group>
          {repo !== null && (
            <Row>
              <RowText title={t("history.repo.onGit")} detail={<span className="numeric">{repo.owner}/{repo.name}</span>} />
              <a
                href={repo.url}
                target="_blank"
                rel="noopener"
                className="shrink-0 text-[13px] text-accent underline-offset-4 hover:underline"
                data-testid="history-repo-link"
              >
                {t("history.repo.open")}
              </a>
            </Row>
          )}
          {config.head !== null && (
            <Row>
              <RowText title={t("history.repo.branch")} />
              <RowValue className="text-ink">{config.head.branch ?? "—"}</RowValue>
            </Row>
          )}
          <Row last={cloneAddress === null}>
            <RowText title={t("history.repo.head")} />
            <RowValue className="text-ink">{config.head === null ? "—" : short(config.head.sha)}</RowValue>
          </Row>
          {cloneAddress !== null && (
            <StackRow last>
              <p className="text-sm leading-snug text-ink">{t("history.repo.clone")}</p>
              <code className="numeric block rounded-control bg-sunk px-2.5 py-1.5 text-[12.5px] break-all select-all">
                {cloneAddress}
              </code>
            </StackRow>
          )}
        </Group>
        <GroupCaption>{repo === null ? t("history.repo.off") : t("history.repo.caption")}</GroupCaption>
      </PaneSection>

      {repo !== null && (
        <PaneSection>
          <GroupTitle>{t("history.sync.title")}</GroupTitle>
          <Group>
            <Row last>
              <RowText
                title={
                  <span className="flex flex-wrap items-center gap-2">
                    <Badge variant={syncTone(config.sync.state)} data-testid="history-sync-state">
                      {t(`history.sync.state.${config.sync.state}`)}
                    </Badge>
                    {config.sync.syncedAt !== null && (
                      <span className="text-[12.5px] text-muted">
                        {t("history.sync.lastAt", { when: when(config.sync.syncedAt) })}
                      </span>
                    )}
                  </span>
                }
                detail={
                  <>
                    {config.sync.detail}
                    {config.sync.remoteHead !== null && (
                      <span className="numeric block text-faint">
                        {t("history.sync.remote", { sha: short(config.sync.remoteHead) })}
                      </span>
                    )}
                  </>
                }
              />
              <Button variant="secondary" size="sm" disabled={locked || syncing} onClick={() => void syncNow()}>
                {syncing ? t("history.sync.syncing") : t("history.sync.now")}
              </Button>
            </Row>
          </Group>
        </PaneSection>
      )}

      <PaneSection>
        <GroupTitle>{t("history.commits.title")}</GroupTitle>
        <Group>
          {config.log.length === 0 ? (
            <Row last>
              <RowText title={<span className="text-muted">{t("history.commits.empty")}</span>} />
            </Row>
          ) : (
            config.log.map((entry, index) => (
              <Row key={entry.sha} last={index === config.log.length - 1} data-commit={entry.sha}>
                <RowText title={entry.subject} detail={when(entry.when)} />
                <RowValue>{short(entry.sha)}</RowValue>
              </Row>
            ))
          )}
        </Group>
        <GroupCaption>{t("history.commits.caption")}</GroupCaption>
      </PaneSection>
    </>
  );
}

/* The document with every field present, whatever the box sent: an older
 * daemon answers the route with an empty object, and a missing field must
 * read as "nothing there" rather than throw mid-render. */
function normalize(config: Partial<ConfigResponse>): ConfigResponse {
  const repository = config.repository ?? null;
  const head = config.head ?? null;
  const sync = config.sync ?? {
    state: "pending",
    detail: "",
    syncedAt: null,
    remoteHead: null,
  };
  return {
    enabled: config.enabled === true && repository !== null,
    repository: repository !== null && typeof repository.url === "string" ? repository : null,
    head: head !== null && typeof head.sha === "string" ? { sha: head.sha, branch: head.branch ?? null } : null,
    log: Array.isArray(config.log) ? config.log : [],
    sync: {
      state: SYNC_STATES.has(sync.state) ? sync.state : "pending",
      detail: typeof sync.detail === "string" ? sync.detail : "",
      syncedAt: sync.syncedAt ?? null,
      remoteHead: sync.remoteHead ?? null,
    },
  };
}

const SYNC_STATES: ReadonlySet<string> = new Set([
  "off",
  "pending",
  "ok",
  "waiting",
  "refused",
  "diverged",
  "unavailable",
  "error",
]);

function short(sha: string): string {
  return sha.slice(0, 10);
}

function syncTone(state: ConfigSyncState): BadgeProps["variant"] {
  switch (state) {
    case "ok":
      return "ok";
    case "waiting":
    case "pending":
    case "off":
      return "neutral";
    case "unavailable":
    case "diverged":
      return "warn";
    case "refused":
    case "error":
      return "crit";
    default: {
      const exhaustive: never = state;
      return exhaustive;
    }
  }
}

function formatWhen(iso: string, _locale: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return new Intl.DateTimeFormat(intlTag(), { dateStyle: "medium", timeStyle: "short" }).format(date);
}

function browserOrigin(): string {
  return typeof window === "undefined" ? "" : window.location.origin;
}
