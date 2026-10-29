import * as React from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Alert02Icon,
  ArrowUp01Icon,
  ComputerTerminal01Icon,
  Download04Icon,
  PauseIcon,
  PlayIcon,
  Search01Icon,
} from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge, StatusDot } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { MESSAGES } from "@/i18n";
import {
  downloadLogs,
  getLogs,
  isAbort,
  type LogFilter,
  type LogLine,
  type LogSpan,
} from "@/lib/api";
import { t as translate, type MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

/* Settings › Logs: what the box's services wrote to the journal, so an owner
 * sees why something failed without signing in to the box.
 *
 * The lines come from the log reader (backend/src/logs/), not lososd: it
 * runs on its own and keeps answering while lososd is down, and it hides
 * passwords, keys and tokens before a line leaves the journal. The page
 * without JavaScript, /basic/settings/logs, is drawn by that reader from
 * the same markup (backend/plugins/page/src/logs.rs); keep the two alike.
 *
 * Follow asks for what came after the newest line every few seconds and
 * keeps at most MAX_LINES on screen, dropping the oldest. */

const PRIORITIES = [3, 4, 5, 6, 7] as const;
const SPANS: readonly LogSpan[] = ["boot", "previous", "hour", "day", "week"];
const FOLLOW_MS = 3000;
const MAX_LINES = 2000;

const DEFAULT_FILTER: LogFilter = { unit: "lososd", priority: 6, span: "boot", text: "" };

function filterFrom(search: string): LogFilter {
  const q = new URLSearchParams(search);
  const priority = Number(q.get("priority"));
  const span = q.get("span");
  return {
    unit: q.get("unit") || DEFAULT_FILTER.unit,
    priority: Number.isInteger(priority) && priority >= 0 && priority <= 7 && q.has("priority") ? priority : DEFAULT_FILTER.priority,
    span: SPANS.includes(span as LogSpan) ? (span as LogSpan) : DEFAULT_FILTER.span,
    text: q.get("q") ?? "",
  };
}

function searchOf(filter: LogFilter): string {
  const q = new URLSearchParams({ unit: filter.unit, priority: String(filter.priority), span: filter.span });
  if (filter.text.trim() !== "") q.set("q", filter.text.trim());
  return `?${q.toString()}`;
}

/** A source's name on the list, or its id for one this page has no words for. */
function sourceLabel(id: string): string {
  const key = `logs.source.${id}`;
  return key in MESSAGES ? translate(key as MessageKey) : id;
}

function levelVariant(priority: number): "crit" | "warn" | "local" | "neutral" | "outline" {
  if (priority <= 3) return "crit";
  if (priority === 4) return "warn";
  if (priority === 5) return "local";
  if (priority === 6) return "neutral";
  return "outline";
}

type View =
  | { kind: "loading" }
  | { kind: "ready"; sources: string[]; lines: LogLine[]; older: string | null; newer: string | null; error: string | null }
  | { kind: "failed"; message: string };

export function LogsPane({ locked }: { locked: boolean }) {
  const t = useT();
  const location = useLocation();
  const navigate = useNavigate();
  const filter = React.useMemo(() => filterFrom(location.search), [location.search]);
  const [draft, setDraft] = React.useState<LogFilter>(filter);
  const [follow, setFollow] = React.useState(false);
  const [view, setView] = React.useState<View>({ kind: "loading" });
  const [loadingOlder, setLoadingOlder] = React.useState(false);
  const [downloadFailed, setDownloadFailed] = React.useState(false);
  const [attempt, setAttempt] = React.useState(0);
  // One more for every answer while following, so the next ask is set up
  // even when nothing new came.
  const [tick, setTick] = React.useState(0);
  const listRef = React.useRef<HTMLDivElement>(null);
  // The newest lines are at the bottom: a fresh answer, and the start of
  // following, open the list there. Older lines brought in above leave it.
  const openedAt = React.useRef("");
  React.useLayoutEffect(() => {
    if (view.kind !== "ready") return;
    const key = `${location.search}#${attempt}#${follow}`;
    if (openedAt.current === key) return;
    openedAt.current = key;
    const el = listRef.current;
    if (el !== null) el.scrollTop = el.scrollHeight;
  }, [view.kind, location.search, attempt, follow]);

  React.useEffect(() => setDraft(filter), [filter]);

  // The newest lines for the filter on the address.
  React.useEffect(() => {
    if (locked) return;
    const abort = new AbortController();
    setView({ kind: "loading" });
    getLogs(filter, {}, { signal: abort.signal })
      .then((r) =>
        setView({
          kind: "ready",
          sources: Array.isArray(r.sources) ? r.sources : [],
          lines: Array.isArray(r.lines) ? r.lines : [],
          older: r.older ?? null,
          newer: r.newer ?? null,
          error: r.error ?? null,
        }),
      )
      .catch((e: unknown) => {
        if (isAbort(e)) return;
        setView({ kind: "failed", message: e instanceof Error ? e.message : String(e) });
      });
    return () => abort.abort();
  }, [filter, locked, attempt]);

  // Following: what came after the newest line, every few seconds.
  const newer = view.kind === "ready" ? view.newer : null;
  React.useEffect(() => {
    if (!follow || locked || view.kind !== "ready") return;
    const abort = new AbortController();
    const timer = window.setTimeout(() => {
      getLogs(filter, newer === null ? {} : { after: newer }, { signal: abort.signal })
        .then((r) => {
          const box = listRef.current;
          const atBottom = box === null || box.scrollHeight - box.scrollTop - box.clientHeight < 48;
          setView((v) => {
            if (v.kind !== "ready") return v;
            const added = Array.isArray(r.lines) ? r.lines : [];
            const lines = newer === null ? added : [...v.lines, ...added];
            const cut = Math.max(0, lines.length - MAX_LINES);
            return {
              ...v,
              lines: cut > 0 ? lines.slice(cut) : lines,
              older: cut > 0 ? lines[cut]!.cursor : v.older,
              newer: r.newer ?? v.newer,
              error: r.error ?? null,
            };
          });
          if (atBottom) {
            window.requestAnimationFrame(() => {
              const el = listRef.current;
              if (el !== null) el.scrollTop = el.scrollHeight;
            });
          }
        })
        .catch(() => {
          /* the next tick asks again */
        })
        .finally(() => {
          if (!abort.signal.aborted) setTick((n) => n + 1);
        });
    }, FOLLOW_MS);
    return () => {
      window.clearTimeout(timer);
      abort.abort();
    };
  }, [follow, locked, filter, newer, view.kind, tick]);

  const show = (next: LogFilter) => {
    const search = searchOf(next);
    if (search === location.search) setAttempt((n) => n + 1);
    else navigate(`/settings/logs${search}`, { replace: true });
  };

  const older = async () => {
    if (view.kind !== "ready" || view.older === null) return;
    setLoadingOlder(true);
    try {
      const r = await getLogs(filter, { before: view.older });
      const added = Array.isArray(r.lines) ? r.lines : [];
      setView((v) => (v.kind === "ready" ? { ...v, lines: [...added, ...v.lines], older: r.older ?? null } : v));
    } catch {
      /* the button stays for another try */
    } finally {
      setLoadingOlder(false);
    }
  };

  const download = async () => {
    setDownloadFailed(false);
    try {
      const { blob, name } = await downloadLogs(filter);
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = name;
      document.body.append(a);
      a.click();
      a.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch {
      setDownloadFailed(true);
    }
  };

  // The list stays whole while another filter loads, so a second choice
  // made meanwhile still finds its entry.
  const known = React.useRef<string[]>([]);
  if (view.kind === "ready" && view.sources.length > 0) known.current = view.sources;
  const sources = view.kind === "ready" ? view.sources : known.current.length > 0 ? known.current : [filter.unit];
  const options = sources.includes(draft.unit) ? sources : [...sources, draft.unit];

  return (
    <div data-testid="logs-pane">
      <form
        role="search"
        data-slot="log-filters"
        className="mb-4 grid gap-3 rounded-card border border-line bg-surface p-3.5 shadow-card sm:grid-cols-2 lg:grid-cols-[minmax(0,1.4fr)_minmax(0,1.25fr)_minmax(0,1fr)_minmax(0,0.85fr)]"
        onSubmit={(e) => {
          e.preventDefault();
          show(draft);
        }}
      >
        <Field className="min-w-0">
          <FieldLabel htmlFor="logs-unit">{t("logs.filter.source")}</FieldLabel>
          <NativeSelect
            id="logs-unit"
            className="w-full"
            value={draft.unit}
            disabled={locked}
            onChange={(e) => show({ ...draft, unit: e.target.value })}
          >
            {options.map((id) => (
              <NativeSelectOption key={id} value={id}>
                {sourceLabel(id)}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>
        <Field className="min-w-0">
          <FieldLabel htmlFor="logs-priority">{t("logs.filter.priority")}</FieldLabel>
          <NativeSelect
            id="logs-priority"
            className="w-full"
            value={String(draft.priority)}
            disabled={locked}
            onChange={(e) => show({ ...draft, priority: Number(e.target.value) })}
          >
            {PRIORITIES.map((n) => (
              <NativeSelectOption key={n} value={String(n)}>
                {t(`logs.priority.${n}`)}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>
        <Field className="min-w-0">
          <FieldLabel htmlFor="logs-span">{t("logs.filter.span")}</FieldLabel>
          <NativeSelect
            id="logs-span"
            className="w-full"
            value={draft.span}
            disabled={locked}
            onChange={(e) => show({ ...draft, span: e.target.value as LogSpan })}
          >
            {SPANS.map((s) => (
              <NativeSelectOption key={s} value={s}>
                {t(`logs.span.${s}`)}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>
        <Field className="min-w-0">
          <FieldLabel htmlFor="logs-q">{t("logs.filter.text")}</FieldLabel>
          <Input
            id="logs-q"
            type="search"
            autoComplete="off"
            className="w-full"
            placeholder={t("logs.filter.textPlaceholder")}
            value={draft.text}
            disabled={locked}
            onChange={(e) => setDraft({ ...draft, text: e.target.value })}
          />
        </Field>
        <div className="flex flex-wrap items-center gap-2 sm:col-span-full">
          <Button type="submit" disabled={locked}>
            <Glyph icon={Search01Icon} />
            <span>{t("logs.filter.show")}</span>
          </Button>
          <Button
            type="button"
            variant="secondary"
            aria-pressed={follow}
            disabled={locked}
            onClick={() => setFollow((f) => !f)}
          >
            <Glyph icon={follow ? PauseIcon : PlayIcon} />
            <span>{t(follow ? "logs.stop" : "logs.follow")}</span>
          </Button>
          <div className="flex-1" />
          <Button type="button" variant="ghost" disabled={locked} onClick={() => void download()}>
            <Glyph icon={Download04Icon} />
            <span>{t("logs.download")}</span>
          </Button>
        </div>
      </form>

      {downloadFailed && <ErrorAlert text={t("logs.downloadFailed")} />}
      {view.kind === "ready" && view.error !== null && (
        <ErrorAlert text={t("logs.error", { message: view.error })} />
      )}

      {view.kind === "loading" && (
        <div className="flex items-center gap-2 rounded-card border border-line bg-surface px-3.5 py-3 text-[13px] text-muted shadow-card">
          <Spinner size={16} />
          <span>{t("logs.loading")}</span>
        </div>
      )}
      {view.kind === "failed" && (
        <Alert variant="crit" className="mb-4">
          <HugeiconsIcon icon={Alert02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertDescription>
            <p className="text-ink">{t("logs.error", { message: view.message })}</p>
            <p>
              <Button size="sm" variant="secondary" onClick={() => setAttempt((n) => n + 1)}>
                {t("logs.retry")}
              </Button>
            </p>
          </AlertDescription>
        </Alert>
      )}
      {view.kind === "ready" &&
        (view.lines.length === 0 && view.error === null ? (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={ComputerTerminal01Icon} size={24} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>{t("logs.empty.title")}</EmptyTitle>
              <EmptyDescription>{t("logs.empty.body")}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <div
            data-slot="log-view"
            ref={listRef}
            className="max-h-[70dvh] overflow-auto rounded-card border border-line bg-surface shadow-card animate-rise"
          >
            {view.older !== null && (
              <div className="flex justify-center border-b border-hair px-3.5 py-1.5">
                <Button size="sm" variant="ghost" disabled={loadingOlder} onClick={() => void older()}>
                  {loadingOlder ? <Spinner size={16} /> : <Glyph icon={ArrowUp01Icon} />}
                  <span>{t("logs.older")}</span>
                </Button>
              </div>
            )}
            <Lines lines={view.lines} />
            {follow && (
              <p
                role="status"
                data-slot="log-following"
                className="flex items-center gap-2 border-t border-hair px-3.5 py-2 text-[12.5px] text-muted"
              >
                <StatusDot state="pending" />
                <span>{t("logs.following")}</span>
              </p>
            )}
          </div>
        ))}
      <p className="mt-2 text-[12.5px] leading-snug text-muted">{t("logs.hidden")}</p>
    </div>
  );
}

function Lines({ lines }: { lines: LogLine[] }) {
  const t = useT();
  const rows: React.ReactNode[] = [];
  let day = "";
  for (const line of lines) {
    if (line.date !== day) {
      day = line.date;
      rows.push(
        <li
          key={`day-${line.cursor}`}
          data-slot="log-day"
          className="sticky top-0 z-[1] border-b border-hair bg-sunk/90 px-3.5 py-1 font-sans text-[12px] font-medium text-muted backdrop-blur-sm"
        >
          {day}
        </li>,
      );
    }
    const who = line.pid === null ? line.unit : `${line.unit}[${line.pid}]`;
    rows.push(
      <li
        key={line.cursor}
        data-slot="log-line"
        data-priority={line.priority}
        className="grid grid-cols-[auto_auto_minmax(0,1fr)] items-baseline gap-x-3 gap-y-0.5 border-b border-hair px-3.5 py-1.5 last:border-b-0 sm:grid-cols-[4.5rem_5.5rem_minmax(0,10rem)_minmax(0,1fr)]"
      >
        <time dateTime={line.stamp} title={`${line.date} ${line.time}`} className="text-faint tabular-nums">
          {line.time}
        </time>
        <span className="leading-none">
          <Badge variant={levelVariant(line.priority)}>
            {t(`logs.level.${Math.min(7, Math.max(0, line.priority)) as 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7}`)}
          </Badge>
        </span>
        <span title={who} className="truncate text-muted">
          {line.unit.replace(/\.service$/, "")}
        </span>
        <span
          className={cn(
            "col-span-full min-w-0 break-words whitespace-pre-wrap sm:col-span-1",
            line.priority >= 7 ? "text-muted" : "text-ink",
          )}
        >
          {line.message}
        </span>
      </li>,
    );
  }
  return (
    <ol data-slot="log-lines" aria-label={t("settings.panes.logs.label")} className="font-mono text-[12.5px] leading-relaxed">
      {rows}
    </ol>
  );
}

function ErrorAlert({ text }: { text: string }) {
  return (
    <Alert variant="crit" className="mb-4">
      <HugeiconsIcon icon={Alert02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <AlertDescription>
        <p className="text-ink">{text}</p>
      </AlertDescription>
    </Alert>
  );
}

function Glyph({ icon }: { icon: IconSvgElement }) {
  return <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />;
}
