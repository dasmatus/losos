import * as React from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/progress";
import { useT } from "@/lib/i18n-react";
import { CapacityMeter } from "./capacity-meter";
import { formatBytes } from "./format";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow } from "./rows";
import type { SettingsForm } from "./use-settings-form";
import type { Storage } from "./use-storage";

/* The disk only: how big it is, what it holds, and the reserve. Sharing it
 * with the mesh used to be the second group here and now lives on the Market
 * pane (pane-market.tsx), because lending disk to other boxes and being paid
 * for it are one decision, and the market is where that decision is made.
 * `form` stays in the props: Apply still writes every pane's slice at once. */
export interface StoragePaneProps {
  form: SettingsForm;
  storage: Storage;
}

export function StoragePane({ form, storage }: StoragePaneProps) {
  const t = useT();
  const [confirming, setConfirming] = React.useState(false);
  const titleId = React.useId();
  const bodyId = React.useId();

  /* The reserve comes from the box (GET /api/storage), so once it has been
   * claimed the button says so in every tab, after a reload too. Unknown is
   * not "available": the button stays off until the box reports space. */
  const reserve = storage.facts.reserveBytes;
  const hasReserve = reserve !== null && reserve > 0;
  const spent = reserve === 0 && !storage.growing;

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
              detail={t(spent ? "panes.storage.held.spent" : "panes.storage.held.detail")}
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
                {storage.growing
                  ? t("panes.storage.claiming")
                  : spent
                    ? t("panes.storage.reserveUsed")
                    : t("panes.storage.useReserve")}
              </Button>
            </div>
          </Row>

        </Group>
        <GroupCaption>{t("panes.storage.diskCaption")}</GroupCaption>
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

