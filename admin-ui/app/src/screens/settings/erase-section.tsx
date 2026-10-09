import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/progress";
import type { Erase, EraseOutside, ErasePhase } from "@/lib/api";
import { intlTag, type MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { formatMinutes } from "./format";
import {
  Group,
  GroupCaption,
  GroupTitle,
  PaneSection,
  Row,
  RowText,
  RowValue,
  StackRow,
  SwitchRow,
} from "./rows";
import { useSecondsLeft, type BackupData } from "./use-backup";

/* Erasing the box: the data, not just the settings.
 *
 * lososd drives it (backend/src/erase.rs), so it carries on with this tab
 * closed: an optional backup to the owner's bucket, a countdown, then the
 * box gives up what it holds outside itself (custom domains, market
 * listings, its entry on the edge), puts the settings back and restarts,
 * and the data goes at the start of the next boot. Until the box starts
 * leaving its edge, Cancel stops all of it and nothing has changed.
 *
 * The section shows whichever of three things is true: an erase under way
 * (with the countdown and Cancel), else the button, plus the report the last
 * erase left behind, which survives the wipe so the next sign-in can see
 * what was given up. */

const PHASE: Record<Exclude<ErasePhase, "waiting">, { title: MessageKey; detail: MessageKey }> = {
  backingUp: { title: "panes.erase.phase.backingUp", detail: "panes.erase.phase.backingUpDetail" },
  leaving: { title: "panes.erase.phase.leaving", detail: "panes.erase.phase.noStopping" },
  resetting: { title: "panes.erase.phase.resetting", detail: "panes.erase.phase.noStopping" },
  restarting: { title: "panes.erase.phase.restarting", detail: "panes.erase.phase.restartingDetail" },
  failed: { title: "panes.erase.phase.failed", detail: "panes.erase.phase.failedDetail" },
};

export function EraseSection({
  locked,
  backup,
  rebuilding,
}: {
  locked: boolean;
  /** The Reset pane's reading, which it also uses to hold its own button. */
  backup: BackupData;
  rebuilding: boolean;
}) {
  const t = useT();
  const seconds = useSecondsLeft(backup.state);
  const [confirming, setConfirming] = React.useState(false);

  if (backup.state.kind !== "ready") {
    return (
      <PaneSection data-testid="erase">
        <GroupTitle>{t("panes.erase.title")}</GroupTitle>
        <Group>
          <Row last>
            <RowText
              title={t("panes.erase.row")}
              detail={backup.state.kind === "failed" ? backup.state.message : undefined}
            />
            {backup.state.kind === "loading" ? (
              <Spinner size={16} />
            ) : (
              <Button size="sm" variant="secondary" disabled={locked} onClick={backup.refresh}>
                {t("panes.market.refresh")}
              </Button>
            )}
          </Row>
        </Group>
      </PaneSection>
    );
  }

  const { view } = backup.state;
  const grace = formatMinutes(Math.round(view.graceSeconds / 60));
  const running = view.job?.state === "running";

  return (
    <>
      <PaneSection data-testid="erase">
        <GroupTitle>{t("panes.erase.title")}</GroupTitle>
        {view.erase !== null ? (
          <Underway erase={view.erase} seconds={seconds} gone={backup.gone} backup={backup} />
        ) : (
          <Group className="border-crit/35">
            <Row last>
              <RowText
                title={t("panes.erase.row")}
                detail={
                  running
                    ? t("panes.erase.waitBackup")
                    : rebuilding
                      ? t("panes.backup.waiting.detail")
                      : t("panes.erase.rowDetail")
                }
              />
              <Button
                variant="destructive"
                disabled={locked || backup.busy || running || rebuilding}
                onClick={() => setConfirming(true)}
              >
                {t("panes.erase.button")}
              </Button>
            </Row>
          </Group>
        )}
        <GroupCaption>{t("panes.erase.caption", { grace })}</GroupCaption>
      </PaneSection>

      {view.erase === null && view.lastErase !== null && view.lastErase.erasedAt > 0 && (
        <LastErase report={view.lastErase} />
      )}

      <EraseDialog
        open={confirming}
        onOpenChange={setConfirming}
        hasTarget={view.target !== null}
        grace={grace}
        onConfirm={(withBackup) => {
          setConfirming(false);
          void backup.erase(withBackup);
        }}
      />
    </>
  );
}

function clock(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function Underway({
  erase,
  seconds,
  gone,
  backup,
}: {
  erase: Erase;
  seconds: number | null;
  gone: boolean;
  backup: BackupData;
}) {
  const t = useT();
  const cancel = (
    <Button
      size="sm"
      variant={erase.phase === "failed" ? "secondary" : "primary"}
      disabled={backup.busy || !erase.cancellable}
      onClick={() => void backup.cancelErase()}
    >
      {t(erase.phase === "failed" ? "panes.erase.dismiss" : "panes.erase.cancel")}
    </Button>
  );

  if (erase.phase === "waiting") {
    return (
      <Group className="border-crit/35" data-testid="erase-countdown">
        <Row last>
          <RowText
            title={t("panes.erase.countdown")}
            detail={t(erase.backup ? "panes.erase.countdownBackedUp" : "panes.erase.countdownDetail")}
          />
          <div className="flex items-center gap-3">
            <span
              className="numeric text-[19px] font-semibold text-crit tabular-nums"
              role="timer"
              aria-live="off"
            >
              {clock(seconds ?? erase.secondsLeft ?? 0)}
            </span>
            {cancel}
          </div>
        </Row>
      </Group>
    );
  }

  const phase = PHASE[erase.phase];
  const restarted = gone && erase.phase !== "failed";
  return (
    <Group className="border-crit/35" data-testid="erase-underway" data-phase={erase.phase}>
      <Row last={erase.phase !== "failed" || erase.message.length === 0}>
        <RowText
          title={t(restarted ? "panes.erase.phase.gone" : phase.title)}
          detail={t(restarted ? "panes.erase.phase.goneDetail" : phase.detail)}
        />
        {erase.cancellable ? cancel : <Spinner size={16} />}
      </Row>
      {erase.phase === "failed" && erase.message.length > 0 && (
        <StackRow last>
          <p className="text-[12.5px] leading-snug text-muted">{erase.message}</p>
        </StackRow>
      )}
    </Group>
  );
}

function LastErase({ report }: { report: EraseOutside }) {
  const t = useT();
  useLocale();
  const when = new Intl.DateTimeFormat(intlTag(), { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(report.erasedAt * 1000),
  );
  return (
    <PaneSection data-testid="erase-report">
      <GroupTitle>{t("panes.erase.report.title")}</GroupTitle>
      <Group>
        <Row>
          <RowText title={t("panes.erase.report.when")} />
          <RowValue>{when}</RowValue>
        </Row>
        {report.hadEdge ? (
          <>
            <Row>
              <RowText title={t("panes.erase.report.domains")} />
              <RowValue>{report.domainsRemoved}</RowValue>
            </Row>
            <Row>
              <RowText title={t("panes.erase.report.listings")} />
              <RowValue>{report.listingsClosed}</RowValue>
            </Row>
            <Row last={report.problems.length === 0}>
              <RowText title={t("panes.erase.report.edge")} />
              <RowValue>{t(report.leftEdge ? "panes.erase.report.yes" : "panes.erase.report.no")}</RowValue>
            </Row>
            {report.problems.length > 0 && (
              <StackRow last>
                <p className="text-[12.5px] leading-snug text-muted">
                  {t("panes.erase.report.problems")}
                </p>
              </StackRow>
            )}
          </>
        ) : (
          <Row last>
            <RowText title={t("panes.erase.report.noEdge")} />
          </Row>
        )}
      </Group>
    </PaneSection>
  );
}

function EraseDialog({
  open,
  onOpenChange,
  hasTarget,
  grace,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  hasTarget: boolean;
  grace: string;
  onConfirm: (backup: boolean) => void;
}) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const switchId = React.useId();
  const [withBackup, setWithBackup] = React.useState(hasTarget);
  React.useEffect(() => {
    if (open) setWithBackup(hasTarget);
  }, [open, hasTarget]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy={titleId} describedBy={bodyId}>
      <DialogHeader>
        <DialogTitle id={titleId} className="flex items-center gap-2">
          <HugeiconsIcon
            icon={Alert02Icon}
            size={19}
            strokeWidth={1.5}
            color="currentColor"
            className="text-crit"
            aria-hidden="true"
          />
          {t("panes.erase.dialogTitle")}
        </DialogTitle>
        <DialogDescription id={bodyId}>{t("panes.erase.dialogBody", { grace })}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        <ul className="flex flex-col gap-1.5 text-[13px] leading-snug text-muted">
          <li className="flex gap-2">
            <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-crit" />
            {t("panes.erase.goes")}
          </li>
          <li className="flex gap-2">
            <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-crit" />
            {t("panes.erase.outside")}
          </li>
          <li className="flex gap-2">
            <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-ok" />
            {t("panes.erase.stays")}
          </li>
        </ul>
        <Group>
          <SwitchRow
            id={switchId}
            title={t("panes.erase.backupFirst")}
            description={t(hasTarget ? "panes.erase.backupFirstDetail" : "panes.erase.noTarget")}
            checked={withBackup && hasTarget}
            disabled={!hasTarget}
            onCheckedChange={setWithBackup}
            last
          />
        </Group>
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={() => onOpenChange(false)}>
          {t("panes.erase.keep")}
        </Button>
        <Button variant="destructive" onClick={() => onConfirm(withBackup && hasTarget)}>
          {t("panes.erase.confirm")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
