import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { UserCheck01Icon, ViewOffSlashIcon } from "@hugeicons/core-free-icons";
import { decode } from "@/lib/blurhash";
import { useT } from "@/lib/i18n-react";
import { useNudity } from "@/lib/nudity";
import { usePictureSettings } from "@/lib/pictures";
import { cn } from "@/lib/utils";

/* A picture someone sent, before the person chose to see it: its colours
 * (the BlurHash the sender's page made, when there is one) under a button
 * that says who sent it. Nothing of the picture itself is fetched until the
 * button is pressed. Chat and Mail draw it in place of the picture. */

function BlurCanvas({ hash }: { hash: string }) {
  const ref = React.useRef<HTMLCanvasElement>(null);
  React.useEffect(() => {
    const pixels = decode(hash, 32, 32);
    const context = ref.current?.getContext("2d");
    if (pixels === null || context === null || context === undefined) return;
    context.putImageData(new ImageData(pixels, 32, 32), 0, 0);
  }, [hash]);
  return <canvas ref={ref} width={32} height={32} aria-hidden className="absolute inset-0 size-full" />;
}

export function HiddenPicture({
  from,
  hash,
  ratio,
  onReveal,
  onTrust,
  className,
}: {
  /** The sender as the page names them. */
  from: string;
  hash?: string | null;
  /** Height over width, when the sender said. */
  ratio?: number;
  onReveal: () => void;
  /** Always show this sender's pictures; left out where it cannot be. */
  onTrust?: () => void;
  className?: string;
}) {
  const t = useT();
  const [trusting, setTrusting] = React.useState(false);
  const shape = ratio !== undefined && ratio > 0 ? Math.min(Math.max(ratio, 0.4), 1.5) : 0.66;
  return (
    <div
      data-hidden-picture
      className={cn("relative w-72 max-w-full overflow-hidden rounded-control border border-hair bg-sunk", className)}
      style={{ aspectRatio: `${1 / shape}` }}
    >
      {typeof hash === "string" && <BlurCanvas hash={hash} />}
      <button
        type="button"
        onClick={onReveal}
        className="absolute inset-0 flex flex-col items-center justify-center gap-1 p-4 text-center transition-colors duration-150 hover:bg-ink/5 focus-visible:outline-2 focus-visible:outline-offset-[-4px] focus-visible:outline-accent"
        aria-label={t("pictures.hidden.label", { name: from })}
      >
        <span className="flex max-w-full flex-col items-center gap-1 rounded-card bg-surface/85 px-3.5 py-2 shadow-sm backdrop-blur-sm">
          <HugeiconsIcon icon={ViewOffSlashIcon} className="size-5 text-muted" />
          <span className="max-w-full truncate text-[13px] font-medium text-ink">{t("pictures.hidden.from", { name: from })}</span>
          <span className="text-[12px] text-muted">{t("pictures.hidden.press")}</span>
        </span>
      </button>
      {onTrust !== undefined && (
        <button
          type="button"
          disabled={trusting}
          onClick={() => {
            setTrusting(true);
            onTrust();
          }}
          className="absolute right-2 bottom-2 left-2 flex items-center justify-center gap-1.5 truncate rounded-full bg-surface/85 px-3 py-1 text-[12px] text-muted backdrop-blur-sm transition-colors duration-150 hover:text-ink disabled:opacity-60"
        >
          <HugeiconsIcon icon={UserCheck01Icon} className="size-3.5 shrink-0" />
          <span className="truncate">{t("pictures.hidden.trust", { name: from })}</span>
        </button>
      )}
    </div>
  );
}

/* A picture that is on screen, held behind a stronger blur while the
 * person's nudity warning thinks it may show nudity. The score is worked
 * out in this page (lib/nudity.ts); the box never sees it. */
export function NudityGuard({ src, children }: { src: string | null; children: React.ReactNode }) {
  const t = useT();
  const settings = usePictureSettings();
  const on = settings?.nudityCheck === true;
  const verdict = useNudity(on ? src : null);
  const [shown, setShown] = React.useState(false);
  if (!on || shown || verdict === "clear" || verdict === "failed") return <>{children}</>;
  const flagged = verdict === "flagged";
  return (
    <div data-nudity={verdict} className="relative w-fit max-w-full overflow-hidden rounded-control">
      <div aria-hidden className="pointer-events-none scale-110 blur-2xl">
        {children}
      </div>
      <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-ink/30 p-4 text-center">
        {flagged ? (
          <>
            <span className="max-w-64 rounded-card bg-surface/90 px-3.5 py-2 text-[13px] text-ink shadow-sm">
              <span className="block font-medium">{t("pictures.nudity.title")}</span>
              <span className="block text-[12px] text-muted">
                {settings?.child === true ? t("pictures.nudity.childBody") : t("pictures.nudity.body")}
              </span>
            </span>
            <button
              type="button"
              onClick={() => setShown(true)}
              className="rounded-full bg-surface/90 px-3 py-1 text-[12px] font-medium text-ink transition-colors duration-150 hover:bg-surface"
            >
              {t("pictures.nudity.show")}
            </button>
          </>
        ) : (
          <span className="rounded-full bg-surface/85 px-3 py-1 text-[12px] text-muted">{t("pictures.nudity.checking")}</span>
        )}
      </div>
    </div>
  );
}
