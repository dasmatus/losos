import * as React from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { useT } from "@/lib/i18n-react";
import { flaggedBeforeSending } from "@/lib/nudity";
import { usePictureSettings } from "@/lib/pictures";

/* "Send anyway?" for a picture the nudity warning flags, asked before it is
 * encrypted and uploaded. Only while the person's warning is on (always, on
 * an account marked as a child's). The check runs in this page; nothing is
 * told to the box or to anyone else, whichever way the person answers. */

const PICTURE = /^image\/(png|jpeg|gif|webp|avif)$/;

export function useSendCheck(): { check: (file: Blob) => Promise<boolean>; dialog: React.ReactNode } {
  const t = useT();
  const settings = usePictureSettings();
  const [asking, setAsking] = React.useState<{ answer: (send: boolean) => void } | null>(null);
  const on = settings?.nudityCheck === true;
  const child = settings?.child === true;

  const check = React.useCallback(
    async (file: Blob): Promise<boolean> => {
      if (!on || !PICTURE.test(file.type)) return true;
      if (!(await flaggedBeforeSending(file))) return true;
      return new Promise<boolean>((resolve) => setAsking({ answer: resolve }));
    },
    [on],
  );

  const answer = (send: boolean) => {
    asking?.answer(send);
    setAsking(null);
  };

  const dialog = (
    <Dialog open={asking !== null} onOpenChange={(open) => !open && answer(false)} labelledBy="send-check-title" describedBy="send-check-body">
      <DialogHeader>
        <DialogTitle id="send-check-title">{t("pictures.send.title")}</DialogTitle>
        <DialogDescription id="send-check-body">{child ? t("pictures.send.childBody") : t("pictures.send.body")}</DialogDescription>
      </DialogHeader>
      <DialogBody>
        <p className="text-[13px] text-muted">{t("pictures.send.private")}</p>
      </DialogBody>
      <DialogFooter>
        <Button variant="secondary" onClick={() => answer(true)} data-send-anyway>
          {t("pictures.send.anyway")}
        </Button>
        <Button autoFocus onClick={() => answer(false)}>
          {t("pictures.send.cancel")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
  return { check, dialog };
}
