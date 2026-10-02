import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Alert02Icon, CheckmarkCircle02Icon, InformationCircleIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/progress";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { t as translate } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { CapacityMeter } from "./capacity-meter";
import { formatBytes } from "./format";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow } from "./rows";
import type { SettingsForm } from "./use-settings-form";
import type { GrowOutcome, Storage } from "./use-storage";

export interface StoragePaneProps {
  form: SettingsForm;
  storage: Storage;
}

export function StoragePane({ form, storage }: StoragePaneProps) {
  const t = useT();
  const [confirming, setConfirming] = React.useState(false);
  const titleId = React.useId();
  const bodyId = React.useId();
  const shareId = React.useId();

  const draft = form.draft;
  const sharing = draft?.sharingMyStorage ?? false;
  const reserve = storage.facts.reserveBytes;
  const hasReserve = reserve === null || reserve > 0;

  return (
    <>
      <PaneSection>
        <GroupTitle>{t("panes.storage.disk")}</GroupTitle>
        <Group>
          <StackRow>
            <CapacityMeter facts={storage.facts} source={storage.source} />
          </StackRow>

          <Row>
            <RowText
              title={t("panes.storage.held.title")}
              detail={t("panes.storage.held.detail")}
            />
            <div className="flex items-center gap-2.5">
              <RowValue>{reserve === null ? t("panes.storage.notReported") : formatBytes(reserve)}</RowValue>
              <Button
                variant="secondary"
                size="sm"
                disabled={form.locked || storage.growing || !hasReserve}
                onClick={() => setConfirming(true)}
              >
                {storage.growing ? <Spinner size={15} label={t("panes.storage.claiming")} /> : null}
                {t("panes.storage.useReserve")}
              </Button>
            </div>
          </Row>

          {storage.outcome !== null && (
            <StackRow last>
              <GrowReport outcome={storage.outcome} onDismiss={storage.dismissOutcome} />
            </StackRow>
          )}
        </Group>
        <GroupCaption>{t("panes.storage.diskCaption")}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.storage.mesh")}</GroupTitle>
        <Group>
          <Row>
            <RowText htmlFor={shareId} title={t("panes.storage.share")} />
            <Switch
              id={shareId}
              checked={sharing}
              disabled={form.locked || !form.ready}
              onCheckedChange={(next) => form.set("sharingMyStorage", next)}
            />
          </Row>
          <Row last>
            <RowText title={t("panes.storage.ifDies")} />
            <RowValue className={sharing ? "text-ok" : undefined}>
              {sharing ? t("panes.storage.rebuilds") : t("panes.storage.notCopied")}
            </RowValue>
          </Row>
        </Group>
        <GroupCaption>{t("panes.storage.meshCaption")}</GroupCaption>
      </PaneSection>

      <Dialog
        open={confirming}
        onOpenChange={setConfirming}
        labelledBy={titleId}
        describedBy={bodyId}
      >
        <DialogHeader>
          <DialogTitle id={titleId}>{t("panes.storage.dialogTitle")}</DialogTitle>
          <DialogDescription id={bodyId}>
            {reserve === null
              ? t("panes.storage.dialogUnknown")
              : t("panes.storage.dialogAdds", { size: formatBytes(reserve) })}{" "}
            {t("panes.storage.dialogTail")}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setConfirming(false)}>
            {t("panes.storage.cancel")}
          </Button>
          <Button
            onClick={() => {
              setConfirming(false);
              storage.grow();
            }}
          >
            {t("panes.storage.useIt")}
          </Button>
        </DialogFooter>
      </Dialog>
    </>
  );
}

/* What the grow actually did.
 *
 * `grew` is the only field worth reporting: the daemon measured the
 * filesystem on both sides rather than trusting three exit statuses, because
 * a resize2fs run against an unresized mapping prints "Nothing to do!" and
 * exits 0. So a false `grew` says nothing changed, in those words. It does
 * not say "done", and it does not quietly show the old number back as if it
 * were new. */
function GrowReport({
  outcome,
  onDismiss,
}: {
  outcome: GrowOutcome;
  onDismiss: () => void;
}) {
  const t = useT();
  const view = describeOutcome(outcome);
  return (
    <div className="animate-fade-in flex items-start gap-2.5">
      <HugeiconsIcon
        icon={view.icon}
        size={18}
        strokeWidth={1.5}
        color="currentColor"
        className={cn("mt-0.5 shrink-0", view.tone)}
        aria-hidden="true"
      />
      <div className="min-w-0 flex-1" role="status">
        <p className="text-[13px] leading-snug font-medium text-ink">{view.title}</p>
        <p className="mt-0.5 text-[12.5px] leading-snug text-muted">{view.detail}</p>
      </div>
      <Button variant="ghost" size="sm" onClick={onDismiss}>
        {t("panes.storage.dismiss")}
      </Button>
    </div>
  );
}

function describeOutcome(outcome: GrowOutcome): {
  icon: IconSvgElement;
  tone: string;
  title: string;
  detail: string;
} {
  switch (outcome.kind) {
    case "grew":
      return {
        icon: CheckmarkCircle02Icon,
        tone: "text-ok",
        title: translate("panes.storage.grew.title"),
        detail: translate("panes.storage.grew.detail", {
          before: formatBytes(outcome.beforeBytes),
          after: formatBytes(outcome.afterBytes),
        }),
      };
    case "nothing":
      return {
        icon: InformationCircleIcon,
        tone: "text-muted",
        title: translate("panes.storage.nothing.title"),
        detail:
          outcome.claimedBytes > 0
            ? translate("panes.storage.nothing.claimed", {
                size: formatBytes(outcome.claimedBytes),
              })
            : translate("panes.storage.nothing.none"),
      };
    case "failed":
      return {
        icon: Alert02Icon,
        tone: "text-crit",
        title: translate("panes.storage.failed.title"),
        detail: outcome.message,
      };
    default: {
      const exhaustive: never = outcome;
      return exhaustive;
    }
  }
}
