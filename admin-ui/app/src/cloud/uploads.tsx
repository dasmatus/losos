import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon, CheckmarkCircle02Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { formatBytes } from "./format";
import { cancel, clearFinished, keepBoth, replace, retry, useUploads, type Upload } from "./store";

/** The uploads in the corner: each one's progress, and the question when a
 * file of that name is already there. */
export function UploadsPanel() {
  const t = useT();
  const uploads = useUploads();
  if (uploads.length === 0) return null;
  const active = uploads.filter((u) => u.state === "sending" || u.state === "waiting").length;
  const sent = uploads.reduce((n, u) => n + (u.state === "done" ? u.total : u.state === "sending" ? u.sent : 0), 0);
  const total = uploads.filter((u) => u.state !== "cancelled").reduce((n, u) => n + u.total, 0);

  return (
    <section
      aria-label={t("cloud.upload.panel")}
      aria-live="polite"
      data-uploads
      className="fixed right-4 bottom-[calc(1rem+var(--bottom-nav))] z-30 flex max-h-[60dvh] w-[min(24rem,calc(100vw-2rem))] flex-col rounded-card border border-line bg-surface shadow-pop"
    >
      <header className="flex items-center gap-2 border-b border-hair px-4 py-2.5">
        <p className="flex-1 text-[13px] font-medium">
          {active > 0 ? t("cloud.upload.sending", { count: active }) : t("cloud.upload.finished")}
        </p>
        <Button size="icon-xs" variant="ghost" aria-label={t("cloud.close")} title={t("cloud.close")} onClick={clearFinished} disabled={active > 0}>
          <HugeiconsIcon icon={Cancel01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
      </header>
      {active > 0 && total > 0 && <Progress value={(sent / total) * 100} label={t("cloud.upload.panel")} className="mx-4 mt-2" />}
      <ul className="min-h-0 overflow-y-auto px-2 py-1.5">
        {uploads.map((u) => (
          <Row key={u.id} u={u} />
        ))}
      </ul>
    </section>
  );
}

function Row({ u }: { u: Upload }) {
  const t = useT();
  return (
    <li className="flex flex-col gap-1 rounded-control px-2 py-1.5">
      <div className="flex items-center gap-2">
        <span className={cn("min-w-0 flex-1 truncate text-[13px]", u.state === "cancelled" && "text-faint line-through")}>{u.name}</span>
        {u.state === "done" ? (
          <HugeiconsIcon icon={CheckmarkCircle02Icon} size={16} strokeWidth={1.5} color="currentColor" className="text-ok" aria-label={t("cloud.upload.done")} />
        ) : u.state === "sending" || u.state === "waiting" ? (
          <>
            <span className="numeric text-[12px] text-faint">{u.state === "waiting" ? t("cloud.upload.waiting") : `${formatBytes(u.sent)} / ${formatBytes(u.total)}`}</span>
            <Button size="icon-xs" variant="ghost" aria-label={t("cloud.upload.cancel", { name: u.name })} onClick={() => cancel(u.id)}>
              <HugeiconsIcon icon={Cancel01Icon} size={12} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </Button>
          </>
        ) : u.state === "failed" ? (
          <Button size="xs" variant="ghost" className="text-ink" onClick={() => retry(u.id)}>
            {t("cloud.upload.retry")}
          </Button>
        ) : null}
      </div>
      {u.state === "sending" && <Progress value={u.total > 0 ? (u.sent / u.total) * 100 : null} label={u.name} />}
      {u.state === "failed" && <p className="text-[12px] text-crit">{u.error}</p>}
      {u.state === "exists" && (
        <div className="flex flex-wrap items-center gap-1.5">
          <p className="mr-auto text-[12px] text-warn">{t("cloud.upload.exists")}</p>
          <Button size="xs" variant="secondary" onClick={() => replace(u.id)}>
            {t("cloud.upload.replace")}
          </Button>
          <Button size="xs" variant="secondary" onClick={() => void keepBoth(u.id)}>
            {t("cloud.upload.keepBoth")}
          </Button>
          <Button size="xs" variant="ghost" onClick={() => cancel(u.id)}>
            {t("cloud.upload.skip")}
          </Button>
        </div>
      )}
    </li>
  );
}
