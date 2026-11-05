/* The voice changer over a call: one choice, heard by everyone in the call
 * and remembered for the next. */

import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { VOICE_PRESETS, type VoicePreset } from "./voice-changer";

export interface VoicePanelProps {
  voice: VoicePreset;
  busy: boolean;
  onChoose(preset: VoicePreset): void;
  onClose(): void;
}

export function VoicePanel(p: VoicePanelProps) {
  const t = useT();
  return (
    <section
      role="dialog"
      aria-label={t("calls.voice.title")}
      data-voice-panel=""
      className="absolute inset-x-3 top-16 z-10 mx-auto flex max-w-xl flex-col gap-4 rounded-card border border-line bg-surface p-4 shadow-pop sm:inset-x-6"
    >
      <header className="flex items-start justify-between gap-3">
        <div>
          <h3 className="font-medium text-ink">{t("calls.voice.title")}</h3>
          <p className="text-[13px] text-muted">{t("calls.voice.intro")}</p>
        </div>
        <Button variant="ghost" size="sm" className="size-8 p-0" aria-label={t("calls.close")} onClick={p.onClose}>
          <HugeiconsIcon icon={Cancel01Icon} size={16} />
        </Button>
      </header>
      <div role="radiogroup" aria-label={t("calls.voice.title")} className="grid grid-cols-2 gap-2 sm:grid-cols-3">
        {VOICE_PRESETS.map((preset) => (
          <button
            key={preset}
            type="button"
            role="radio"
            aria-checked={p.voice === preset}
            data-voice={preset}
            disabled={p.busy}
            onClick={() => p.onChoose(preset)}
            className={cn(
              "rounded-card border px-3 py-2 text-left text-[14px] transition-colors disabled:opacity-50",
              p.voice === preset ? "border-accent bg-accent-wash text-ink" : "border-line text-muted hover:text-ink",
            )}
          >
            <span className="block font-medium">{t(`calls.voice.${preset}`)}</span>
            <span className="block text-[12px]">{t(`calls.voice.${preset}.detail`)}</span>
          </button>
        ))}
      </div>
      <p className="text-[13px] text-muted">{t("calls.voice.note")}</p>
    </section>
  );
}
