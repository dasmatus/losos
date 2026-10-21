/* The podcast panel over a call: where the stream goes, what it says, and
 * the picture as it goes out. podcast.ts draws and records; this asks for
 * the address and shows what the stream is before anyone goes live. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon, Cancel01Icon, LiveStreaming02Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useT } from "@/lib/i18n-react";
import { validRtmp, type Podcast } from "./podcast";

export interface PodcastFields {
  rtmp: string;
  title: string;
  subtitle: string;
  overlay: ImageBitmap | null;
}

export interface PodcastPanelProps {
  podcast: Podcast;
  fields: PodcastFields;
  live: boolean;
  starting: boolean;
  onChange(next: PodcastFields): void;
  onGoLive(): void;
  onStop(): void;
  onClose(): void;
}

function Preview({ canvas }: { canvas: HTMLCanvasElement }) {
  const ref = React.useRef<HTMLDivElement>(null);
  React.useEffect(() => {
    const host = ref.current;
    if (host === null) return;
    canvas.className = "block h-auto w-full";
    host.replaceChildren(canvas);
    return () => {
      if (canvas.parentElement === host) host.removeChild(canvas);
    };
  }, [canvas]);
  return <div ref={ref} data-podcast-preview="" className="overflow-hidden rounded-control border border-line bg-black" />;
}

export function PodcastPanel(p: PodcastPanelProps) {
  const t = useT();
  const [touched, setTouched] = React.useState(false);
  const rtmpOk = validRtmp(p.fields.rtmp);
  const set = (patch: Partial<PodcastFields>): void => p.onChange({ ...p.fields, ...patch });

  const pickOverlay = (file: File | undefined): void => {
    if (file === undefined) return;
    void createImageBitmap(file)
      .then((overlay) => set({ overlay }))
      .catch(() => set({ overlay: null }));
  };

  return (
    <section
      role="dialog"
      aria-label={t("calls.podcast")}
      data-podcast-panel=""
      className="absolute inset-x-3 top-16 bottom-24 z-10 mx-auto flex max-w-3xl flex-col gap-4 overflow-auto rounded-card border border-line bg-surface p-4 shadow-lg sm:inset-x-6"
    >
      <header className="flex items-start justify-between gap-3">
        <div>
          <h3 className="font-medium text-ink">{t("calls.podcast")}</h3>
          <p className="text-[13px] text-muted">{t("calls.podcastIntro")}</p>
        </div>
        <Button variant="ghost" size="sm" className="size-8 p-0" aria-label={t("calls.close")} onClick={p.onClose}>
          <HugeiconsIcon icon={Cancel01Icon} size={16} />
        </Button>
      </header>

      <Preview canvas={p.podcast.preview} />

      <Alert variant="warn" data-podcast-public="">
        <HugeiconsIcon icon={Alert02Icon} />
        <AlertDescription>{t("calls.podcastPublic")}</AlertDescription>
      </Alert>

      <FieldGroup>
        <Field invalid={touched && !rtmpOk}>
          <FieldLabel htmlFor="podcast-rtmp">{t("calls.rtmp")}</FieldLabel>
          <Input
            id="podcast-rtmp"
            name="rtmp"
            type="url"
            inputMode="url"
            autoComplete="off"
            spellCheck={false}
            placeholder="rtmp://a.rtmp.youtube.com/live2/…"
            value={p.fields.rtmp}
            disabled={p.live || p.starting}
            onChange={(e) => set({ rtmp: e.target.value.trim() })}
            onBlur={() => setTouched(true)}
          />
          {touched && !rtmpOk ? <FieldError>{t("calls.rtmpInvalid")}</FieldError> : null}
        </Field>
        <div className="grid gap-4 sm:grid-cols-2">
          <Field>
            <FieldLabel htmlFor="podcast-title">{t("calls.podcastTitle")}</FieldLabel>
            <Input id="podcast-title" name="title" maxLength={80} value={p.fields.title} onChange={(e) => set({ title: e.target.value })} />
          </Field>
          <Field>
            <FieldLabel htmlFor="podcast-subtitle">{t("calls.podcastSubtitle")}</FieldLabel>
            <Input
              id="podcast-subtitle"
              name="subtitle"
              maxLength={120}
              value={p.fields.subtitle}
              onChange={(e) => set({ subtitle: e.target.value })}
            />
          </Field>
        </div>
        <Field>
          <FieldLabel htmlFor="podcast-overlay">{t("calls.overlay")}</FieldLabel>
          <div className="flex flex-wrap items-center gap-2">
            <Input
              id="podcast-overlay"
              name="overlay"
              type="file"
              accept="image/png"
              className="max-w-sm"
              onChange={(e) => pickOverlay(e.target.files?.[0])}
            />
            {p.fields.overlay !== null ? (
              <Button variant="ghost" size="sm" onClick={() => set({ overlay: null })}>
                {t("calls.overlayClear")}
              </Button>
            ) : null}
          </div>
        </Field>
      </FieldGroup>

      <div className="flex justify-end gap-2">
        {p.live ? (
          <Button variant="destructive" onClick={p.onStop} data-podcast-stop="">
            {t("calls.stopLive")}
          </Button>
        ) : (
          <Button
            disabled={!rtmpOk || p.starting}
            onClick={() => {
              setTouched(true);
              if (rtmpOk) p.onGoLive();
            }}
            data-podcast-go=""
          >
            <HugeiconsIcon icon={LiveStreaming02Icon} size={16} />
            {t("calls.goLive")}
          </Button>
        )}
      </div>
    </section>
  );
}
