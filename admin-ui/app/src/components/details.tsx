import * as React from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { useT } from "@/lib/i18n-react";
import { usePictureSettings } from "@/lib/pictures";
import { detailSpans, findDetails, type Detail, type DetailKind } from "@/lib/sensitive";

/* Private details in Chat and Mail (lib/sensitive.ts): card, IBAN,
 * passport and ID card, birth and Social Security numbers. Where a message
 * is drawn they are black bars until pressed, and before one goes out the
 * person is asked. Both follow the person's `detailsCheck` (on unless they
 * turn it off; always on for a child's account). Everything happens in
 * this page; the box never sees what was found. */

/** Whether private details are blacked out and asked about. On while the
 *  settings are still loading. */
export function useDetailsOn(): boolean {
  return usePictureSettings()?.detailsCheck ?? true;
}

function Bar({ value, kind }: { value: string; kind: DetailKind }) {
  const t = useT();
  const [shown, setShown] = React.useState(false);
  const name = t(`details.kind.${kind}`);
  return (
    <button
      type="button"
      data-detail={kind}
      data-shown={shown ? "" : undefined}
      onClick={(event) => {
        event.stopPropagation();
        setShown((s) => !s);
      }}
      aria-label={t(shown ? "details.shown" : "details.hidden", { kind: name })}
      title={shown ? undefined : name}
      className={
        shown
          ? "rounded-[3px] bg-sunk px-0.5 font-mono text-[0.95em] break-all text-ink outline-offset-1 transition-colors duration-150"
          : "rounded-[3px] bg-black px-0.5 font-mono text-[0.95em] text-transparent shadow-[inset_0_0_0_1px_rgb(255_255_255/0.14)] select-none outline-offset-1 transition-colors duration-150 hover:bg-black/85"
      }
    >
      {/* Not the number itself, so it is neither read out nor copied:
          figure spaces as wide as its digits. */}
      {shown ? value : " ".repeat(value.length)}
    </button>
  );
}

/* A bar inside something that is itself pressed (a row of a list): black,
 * and nothing more. */
function StillBar({ value }: { value: string }) {
  return (
    <span aria-hidden className="rounded-[3px] bg-black px-0.5 font-mono text-[0.95em] text-transparent select-none">
      {"\u2007".repeat(value.length)}
    </span>
  );
}

/** `text` with every private detail in it drawn as a black bar, pressed to
 *  show it; `still` for a bar that cannot be pressed. */
export function Masked({ text, still = false }: { text: string; still?: boolean }) {
  const on = useDetailsOn();
  const spans = React.useMemo(() => (on ? detailSpans(text) : []), [on, text]);
  if (spans.length === 0) return <>{text}</>;
  const parts: React.ReactNode[] = [];
  let last = 0;
  for (const span of spans) {
    if (span.start > last) parts.push(text.slice(last, span.start));
    const value = text.slice(span.start, span.end);
    parts.push(still ? <StillBar key={span.start} value={value} /> : <Bar key={span.start} kind={span.kind} value={value} />);
    last = span.end;
  }
  if (last < text.length) parts.push(text.slice(last));
  return <>{parts}</>;
}

function line(t: ReturnType<typeof useT>, d: Detail): string {
  const kind = t(`details.kind.${d.kind}`);
  return d.last === "" ? kind : t("details.ending", { kind, last: d.last });
}

/** "Send anyway?" for a message with private details in its text. */
export function useDetailsCheck(): { check: (text: string) => Promise<boolean>; dialog: React.ReactNode } {
  const t = useT();
  const settings = usePictureSettings();
  const on = settings?.detailsCheck ?? true;
  const child = settings?.child === true;
  const [asking, setAsking] = React.useState<{ found: Detail[]; answer: (send: boolean) => void } | null>(null);

  const check = React.useCallback(
    async (text: string): Promise<boolean> => {
      if (!on) return true;
      const found = findDetails(text);
      if (found.length === 0) return true;
      return new Promise<boolean>((resolve) => setAsking({ found, answer: resolve }));
    },
    [on],
  );

  const answer = (send: boolean) => {
    asking?.answer(send);
    setAsking(null);
  };

  const dialog = (
    <Dialog open={asking !== null} onOpenChange={(open) => !open && answer(false)} labelledBy="details-check-title" describedBy="details-check-body">
      <DialogHeader>
        <DialogTitle id="details-check-title">{t("details.send.title")}</DialogTitle>
        <DialogDescription id="details-check-body">{child ? t("details.send.childBody") : t("details.send.body")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        <ul className="flex flex-col gap-1.5" data-testid="details-found">
          {asking?.found.map((d, i) => (
            <li key={i} className="flex items-center gap-2 text-[13px] text-ink">
              <span aria-hidden className="inline-block h-3.5 w-14 shrink-0 rounded-[3px] bg-black" />
              {line(t, d)}
            </li>
          ))}
        </ul>
        <p className="text-[13px] text-muted">{t("details.send.private")}</p>
      </DialogBody>
      <DialogFooter>
        <Button variant="secondary" onClick={() => answer(true)} data-send-anyway>
          {t("details.send.anyway")}
        </Button>
        <Button autoFocus onClick={() => answer(false)}>
          {t("details.send.cancel")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
  return { check, dialog };
}
