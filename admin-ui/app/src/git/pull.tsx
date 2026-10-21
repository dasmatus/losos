import * as React from "react";
import { Link, useParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert02Icon,
  CheckmarkCircle02Icon,
  Comment01Icon,
  GitMergeIcon,
  LaptopIcon,
  Cancel01Icon,
} from "@hugeicons/core-free-icons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge, StatusDot, type DotState } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import {
  git,
  GitError,
  type AiReview,
  type Checks,
  type Comment,
  type LineComment,
  type MergeStyle,
  type NewLine,
  type Pull,
  type Repo,
  type Review,
  type ReviewEvent,
  type Reviewer,
} from "./api";
import { DiffFiles, threads, type ReviewHooks } from "./diff";
import { short, when, whenFull } from "./format";
import { CommitTable } from "./history";
import { Markdown } from "./markdown";
import { ErrorAlert, failed, Loading, NotFound, Person, RepoHeader, Sha, SwitchField, WithRepo } from "./parts";
import { PullState } from "./pulls";
import { notFound, useLoad, useMe } from "./store";

/* One pull request: what it is about and who said what (Conversation), its
 * commits, and its changes with the line comments (Files changed). The
 * reviewers, the checks the commit statuses report, and the merge box sit
 * beside the conversation. */

export type PullTab = "conversation" | "commits" | "files";

export function PullView({ tab }: { tab: PullTab }) {
  return <WithRepo>{(repo) => <PullPage repo={repo} tab={tab} />}</WithRepo>;
}

function PullPage({ repo, tab }: { repo: Repo; tab: PullTab }) {
  const { number = "" } = useParams();
  const n = Number(number);
  const o = repo.owner.login;
  const pull = useLoad((signal) => git.pull(o, repo.name, n, signal), [o, repo.name, n]);
  const lines = useLoad((signal) => git.lineComments(o, repo.name, n, signal), [o, repo.name, n]);
  if (!Number.isInteger(n) || n <= 0 || notFound(pull.error)) return <NotFound />;
  if (pull.error !== null) return <ErrorAlert error={pull.error} />;
  if (pull.data === null) return <Loading rows={6} />;
  return <PullBody repo={repo} pr={pull.data} reload={pull.reload} lines={lines.data ?? []} reloadLines={lines.reload} tab={tab} />;
}

function PullBody({
  repo,
  pr,
  reload,
  lines,
  reloadLines,
  tab,
}: {
  repo: Repo;
  pr: Pull;
  reload: () => void;
  lines: readonly LineComment[];
  reloadLines: () => void;
  tab: PullTab;
}) {
  const t = useT();
  const o = repo.owner.login;
  const base = `/${o}/${repo.name}/pulls/${pr.number}`;
  const me = useMe();
  const [pending, setPending] = React.useState<NewLine[]>([]);
  const canComment = me !== null && pr.permission !== "none";

  const hooks: ReviewHooks = {
    threads: threads(lines),
    pending,
    addPending: (c) => setPending((p) => [...p, c]),
    dropPending: (i) => setPending((p) => p.filter((_, j) => j !== i)),
    commentNow: async (c) => {
      try {
        await git.review(o, repo.name, pr.number, { event: "COMMENT", body: "", commit_id: pr.head.sha, comments: [c] });
        reloadLines();
        reload();
      } catch (err) {
        failed(t("git.review.failed"), err);
        throw err;
      }
    },
    reply: async (to, body) => {
      try {
        await git.reply(o, repo.name, pr.number, to, body);
        reloadLines();
      } catch (err) {
        failed(t("git.review.failed"), err);
        throw err;
      }
    },
    resolve: async (id, resolved) => {
      try {
        await git.resolve(o, repo.name, id, resolved);
        reloadLines();
      } catch (err) {
        failed(t("git.review.failed"), err);
        throw err;
      }
    },
    canComment,
  };

  const tabs: { id: PullTab; to: string; label: MessageKey; count: number | null }[] = [
    { id: "conversation", to: base, label: "git.pull.tab.conversation", count: pr.comments },
    { id: "commits", to: `${base}/commits`, label: "git.pull.tab.commits", count: pr.ahead_by },
    { id: "files", to: `${base}/files`, label: "git.pull.tab.files", count: null },
  ];

  return (
    <div className="flex flex-col gap-4">
      <RepoHeader repo={repo} />
      <div className="flex flex-col gap-2">
        <h2 className="text-[19px] leading-tight font-semibold tracking-tight" data-pull-title>
          {pr.title} <span className="font-normal text-muted">#{pr.number}</span>
        </h2>
        <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1.5 text-[13px] text-muted">
          <PullState pull={pr} />
          <Person login={pr.user.login} />
          <span>
            {t(pr.merged ? "git.pull.mergedLine" : "git.pull.wantsLine", { count: pr.ahead_by })}{" "}
            <code className="rounded-sm bg-sunk px-1.5 py-0.5 font-mono text-[12px] text-ink">{pr.base.ref}</code>{" "}
            {t("git.pull.fromLine")}{" "}
            <code className="rounded-sm bg-sunk px-1.5 py-0.5 font-mono text-[12px] text-ink">{pr.head.ref}</code>
          </span>
          {pr.worktree !== null && <WorktreeBadge worktree={pr.worktree} />}
        </div>
      </div>
      <nav className="flex items-center gap-1 border-b border-line" aria-label={t("git.pull.tabs")}>
        {tabs.map((x) => (
          <Link
            key={x.id}
            to={x.to}
            aria-current={tab === x.id ? "page" : undefined}
            className={cn(
              "-mb-px flex items-center gap-1.5 border-b-2 px-3 py-2 text-[13px] font-medium",
              tab === x.id ? "border-accent text-ink" : "border-transparent text-muted hover:text-ink",
            )}
          >
            {t(x.label)}
            {x.count !== null && <Badge variant="neutral" className="px-1.5 py-0 text-[11px]">{x.count}</Badge>}
          </Link>
        ))}
      </nav>
      {tab === "conversation" && <Conversation repo={repo} pr={pr} reload={reload} lines={lines} hooks={hooks} />}
      {tab === "commits" && <PullCommits repo={repo} pr={pr} />}
      {tab === "files" && (
        <FilesTab repo={repo} pr={pr} hooks={hooks} pending={pending} clearPending={() => setPending([])} reload={() => {
          reload();
          reloadLines();
        }} />
      )}
    </div>
  );
}

function WorktreeBadge({ worktree }: { worktree: NonNullable<Pull["worktree"]> }) {
  const t = useT();
  const where = worktree.computer === "" ? worktree.name : t("git.worktree.on", { name: worktree.name, computer: worktree.computer });
  return (
    <Badge variant="outline" title={t("git.worktree.badgeTitle", { owner: worktree.owner.login })} data-worktree-badge>
      <HugeiconsIcon icon={LaptopIcon} size={12} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      {where}
    </Badge>
  );
}

// ── Conversation ────────────────────────────────────────────────────────

type Event =
  | { kind: "comment"; at: string; comment: Comment }
  | { kind: "review"; at: string; review: Review; lines: number }
  | { kind: "merged"; at: string };

function Conversation({
  repo,
  pr,
  reload,
  lines,
  hooks,
}: {
  repo: Repo;
  pr: Pull;
  reload: () => void;
  lines: readonly LineComment[];
  hooks: ReviewHooks;
}) {
  const t = useT();
  const o = repo.owner.login;
  const comments = useLoad((signal) => git.comments(o, repo.name, pr.number, signal), [o, repo.name, pr.number, pr.comments]);
  const reviews = useLoad((signal) => git.reviews(o, repo.name, pr.number, signal), [o, repo.name, pr.number, pr.reviewers]);

  const events: Event[] = [
    ...(comments.data ?? []).map((c): Event => ({ kind: "comment", at: c.created_at, comment: c })),
    ...(reviews.data ?? []).map(
      (r): Event => ({ kind: "review", at: r.submitted_at, review: r, lines: lines.filter((l) => l.review_id === r.id).length }),
    ),
    ...(pr.merged && pr.merged_at !== null ? [{ kind: "merged", at: pr.merged_at } as Event] : []),
  ].sort((a, b) => a.at.localeCompare(b.at));
  const open = hooks.threads.filter((th) => !th.root.resolved).length;

  return (
    <div className="grid gap-6 md:grid-cols-[minmax(0,1fr)_16rem]">
      <div className="flex min-w-0 flex-col gap-4">
        <CommentCard login={pr.user.login} at={pr.created_at} body={pr.body} empty={t("git.pull.noDescription")} />
        {events.map((ev, i) => (
          <EventView key={i} event={ev} pr={pr} repo={repo} />
        ))}
        {open > 0 && (
          <Link to={`/${o}/${repo.name}/pulls/${pr.number}/files`} className="text-[13px] text-accent hover:underline">
            {t("git.pull.openThreads", { count: open })}
          </Link>
        )}
        <ChecksBox repo={repo} pr={pr} />
        <MergeBox repo={repo} pr={pr} reload={reload} />
        {hooks.canComment && (
          <CommentComposer
            onSend={async (body) => {
              await git.comment(o, repo.name, pr.number, body);
              comments.reload();
              reload();
            }}
          />
        )}
      </div>
      <aside className="flex flex-col gap-5">
        <Reviewers repo={repo} pr={pr} reload={reload} />
      </aside>
    </div>
  );
}

function CommentCard({ login, at, body, empty }: { login: string; at: string; body: string; empty?: string }) {
  return (
    <article className="overflow-hidden rounded-card border border-line bg-surface" data-comment>
      <header className="flex flex-wrap items-center gap-2 border-b border-line bg-sunk/60 px-4 py-2 text-[12.5px] text-muted">
        <Person login={login} />
        <span title={whenFull(at)}>{when(at)}</span>
      </header>
      {body.trim() === "" ? (
        <p className="px-4 py-3 text-[13px] text-muted italic">{empty}</p>
      ) : (
        <Markdown source={body} className="px-4 py-3" />
      )}
    </article>
  );
}

const VERDICT: Record<Review["state"], { key: MessageKey; icon: typeof CheckmarkCircle02Icon; tone: string }> = {
  approved: { key: "git.review.event.approved", icon: CheckmarkCircle02Icon, tone: "text-ok" },
  changes_requested: { key: "git.review.event.changes", icon: Alert02Icon, tone: "text-crit" },
  commented: { key: "git.review.event.commented", icon: Comment01Icon, tone: "text-muted" },
};

function EventView({ event, pr, repo }: { event: Event; pr: Pull; repo: Repo }) {
  const t = useT();
  if (event.kind === "comment") {
    return <CommentCard login={event.comment.user.login} at={event.comment.created_at} body={event.comment.body} />;
  }
  if (event.kind === "merged") {
    return (
      <div className="flex flex-wrap items-center gap-2 px-1 text-[13px] text-muted" data-event="merged">
        <HugeiconsIcon icon={GitMergeIcon} size={16} strokeWidth={1.5} color="currentColor" className="text-accent" aria-hidden="true" />
        {pr.merged_by !== null && <Person login={pr.merged_by.login} />}
        <span>{t("git.pull.mergedEvent", { base: pr.base.ref })}</span>
        {pr.merge_commit_sha !== null && <Sha sha={pr.merge_commit_sha} />}
        <span title={whenFull(event.at)}>{when(event.at)}</span>
      </div>
    );
  }
  const { review } = event;
  const v = VERDICT[review.state];
  const files = `/${repo.owner.login}/${repo.name}/pulls/${pr.number}/files`;
  return (
    <div className="flex flex-col gap-2" data-event="review">
      <div className="flex flex-wrap items-center gap-2 px-1 text-[13px] text-muted">
        <HugeiconsIcon icon={v.icon} size={16} strokeWidth={1.5} color="currentColor" className={v.tone} aria-hidden="true" />
        <Person login={review.user.login} />
        <span>{t(v.key)}</span>
        {event.lines > 0 && (
          <Link to={files} className="text-accent hover:underline">
            {t("git.review.lineCount", { count: event.lines })}
          </Link>
        )}
        <Sha sha={review.commit_id} />
        <span title={whenFull(review.submitted_at)}>{when(review.submitted_at)}</span>
      </div>
      {review.body.trim() !== "" && (
        <div className="ml-6 rounded-card border border-line bg-surface">
          <Markdown source={review.body} className="px-4 py-3" />
        </div>
      )}
    </div>
  );
}

function CommentComposer({ onSend }: { onSend: (body: string) => Promise<void> }) {
  const t = useT();
  const [text, setText] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  return (
    <form
      className="flex flex-col gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        if (text.trim() === "" || busy) return;
        setBusy(true);
        onSend(text.trim()).then(
          () => {
            setText("");
            setBusy(false);
          },
          (err: unknown) => {
            setBusy(false);
            failed(t("git.pull.commentFailed"), err);
          },
        );
      }}
    >
      <Textarea rows={4} value={text} onChange={(e) => setText(e.target.value)} aria-label={t("git.pull.comment")} placeholder={t("git.pull.commentPlaceholder")} />
      <div className="flex justify-end">
        <Button type="submit" size="sm" disabled={text.trim() === "" || busy}>
          {t("git.pull.comment")}
        </Button>
      </div>
    </form>
  );
}

// ── Reviewers ───────────────────────────────────────────────────────────

const REVIEWER_STATE: Record<Reviewer["state"], { key: MessageKey; dot: DotState }> = {
  approved: { key: "git.reviewer.approved", dot: "ok" },
  changes_requested: { key: "git.reviewer.changes", dot: "crit" },
  commented: { key: "git.reviewer.commented", dot: "idle" },
  requested: { key: "git.reviewer.waiting", dot: "pending" },
};

function Reviewers({ repo, pr, reload }: { repo: Repo; pr: Pull; reload: () => void }) {
  const t = useT();
  const o = repo.owner.login;
  const writer = pr.permission === "write" || pr.permission === "admin";
  const collaborators = useLoad(
    (signal) => (writer ? git.collaborators(o, repo.name, signal).catch(() => []) : Promise.resolve([])),
    [o, repo.name, writer],
  );
  const ai = useLoad((signal) => git.aiReviewSetting(o, repo.name, signal).catch(() => null), [o, repo.name]);
  const [pick, setPick] = React.useState("");
  const taken = new Set([pr.user.login, ...pr.reviewers.filter((r) => r.requested).map((r) => r.login)]);
  const candidates: { login: string; label: string }[] = [
    { login: o, label: o },
    ...(collaborators.data ?? []).map((c) => ({ login: c.login, label: c.full_name || c.login })),
    ...(ai.data?.enabled === true && ai.data.available ? [{ login: ai.data.reviewer.login, label: ai.data.reviewer.full_name || ai.data.reviewer.login }] : []),
  ].filter((c, i, all) => !taken.has(c.login) && all.findIndex((x) => x.login === c.login) === i);
  const open = pr.state === "open";
  const candidateName = (login: string): string | undefined =>
    login === ai.data?.reviewer.login
      ? ai.data.reviewer.full_name || undefined
      : collaborators.data?.find((c) => c.login === login)?.full_name || undefined;

  return (
    <section className="flex flex-col gap-3" data-reviewers>
      <h3 className="text-[13px] font-semibold">{t("git.reviewer.title")}</h3>
      {pr.reviewers.length === 0 ? (
        <p className="text-[12.5px] text-muted">{t("git.reviewer.none")}</p>
      ) : (
        <ul className="flex flex-col gap-2.5">
          {pr.reviewers.map((r) => (
            <ReviewerRow
              key={r.login}
              repo={repo}
              pr={pr}
              reviewer={r}
              name={candidateName(r.login)}
              canDrop={writer && open && r.requested}
              reload={reload}
            />
          ))}
        </ul>
      )}
      {writer && open && candidates.length > 0 && (
        <form
          className="flex items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            const login = pick !== "" ? pick : (candidates[0]?.login ?? "");
            if (login === "") return;
            git.requestReviewers(o, repo.name, pr.number, [login]).then(
              () => {
                setPick("");
                reload();
              },
              (err: unknown) => failed(t("git.reviewer.askFailed"), err),
            );
          }}
        >
          <NativeSelect aria-label={t("git.reviewer.who")} value={pick !== "" ? pick : (candidates[0]?.login ?? "")} onChange={(e) => setPick(e.target.value)} className="min-w-0 flex-1">
            {candidates.map((c) => (
              <NativeSelectOption key={c.login} value={c.login}>
                {c.label}
              </NativeSelectOption>
            ))}
          </NativeSelect>
          <Button type="submit" size="sm" variant="secondary">
            {t("git.reviewer.ask")}
          </Button>
        </form>
      )}
    </section>
  );
}

function ReviewerRow({
  repo,
  pr,
  reviewer,
  name,
  canDrop,
  reload,
}: {
  repo: Repo;
  pr: Pull;
  reviewer: Reviewer;
  name: string | undefined;
  canDrop: boolean;
  reload: () => void;
}) {
  const t = useT();
  const s = REVIEWER_STATE[reviewer.state];
  const stale = reviewer.commit_id !== undefined && pr.head.sha !== null && reviewer.commit_id !== pr.head.sha;
  return (
    <li className="flex flex-col gap-1" data-reviewer={reviewer.login}>
      <div className="flex items-center gap-2">
        <Person login={reviewer.login} {...(name !== undefined ? { name } : {})} bot={reviewer.is_bot === true} className="min-w-0 flex-1 text-[13px]" />
        {canDrop && (
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={t("git.reviewer.drop", { name: reviewer.login })}
            onClick={() => {
              git.dropReviewer(repo.owner.login, repo.name, pr.number, reviewer.login).then(reload, (err: unknown) => failed(t("git.reviewer.askFailed"), err));
            }}
          >
            <HugeiconsIcon icon={Cancel01Icon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          </Button>
        )}
      </div>
      <p className="flex items-center gap-1.5 pl-8 text-[12px] text-muted">
        <StatusDot state={s.dot} />
        {t(s.key)}
        {stale && reviewer.state !== "requested" && ` · ${t("git.reviewer.earlier", { sha: short(reviewer.commit_id) })}`}
      </p>
      {reviewer.is_bot === true && <BotStatus repo={repo} pr={pr} />}
    </li>
  );
}

/** How far the AI reviewer has got with this pull request. */
function BotStatus({ repo, pr }: { repo: Repo; pr: Pull }) {
  const t = useT();
  const status = useLoad(
    (signal) => git.aiReview(repo.owner.login, repo.name, pr.number, signal).catch(() => null),
    [repo.owner.login, repo.name, pr.number, pr.head.sha],
  );
  const busy = status.data?.state === "queued" || status.data?.state === "reviewing";
  const { reload } = status;
  // While it works, look again every few seconds.
  React.useEffect(() => {
    if (!busy) return;
    const id = window.setInterval(reload, 4000);
    return () => window.clearInterval(id);
  }, [busy, reload]);
  const s: AiReview | null = status.data;
  if (s === null) return null;
  const line = ((): string | null => {
    switch (s.state) {
      case "queued":
        return t("git.bot.queued");
      case "reviewing":
        return t("git.bot.reviewing", { done: s.filesDone, total: s.filesTotal });
      case "done":
        return s.ahead > 0 ? t("git.bot.behind", { count: s.ahead }) : t("git.bot.done");
      case "failed":
        return t("git.bot.failed", { error: s.error ?? "" });
      case "off":
      case "unavailable":
        return t("git.bot.off");
      default:
        return null;
    }
  })();
  if (line === null) return null;
  return (
    <p className="pl-8 text-[12px] text-muted" data-bot-status={s.state} role="status">
      {line}
    </p>
  );
}

// ── Checks ──────────────────────────────────────────────────────────────

const CHECK_DOT: Record<Checks["state"], DotState> = { pending: "pending", success: "ok", failure: "crit", error: "crit" };
const CHECK_KEY: Record<Checks["state"], MessageKey> = {
  pending: "git.checks.pending",
  success: "git.checks.success",
  failure: "git.checks.failure",
  error: "git.checks.error",
};

function ChecksBox({ repo, pr }: { repo: Repo; pr: Pull }) {
  const t = useT();
  const sha = pr.head.sha;
  const checks = useLoad(
    (signal) => (sha === null ? Promise.resolve(null) : git.checks(repo.owner.login, repo.name, sha, signal).catch(() => null)),
    [repo.owner.login, repo.name, sha],
  );
  const c = checks.data;
  if (c === null || c.statuses.length === 0) return null;
  return (
    <section className="flex flex-col gap-2 rounded-card border border-line bg-surface px-4 py-3" data-checks>
      <h3 className="flex items-center gap-2 text-[13px] font-semibold">
        <StatusDot state={CHECK_DOT[c.state]} />
        {t(CHECK_KEY[c.state])}
      </h3>
      <ul className="flex flex-col gap-1.5">
        {c.statuses.map((s) => (
          <li key={s.context} className="flex flex-wrap items-center gap-2 text-[12.5px]">
            <StatusDot state={CHECK_DOT[s.state]} />
            <span className="font-medium text-ink">{s.context}</span>
            <span className="min-w-0 flex-1 truncate text-muted">{s.description}</span>
            {s.targetUrl !== null && s.targetUrl !== "" && (
              <a href={s.targetUrl} className="text-accent hover:underline" rel="noreferrer noopener">
                {t("git.checks.details")}
              </a>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

// ── Merging ─────────────────────────────────────────────────────────────

function MergeBox({ repo, pr, reload }: { repo: Repo; pr: Pull; reload: () => void }) {
  const t = useT();
  const me = useMe();
  const o = repo.owner.login;
  const [style, setStyle] = React.useState<MergeStyle>("merge");
  const [dropBranch, setDropBranch] = React.useState(true);
  const [busy, setBusy] = React.useState(false);
  const author = me !== null && me.login === pr.user.login;
  const writer = pr.permission === "write" || pr.permission === "admin";

  const setState = (state: "open" | "closed") => {
    setBusy(true);
    git.editPull(o, repo.name, pr.number, { state }).then(
      () => {
        setBusy(false);
        reload();
      },
      (err: unknown) => {
        setBusy(false);
        failed(t("git.merge.failed"), err);
      },
    );
  };

  if (pr.merged) {
    return (
      <Alert variant="ok" data-merge-box="merged">
        <HugeiconsIcon icon={GitMergeIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        <AlertTitle>{t("git.merge.done")}</AlertTitle>
        <AlertDescription>{t("git.merge.doneBody", { base: pr.base.ref })}</AlertDescription>
      </Alert>
    );
  }
  if (pr.state === "closed") {
    return (
      <Alert data-merge-box="closed">
        <AlertTitle>{t("git.merge.closed")}</AlertTitle>
        {(writer || author) && (
          <AlertDescription>
            <Button size="sm" variant="secondary" className="mt-2" disabled={busy} onClick={() => setState("open")}>
              {t("git.merge.reopen")}
            </Button>
          </AlertDescription>
        )}
      </Alert>
    );
  }

  const changes = pr.reviewers.filter((r) => r.state === "changes_requested");
  const approvals = pr.reviewers.filter((r) => r.state === "approved");
  const styles: { id: MergeStyle; label: MessageKey; hint: MessageKey; ok: boolean }[] = [
    { id: "merge", label: "git.merge.style.merge", hint: "git.merge.hint.merge", ok: true },
    { id: "squash", label: "git.merge.style.squash", hint: "git.merge.hint.squash", ok: true },
    { id: "fast-forward", label: "git.merge.style.ff", hint: "git.merge.hint.ff", ok: pr.fast_forward },
  ];
  const chosen = styles.find((s) => s.id === style) ?? styles[0];

  return (
    <section className="flex flex-col gap-3 rounded-card border border-line bg-surface p-4" data-merge-box="open">
      {pr.conflicts.length > 0 || !pr.mergeable ? (
        <Alert variant="crit">
          <HugeiconsIcon icon={Alert02Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t("git.merge.conflicts")}</AlertTitle>
          <AlertDescription>
            <p>{t("git.merge.conflictsBody", { base: pr.base.ref, head: pr.head.ref })}</p>
            {pr.conflicts.length > 0 && (
              <ul className="mt-1.5 list-disc pl-5 font-mono text-[12px]">
                {pr.conflicts.map((f) => (
                  <li key={f}>{f}</li>
                ))}
              </ul>
            )}
          </AlertDescription>
        </Alert>
      ) : (
        <Alert variant={changes.length > 0 ? "warn" : "ok"}>
          <HugeiconsIcon icon={changes.length > 0 ? Alert02Icon : CheckmarkCircle02Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertTitle>{t(changes.length > 0 ? "git.merge.changesRequested" : "git.merge.clean")}</AlertTitle>
          <AlertDescription>
            {changes.length > 0
              ? t("git.merge.changesBy", { names: changes.map((r) => r.login).join(", ") })
              : approvals.length > 0
                ? t("git.merge.approvedBy", { names: approvals.map((r) => r.login).join(", ") })
                : t("git.merge.noReview")}
          </AlertDescription>
        </Alert>
      )}
      {pr.behind_by > 0 && <p className="text-[12.5px] text-muted">{t("git.merge.behind", { count: pr.behind_by, base: pr.base.ref })}</p>}
      {pr.can_merge && pr.mergeable && (
        <div className="flex flex-col gap-3">
          <ToggleGroup value={style} onValueChange={(v) => setStyle(v as MergeStyle)} aria-label={t("git.merge.style")}>
            {styles.map((s) => (
              <ToggleGroupItem key={s.id} value={s.id} disabled={!s.ok}>
                {t(s.label)}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
          {chosen !== undefined && <p className="text-[12.5px] text-muted">{t(chosen.hint)}</p>}
          <SwitchField label={t("git.merge.deleteBranch", { branch: pr.head.ref })} checked={dropBranch} onChange={setDropBranch} />
          <div className="flex flex-wrap items-center gap-2">
            <Button
              disabled={busy}
              onClick={() => {
                setBusy(true);
                git.merge(o, repo.name, pr.number, { style, head_commit_id: pr.head.sha, delete_branch: dropBranch }).then(
                  () => {
                    setBusy(false);
                    toast.success(t("git.merge.doneToast", { number: pr.number }));
                    reload();
                  },
                  (err: unknown) => {
                    setBusy(false);
                    failed(t("git.merge.failed"), err);
                    if (err instanceof GitError && err.status === 409) reload();
                  },
                );
              }}
              data-merge
            >
              <HugeiconsIcon icon={GitMergeIcon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("git.merge.button")}
            </Button>
          </div>
        </div>
      )}
      {(writer || author) && (
        <div className="flex justify-end">
          <Button size="sm" variant="ghost" disabled={busy} onClick={() => setState("closed")}>
            {t("git.merge.close")}
          </Button>
        </div>
      )}
      {!pr.can_merge && !author && <p className="text-[12.5px] text-muted">{t("git.merge.notYours")}</p>}
    </section>
  );
}

// ── Commits and files ───────────────────────────────────────────────────

function PullCommits({ repo, pr }: { repo: Repo; pr: Pull }) {
  const commits = useLoad((signal) => git.pullCommits(repo.owner.login, repo.name, pr.number, signal), [repo.owner.login, repo.name, pr.number, pr.head.sha]);
  if (commits.error !== null) return <ErrorAlert error={commits.error} />;
  if (commits.data === null) return <Loading />;
  return <CommitTable commits={commits.data} />;
}

function FilesTab({
  repo,
  pr,
  hooks,
  pending,
  clearPending,
  reload,
}: {
  repo: Repo;
  pr: Pull;
  hooks: ReviewHooks;
  pending: readonly NewLine[];
  clearPending: () => void;
  reload: () => void;
}) {
  const files = useLoad((signal) => git.pullFiles(repo.owner.login, repo.name, pr.number, signal), [repo.owner.login, repo.name, pr.number, pr.head.sha]);
  return (
    <div className="flex flex-col gap-4">
      {hooks.canComment && pr.state === "open" && <ReviewPanel repo={repo} pr={pr} pending={pending} clearPending={clearPending} reload={reload} />}
      {files.error !== null ? <ErrorAlert error={files.error} /> : files.data === null ? <Loading rows={6} /> : <DiffFiles files={files.data} review={hooks} />}
    </div>
  );
}

function ReviewPanel({
  repo,
  pr,
  pending,
  clearPending,
  reload,
}: {
  repo: Repo;
  pr: Pull;
  pending: readonly NewLine[];
  clearPending: () => void;
  reload: () => void;
}) {
  const t = useT();
  const me = useMe();
  const id = React.useId();
  const own = me !== null && me.login === pr.user.login;
  const [event, setEvent] = React.useState<ReviewEvent>("COMMENT");
  const [body, setBody] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const choices: { id: ReviewEvent; label: MessageKey; ok: boolean }[] = [
    { id: "COMMENT", label: "git.review.kind.comment", ok: true },
    { id: "APPROVE", label: "git.review.kind.approve", ok: !own },
    { id: "REQUEST_CHANGES", label: "git.review.kind.changes", ok: !own },
  ];
  const empty = body.trim() === "" && pending.length === 0 && event !== "APPROVE";
  return (
    <section className="flex flex-col gap-3 rounded-card border border-line bg-surface p-4" data-review-panel>
      <div className="flex flex-wrap items-center gap-2">
        <h3 className="text-[14px] font-semibold">{t("git.review.title")}</h3>
        {pending.length > 0 && <Badge variant="local">{t("git.review.pendingCount", { count: pending.length })}</Badge>}
        <span className="ml-auto text-[12px] text-muted">{t("git.review.at", { sha: short(pr.head.sha) })}</span>
      </div>
      <p className="text-[12.5px] text-muted">{t("git.review.hint")}</p>
      <Textarea id={`${id}-body`} rows={3} value={body} onChange={(e) => setBody(e.target.value)} aria-label={t("git.review.summary")} placeholder={t("git.review.summaryPlaceholder")} />
      <div className="flex flex-wrap items-center gap-3">
        <ToggleGroup value={event} onValueChange={(v) => setEvent(v as ReviewEvent)} aria-label={t("git.review.kind")}>
          {choices.map((c) => (
            <ToggleGroupItem key={c.id} value={c.id} disabled={!c.ok}>
              {t(c.label)}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
        <Button
          className="ml-auto"
          disabled={busy || empty}
          data-submit-review
          onClick={() => {
            setBusy(true);
            git.review(repo.owner.login, repo.name, pr.number, { event, body: body.trim(), commit_id: pr.head.sha, comments: [...pending] }).then(
              () => {
                setBusy(false);
                setBody("");
                clearPending();
                toast.success(t("git.review.sent"));
                reload();
              },
              (err: unknown) => {
                setBusy(false);
                failed(t("git.review.failed"), err);
                if (err instanceof GitError && err.status === 409) reload();
              },
            );
          }}
        >
          {t("git.review.submit")}
        </Button>
      </div>
      {own && <p className="text-[12px] text-muted">{t("git.review.ownPull")}</p>}
    </section>
  );
}
