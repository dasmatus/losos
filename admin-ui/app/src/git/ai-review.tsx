import * as React from "react";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { git, type AiReviewSetting as Setting } from "./api";
import { failed, SwitchField } from "./parts";
import { useLoad } from "./store";

/* The repository's AI review switch: whether LosOS Reviewer, the box's
 * own code model, reviews this repository's pull requests. Off until an
 * administrator turns it on; it cannot be turned on while the box has no
 * code model (Build with AI off). */

export function AiReviewSetting({ owner, repo }: { owner: string; repo: string }) {
  const t = useT();
  const loaded = useLoad((signal) => git.aiReviewSetting(owner, repo, signal).catch(() => null), [owner, repo]);
  const [setting, setSetting] = React.useState<Setting | null>(null);
  const [busy, setBusy] = React.useState(false);
  const s = setting ?? loaded.data;
  if (s === null) return null;
  const description = !s.available
    ? t("gitReview.unavailable")
    : s.model !== undefined && s.model !== null
      ? t("gitReview.hintModel", { model: s.model })
      : t("gitReview.hint");

  return (
    <section className="flex max-w-xl flex-col gap-3 rounded-card border border-line bg-surface p-4" data-ai-review-setting={s.enabled ? "on" : "off"}>
      <h2 className="text-[14px] font-semibold">{t("gitReview.title")}</h2>
      <SwitchField
        label={t("gitReview.label")}
        description={description}
        checked={s.enabled}
        disabled={busy || s.canChange === false || (!s.enabled && !s.available)}
        onChange={(on) => {
          if (busy) return;
          setBusy(true);
          git.setAiReview(owner, repo, on).then(
            (next) => {
              setBusy(false);
              setSetting(next);
              toast.success(t(on ? "gitReview.turnedOn" : "gitReview.turnedOff"));
            },
            (err: unknown) => {
              setBusy(false);
              failed(t("gitReview.failed"), err);
            },
          );
        }}
      />
    </section>
  );
}
