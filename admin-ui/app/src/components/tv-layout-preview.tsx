import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

/* A miniature still of the leanback shell: wide bar, large tiles and a
 * visible focus ring on the middle tile, so the person menu can show what
 * TV layout looks like before they force it on a desktop browser. */

export function TvLayoutPreview({ className }: { className?: string }) {
  const t = useT();
  return (
    <div
      className={cn(
        "rounded-control border border-line bg-sunk p-2.5",
        "text-[10px] leading-tight text-muted",
        className,
      )}
      data-testid="tv-layout-preview"
      aria-hidden="true"
    >
      <p className="mb-1.5 font-medium text-ink">{t("ui.tv.previewTitle")}</p>
      <div className="overflow-hidden rounded-[4px] border border-line bg-surface">
        <div className="h-2 border-b border-line bg-sunk" />
        <div className="grid grid-cols-3 gap-1.5 p-2">
          <PreviewTile />
          <PreviewTile focused />
          <PreviewTile />
        </div>
      </div>
      <p className="mt-1.5">{t("ui.tv.previewHint")}</p>
    </div>
  );
}

function PreviewTile({ focused = false }: { focused?: boolean }) {
  return (
    <div
      className={cn(
        "flex min-h-[2.25rem] flex-col justify-end rounded-[3px] bg-accent-wash p-1",
        focused && "ring-2 ring-accent ring-offset-1 ring-offset-surface",
      )}
    >
      <span className="block h-1 w-[70%] rounded-sm bg-muted/50" />
      <span className="mt-0.5 block h-1.5 w-full rounded-sm bg-ink/20" />
    </div>
  );
}
