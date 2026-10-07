import { Button } from "@/components/ui/button";
import { Progress, Spinner } from "@/components/ui/progress";
import { LogView } from "@/components/ui/scroll-area";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import type { SettingsForm } from "./use-settings-form";

/* The bar that admits a settings change on this box is not a save.
 *
 * Every write goes out as the whole of modules/overrides.nix and starts a
 * `nixos-rebuild switch`. The box stays reachable while it works, lososd
 * restarts itself halfway through, and the outcome arrives minutes later — so
 * the screen cannot pretend a toggle took effect when it was flipped. It
 * stays pending until Apply, and then the bar becomes the progress report.
 * The outcome, done or failed, is a toast from use-settings-form, and the bar
 * leaves with it.
 *
 * It is sticky at the bottom of the pane column rather than fixed to the
 * viewport: fixed would cover the last row of whichever pane is open, and the
 * last row of the Reset pane is the reset button.
 */

export function ApplyBar({ form }: { form: SettingsForm }) {
  const t = useT();
  const { rebuild } = form;

  if (rebuild !== null) {
    return (
      <div className={cn(BAR, "animate-rise flex-col items-stretch gap-2.5")}>
        <div className="flex items-center gap-2.5">
          <Spinner className="text-accent" label={t("settings.applyBar.spinner")} />
          <p className="flex-1 text-[13.5px] font-medium">{rebuild.title}</p>
        </div>

        {/* lososd reports 0 for the whole of a rebuild and 100 only at the
            end, so a determinate bar would sit at zero for minutes and read
            as stuck. The indeterminate band is the honest one. */}
        <Progress label={t("settings.form.applyingYours")} />

        {rebuild.message.length > 0 && <LogView>{rebuild.message}</LogView>}
      </div>
    );
  }

  if (!form.dirty) return null;

  const count = form.changeCount;

  return (
    <div className={cn(BAR, "animate-rise items-center gap-3")}>
      <div className="min-w-0 flex-1">
        <p className="text-[13.5px] leading-snug font-medium">
          {t("settings.applyBar.pending", { count })}
        </p>
        <p className="mt-0.5 text-[12.5px] leading-snug text-muted">
          {form.valid
            ? t("settings.applyBar.valid")
            : t("settings.applyBar.invalid")}
        </p>
      </div>
      <Button variant="ghost" size="sm" onClick={form.discard} disabled={form.applying}>
        {t("settings.applyBar.discard")}
      </Button>
      <Button size="sm" onClick={form.apply} disabled={!form.valid || form.applying}>
        {t("settings.applyBar.apply")}
      </Button>
    </div>
  );
}

const BAR = cn(
  "sticky bottom-4 z-30 mt-5 flex",
  "rounded-card border border-line bg-surface/95 p-3.5 shadow-pop backdrop-blur-sm",
);
