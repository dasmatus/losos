/* Page setup: paper size, orientation and margins, written into the
 * document's front matter (page.ts) so the page, the print and an export
 * all follow it. */

import * as React from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { useT } from "@/lib/i18n-react";
import { MARGINS, ORIENTATIONS, SIZES, type PageSetup } from "./page";

export function PageSetupDialog({
  open,
  onOpenChange,
  page,
  onApply,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  page: PageSetup;
  onApply: (page: PageSetup) => void;
}) {
  const t = useT();
  const id = React.useId();
  const [draft, setDraft] = React.useState(page);
  React.useEffect(() => {
    if (open) setDraft(page);
  }, [open, page]);

  const row = (key: keyof PageSetup, label: string, options: readonly string[], word: (v: string) => string) => (
    <label className="flex items-center justify-between gap-4 text-[13px]">
      <span>{label}</span>
      <NativeSelect
        className="w-44"
        value={draft[key]}
        data-field={key}
        onChange={(e) => setDraft((d) => ({ ...d, [key]: e.target.value }))}
      >
        {options.map((o) => (
          <NativeSelectOption key={o} value={o}>
            {word(o)}
          </NativeSelectOption>
        ))}
      </NativeSelect>
    </label>
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy={`${id}-title`} data-testid="paper-page-setup">
      <DialogHeader>
        <DialogTitle id={`${id}-title`}>{t("paper.pageSetup")}</DialogTitle>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        {row("size", t("paper.page.size"), SIZES, (v) => v)}
        {row("orientation", t("paper.page.orientation"), ORIENTATIONS, (v) => t(v === "portrait" ? "paper.page.portrait" : "paper.page.landscape"))}
        {row("margin", t("paper.page.margin"), MARGINS, (v) => t(v === "narrow" ? "paper.page.narrow" : v === "wide" ? "paper.page.wide" : "paper.page.normal"))}
      </DialogBody>
      <DialogFooter>
        <Button variant="secondary" onClick={() => onOpenChange(false)}>
          {t("cloud.cancel")}
        </Button>
        <Button
          onClick={() => {
            onApply(draft);
            onOpenChange(false);
          }}
          data-action="apply"
        >
          {t("paper.page.apply")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
