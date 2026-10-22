/* Choose files from LosOS Cloud, from any app: Mail attaches them, Chat
 * sends them, Diagrams opens one. One picker so every app picks the same
 * way, on the native <dialog> like every other modal here (the admin CSP
 * refuses the style elements a modal library would inject).
 *
 *   <CloudPicker open={picking} onOpenChange={setPicking} multiple
 *     onPick={(entries) => attach(entries)} />
 *
 * It walks folders with /cloud/api/files (lib/cloud-files.ts) as the
 * person, so it lists exactly what they could open in Cloud. `accept`
 * narrows what can be ticked; folders are always there to walk into. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, ArrowRight01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { formatBytes, iconFor } from "@/cloud/format";
import { listCloudFolder, type CloudEntry } from "@/lib/cloud-files";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

export interface CloudPickerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onPick: (entries: CloudEntry[]) => void;
  /** Several files at once. */
  multiple?: boolean;
  /** Which files can be picked; the rest are listed, not tickable. */
  accept?: (entry: CloudEntry) => boolean;
  /** The button's words, "Attach" by default. */
  action?: string;
  title?: string;
}

type Listing = { state: "loading" } | { state: "error" } | { state: "ready"; entries: CloudEntry[] };

const byName = (a: CloudEntry, b: CloudEntry): number =>
  a.dir === b.dir ? a.name.localeCompare(b.name, undefined, { numeric: true }) : a.dir ? -1 : 1;

export function CloudPicker({ open, onOpenChange, onPick, multiple = false, accept, action, title }: CloudPickerProps) {
  const t = useT();
  const id = React.useId();
  const [folder, setFolder] = React.useState("");
  const [listing, setListing] = React.useState<Listing>({ state: "loading" });
  const [picked, setPicked] = React.useState<Map<string, CloudEntry>>(new Map());

  React.useEffect(() => {
    if (!open) {
      setPicked(new Map());
      return;
    }
    const ctl = new AbortController();
    setListing({ state: "loading" });
    listCloudFolder(folder, ctl.signal).then(
      (r) => setListing({ state: "ready", entries: [...r.entries].sort(byName) }),
      (err: unknown) => {
        if (!(err instanceof DOMException && err.name === "AbortError")) setListing({ state: "error" });
      },
    );
    return () => ctl.abort();
  }, [open, folder]);

  const takes = (e: CloudEntry): boolean => !e.dir && (accept?.(e) ?? true);

  const toggle = (e: CloudEntry): void =>
    setPicked((current) => {
      const next = new Map(multiple ? current : []);
      if (current.has(e.path)) next.delete(e.path);
      else next.set(e.path, e);
      return next;
    });

  const done = (): void => {
    if (picked.size === 0) return;
    onPick([...picked.values()]);
    onOpenChange(false);
  };

  const parts = folder === "" ? [] : folder.split("/");

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      labelledBy={`${id}-title`}
      describedBy={`${id}-where`}
      dialogClassName="w-[min(34rem,calc(100vw-2rem))]"
      data-slot="cloud-picker"
    >
      <DialogHeader>
        <DialogTitle id={`${id}-title`}>{title ?? t("together.picker.title")}</DialogTitle>
        <DialogDescription id={`${id}-where`} className="flex min-w-0 flex-wrap items-center gap-1">
          {parts.length > 0 && (
            <button
              type="button"
              className="-ml-1 inline-flex size-6 items-center justify-center rounded-control text-muted hover:bg-sunk hover:text-ink"
              aria-label={t("together.picker.up")}
              onClick={() => setFolder(parts.slice(0, -1).join("/"))}
            >
              <HugeiconsIcon icon={ArrowLeft01Icon} size={14} strokeWidth={1.5} color="currentColor" />
            </button>
          )}
          <button type="button" className="hover:text-ink" onClick={() => setFolder("")}>
            {t("together.picker.home")}
          </button>
          {parts.map((part, i) => (
            <React.Fragment key={i}>
              <HugeiconsIcon icon={ArrowRight01Icon} size={12} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              <button type="button" className="truncate hover:text-ink" onClick={() => setFolder(parts.slice(0, i + 1).join("/"))}>
                {part}
              </button>
            </React.Fragment>
          ))}
        </DialogDescription>
      </DialogHeader>
      <DialogBody className="px-2 py-1">
        <div className="h-[min(22rem,55dvh)] overflow-y-auto">
          {listing.state === "loading" && (
            <div className="flex h-full items-center justify-center">
              <Spinner label={t("together.picker.loading")} />
            </div>
          )}
          {listing.state === "error" && <p className="px-3 py-6 text-[13px] text-muted">{t("together.picker.failed")}</p>}
          {listing.state === "ready" && listing.entries.length === 0 && (
            <p className="px-3 py-6 text-[13px] text-muted">{t("together.picker.empty")}</p>
          )}
          {listing.state === "ready" && listing.entries.length > 0 && (
            <ul role="listbox" aria-multiselectable={multiple} aria-label={t("together.picker.title")}>
              {listing.entries.map((e) => {
                const on = picked.has(e.path);
                const can = takes(e);
                return (
                  <li key={e.path} role="option" aria-selected={on} aria-disabled={!e.dir && !can}>
                    <button
                      type="button"
                      data-entry={e.name}
                      disabled={!e.dir && !can}
                      className={cn(
                        "flex w-full items-center gap-3 rounded-control px-3 py-2 text-left text-[13px] transition-colors",
                        "hover:bg-sunk disabled:cursor-default disabled:opacity-50 disabled:hover:bg-transparent",
                        on && "bg-accent-wash text-ink hover:bg-accent-wash",
                      )}
                      onClick={() => (e.dir ? setFolder(e.path) : toggle(e))}
                      onDoubleClick={() => {
                        if (!e.dir && can) {
                          onPick([e]);
                          onOpenChange(false);
                        }
                      }}
                    >
                      <HugeiconsIcon icon={iconFor(e)} size={18} strokeWidth={1.5} color="currentColor" className="shrink-0 text-muted" />
                      <span className="min-w-0 flex-1 truncate">{e.name}</span>
                      {!e.dir && <span className="shrink-0 text-faint tabular-nums">{formatBytes(e.size)}</span>}
                      {e.dir && <HugeiconsIcon icon={ArrowRight01Icon} size={14} strokeWidth={1.5} color="currentColor" className="text-faint" />}
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </DialogBody>
      <DialogFooter>
        <span className="mr-auto text-[12px] text-muted" aria-live="polite">
          {picked.size > 0 ? t("together.picker.picked", { count: picked.size }) : ""}
        </span>
        <Button variant="ghost" onClick={() => onOpenChange(false)}>
          {t("together.cancel")}
        </Button>
        <Button disabled={picked.size === 0} onClick={done} data-action="pick">
          {action ?? t("together.picker.attach")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
