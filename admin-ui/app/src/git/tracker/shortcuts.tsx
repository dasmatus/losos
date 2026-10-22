import * as React from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";

/* The keys of the issues pages, behind ?. */

const KEYS: [string[], MessageKey][] = [
  [["C"], "tracker.shortcuts.create"],
  [["J", "K"], "tracker.shortcuts.move"],
  [["Enter"], "tracker.shortcuts.open"],
  [["X"], "tracker.shortcuts.select"],
  [["S"], "tracker.shortcuts.state"],
  [["P"], "tracker.shortcuts.priority"],
  [["0", "4"], "tracker.shortcuts.priorityKeys"],
  [["A"], "tracker.shortcuts.assignee"],
  [["L"], "tracker.shortcuts.labels"],
  [["B"], "tracker.shortcuts.board"],
  [["/"], "tracker.shortcuts.search"],
  [["Ctrl", "K"], "tracker.shortcuts.palette"],
  [["Esc"], "tracker.shortcuts.close"],
];

export function Shortcuts({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useT();
  const id = React.useId();
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()} labelledBy={`${id}-title`}>
      <DialogHeader>
        <DialogTitle id={`${id}-title`}>{t("tracker.shortcuts.title")}</DialogTitle>
      </DialogHeader>
      <DialogBody>
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-[13.5px]" data-shortcuts>
          {KEYS.map(([keys, what]) => (
            <React.Fragment key={what}>
              <dt className="flex gap-1">
                {keys.map((k, i) => (
                  <React.Fragment key={k}>
                    {i > 0 && <span className="text-faint">{what === "tracker.shortcuts.priorityKeys" ? "–" : what === "tracker.shortcuts.palette" ? "+" : "/"}</span>}
                    <kbd className="min-w-6 rounded-control border border-line bg-sunk px-1.5 text-center font-mono text-[12px]">{k}</kbd>
                  </React.Fragment>
                ))}
              </dt>
              <dd className="text-muted">{t(what)}</dd>
            </React.Fragment>
          ))}
        </dl>
      </DialogBody>
      <DialogFooter>
        <Button variant="secondary" onClick={onClose}>
          {t("git.close")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
