import * as React from "react";
import {
  ApiError,
  getModeration,
  isAbort,
  isUnauthorized,
  postHold,
  postReportAction,
  type ModerationView,
  type ReportAction,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import { notPresent } from "./http";

/* The Moderation pane's data (`GET /api/moderation`): the reports, everyone
 * with what holds them, and the log. Every action is its own request, then
 * a fresh read, so the log line it wrote shows at once. */

export type ModerationState =
  | { kind: "loading" }
  | { kind: "unavailable" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; view: ModerationView };

export type HoldKind = "mute" | "unmute" | "suspend" | "unsuspend";

export interface ModerationData {
  state: ModerationState;
  busy: boolean;
  refresh: () => void;
  act: (id: string, action: ReportAction) => Promise<boolean>;
  hold: (kind: HoldKind, name: string, minutes?: number | null, reason?: string) => Promise<boolean>;
}

function messageOf(error: unknown): string {
  return error instanceof ApiError || error instanceof Error ? error.message : "";
}

function viewOf(reply: Partial<ModerationView>): ModerationView {
  return {
    standalone: reply.standalone === true,
    you: typeof reply.you === "string" ? reply.you : null,
    admin: reply.admin === true,
    hides: Array.isArray(reply.hides) ? reply.hides : [],
    reports: Array.isArray(reply.reports) ? reply.reports : [],
    people: Array.isArray(reply.people) ? reply.people : [],
    log: Array.isArray(reply.log) ? reply.log : [],
  };
}

export function useModeration(enabled: boolean): ModerationData {
  const [state, setState] = React.useState<ModerationState>({ kind: "loading" });
  const [busy, setBusy] = React.useState(false);
  const [tick, setTick] = React.useState(0);
  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const view = viewOf(await getModeration({ signal: controller.signal }));
        if (!controller.signal.aborted) setState({ kind: "ready", view });
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        setState(notPresent(error) ? { kind: "unavailable" } : { kind: "failed", message: messageOf(error) });
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  const run = React.useCallback(
    async (send: () => Promise<unknown>, done: string): Promise<boolean> => {
      setBusy(true);
      try {
        await send();
        toast.done(done);
        return true;
      } catch (error) {
        if (!isUnauthorized(error)) {
          toast.error(t("moderation.actionFailed"), messageOf(error) || undefined, { help: "moderation" });
        }
        return false;
      } finally {
        setBusy(false);
        refresh();
      }
    },
    [refresh],
  );

  const act = React.useCallback(
    (id: string, action: ReportAction) => run(() => postReportAction(id, action), t(`moderation.done.${action}`)),
    [run],
  );

  const hold = React.useCallback(
    (kind: HoldKind, name: string, minutes?: number | null, reason?: string) =>
      run(
        () =>
          postHold(kind, {
            name,
            ...(minutes !== undefined ? { minutes } : {}),
            ...(reason !== undefined && reason.trim().length > 0 ? { reason: reason.trim() } : {}),
          }),
        t(`moderation.done.${kind}`, { name }),
      ),
    [run],
  );

  return { state, busy, refresh, act, hold };
}
