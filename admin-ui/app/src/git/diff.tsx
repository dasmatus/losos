import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon, ArrowDown01Icon, ArrowRight01Icon, CheckmarkCircle02Icon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import type { FileDiff, LineComment, NewLine, Side } from "./api";
import { when, whenFull } from "./format";
import { Markdown } from "./markdown";
import { Person } from "./parts";

/* The changes of a pull request (or of a comparison), file by file, as
 * unified diffs with both line numbers. On a pull request each line the
 * diff shows takes a comment: on the new side for an added or unchanged
 * line, on the old side for a removed one, the same rule LosOS Git checks
 * (`commentable` in backend/src/forge/pulls.rs). Threads sit under their
 * line; the comments of a review in progress sit there too, marked. */

export type RowKind = "hunk" | "add" | "del" | "ctx" | "note";

export interface Row {
  kind: RowKind;
  old: number | null;
  new: number | null;
  text: string;
}

/** The rows of one file's patch, numbered. */
export function parsePatch(patch: string): Row[] {
  const rows: Row[] = [];
  let o = 0;
  let n = 0;
  for (const line of patch.split("\n")) {
    if (line.startsWith("@@")) {
      const m = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)$/u.exec(line);
      o = Number(m?.[1] ?? 0);
      n = Number(m?.[2] ?? 0);
      rows.push({ kind: "hunk", old: null, new: null, text: line });
      continue;
    }
    const mark = line[0];
    const text = line.slice(1);
    if (mark === "+") rows.push({ kind: "add", old: null, new: n++, text });
    else if (mark === "-") rows.push({ kind: "del", old: o++, new: null, text });
    else if (mark === "\\") rows.push({ kind: "note", old: null, new: null, text: line });
    else if (mark === " ") rows.push({ kind: "ctx", old: o++, new: n++, text });
  }
  return rows;
}

/** Where a comment on this row lands: removed lines on the old side. */
export function anchor(row: Row): { side: Side; line: number } | null {
  if (row.kind === "del" && row.old !== null) return { side: "old", line: row.old };
  if ((row.kind === "add" || row.kind === "ctx") && row.new !== null) return { side: "new", line: row.new };
  return null;
}

/** A thread: a comment on a line and the replies to it. */
export interface Thread {
  root: LineComment;
  replies: LineComment[];
}

export function threads(comments: readonly LineComment[]): Thread[] {
  const roots = comments.filter((c) => c.in_reply_to === null);
  return roots.map((root) => ({
    root,
    replies: comments.filter((c) => c.in_reply_to === root.id).sort((a, b) => a.id - b.id),
  }));
}

export interface ReviewHooks {
  /** Threads already on the pull request. */
  threads: readonly Thread[];
  /** Line comments of the review being written, not yet sent. */
  pending: readonly NewLine[];
  addPending: (c: NewLine) => void;
  dropPending: (index: number) => void;
  /** One comment sent at once, as a review of just that comment. */
  commentNow: (c: NewLine) => Promise<void>;
  reply: (to: number, body: string) => Promise<void>;
  resolve: (id: number, resolved: boolean) => Promise<void>;
  /** May the person write here at all. */
  canComment: boolean;
}

const STATUS: Record<FileDiff["status"], { tone: "ok" | "crit" | "warn" | "neutral"; key: MessageKey }> = {
  added: { tone: "ok", key: "git.diff.status.added" },
  deleted: { tone: "crit", key: "git.diff.status.deleted" },
  modified: { tone: "neutral", key: "git.diff.status.modified" },
  renamed: { tone: "warn", key: "git.diff.status.renamed" },
  copied: { tone: "warn", key: "git.diff.status.copied" },
  changed: { tone: "neutral", key: "git.diff.status.changed" },
};

export function DiffFiles({ files, review }: { files: readonly FileDiff[]; review?: ReviewHooks }) {
  const t = useT();
  const adds = files.reduce((s, f) => s + f.additions, 0);
  const dels = files.reduce((s, f) => s + f.deletions, 0);
  return (
    <div className="flex flex-col gap-3" data-diff>
      <p className="text-[13px] text-muted">
        {t("git.diff.summary", { count: files.length })}{" "}
        <span className="font-medium text-ok">+{adds}</span> <span className="font-medium text-crit">−{dels}</span>
      </p>
      {files.map((f) => (
        <DiffFile key={f.filename} file={f} review={review} />
      ))}
    </div>
  );
}

function DiffFile({ file, review }: { file: FileDiff; review: ReviewHooks | undefined }) {
  const t = useT();
  const [open, setOpen] = React.useState(true);
  const rows = React.useMemo(() => (file.patch === null ? [] : parsePatch(file.patch)), [file.patch]);
  const [composing, setComposing] = React.useState<string | null>(null);
  const mine = review?.threads.filter((th) => th.root.path === file.filename) ?? [];
  const pending = (review?.pending ?? [])
    .map((c, i) => ({ c, i }))
    .filter(({ c }) => c.path === file.filename);
  // Threads whose line the diff no longer shows still belong to this file.
  const shown = new Set(rows.map(anchor).filter((a) => a !== null).map((a) => `${a.side}:${a.line}`));
  const outdated = mine.filter((th) => !shown.has(`${th.root.side}:${th.root.line}`));

  return (
    <section className="overflow-hidden rounded-card border border-line bg-surface" data-file-diff={file.filename}>
      <header className="flex flex-wrap items-center gap-2 border-b border-line bg-sunk/60 px-3 py-2 text-[13px]">
        <Button variant="ghost" size="icon-xs" aria-expanded={open} aria-label={t(open ? "git.diff.fold" : "git.diff.unfold")} onClick={() => setOpen(!open)}>
          <HugeiconsIcon icon={open ? ArrowDown01Icon : ArrowRight01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        <span className="min-w-0 truncate font-mono text-[12.5px] font-medium text-ink">
          {file.previous_filename !== null && file.previous_filename !== file.filename ? `${file.previous_filename} → ` : ""}
          {file.filename}
        </span>
        <Badge variant={(STATUS[file.status] ?? STATUS.modified).tone}>{t((STATUS[file.status] ?? STATUS.modified).key)}</Badge>
        <span className="ml-auto text-[12px] whitespace-nowrap">
          <span className="text-ok">+{file.additions}</span> <span className="text-crit">−{file.deletions}</span>
        </span>
      </header>
      {open &&
        (file.binary ? (
          <p className="px-4 py-4 text-[13px] text-muted">{t("git.diff.binary")}</p>
        ) : file.too_large ? (
          <p className="px-4 py-4 text-[13px] text-muted">{t("git.diff.tooLarge")}</p>
        ) : rows.length === 0 ? (
          <p className="px-4 py-4 text-[13px] text-muted">{t("git.diff.nothing")}</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full border-collapse font-mono text-[12.5px] leading-[1.6]">
              <tbody>
                {rows.map((row, i) => {
                  const a = anchor(row);
                  const key = a === null ? null : `${a.side}:${a.line}`;
                  const here = a === null ? [] : mine.filter((th) => th.root.side === a.side && th.root.line === a.line);
                  const pendingHere = a === null ? [] : pending.filter(({ c }) => c.side === a.side && c.line === a.line);
                  return (
                    <React.Fragment key={i}>
                      <DiffRow
                        row={row}
                        canComment={review !== undefined && review.canComment && a !== null}
                        onComment={() => setComposing(key)}
                      />
                      {(here.length > 0 || pendingHere.length > 0 || (composing !== null && composing === key)) && review !== undefined && a !== null && (
                        <tr>
                          <td colSpan={3} className="border-y border-line bg-ground px-3 py-3 font-sans">
                            <div className="flex max-w-3xl flex-col gap-3">
                              {here.map((th) => (
                                <ThreadView key={th.root.id} thread={th} review={review} />
                              ))}
                              {pendingHere.map(({ c, i: at }) => (
                                <PendingComment key={at} comment={c} onDrop={() => review.dropPending(at)} />
                              ))}
                              {composing === key && (
                                <LineComposer
                                  onCancel={() => setComposing(null)}
                                  onAdd={(body) => {
                                    review.addPending({ path: file.filename, side: a.side, line: a.line, body });
                                    setComposing(null);
                                  }}
                                  onNow={async (body) => {
                                    await review.commentNow({ path: file.filename, side: a.side, line: a.line, body });
                                    setComposing(null);
                                  }}
                                />
                              )}
                            </div>
                          </td>
                        </tr>
                      )}
                    </React.Fragment>
                  );
                })}
              </tbody>
            </table>
          </div>
        ))}
      {open && outdated.length > 0 && review !== undefined && (
        <div className="flex flex-col gap-3 border-t border-line bg-ground px-3 py-3">
          <p className="text-[12px] text-muted">{t("git.review.outdated")}</p>
          {outdated.map((th) => (
            <ThreadView key={th.root.id} thread={th} review={review} />
          ))}
        </div>
      )}
    </section>
  );
}

const ROW_TONE: Record<RowKind, string> = {
  add: "bg-ok/10",
  del: "bg-crit/10",
  ctx: "",
  hunk: "bg-accent-wash/60 text-muted",
  note: "text-muted",
};

function DiffRow({ row, canComment, onComment }: { row: Row; canComment: boolean; onComment: () => void }) {
  const t = useT();
  const mark = row.kind === "add" ? "+" : row.kind === "del" ? "−" : " ";
  return (
    <tr className={cn("group", ROW_TONE[row.kind])} data-row={row.kind}>
      <td className="w-0 px-2 text-right align-top text-faint select-none">{row.old ?? ""}</td>
      <td className="w-0 px-2 text-right align-top text-faint select-none">{row.new ?? ""}</td>
      <td className="relative pr-4 pl-6 whitespace-pre text-ink">
        {canComment && (
          <button
            type="button"
            aria-label={t("git.review.commentLine", { line: row.new ?? row.old ?? 0 })}
            onClick={onComment}
            className={cn(
              "absolute top-0.5 left-0.5 inline-flex size-[18px] items-center justify-center rounded-sm bg-accent text-surface",
              "opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
            )}
            data-comment-line
          >
            <HugeiconsIcon icon={Add01Icon} size={12} strokeWidth={2} color="currentColor" aria-hidden="true" />
          </button>
        )}
        {row.kind === "hunk" || row.kind === "note" ? (
          row.text
        ) : (
          <>
            <span className="text-faint select-none">{mark}</span>
            {row.text}
          </>
        )}
      </td>
    </tr>
  );
}

function CommentBody({ comment, tone }: { comment: LineComment; tone?: "reply" }) {
  return (
    <div className={cn("flex flex-col gap-1.5", tone === "reply" && "border-t border-line pt-3")}>
      <div className="flex flex-wrap items-center gap-2 text-[12.5px] text-muted">
        <Person login={comment.user.login} bot={comment.user.is_bot === true} />
        <span title={whenFull(comment.created_at)}>{when(comment.created_at)}</span>
      </div>
      <Markdown source={comment.body} className="text-[13.5px]" />
    </div>
  );
}

export function ThreadView({ thread, review }: { thread: Thread; review: ReviewHooks }) {
  const t = useT();
  const [replying, setReplying] = React.useState(false);
  const [text, setText] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [folded, setFolded] = React.useState(thread.root.resolved);
  const { root } = thread;
  if (folded) {
    return (
      <div className="flex flex-wrap items-center gap-2 rounded-control border border-line bg-surface px-3 py-2 text-[12.5px] text-muted" data-thread={root.id}>
        <HugeiconsIcon icon={CheckmarkCircle02Icon} size={15} strokeWidth={1.5} color="currentColor" className="text-ok" aria-hidden="true" />
        {root.resolved_by !== null ? t("git.review.resolvedBy", { name: root.resolved_by.login }) : t("git.review.resolved")}
        <Button variant="link" size="xs" className="ml-auto" onClick={() => setFolded(false)}>
          {t("git.review.show")}
        </Button>
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-3 rounded-control border border-line bg-surface px-3 py-3" data-thread={root.id}>
      <CommentBody comment={root} />
      {thread.replies.map((r) => (
        <CommentBody key={r.id} comment={r} tone="reply" />
      ))}
      {replying ? (
        <form
          className="flex flex-col gap-2 border-t border-line pt-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (text.trim() === "" || busy) return;
            setBusy(true);
            review.reply(root.id, text.trim()).then(
              () => {
                setText("");
                setReplying(false);
                setBusy(false);
              },
              () => setBusy(false),
            );
          }}
        >
          <Textarea rows={3} value={text} onChange={(e) => setText(e.target.value)} aria-label={t("git.review.reply")} autoFocus />
          <div className="flex justify-end gap-2">
            <Button type="button" size="sm" variant="secondary" onClick={() => setReplying(false)}>
              {t("git.cancel")}
            </Button>
            <Button type="submit" size="sm" disabled={text.trim() === "" || busy}>
              {t("git.review.reply")}
            </Button>
          </div>
        </form>
      ) : (
        review.canComment && (
          <div className="flex flex-wrap items-center gap-2 border-t border-line pt-2.5">
            <Button size="xs" variant="secondary" onClick={() => setReplying(true)}>
              {t("git.review.reply")}
            </Button>
            <Button
              size="xs"
              variant="ghost"
              onClick={() => {
                void review.resolve(root.id, !root.resolved).then(() => setFolded(!root.resolved));
              }}
            >
              {t(root.resolved ? "git.review.unresolve" : "git.review.resolve")}
            </Button>
          </div>
        )
      )}
    </div>
  );
}

function PendingComment({ comment, onDrop }: { comment: NewLine; onDrop: () => void }) {
  const t = useT();
  return (
    <div className="flex flex-col gap-2 rounded-control border border-dashed border-accent bg-surface px-3 py-3" data-pending>
      <div className="flex items-center gap-2">
        <Badge variant="local">{t("git.review.pending")}</Badge>
        <Button variant="ghost" size="xs" className="ml-auto" onClick={onDrop}>
          {t("git.review.drop")}
        </Button>
      </div>
      <Markdown source={comment.body} className="text-[13.5px]" />
    </div>
  );
}

function LineComposer({
  onCancel,
  onAdd,
  onNow,
}: {
  onCancel: () => void;
  onAdd: (body: string) => void;
  onNow: (body: string) => Promise<void>;
}) {
  const t = useT();
  const [text, setText] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const body = text.trim();
  return (
    <div className="flex flex-col gap-2 rounded-control border border-line bg-surface px-3 py-3" data-composer>
      <Textarea rows={3} value={text} onChange={(e) => setText(e.target.value)} aria-label={t("git.review.lineComment")} placeholder={t("git.review.linePlaceholder")} autoFocus />
      <div className="flex flex-wrap justify-end gap-2">
        <Button size="sm" variant="ghost" onClick={onCancel}>
          {t("git.cancel")}
        </Button>
        <Button
          size="sm"
          variant="secondary"
          disabled={body === "" || busy}
          onClick={() => {
            setBusy(true);
            onNow(body).catch(() => setBusy(false));
          }}
        >
          {t("git.review.commentNow")}
        </Button>
        <Button size="sm" disabled={body === "" || busy} onClick={() => onAdd(body)}>
          {t("git.review.addToReview")}
        </Button>
      </div>
    </div>
  );
}
