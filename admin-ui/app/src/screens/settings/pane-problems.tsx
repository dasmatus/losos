import * as React from "react";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { HelpLink } from "@/components/ui/help-link";
import { Spinner } from "@/components/ui/progress";
import { Textarea } from "@/components/ui/textarea";
import type { Problem, ProblemSending } from "@/lib/api";
import { HANDBOOK_BASE } from "@/lib/handbook";
import { intlTag } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";
import { useProblems, type ProblemsData } from "./use-problems";

/* What went wrong on this box, and a report of each that the owner can read
 * and send.
 *
 * The box reads its journal every minute (backend/plugins/problems): a
 * service that ended in a failed result, a program that crashed, one the
 * kernel killed because memory ran out. Each is one row, however often it
 * happened. A problem the handbook already explains links that page instead
 * of offering a report. The rest open the report as the box wrote it, the
 * whole text, and Send asks GitHub for a code the owner enters on
 * github.com/login/device: nothing leaves the box before that, and the
 * report goes to an issue anyone can read. */

export function ProblemsPane({ locked }: { locked: boolean }) {
  const t = useT();
  const data = useProblems(!locked);
  const { state } = data;

  if (state.kind === "loading") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("problems.loading")} />
            <Spinner size={16} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "unavailable") {
    return (
      <PaneSection data-testid="problems-unavailable">
        <Group>
          <Row last>
            <RowText title={t("problems.unavailable")} />
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
            <RowText title={t("problems.failed")} detail={state.message} />
            <Button size="sm" variant="secondary" disabled={locked} onClick={data.refresh}>
              {t("problems.retry")}
            </Button>
          </Row>
        </Group>
      </PaneSection>
    );
  }

  const { view } = state;
  const open = view.problems.filter((p) => !p.dismissed);
  const dismissed = view.problems.filter((p) => p.dismissed);
  return (
    <>
      <PaneSection data-testid="problems-reporting">
        <GroupTitle>{t("problems.reports.title")}</GroupTitle>
        <Group>
          <Row last>
            <RowText
              title={
                view.reporting === null
                  ? t("problems.reports.off")
                  : t("problems.reports.on", { repository: view.reporting.repository })
              }
              detail={t(view.reporting === null ? "problems.reports.offDetail" : "problems.reports.onDetail")}
            />
            <Badge variant={view.reporting === null ? "neutral" : "local"} className="shrink-0">
              {t(view.reporting === null ? "problems.badge.localOnly" : "problems.badge.askFirst")}
            </Badge>
          </Row>
        </Group>
        <GroupCaption>
          {t("problems.reports.caption")}{" "}
          <HelpLink entry="problem-reports" label={t("problems.reports.more")} />
        </GroupCaption>
      </PaneSection>

      <PaneSection data-testid="problems-list">
        <GroupTitle>{t("problems.list.title")}</GroupTitle>
        <Group>
          {open.length === 0 ? (
            <Row last>
              <RowText title={t("problems.none")} detail={t("problems.noneDetail")} />
            </Row>
          ) : (
            open.map((p, i) => (
              <ProblemRow
                key={p.id}
                problem={p}
                last={i === open.length - 1}
                data={data}
                locked={locked}
                canSend={view.reporting !== null}
              />
            ))
          )}
        </Group>
      </PaneSection>

      {dismissed.length > 0 && (
        <PaneSection data-testid="problems-dismissed">
          <GroupTitle>{t("problems.dismissed.title")}</GroupTitle>
          <Group>
            {dismissed.map((p, i) => (
              <ProblemRow
                key={p.id}
                problem={p}
                last={i === dismissed.length - 1}
                data={data}
                locked={locked}
                canSend={view.reporting !== null}
              />
            ))}
          </Group>
          <GroupCaption>{t("problems.dismissed.caption")}</GroupCaption>
        </PaneSection>
      )}

      <ReportDialog data={data} canSend={view.reporting !== null} repository={view.reporting?.repository ?? null} />
    </>
  );
}

/* Reads the language on screen, so a switch of language re-renders it. */
function useWhen(): (seconds: number) => string {
  useLocale();
  return (seconds) =>
    new Intl.DateTimeFormat(intlTag(), { dateStyle: "medium", timeStyle: "short" }).format(
      new Date(seconds * 1000),
    );
}

/** What happened, in the reader's language. */
export function problemSentence(p: Problem, t: ReturnType<typeof useT>): string {
  const subject = p.unit ?? p.process ?? "?";
  switch (p.kind) {
    case "crashed":
      return t("problems.kind.crashed", { subject, signal: `SIG${p.signal ?? "?"}` });
    case "out-of-memory":
      return t("problems.kind.outOfMemory", { subject });
    default:
      return t("problems.kind.failed", { subject, result: p.result ?? "failed" });
  }
}

function ProblemRow({
  problem: p,
  last,
  data,
  locked,
  canSend,
}: {
  problem: Problem;
  last: boolean;
  data: ProblemsData;
  locked: boolean;
  canSend: boolean;
}) {
  const t = useT();
  const when = useWhen();
  const locale = useLocale();
  return (
    <StackRow last={last} data-testid="problem" data-id={p.id} data-kind={p.kind} className="gap-1.5">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <p className="text-sm leading-snug break-words text-ink">{problemSentence(p, t)}</p>
          <p className="text-[12.5px] leading-snug text-muted">
            {p.count > 1
              ? t("problems.times", { count: p.count, first: when(p.first), last: when(p.last) })
              : t("problems.once", { at: when(p.last) })}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {p.documented === null ? (
            <Button
              size="sm"
              variant="secondary"
              disabled={locked}
              onClick={() => data.open(p.id)}
              data-testid="problem-read"
            >
              {t(canSend && p.sent === null ? "problems.readAndSend" : "problems.read")}
            </Button>
          ) : (
            <a
              href={`${HANDBOOK_BASE}${p.documented.page}/`}
              target="_blank"
              rel="noopener"
              className={buttonVariants({ variant: "secondary", size: "sm" })}
              data-testid="problem-documented"
            >
              {t("problems.whatToDo")}
            </a>
          )}
          {!p.dismissed && (
            <Button size="sm" variant="ghost" disabled={locked} onClick={() => void data.dismiss(p.id)}>
              {t("problems.dismiss")}
            </Button>
          )}
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-1.5 text-[12.5px] leading-snug text-muted">
        <Badge variant={p.kind === "failed" ? "warn" : "crit"}>{t(`problems.badge.${badgeKey(p)}`)}</Badge>
        {p.documented !== null && (
          <Badge variant="ok">
            {t("problems.badge.documented", {
              title: p.documented.title[locale] ?? p.documented.title.en,
            })}
          </Badge>
        )}
        {p.sent !== null && (
          <a href={p.sent.url} target="_blank" rel="noopener noreferrer" className="text-accent hover:underline">
            {t(p.sent.existing ? "problems.sentExisting" : "problems.sent")}
          </a>
        )}
        <span className="numeric tabular-nums">{p.id}</span>
      </div>
    </StackRow>
  );
}

function badgeKey(p: Problem): "failed" | "crashed" | "outOfMemory" {
  return p.kind === "out-of-memory" ? "outOfMemory" : p.kind;
}

// ── The report ──────────────────────────────────────────────────────────

function ReportDialog({
  data,
  canSend,
  repository,
}: {
  data: ProblemsData;
  canSend: boolean;
  repository: string | null;
}) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const [note, setNote] = React.useState("");
  const { report } = data;
  const reportId = report.kind === "closed" ? null : report.id;
  React.useEffect(() => setNote(""), [reportId]);

  const sending: ProblemSending = report.kind === "ready" ? report.sending : { state: "idle" };
  const busy = sending.state === "starting" || sending.state === "sign-in";

  return (
    <Dialog
      open={report.kind !== "closed"}
      onOpenChange={(open) => {
        if (!open && !busy) data.close();
      }}
      dismissible={!busy}
      labelledBy={titleId}
      describedBy={bodyId}
      dialogClassName="w-[min(46rem,calc(100vw-2rem))]"
    >
      <DialogHeader>
        <DialogTitle id={titleId} className="break-words">
          {report.kind === "ready" ? report.report.title : t("problems.report.title")}
        </DialogTitle>
        <DialogDescription id={bodyId}>
          {canSend
            ? t("problems.report.public", { repository: repository ?? "" })
            : t("problems.report.localOnly")}
        </DialogDescription>
      </DialogHeader>

      {report.kind === "loading" && (
        <DialogBody className="flex items-center gap-2 py-6 text-sm text-muted">
          <Spinner size={16} />
          {t("problems.report.writing")}
        </DialogBody>
      )}
      {report.kind === "failed" && (
        <DialogBody>
          <p className="text-sm text-crit" role="alert">
            {t("problems.report.failed")} {report.message}
          </p>
        </DialogBody>
      )}
      {report.kind === "ready" && (
        <DialogBody className="flex flex-col gap-3">
          <pre
            className="max-h-[45vh] overflow-auto rounded-control border border-line bg-sunk p-3 font-mono text-[12px] leading-snug whitespace-pre-wrap break-words text-ink"
            data-testid="problem-report-body"
            tabIndex={0}
          >
            {report.report.body}
          </pre>
          {canSend && sending.state === "idle" && report.report.problem.sent === null && (
            <label className="flex flex-col gap-1.5 text-[12.5px] text-muted">
              {t("problems.report.note")}
              <Textarea
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder={t("problems.report.notePlaceholder")}
                maxLength={4000}
                data-testid="problem-note"
              />
            </label>
          )}
          <SendingState sending={sending} />
        </DialogBody>
      )}

      <DialogFooter>
        {busy ? (
          <Button variant="ghost" onClick={data.cancel}>
            {t("problems.report.stop")}
          </Button>
        ) : (
          <Button variant="ghost" onClick={data.close}>
            {t("problems.report.close")}
          </Button>
        )}
        {canSend && report.kind === "ready" && !busy && sending.state !== "sent" && (
          <Button onClick={() => data.send(note)} data-testid="problem-send">
            {t(sending.state === "idle" ? "problems.report.send" : "problems.report.sendAgain")}
          </Button>
        )}
      </DialogFooter>
    </Dialog>
  );
}

function SendingState({ sending }: { sending: ProblemSending }) {
  const t = useT();
  switch (sending.state) {
    case "idle":
      return null;
    case "starting":
      return (
        <p className="flex items-center gap-2 text-sm text-muted" role="status">
          <Spinner size={14} />
          {t("problems.send.starting")}
        </p>
      );
    case "sign-in":
      return (
        <div
          className="flex flex-col items-start gap-2 rounded-control border border-line bg-surface p-3"
          role="status"
          data-testid="problem-sign-in"
        >
          <p className="text-sm text-ink">{t("problems.send.enterCode")}</p>
          <p className="font-mono text-2xl font-medium tracking-[0.2em] text-ink select-all" data-testid="problem-code">
            {sending.code}
          </p>
          <a
            href={sending.uri}
            target="_blank"
            rel="noopener noreferrer"
            className={buttonVariants({ variant: "secondary", size: "sm" })}
          >
            {t("problems.send.open", { address: sending.uri.replace(/^https?:\/\//, "") })}
          </a>
          <p className="flex items-center gap-2 text-[12.5px] text-muted">
            <Spinner size={12} />
            {t("problems.send.waiting")}
          </p>
        </div>
      );
    case "sent":
      return (
        <p className="text-sm text-ok" role="status" data-testid="problem-sent">
          {t(sending.existing ? "problems.send.addedTo" : "problems.send.opened")}{" "}
          <a href={sending.url} target="_blank" rel="noopener noreferrer" className="font-medium underline">
            {sending.url.replace(/^https?:\/\//, "")}
          </a>
        </p>
      );
    case "cancelled":
      return <p className="text-sm text-muted">{t("problems.send.cancelled")}</p>;
    case "failed":
      return (
        <p className="text-sm text-crit" role="alert">
          {sending.message}
        </p>
      );
  }
}
