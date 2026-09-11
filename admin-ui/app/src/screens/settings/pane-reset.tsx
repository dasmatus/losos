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
 */

export function ResetPane({ form }: { form: SettingsForm }) {
  const [confirming, setConfirming] = React.useState(false);
  const titleId = React.useId();
  const bodyId = React.useId();

  const blocked = form.locked || !form.ready || form.applying;

  return (
    <>
      <PaneSection>
        <GroupTitle>Start over</GroupTitle>
        <Group className="border-crit/35">
          <Row last>
            <RowText
              title="Put every setting back"
              detail="The name, the network, the mesh, the apps. All of it, back to the way this box came."
            />
            <Button variant="destructive" disabled={blocked} onClick={() => setConfirming(true)}>
              Reset…
            </Button>
          </Row>
        </Group>
        <GroupCaption>
          Your files, your photos and your repositories are not touched. This only undoes the
          choices made on this screen. The box rebuilds itself afterwards and comes back under its
          original name, so the address you use to reach it will change back too.
        </GroupCaption>
      </PaneSection>

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
            Put every setting back?
          </DialogTitle>
          <DialogDescription id={bodyId}>
            This cannot be undone from here, and there is no other way into this box.
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <ul className="flex flex-col gap-1.5 text-[13px] leading-snug text-muted">
            <li className="flex gap-2">
              <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-crit" />
              Every setting goes back to its original value, including the name this box answers to.
            </li>
            <li className="flex gap-2">
              <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-ok" />
              Your files and repositories stay exactly where they are.
            </li>
            <li className="flex gap-2">
              <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-faint" />
              The box rebuilds itself, which takes a few minutes. It stays reachable while it works.
            </li>
          </ul>
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setConfirming(false)}>
            Keep my settings
          </Button>
          <Button
            variant="destructive"
            onClick={() => {
              setConfirming(false);
              form.factoryReset();
            }}
          >
            Put everything back
          </Button>
        </DialogFooter>
      </Dialog>
    </>
  );
}
