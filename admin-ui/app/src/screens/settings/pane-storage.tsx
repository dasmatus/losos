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
        <GroupTitle>This box&apos;s disk</GroupTitle>
        <Group>
          <StackRow>
            <CapacityMeter facts={storage.facts} source={storage.source} />
          </StackRow>

          <Row>
            <RowText
              title="Held back for later"
              detail="Space this box did not claim when it was set up."
            />
            <div className="flex items-center gap-2.5">
              <RowValue>{reserve === null ? "not reported" : formatBytes(reserve)}</RowValue>
              <Button
                variant="secondary"
                size="sm"
                disabled={form.locked || storage.growing || !hasReserve}
                onClick={() => setConfirming(true)}
              >
                {storage.growing ? <Spinner size={15} label="Claiming the reserve" /> : null}
                Use reserve…
              </Button>
            </div>
          </Row>

          {storage.outcome !== null && (
            <StackRow last>
              <GrowReport outcome={storage.outcome} onDismiss={storage.dismissOutcome} />
            </StackRow>
          )}
        </Group>
        <GroupCaption>
          This box deliberately left part of its disk unclaimed so it could be made bigger later
          without opening the case. Using the reserve happens while everything keeps running, and it
          only goes one way. The disk cannot be made smaller again afterwards.
        </GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>The mesh</GroupTitle>
        <Group>
          <Row>
            <RowText htmlFor={shareId} title="Share this box’s disk with the mesh" />
            <Switch
              id={shareId}
              checked={sharing}
              disabled={form.locked || !form.ready}
              onCheckedChange={(next) => form.set("sharingMyStorage", next)}
            />
          </Row>
          <Row last>
            <RowText title="If this disk dies" />
            <RowValue className={sharing ? "text-ok" : undefined}>
              {sharing ? "rebuilds in about 40 minutes" : "nothing here is copied anywhere"}
            </RowValue>
          </Row>
        </Group>
        <GroupCaption>
          Your box lends its spare room to other people&apos;s boxes, and copies of your own files
          are kept on theirs. The two travel together, because a pool you take from but never give
          to is not a pool. While this is off, the shared half of the disk stays locked and
          unreadable by anything on this box, including the box itself.
        </GroupCaption>
      </PaneSection>

      <Dialog
        open={confirming}
        onOpenChange={setConfirming}
        labelledBy={titleId}
        describedBy={bodyId}
      >
        <DialogHeader>
          <DialogTitle id={titleId}>Use the space held back?</DialogTitle>
          <DialogDescription id={bodyId}>
            {reserve === null
              ? "This box has not said how much it is holding back. It will take whatever is there."
              : `This adds about ${formatBytes(reserve)} to this box's storage.`}{" "}
            It happens while everything keeps running, takes a few minutes, and cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setConfirming(false)}>
            Cancel
          </Button>
          <Button
            onClick={() => {
              setConfirming(false);
              storage.grow();
            }}
          >
            Use it
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
        Dismiss
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
        title: "This box has more room",
        detail: `${formatBytes(outcome.beforeBytes)} before, ${formatBytes(outcome.afterBytes)} now.`,
      };
    case "nothing":
      return {
        icon: InformationCircleIcon,
        tone: "text-muted",
        title: "Nothing changed",
        detail:
          outcome.claimedBytes > 0
            ? `${formatBytes(outcome.claimedBytes)} was taken from the spare space, but the storage itself did not end up any bigger.`
            : "There was no space left to take. The disk is already using all of itself.",
      };
    case "failed":
      return {
        icon: Alert02Icon,
        tone: "text-crit",
        title: "That did not work",
        detail: outcome.message,
      };
    default: {
      const exhaustive: never = outcome;
      return exhaustive;
    }
  }
}
