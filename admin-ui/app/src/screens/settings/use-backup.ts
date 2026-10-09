import * as React from "react";
import {
  ApiError,
  deleteBackupTarget,
  getBackup,
  isAbort,
  isUnauthorized,
  postBackupRestore,
  postBackupRun,
  postBackupTarget,
  postErase,
  postEraseCancel,
  type BackupResponse,
  type BackupTargetInput,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import type { MessageKey } from "@/lib/i18n";

/* Backups and the full erase, as lososd sees them (`GET /api/backup`).
 *
 * One view feeds both panes: the Backup pane shows the bucket, the last
 * backup and a restore; the Reset pane shows an erase under way and the
 * report the last one left. While a backup, a restore or an erase is going,
 * the view is asked for every two seconds, which is also lososd's own tick,
 * so the countdown and the phase move without a reload. Nothing going on
 * means one request.
 *
 * Once an erase reaches the restart the box goes away mid-poll. A failed
 * request then is the expected outcome, not an error, so the hook keeps
 * the last view and says the box is restarting. */

export const BACKUP_POLL_MS = 2000;

export type BackupState =
  | { kind: "loading" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; view: BackupResponse; at: number };

export interface BackupData {
  state: BackupState;
  /** The box stopped answering after an erase reached its restart. */
  gone: boolean;
  busy: boolean;
  refresh: () => void;
  saveTarget: (target: BackupTargetInput) => Promise<boolean>;
  forgetTarget: () => Promise<boolean>;
  backUp: () => Promise<boolean>;
  restore: (code: string) => Promise<boolean>;
  erase: (backup: boolean) => Promise<boolean>;
  cancelErase: () => Promise<boolean>;
}

/* Every field has a fallback, so a box that answers with less (an older
 * lososd, or one that does not serve the route at all) shows "no bucket"
 * rather than a broken pane. */
function viewOf(reply: Partial<BackupResponse>): BackupResponse {
  return {
    target: reply.target ?? null,
    last: reply.last ?? null,
    job: reply.job ?? null,
    erase: reply.erase ?? null,
    lastErase: reply.lastErase ?? null,
    graceSeconds: reply.graceSeconds ?? 900,
  };
}

function active(view: BackupResponse): boolean {
  return view.erase !== null || view.job?.state === "running";
}

export function useBackup(enabled: boolean): BackupData {
  const [state, setState] = React.useState<BackupState>({ kind: "loading" });
  const [gone, setGone] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [tick, setTick] = React.useState(0);
  const refresh = React.useCallback(() => setTick((n) => n + 1), []);
  const restarting =
    state.kind === "ready" &&
    (state.view.erase?.phase === "restarting" || state.view.erase?.phase === "resetting");
  const restartingRef = React.useRef(restarting);
  restartingRef.current = restarting;

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const view = viewOf(await getBackup({ signal: controller.signal }));
        if (controller.signal.aborted) return;
        setState({ kind: "ready", view, at: Date.now() });
        setGone(false);
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        if (restartingRef.current) {
          setGone(true);
          return;
        }
        setState({ kind: "failed", message: error instanceof Error ? error.message : "" });
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  const polling = (state.kind === "ready" && active(state.view)) || gone;
  React.useEffect(() => {
    if (!enabled || !polling) return;
    const timer = window.setTimeout(refresh, BACKUP_POLL_MS);
    return () => window.clearTimeout(timer);
  }, [enabled, polling, tick, refresh]);

  /* One request at a time. Every action is followed by a fresh read rather
   * than trusting its reply, because the reply of each is only a piece of
   * the view. A refusal is a toast with lososd's own sentence. */
  const act = React.useCallback(
    async (fn: () => Promise<unknown>, failedKey: MessageKey, done?: () => void) => {
      setBusy(true);
      try {
        await fn();
        done?.();
        return true;
      } catch (error) {
        if (!isUnauthorized(error)) {
          const said = error instanceof ApiError || error instanceof Error ? error.message : "";
          toast.error(t(failedKey), said, { help: "backup-and-erase" });
        }
        return false;
      } finally {
        setBusy(false);
        refresh();
      }
    },
    [refresh],
  );

  return {
    state,
    gone,
    busy,
    refresh,
    saveTarget: (target) =>
      act(
        () => postBackupTarget(target),
        "panes.backup.toast.targetFailed",
        () => toast.done(t("panes.backup.toast.targetSaved")),
      ),
    forgetTarget: () =>
      act(
        () => deleteBackupTarget(),
        "panes.backup.toast.targetFailed",
        () => toast.done(t("panes.backup.toast.targetForgotten")),
      ),
    backUp: () => act(() => postBackupRun(), "panes.backup.toast.backupFailed"),
    restore: (code) => act(() => postBackupRestore(code), "panes.backup.toast.restoreFailed"),
    erase: (backup) => act(() => postErase(backup), "panes.erase.toast.failed"),
    cancelErase: () =>
      act(
        () => postEraseCancel(),
        "panes.erase.toast.cancelFailed",
        () => toast.success(t("panes.erase.toast.cancelled"), t("panes.erase.toast.cancelledBody")),
      ),
  };
}

/** Seconds left on the countdown, counted down locally between polls. */
export function useSecondsLeft(state: BackupState): number | null {
  const [now, setNow] = React.useState(() => Date.now());
  const left =
    state.kind === "ready" && state.view.erase?.secondsLeft !== undefined
      ? state.view.erase.secondsLeft
      : null;
  React.useEffect(() => {
    if (left === null) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [left]);
  if (left === null || state.kind !== "ready") return null;
  return Math.max(0, left - Math.floor((now - state.at) / 1000));
}
