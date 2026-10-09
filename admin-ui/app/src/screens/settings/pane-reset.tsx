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
import { useT } from "@/lib/i18n-react";
import { EraseSection } from "./erase-section";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText } from "./rows";
import type { SettingsForm } from "./use-settings-form";

/* Factory reset: the only button on this screen that cannot be taken back.
 *
 * It is behind a dialog rather than a `confirm()` — the old plain-JS page
 * used the browser's, which a reader dismisses without reading because it
 * looks like every other one they have ever dismissed. The dialog below says
 * what survives, because that is the question someone about to press it
 * actually has, and the answer is the reassuring half: settings go, files
 * stay.
 *
 * The button stays dead until the settings have loaded, which is also what
 * keeps it out of reach behind the unlock prompt, and while a rebuild is
 * already running.
 *
 * Under it sits the erase (erase-section.tsx), which is the other kind of
 * reset: the data goes too. It has its own countdown and its own Cancel,
 * because lososd drives it whether or not this tab stays open.
 */

export function ResetPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const [confirming, setConfirming] = React.useState(false);
  const titleId = React.useId();
  const bodyId = React.useId();

  const blocked = form.locked || !form.ready || form.applying;

  return (
    <>
      <PaneSection>
        <GroupTitle>{t("panes.reset.startOver")}</GroupTitle>
        <Group className="border-crit/35">
          <Row last>
            <RowText
              title={t("panes.reset.title")}
              detail={t("panes.reset.detail")}
            />
            <Button variant="destructive" disabled={blocked} onClick={() => setConfirming(true)}>
              {t("panes.reset.button")}
            </Button>
          </Row>
        </Group>
        <GroupCaption>{t("panes.reset.caption")}</GroupCaption>
      </PaneSection>

      <EraseSection locked={form.locked} />

      <Dialog
        open={confirming}
        onOpenChange={setConfirming}
        labelledBy={titleId}
        describedBy={bodyId}
      >
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
            {t("panes.reset.dialogTitle")}
          </DialogTitle>
          <DialogDescription id={bodyId}>
            {t("panes.reset.dialogBody")}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <ul className="flex flex-col gap-1.5 text-[13px] leading-snug text-muted">
            <li className="flex gap-2">
              <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-crit" />
              {t("panes.reset.goes")}
            </li>
            <li className="flex gap-2">
              <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-ok" />
              {t("panes.reset.stays")}
            </li>
            <li className="flex gap-2">
              <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-faint" />
              {t("panes.reset.rebuilds")}
            </li>
          </ul>
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setConfirming(false)}>
            {t("panes.reset.keep")}
          </Button>
          <Button
            variant="destructive"
            onClick={() => {
              setConfirming(false);
              form.factoryReset();
            }}
          >
            {t("panes.reset.confirm")}
          </Button>
        </DialogFooter>
      </Dialog>
    </>
  );
}
