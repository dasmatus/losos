import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, ArrowRight01Icon, Cancel01Icon, Download01Icon, Image01Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Spinner } from "@/components/ui/progress";
import { useT } from "@/lib/i18n-react";
import { cloud, contentUrl, hasPreview, isAbort, parentOf, previewUrl, type Entry } from "./api";
import { explain } from "./dialogs";
import { formatFull, formatMonth } from "./format";
import { useRevision } from "./store";

/** Every picture in the cloud, newest first, by month. */
export function PhotosView() {
  const t = useT();
  const revision = useRevision();
  const [photos, setPhotos] = React.useState<Entry[] | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [open, setOpen] = React.useState<number | null>(null);

  React.useEffect(() => {
    const ctl = new AbortController();
    cloud.media(ctl.signal).then(
      (r) => setPhotos(r.entries.filter(hasPreview).sort((a, b) => b.mtime - a.mtime)),
      (err: unknown) => {
        if (!isAbort(err)) setError(explain(err));
      },
    );
    return () => ctl.abort();
  }, [revision]);

  const months = React.useMemo(() => {
    const out: { label: string; items: { entry: Entry; index: number }[] }[] = [];
    (photos ?? []).forEach((entry, index) => {
      const label = formatMonth(entry.mtime);
      const last = out[out.length - 1];
      if (last !== undefined && last.label === label) last.items.push({ entry, index });
      else out.push({ label, items: [{ entry, index }] });
    });
    return out;
  }, [photos]);

  return (
    <div className="flex flex-col gap-4">
      <h1 className="flex min-h-9 items-center text-[19px] leading-tight font-semibold tracking-tight">{t("cloud.nav.photos")}</h1>
      {error !== null ? (
        <Alert variant="crit">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : photos === null ? (
        <div className="grid min-h-60 place-items-center">
          <Spinner />
        </div>
      ) : photos.length === 0 ? (
        <Empty className="min-h-72">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={Image01Icon} size={22} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("cloud.photos.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("cloud.photos.emptyBody")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        months.map((month) => (
          <section key={month.label} aria-label={month.label} className="flex flex-col gap-2">
            <h2 className="text-[13px] font-medium text-muted">{month.label}</h2>
            <ul className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-1.5" data-photos>
              {month.items.map(({ entry, index }) => (
                <li key={entry.id}>
                  <button
                    type="button"
                    onClick={() => setOpen(index)}
                    className="block aspect-square w-full overflow-hidden rounded-[8px] bg-sunk focus-visible:ring-2 focus-visible:ring-accent focus-visible:outline-none"
                    aria-label={entry.name}
                  >
                    <img src={previewUrl(entry, 256)} alt="" loading="lazy" className="size-full object-cover transition-transform duration-200 hover:scale-[1.03]" />
                  </button>
                </li>
              ))}
            </ul>
          </section>
        ))
      )}
      <Viewer photos={photos ?? []} index={open} onIndexChange={setOpen} />
    </div>
  );
}

function Viewer({ photos, index, onIndexChange }: { photos: readonly Entry[]; index: number | null; onIndexChange: (i: number | null) => void }) {
  const t = useT();
  const entry = index === null ? undefined : photos[index];
  const go = React.useCallback(
    (step: number) => {
      if (index === null) return;
      const next = index + step;
      if (next >= 0 && next < photos.length) onIndexChange(next);
    },
    [index, photos.length, onIndexChange],
  );

  React.useEffect(() => {
    if (index === null) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") go(-1);
      if (event.key === "ArrowRight") go(1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [index, go]);

  const icon = (glyph: typeof Cancel01Icon) => <HugeiconsIcon icon={glyph} size={18} strokeWidth={1.5} color="currentColor" aria-hidden="true" />;

  return (
    <Dialog
      open={entry !== undefined}
      onOpenChange={(o) => !o && onIndexChange(null)}
      labelledBy="cloud-viewer-title"
      dialogClassName="w-[min(64rem,calc(100vw-2rem))] max-w-[calc(100vw-2rem)]"
      className="bg-ground"
    >
      {entry !== undefined && (
        <div className="flex flex-col">
          <div className="flex items-center gap-2 border-b border-hair px-4 py-2.5">
            <div className="min-w-0 flex-1">
              <h2 id="cloud-viewer-title" className="truncate text-sm font-medium">
                {entry.name}
              </h2>
              <p className="truncate text-[12px] text-muted">
                {parentOf(entry.path) || t("cloud.nav.files")} · {formatFull(entry.mtime)}
              </p>
            </div>
            <Button size="icon-sm" variant="ghost" aria-label={t("cloud.download")} title={t("cloud.download")} onClick={() => (window.location.href = contentUrl(entry.path, true))}>
              {icon(Download01Icon)}
            </Button>
            <Button size="icon-sm" variant="ghost" aria-label={t("cloud.close")} title={t("cloud.close")} onClick={() => onIndexChange(null)}>
              {icon(Cancel01Icon)}
            </Button>
          </div>
          <div className="relative grid min-h-[50vh] place-items-center bg-sunk">
            <img src={previewUrl(entry, 1024)} alt={entry.name} className="max-h-[75vh] max-w-full object-contain" />
            <Button
              size="icon"
              variant="secondary"
              className="absolute top-1/2 left-3 -translate-y-1/2"
              aria-label={t("cloud.photos.previous")}
              disabled={index === 0}
              onClick={() => go(-1)}
            >
              {icon(ArrowLeft01Icon)}
            </Button>
            <Button
              size="icon"
              variant="secondary"
              className="absolute top-1/2 right-3 -translate-y-1/2"
              aria-label={t("cloud.photos.next")}
              disabled={index === photos.length - 1}
              onClick={() => go(1)}
            >
              {icon(ArrowRight01Icon)}
            </Button>
          </div>
        </div>
      )}
    </Dialog>
  );
}
