import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { LinkSquare02Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import { ApiError, postReport, type ReportReason, type ReportTarget } from "@/lib/api";
import { handbookHref } from "@/lib/handbook";
import { useT } from "@/lib/i18n-react";

/* "Report" on a Chat message or a Mail message. It says, first, how to
 * reach the police or a hotline, because a picture of a child being abused
 * is a crime to report there, not something for the box to judge. Then it
 * can tell the box's moderators, with a reference to the message and never
 * the message itself: in an encrypted room the box cannot read it anyway,
 * and nobody should be shown such a picture to judge it.
 *
 * One host is mounted by the shell; `openReport` opens it from anywhere. */

const REASONS: readonly ReportReason[] = ["child", "sexual", "abuse", "spam", "other"];

let open: ((target: ReportTarget) => void) | null = null;

/** Open the report dialog about one message. */
export function openReport(target: ReportTarget): void {
  open?.(target);
}

export function ReportDialogHost() {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const [target, setTarget] = React.useState<ReportTarget | null>(null);
  const [reason, setReason] = React.useState<ReportReason>("child");
  const [note, setNote] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [noModeration, setNoModeration] = React.useState(false);

  React.useEffect(() => {
    open = (next) => {
      setTarget(next);
      setReason("child");
      setNote("");
      setNoModeration(false);
    };
    return () => {
      open = null;
    };
  }, []);

  const send = async () => {
    if (target === null) return;
    setBusy(true);
    try {
      await postReport({ ...target, reason, note: note.trim() });
      toast.done(t("report.sent"));
      setTarget(null);
    } catch (failure) {
      // A box without the reports route answers 404 or 405, or, through
      // nginx's sections for someone who is not an administrator, 403.
      if (failure instanceof ApiError && [403, 404, 405].includes(failure.status)) setNoModeration(true);
      else toast.error(t("report.failed"), failure instanceof Error ? failure.message : undefined, { help: "pictures" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={target !== null}
      onOpenChange={(next) => !next && setTarget(null)}
      labelledBy={titleId}
      describedBy={bodyId}
      dialogClassName="w-[min(32rem,calc(100vw-2rem))]"
    >
      <DialogHeader>
        <DialogTitle id={titleId}>{t("report.title")}</DialogTitle>
        <DialogDescription id={bodyId}>{t("report.intro")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-4 text-[13.5px]" data-testid="report-dialog">
        <section className="flex flex-col gap-2 rounded-card border border-line bg-sunk p-3">
          <h3 className="font-medium text-ink">{t("report.child.title")}</h3>
          <p className="text-muted">{t("report.child.dont")}</p>
          <ul className="flex flex-col gap-1">
            <li>{t("report.child.danger")}</li>
            <li>{t("report.child.hotline")}</li>
            <li>{t("report.child.helpline")}</li>
          </ul>
          <a
            href={handbookHref("pictures")}
            target="_blank"
            rel="noopener"
            className="flex w-fit items-center gap-1 text-accent underline-offset-4 hover:underline"
            data-testid="report-handbook"
          >
            {t("report.child.where")}
            <HugeiconsIcon icon={LinkSquare02Icon} className="size-3.5" aria-hidden />
          </a>
        </section>
        <section className="flex flex-col gap-2">
          <h3 className="font-medium text-ink">{t("report.box.title")}</h3>
          <p className="text-muted">{t("report.box.detail")}</p>
          <NativeSelect
            aria-label={t("report.box.reason")}
            value={reason}
            onChange={(event) => {
              const found = REASONS.find((r) => r === event.target.value);
              if (found !== undefined) setReason(found);
            }}
          >
            {REASONS.map((r) => (
              <NativeSelectOption key={r} value={r}>
                {t(`report.reason.${r}`)}
              </NativeSelectOption>
            ))}
          </NativeSelect>
          <Textarea
            aria-label={t("report.box.note")}
            placeholder={t("report.box.note")}
            maxLength={500}
            rows={2}
            value={note}
            onChange={(event) => setNote(event.target.value)}
          />
          {noModeration && <p className="text-warn">{t("report.box.none")}</p>}
        </section>
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={() => setTarget(null)}>
          {t("report.close")}
        </Button>
        <Button disabled={busy || noModeration} onClick={() => void send()} data-testid="report-send">
          {t("report.send")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
