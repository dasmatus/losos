/* Mesh CI: a screen of the admin UI at /ci, over lososd's /api/ci (the `ci`
 * plugin, which relays to losos-ci, backend/src/ci/). It exists only on the
 * shared side, like Mail.
 *
 * /ci lists the runs of every repository the person may read, newest
 * first, under a card for this box's runner. /ci/runs/<id> is one run: its
 * jobs on the left, the chosen job's log on the right (one at a time on a
 * phone). LosOS Git links a commit's checks to /ci/runs/<id>#job-<job>, so
 * the hash picks the job.
 *
 * The log is the runner's text with GitHub's markers: each step is a
 * `##[group]` that ends in `##[result] <outcome> <secs>s`, a step's own
 * `::group::` is a group inside it, and `##[error]`, `##[warning]`,
 * `##[notice]` and `##[command]` mark lines. Each step is a <details>, open
 * when it failed or is running. */

import * as React from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  ArrowLeft01Icon,
  CancelCircleIcon,
  CheckmarkCircle02Icon,
  Clock01Icon,
  GitBranchIcon,
  GitCommitIcon,
  Loading03Icon,
  MinusSignCircleIcon,
  PlayIcon,
  RepeatIcon,
  StopCircleIcon,
  WorkflowSquare01Icon,
} from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge, type BadgeProps } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { PaneHeader } from "@/screens/settings/rows";
import {
  ApiError,
  cancelCiRun,
  dispatchCi,
  getCi,
  getCiLog,
  getCiRun,
  getCiRuns,
  getCiWorkflows,
  getGitRepos,
  rerunCiRun,
  setCiLend,
  type CiJob,
  type CiJobSummary,
  type CiLend,
  type CiOverview,
  type CiRun,
  type CiRunDetail,
  type CiState,
  type CiWorkflows,
} from "@/lib/api";
import type { MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

/* How often a run that is not over, and its open log, are read again. */
const RUN_REFRESH_MS = 3_000;
const LOG_REFRESH_MS = 2_000;
/* The list, while anything in it is queued or running. */
const LIST_REFRESH_MS = 5_000;

function over(state: CiState): boolean {
  return state !== "queued" && state !== "running";
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

// ── states ─────────────────────────────────────────────────────────────

const STATE_ICON: Record<CiState, IconSvgElement> = {
  queued: Clock01Icon,
  running: Loading03Icon,
  success: CheckmarkCircle02Icon,
  failure: CancelCircleIcon,
  error: CancelCircleIcon,
  cancelled: StopCircleIcon,
  skipped: MinusSignCircleIcon,
};

const STATE_TONE: Record<CiState, string> = {
  queued: "text-muted",
  running: "text-accent",
  success: "text-ok",
  failure: "text-crit",
  error: "text-crit",
  cancelled: "text-muted",
  skipped: "text-muted",
};

const STATE_BADGE: Record<CiState, BadgeProps["variant"]> = {
  queued: "neutral",
  running: "local",
  success: "ok",
  failure: "crit",
  error: "crit",
  cancelled: "neutral",
  skipped: "neutral",
};

function StateIcon({ state, size = 18, className }: { state: CiState; size?: number; className?: string }) {
  return (
    <HugeiconsIcon
      icon={STATE_ICON[state]}
      size={size}
      strokeWidth={1.5}
      color="currentColor"
      aria-hidden="true"
      className={cn("shrink-0", STATE_TONE[state], state === "running" && "animate-spin", className)}
    />
  );
}

function StateBadge({ state }: { state: CiState }) {
  const t = useT();
  return <Badge variant={STATE_BADGE[state]}>{t(`ci.state.${state}` as MessageKey)}</Badge>;
}

function eventName(event: string, t: ReturnType<typeof useT>): string {
  if (event === "push" || event === "pull_request" || event === "workflow_dispatch") {
    return t(`ci.event.${event}` as MessageKey);
  }
  return event;
}

function refName(ref: string): string {
  return ref.replace(/^refs\/(heads|tags)\//, "").replace(/^refs\/pull\/(\d+)\/merge$/, "#$1");
}

function duration(seconds: number, t: ReturnType<typeof useT>): string {
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return t("ci.time.seconds", { count: s });
  return t("ci.time.minutes", { minutes: Math.floor(s / 60), seconds: s % 60 });
}

function ago(unix: number, locale: string): string {
  const diff = unix - Date.now() / 1000;
  const format = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  const abs = Math.abs(diff);
  if (abs < 60) return format.format(Math.round(diff), "second");
  if (abs < 3600) return format.format(Math.round(diff / 60), "minute");
  if (abs < 86400) return format.format(Math.round(diff / 3600), "hour");
  return format.format(Math.round(diff / 86400), "day");
}

function repoHref(owner: string, repo: string): string {
  return `/git/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`;
}

// ── the screen ─────────────────────────────────────────────────────────

type Loading =
  | { kind: "loading" }
  | { kind: "ready"; overview: CiOverview }
  | { kind: "missing" }
  | { kind: "failed"; message: string };

export default function CiScreen() {
  const t = useT();
  const { id } = useParams();
  const [state, setState] = React.useState<Loading>({ kind: "loading" });

  const load = React.useCallback(async () => {
    try {
      setState({ kind: "ready", overview: await getCi() });
    } catch (error) {
      if (error instanceof ApiError && error.status === 404) setState({ kind: "missing" });
      else setState({ kind: "failed", message: message(error) });
    }
  }, []);

  React.useEffect(() => {
    void load();
  }, [load]);

  if (state.kind === "loading") {
    return (
      <div className="grid min-h-48 place-items-center text-muted">
        <Spinner />
      </div>
    );
  }
  if (state.kind !== "ready") {
    return (
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <HugeiconsIcon icon={WorkflowSquare01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          </EmptyMedia>
          <EmptyTitle>{t("ci.title")}</EmptyTitle>
          <EmptyDescription>
            {state.kind === "missing" ? t("ci.missing") : t("ci.failed", { detail: state.message })}
          </EmptyDescription>
        </EmptyHeader>
        {state.kind === "failed" && <Button onClick={() => void load()}>{t("ci.retry")}</Button>}
      </Empty>
    );
  }
  const runId = id !== undefined ? Number(id) : null;
  if (runId !== null && Number.isSafeInteger(runId)) return <RunPage id={runId} />;
  return <Overview overview={state.overview} onOverview={(overview) => setState({ kind: "ready", overview })} />;
}

// ── the list ───────────────────────────────────────────────────────────

type ListedRun = CiRun & { jobs: CiJobSummary[] };

function Overview({ overview, onOverview }: { overview: CiOverview; onOverview: (o: CiOverview) => void }) {
  const t = useT();
  const [runs, setRuns] = React.useState<ListedRun[] | null>(null);
  const [more, setMore] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [dispatching, setDispatching] = React.useState(false);

  const refresh = React.useCallback(async (signal?: AbortSignal) => {
    try {
      const page = await getCiRuns({}, signal !== undefined ? { signal } : {});
      setRuns((current) => {
        // Keep the older pages already shown; the first page is replaced.
        const fresh = page.runs;
        if (current === null) return fresh;
        const oldest = fresh.at(-1)?.id ?? Infinity;
        return [...fresh, ...current.filter((r) => r.id < oldest)];
      });
      setMore((current) => current || page.runs.length >= 30);
      setError(null);
    } catch (e) {
      if (!(e instanceof DOMException && e.name === "AbortError")) setError(message(e));
    }
  }, []);

  React.useEffect(() => {
    const controller = new AbortController();
    void refresh(controller.signal);
    return () => controller.abort();
  }, [refresh]);

  const busy = runs?.some((r) => !over(r.state)) === true;
  React.useEffect(() => {
    if (!busy) return;
    const timer = window.setInterval(() => void refresh(), LIST_REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [busy, refresh]);

  const loadMore = async () => {
    const before = runs?.at(-1)?.id;
    if (before === undefined) return;
    try {
      const page = await getCiRuns({ before });
      setRuns((current) => [...(current ?? []), ...page.runs]);
      setMore(page.runs.length >= 30);
    } catch (e) {
      setError(message(e));
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <PaneHeader title={t("ci.title")} summary={t("ci.summary")} />
        </div>
        <Button onClick={() => setDispatching(true)} className="shrink-0">
          <HugeiconsIcon icon={PlayIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("ci.dispatch.open")}
        </Button>
      </div>

      <RunnerCard overview={overview} onOverview={onOverview} />

      <Card>
        <CardHeader>
          <CardTitle>{t("ci.runs.title")}</CardTitle>
        </CardHeader>
        <CardContent className="px-0 pb-0">
          {error !== null && (
            <Alert variant="crit" className="mx-5 mb-4">
              <AlertDescription>{t("ci.failed", { detail: error })}</AlertDescription>
            </Alert>
          )}
          {runs === null ? (
            <div className="grid min-h-32 place-items-center text-muted">
              <Spinner />
            </div>
          ) : runs.length === 0 ? (
            <p className="px-5 pb-5 text-[13px] text-muted">{t("ci.runs.empty")}</p>
          ) : (
            <ul className="divide-y divide-hair border-t border-hair">
              {runs.map((run) => (
                <RunRow key={run.id} run={run} />
              ))}
            </ul>
          )}
          {more && (
            <div className="border-t border-hair px-5 py-3">
              <Button variant="ghost" size="sm" onClick={() => void loadMore()}>
                {t("ci.runs.more")}
              </Button>
            </div>
          )}
        </CardContent>
      </Card>

      <DispatchDialog
        open={dispatching}
        onOpenChange={setDispatching}
        onStarted={() => void refresh()}
      />
    </div>
  );
}

function RunRow({ run }: { run: ListedRun }) {
  const t = useT();
  const locale = useLocale();
  return (
    <li>
      <Link
        to={`/ci/runs/${run.id}`}
        className="flex items-start gap-3 px-5 py-3 transition-colors hover:bg-sunk focus-visible:bg-sunk"
      >
        <StateIcon state={run.state} className="mt-0.5" />
        <span className="min-w-0 flex-1">
          <span className="flex flex-wrap items-baseline gap-x-2">
            <span className="truncate text-[14px] font-medium text-ink">
              {t("ci.run.number", { workflow: run.workflowName, number: run.number })}
            </span>
            <span className="truncate text-[13px] text-muted">{run.title}</span>
          </span>
          <span className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[12.5px] text-muted">
            <span>
              {run.owner}/{run.repo}
            </span>
            <span className="inline-flex items-center gap-1">
              <HugeiconsIcon icon={GitBranchIcon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {refName(run.gitRef)}
            </span>
            <span className="font-mono">{run.sha.slice(0, 7)}</span>
            <span>{t("ci.run.by", { event: eventName(run.event, t), actor: run.actor })}</span>
            <span>{ago(run.created, locale)}</span>
          </span>
          {run.jobs.length > 0 && (
            <span className="mt-1.5 flex flex-wrap gap-1.5">
              {run.jobs.map((job, i) => (
                <span
                  key={`${job.name}-${i}`}
                  className="inline-flex max-w-full items-center gap-1 rounded-full border border-line px-2 py-0.5 text-[12px] text-muted"
                >
                  <StateIcon state={job.state} size={13} />
                  <span className="truncate">{job.name}</span>
                </span>
              ))}
            </span>
          )}
        </span>
        <span className="shrink-0">
          <StateBadge state={run.state} />
        </span>
      </Link>
    </li>
  );
}

// ── this box's runner ──────────────────────────────────────────────────

function RunnerCard({ overview, onOverview }: { overview: CiOverview; onOverview: (o: CiOverview) => void }) {
  const t = useT();
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const change = async (lend: string) => {
    if (lend !== "always" && lend !== "window" && lend !== "never") return;
    if (lend === overview.lend) return;
    setSaving(true);
    setError(null);
    try {
      await setCiLend(lend as CiLend);
      onOverview(await getCi());
      toast.done(t("ci.lend.saved"));
    } catch (e) {
      setError(message(e));
    } finally {
      setSaving(false);
    }
  };

  const lines: { ok: boolean; text: string }[] = [
    { ok: overview.lendingNow, text: overview.lendingNow ? t("ci.status.lending") : t("ci.status.notLending") },
    { ok: overview.mesh, text: overview.mesh ? t("ci.status.mesh") : t("ci.status.noMesh") },
    {
      ok: true,
      text: overview.privateOnMesh ? t("ci.status.privateOnMesh") : t("ci.status.private"),
    },
  ];

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("ci.runner.title")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex flex-col gap-2">
          <span id="ci-lend-label" className="text-[13px] font-medium text-ink">
            {t("ci.runner.lend")}
          </span>
          <ToggleGroup
            aria-labelledby="ci-lend-label"
            value={overview.lend}
            onValueChange={(value) => void change(value)}
            className={cn("flex-wrap", saving && "opacity-60")}
          >
            {(["always", "window", "never"] as const).map((lend) => (
              <ToggleGroupItem key={lend} value={lend} disabled={!overview.canChange || saving}>
                {t(`ci.lend.${lend}`)}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
          {overview.window !== null && (
            <p className="text-[12.5px] text-muted">
              {t("ci.lend.windowHint", { start: overview.window.start, end: overview.window.end })}
            </p>
          )}
          {error !== null && <p className="text-[12.5px] text-crit">{error}</p>}
        </div>
        <ul className="flex flex-col gap-1.5 text-[13px]">
          {lines.map((line) => (
            <li key={line.text} className="flex items-start gap-2">
              <span
                aria-hidden="true"
                className={cn("mt-1.5 size-2 shrink-0 rounded-full", line.ok ? "bg-ok" : "bg-line")}
              />
              <span className="text-muted">{line.text}</span>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  );
}

// ── one run ────────────────────────────────────────────────────────────

function RunPage({ id }: { id: number }) {
  const t = useT();
  const navigate = useNavigate();
  const { hash } = useLocation();
  const [run, setRun] = React.useState<CiRunDetail | null>(null);
  const [missing, setMissing] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);

  const fromHash = /^#job-(\d+)$/.exec(hash);
  const [jobId, setJobId] = React.useState<number | null>(fromHash !== null ? Number(fromHash[1]) : null);
  const [column, setColumn] = React.useState<"jobs" | "log">(fromHash !== null ? "log" : "jobs");

  const refresh = React.useCallback(async () => {
    try {
      setRun(await getCiRun(id));
      setError(null);
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) setMissing(true);
      else setError(message(e));
    }
  }, [id]);

  React.useEffect(() => {
    setRun(null);
    setMissing(false);
    void refresh();
  }, [refresh]);

  const running = run !== null && (!over(run.state) || run.jobs.some((j) => !over(j.state)));
  React.useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => void refresh(), RUN_REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [running, refresh]);

  // Without a job in the address: the first that failed, else the first.
  React.useEffect(() => {
    if (run === null || run.jobs.length === 0) return;
    if (jobId !== null && run.jobs.some((j) => j.id === jobId)) return;
    const failed = run.jobs.find((j) => j.state === "failure" || j.state === "error");
    setJobId((failed ?? run.jobs[0])?.id ?? null);
  }, [run, jobId]);

  if (missing) {
    return (
      <Empty>
        <EmptyHeader>
          <EmptyTitle>{t("ci.run.missing")}</EmptyTitle>
        </EmptyHeader>
        <Button variant="secondary" onClick={() => navigate("/ci")}>
          {t("ci.run.back")}
        </Button>
      </Empty>
    );
  }
  if (run === null) {
    return (
      <div className="grid min-h-48 place-items-center text-muted">
        {error !== null ? <p className="text-[13px] text-crit">{t("ci.failed", { detail: error })}</p> : <Spinner />}
      </div>
    );
  }

  const job = run.jobs.find((j) => j.id === jobId) ?? null;
  const took = run.started !== null && run.finished !== null ? run.finished - run.started : null;

  const act = async (what: "cancel" | "rerun") => {
    setBusy(true);
    try {
      if (what === "cancel") {
        await cancelCiRun(run.id);
        toast.done(t("ci.run.cancelled"));
        await refresh();
      } else {
        const next = await rerunCiRun(run.id);
        const fresh = await getCiRun(next.id);
        toast.done(t("ci.run.started", { number: fresh.number }));
        navigate(`/ci/runs/${next.id}`);
      }
    } catch (e) {
      toast.error(t("ci.failed", { detail: "" }).replace(/:\s*$/, ""), message(e), { help: "mesh-ci" });
    } finally {
      setBusy(false);
    }
  };

  const pick = (j: CiJob) => {
    setJobId(j.id);
    setColumn("log");
    window.history.replaceState(window.history.state, "", `#job-${j.id}`);
  };

  return (
    <div className="flex flex-col gap-4">
      <Link to="/ci" className="inline-flex w-fit items-center gap-1 text-[13px] text-muted hover:text-ink">
        <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        {t("ci.run.back")}
      </Link>

      <div className="flex flex-wrap items-start gap-3">
        <StateIcon state={run.state} size={22} className="mt-0.5" />
        <div className="min-w-0 flex-1">
          <h1 className="text-[19px] leading-tight font-semibold tracking-tight">
            {t("ci.run.number", { workflow: run.workflowName, number: run.number })}
          </h1>
          <p className="mt-1 text-[13px] text-muted">{run.title}</p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <StateBadge state={run.state} />
          {run.canChange && !over(run.state) && (
            <Button variant="secondary" size="sm" disabled={busy} onClick={() => void act("cancel")}>
              <HugeiconsIcon icon={StopCircleIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("ci.run.cancel")}
            </Button>
          )}
          {run.canChange && over(run.state) && (
            <Button variant="secondary" size="sm" disabled={busy} onClick={() => void act("rerun")}>
              <HugeiconsIcon icon={RepeatIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("ci.run.rerun")}
            </Button>
          )}
        </div>
      </div>

      <dl className="flex flex-wrap gap-x-5 gap-y-1.5 text-[12.5px] text-muted">
        <div className="flex items-center gap-1.5">
          <dt className="sr-only">{t("ci.run.repo")}</dt>
          <dd>
            <a href={repoHref(run.owner, run.repo)} className="text-ink hover:underline">
              {run.owner}/{run.repo}
            </a>
          </dd>
        </div>
        <div className="flex items-center gap-1">
          <HugeiconsIcon icon={GitBranchIcon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <dd>{refName(run.gitRef)}</dd>
        </div>
        <div className="flex items-center gap-1">
          <dt className="sr-only">{t("ci.run.commit")}</dt>
          <HugeiconsIcon icon={GitCommitIcon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <dd className="font-mono">{run.sha.slice(0, 7)}</dd>
        </div>
        <dd>{t("ci.run.by", { event: eventName(run.event, t), actor: run.actor })}</dd>
        {took !== null && <dd>{t("ci.run.duration", { time: duration(took, t) })}</dd>}
      </dl>

      {run.message !== "" && (
        <Alert variant={run.state === "error" ? "crit" : "default"}>
          <AlertDescription>{run.message}</AlertDescription>
        </Alert>
      )}

      <Card className="grid min-h-[26rem] overflow-hidden md:grid-cols-[17rem_minmax(0,1fr)]">
        <section
          aria-label={t("ci.run.jobs")}
          className={cn("border-hair md:block md:border-r", column === "log" && "hidden")}
        >
          <h2 className="px-4 pt-4 pb-2 text-[12px] font-medium tracking-wide text-muted uppercase">
            {t("ci.run.jobs")}
          </h2>
          <ul className="flex flex-col gap-0.5 px-2 pb-3">
            {run.jobs.map((j) => (
              <li key={j.id}>
                <button
                  type="button"
                  onClick={() => pick(j)}
                  aria-current={j.id === jobId ? "true" : undefined}
                  className={cn(
                    "flex w-full items-start gap-2 rounded-control px-2 py-2 text-left transition-colors hover:bg-sunk",
                    j.id === jobId && "bg-sunk",
                  )}
                >
                  <StateIcon state={j.state} size={16} className="mt-0.5" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13.5px] text-ink">{j.name}</span>
                    <span className="mt-0.5 flex flex-wrap items-center gap-1.5 text-[12px] text-muted">
                      <JobPlace job={j} />
                    </span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
        <section className={cn("min-w-0 md:block", column === "jobs" && "hidden")}>
          {job === null ? (
            <p className="p-5 text-[13px] text-muted">{t("ci.job.pick")}</p>
          ) : (
            <JobLog key={job.id} job={job} onBack={() => setColumn("jobs")} />
          )}
        </section>
      </Card>
    </div>
  );
}

function JobPlace({ job }: { job: CiJob }) {
  const t = useT();
  return (
    <>
      <Badge variant={job.placement === "mesh" ? "mesh" : "local"} className="px-2 py-0 text-[11.5px]">
        {job.placement === "mesh" ? t("ci.job.onMesh") : t("ci.job.onBox")}
      </Badge>
      <span className="truncate">
        {job.runner !== "" ? t("ci.job.ranOn", { runner: job.runner }) : job.state === "queued" ? t("ci.job.waiting") : ""}
      </span>
    </>
  );
}

// ── a job's log ────────────────────────────────────────────────────────

type LineKind = "text" | "command" | "error" | "warning" | "notice" | "debug" | "group";

interface LogLine {
  kind: LineKind;
  text: string;
  depth: number;
}

interface Step {
  title: string;
  outcome: string | null;
  seconds: number | null;
  skipped: boolean;
  lines: LogLine[];
}

/* The runner's log into steps. Lines before the first step are the job's
 * own set-up. A group inside a step indents what follows until its end. */
export function parseLog(text: string, setUp: string): Step[] {
  const steps: Step[] = [];
  let step: Step | null = null;
  let depth = 0;
  const head: Step = { title: setUp, outcome: null, seconds: null, skipped: false, lines: [] };
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  for (const raw of lines) {
    const line = raw.replace(/\r$/, "");
    const marker = /^##\[(\w+)\](.*)$/.exec(line);
    const tag = marker?.[1] ?? null;
    const rest = marker?.[2] ?? line;
    if (tag === "group") {
      if (step === null) {
        step = { title: rest, outcome: null, seconds: null, skipped: false, lines: [] };
        steps.push(step);
        depth = 0;
      } else {
        step.lines.push({ kind: "group", text: rest, depth });
        depth += 1;
      }
      continue;
    }
    if (tag === "endgroup") {
      if (step !== null && depth > 0) depth -= 1;
      else step = null;
      continue;
    }
    const into = step ?? head;
    if (tag === "result" && step !== null) {
      const m = /^(\S+)\s+(\d+)s$/.exec(rest.trim());
      step.outcome = m?.[1] ?? rest.trim();
      step.seconds = m?.[2] !== undefined ? Number(m[2]) : null;
      continue;
    }
    if (tag === "skipped" && step !== null) {
      step.skipped = true;
      step.outcome = "skipped";
      continue;
    }
    const kind: LineKind =
      tag === "command" || tag === "error" || tag === "warning" || tag === "notice" || tag === "debug" ? tag : "text";
    into.lines.push({ kind, text: kind === "text" ? line : rest, depth: step === null ? 0 : depth });
  }
  return head.lines.length > 0 ? [head, ...steps] : steps;
}

function outcomeState(step: Step, jobState: CiState, last: boolean): CiState {
  switch (step.outcome) {
    case "success":
      return "success";
    case "failure":
      return "failure";
    case "cancelled":
      return "cancelled";
    case "skipped":
      return "skipped";
    case null:
      return last && jobState === "running" ? "running" : jobState === "running" ? "success" : "skipped";
    default:
      return "error";
  }
}

const LINE_TONE: Record<LineKind, string> = {
  text: "text-ink",
  command: "text-accent",
  error: "text-crit",
  warning: "text-warn",
  notice: "text-accent",
  debug: "text-muted",
  group: "text-ink font-medium",
};

function JobLog({ job, onBack }: { job: CiJob; onBack: () => void }) {
  const t = useT();
  const [text, setText] = React.useState("");
  const [state, setState] = React.useState<CiState>(job.state);
  const next = React.useRef(0);

  React.useEffect(() => {
    let stopped = false;
    let timer: number | undefined;
    const read = async () => {
      try {
        const chunk = await getCiLog(job.id, next.current);
        if (stopped) return;
        if (chunk.text !== "") setText((current) => current + chunk.text);
        next.current = chunk.next;
        setState(chunk.state);
        if (!over(chunk.state) || chunk.text !== "") {
          timer = window.setTimeout(() => void read(), chunk.text !== "" && over(chunk.state) ? 0 : LOG_REFRESH_MS);
        }
      } catch {
        if (!stopped) timer = window.setTimeout(() => void read(), LOG_REFRESH_MS * 2);
      }
    };
    void read();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  }, [job.id]);

  const steps = React.useMemo(() => parseLog(text, t("ci.job.setUp")), [text, t]);
  const outputs = Object.entries(job.outputs);

  return (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex items-center gap-2 border-b border-hair px-4 py-3">
        <Button variant="ghost" size="icon-sm" className="md:hidden" onClick={onBack} aria-label={t("ci.run.jobs")}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        <StateIcon state={state} size={16} />
        <h2 className="min-w-0 flex-1 truncate text-[14px] font-medium">{job.name}</h2>
        {job.exitCode !== null && job.exitCode !== 0 && (
          <span className="text-[12px] text-muted">{t("ci.job.exit", { code: job.exitCode })}</span>
        )}
        {job.started !== null && job.finished !== null && (
          <span className="text-[12px] text-muted tabular-nums">{duration(job.finished - job.started, t)}</span>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-auto bg-sunk/50 p-2 font-mono text-[12.5px] leading-[1.55]">
        {steps.length === 0 ? (
          <p className="p-3 font-sans text-[13px] text-muted">{state === "queued" ? t("ci.job.waiting") : t("ci.job.noLog")}</p>
        ) : (
          steps.map((step, i) => {
            const last = i === steps.length - 1;
            const s = outcomeState(step, state, last);
            return (
              <details
                key={`${i}-${step.title}`}
                open={s === "failure" || s === "error" || s === "running" || undefined}
                className="group/step rounded-control"
              >
                <summary className="flex cursor-pointer list-none items-center gap-2 rounded-control px-2 py-1.5 font-sans text-[13px] hover:bg-sunk">
                  <StateIcon state={s} size={15} />
                  <span className="min-w-0 flex-1 truncate text-ink">{step.title}</span>
                  {step.seconds !== null && (
                    <span className="shrink-0 text-[12px] text-muted tabular-nums">{duration(step.seconds, t)}</span>
                  )}
                </summary>
                {step.lines.length > 0 && (
                  <ol className="mb-1 ml-[1.6rem] overflow-x-auto border-l border-hair py-1 pl-3">
                    {step.lines.map((line, n) => (
                      <li
                        key={n}
                        className={cn("whitespace-pre-wrap break-all", LINE_TONE[line.kind])}
                        style={line.depth > 0 ? { paddingLeft: `${line.depth}rem` } : undefined}
                      >
                        {line.kind === "command" ? "$ " : line.kind === "group" ? "▸ " : ""}
                        {line.text}
                      </li>
                    ))}
                  </ol>
                )}
              </details>
            );
          })
        )}
        {outputs.length > 0 && over(state) && (
          <div className="mt-2 rounded-control border border-hair bg-surface p-3 font-sans text-[12.5px]">
            <p className="mb-1 font-medium text-ink">{t("ci.job.outputs")}</p>
            <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-0.5">
              {outputs.map(([k, v]) => (
                <React.Fragment key={k}>
                  <dt className="font-mono text-muted">{k}</dt>
                  <dd className="truncate font-mono text-ink">{typeof v === "string" ? v : JSON.stringify(v)}</dd>
                </React.Fragment>
              ))}
            </dl>
          </div>
        )}
      </div>
    </div>
  );
}

// ── run by hand ────────────────────────────────────────────────────────

function DispatchDialog({
  open,
  onOpenChange,
  onStarted,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onStarted: () => void;
}) {
  const t = useT();
  const navigate = useNavigate();
  const [repos, setRepos] = React.useState<string[]>([]);
  const [repo, setRepo] = React.useState("");
  const [ref, setRef] = React.useState("");
  const [found, setFound] = React.useState<CiWorkflows | null>(null);
  const [workflow, setWorkflow] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!open) return;
    setFound(null);
    setError(null);
    getGitRepos()
      .then((answer) => {
        if (!answer.available) return;
        const names = answer.repos.map((r) => `${r.owner}/${r.name}`);
        setRepos(names);
        setRepo((current) => (current === "" ? (names[0] ?? "") : current));
      })
      .catch(() => setRepos([]));
  }, [open]);

  const split = (): [string, string] | null => {
    const m = /^([\w.-]+)\/([\w.-]+)$/.exec(repo.trim());
    return m !== null && m[1] !== undefined && m[2] !== undefined ? [m[1], m[2]] : null;
  };

  const find = async () => {
    const parts = split();
    if (parts === null) return;
    setBusy(true);
    setError(null);
    try {
      const answer = await getCiWorkflows(parts[0], parts[1], ref.trim());
      setFound(answer);
      setWorkflow(answer.workflows[0]?.file ?? "");
    } catch (e) {
      setFound(null);
      setError(message(e));
    } finally {
      setBusy(false);
    }
  };

  const run = async () => {
    const parts = split();
    if (parts === null || workflow === "") return;
    setBusy(true);
    setError(null);
    try {
      const started = await dispatchCi(parts[0], parts[1], workflow, ref.trim());
      onOpenChange(false);
      onStarted();
      navigate(`/ci/runs/${started.id}`);
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      labelledBy="ci-dispatch-title"
      describedBy="ci-dispatch-text"
      dialogClassName="w-[min(30rem,calc(100vw-2rem))]"
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void (found === null ? find() : run());
        }}
      >
        <DialogHeader>
          <DialogTitle id="ci-dispatch-title">{t("ci.dispatch.title")}</DialogTitle>
          <DialogDescription id="ci-dispatch-text">{t("ci.dispatch.text")}</DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-3">
          <Field>
            <FieldLabel htmlFor="ci-dispatch-repo">{t("ci.dispatch.repo")}</FieldLabel>
            {repos.length > 0 ? (
              <NativeSelect
                id="ci-dispatch-repo"
                value={repo}
                onChange={(event) => {
                  setRepo(event.target.value);
                  setFound(null);
                }}
              >
                {repos.map((name) => (
                  <NativeSelectOption key={name} value={name}>
                    {name}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            ) : (
              <Input
                id="ci-dispatch-repo"
                value={repo}
                placeholder={t("ci.dispatch.repoHint")}
                onChange={(event) => {
                  setRepo(event.target.value);
                  setFound(null);
                }}
              />
            )}
          </Field>
          <Field>
            <FieldLabel htmlFor="ci-dispatch-ref">{t("ci.dispatch.ref")}</FieldLabel>
            <Input
              id="ci-dispatch-ref"
              value={ref}
              placeholder={t("ci.dispatch.refHint")}
              onChange={(event) => {
                setRef(event.target.value);
                setFound(null);
              }}
            />
          </Field>
          {found !== null &&
            (found.workflows.length === 0 ? (
              <p className="text-[13px] text-muted">{t("ci.dispatch.none")}</p>
            ) : (
              <Field>
                <FieldLabel htmlFor="ci-dispatch-workflow">{t("ci.dispatch.workflow")}</FieldLabel>
                <NativeSelect
                  id="ci-dispatch-workflow"
                  value={workflow}
                  onChange={(event) => setWorkflow(event.target.value)}
                >
                  {found.workflows.map((w) => (
                    <NativeSelectOption key={w.file} value={w.file}>
                      {w.name !== "" ? `${w.name} (${w.file})` : w.file}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </Field>
            ))}
          {error !== null && <p className="text-[13px] text-crit">{error}</p>}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
            {t("ci.dispatch.close")}
          </Button>
          {found === null ? (
            <Button type="submit" disabled={busy || split() === null}>
              {busy && <Spinner />}
              {t("ci.dispatch.find")}
            </Button>
          ) : (
            <Button type="submit" disabled={busy || workflow === "" || !found.canRun}>
              {busy && <Spinner />}
              <HugeiconsIcon icon={PlayIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("ci.dispatch.run")}
            </Button>
          )}
        </DialogFooter>
      </form>
    </Dialog>
  );
}
