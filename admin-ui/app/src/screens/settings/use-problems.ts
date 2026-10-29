import * as React from "react";
import {
  ApiError,
  getProblemReport,
  getProblems,
  getProblemSending,
  isAbort,
  isUnauthorized,
  postProblemCancel,
  postProblemDismiss,
  postProblemSend,
  type ProblemReport,
  type ProblemSending,
  type ProblemsResponse,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import { notPresent } from "./http";

/* What the box noticed going wrong (`GET /api/problems`), and one report
 * being read and sent.
 *
 * The box writes the report from its own journal when the owner opens it,
 * so the text on screen is the text GitHub would get, with addresses, the
 * box's name and the people on it already taken out. Sending is a job on
 * the box: it asks GitHub for a code, the page shows it, and the owner
 * enters it at github.com/login/device. The page asks where it stands
 * every two seconds until it is sent, refused or cancelled.
 *
 * A 404 is a box whose lososd has no problems plugin yet, said plainly. */

export type ProblemsState =
  | { kind: "loading" }
  | { kind: "unavailable" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; view: ProblemsResponse };

export type ReportState =
  | { kind: "closed" }
  | { kind: "loading"; id: string }
  | { kind: "failed"; id: string; message: string }
  | { kind: "ready"; id: string; report: ProblemReport; sending: ProblemSending };

export interface ProblemsData {
  state: ProblemsState;
  report: ReportState;
  refresh: () => void;
  open: (id: string) => void;
  close: () => void;
  send: (note: string) => void;
  cancel: () => void;
  dismiss: (id: string) => Promise<void>;
}

function messageOf(error: unknown): string {
  return error instanceof ApiError || error instanceof Error ? error.message : "";
}

const POLL_MS = 2000;

function waiting(sending: ProblemSending): boolean {
  return sending.state === "starting" || sending.state === "sign-in";
}

export function useProblems(enabled: boolean): ProblemsData {
  const [state, setState] = React.useState<ProblemsState>({ kind: "loading" });
  const [report, setReport] = React.useState<ReportState>({ kind: "closed" });
  const [tick, setTick] = React.useState(0);
  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const view = await getProblems({ signal: controller.signal });
        if (!controller.signal.aborted) {
          setState({
            kind: "ready",
            view: { reporting: view.reporting ?? null, problems: view.problems ?? [] },
          });
        }
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        setState(notPresent(error) ? { kind: "unavailable" } : { kind: "failed", message: messageOf(error) });
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  const open = React.useCallback((id: string) => {
    setReport({ kind: "loading", id });
    void (async () => {
      try {
        const [written, sending] = await Promise.all([getProblemReport(id), getProblemSending(id)]);
        setReport((now) =>
          now.kind !== "closed" && now.id === id ? { kind: "ready", id, report: written, sending } : now,
        );
      } catch (error) {
        if (isUnauthorized(error)) return;
        setReport((now) =>
          now.kind !== "closed" && now.id === id ? { kind: "failed", id, message: messageOf(error) } : now,
        );
      }
    })();
  }, []);

  const close = React.useCallback(() => setReport({ kind: "closed" }), []);

  // While the box waits for GitHub, ask where it stands.
  const pollId = report.kind === "ready" && waiting(report.sending) ? report.id : null;
  React.useEffect(() => {
    if (pollId === null) return;
    const controller = new AbortController();
    const timer = window.setInterval(() => {
      void (async () => {
        try {
          const sending = await getProblemSending(pollId, { signal: controller.signal });
          setReport((now) => (now.kind === "ready" && now.id === pollId ? { ...now, sending } : now));
          if (sending.state === "sent") refresh();
        } catch (error) {
          if (isAbort(error)) return;
        }
      })();
    }, POLL_MS);
    return () => {
      window.clearInterval(timer);
      controller.abort();
    };
  }, [pollId, refresh]);

  const send = React.useCallback(
    (note: string) => {
      if (report.kind !== "ready") return;
      const id = report.id;
      setReport({ ...report, sending: { state: "starting" } });
      void (async () => {
        try {
          await postProblemSend(id, note);
        } catch (error) {
          if (isUnauthorized(error)) return;
          setReport((now) =>
            now.kind === "ready" && now.id === id
              ? { ...now, sending: { state: "failed", message: messageOf(error) } }
              : now,
          );
        }
      })();
    },
    [report],
  );

  const cancel = React.useCallback(() => {
    if (report.kind !== "ready") return;
    void postProblemCancel(report.id).catch(() => undefined);
  }, [report]);

  const dismiss = React.useCallback(
    async (id: string) => {
      try {
        await postProblemDismiss(id);
      } catch (error) {
        if (!isUnauthorized(error)) {
          toast.error(t("problems.dismissFailed"), messageOf(error), { help: "problem-reports" });
        }
      } finally {
        refresh();
      }
    },
    [refresh],
  );

  return { state, report, refresh, open, close, send, cancel, dismiss };
}
