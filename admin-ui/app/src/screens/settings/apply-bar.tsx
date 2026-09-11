import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon, CheckmarkCircle02Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Progress, Spinner } from "@/components/ui/progress";
import { LogView } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { plural } from "./format";
import type { SettingsForm } from "./use-settings-form";

/* The bar that admits a settings change on this box is not a save.
 *
 * Every write goes out as the whole of modules/overrides.nix and starts a
 * `nixos-rebuild switch`. The box stays reachable while it works, lososd
 * restarts itself halfway through, and the outcome arrives minutes later — so
 * the screen cannot pretend a toggle took effect when it was flipped. It
 * stays pending until Apply, and then the bar becomes the progress report.
 *
 * It is sticky at the bottom of the pane column rather than fixed to the
 * viewport: fixed would cover the last row of whichever pane is open, and the
 * last row of the Reset pane is the reset button.
 */

export function ApplyBar({ form }: { form: SettingsForm }) {
  const { rebuild } = form;

  if (rebuild !== null) {
    return (
      <div className={cn(BAR, "animate-rise flex-col items-stretch gap-2.5")}>
        <div className="flex items-center gap-2.5">
          {rebuild.phase === "building" && <Spinner className="text-accent" label="Applying" />}
          {rebuild.phase === "done" && (
            <HugeiconsIcon
              icon={CheckmarkCircle02Icon}
              size={19}
              strokeWidth={1.5}
              color="currentColor"
              className="text-ok"
              aria-hidden="true"
            />
          )}
          {rebuild.phase === "failed" && (
            <HugeiconsIcon
              icon={Alert02Icon}
              size={19}
              strokeWidth={1.5}
              color="currentColor"
              className="text-crit"
              aria-hidden="true"
            />
          )}
          <p className="flex-1 text-[13.5px] font-medium">{rebuild.title}</p>
          {rebuild.phase !== "building" && (
            <Button variant="ghost" size="sm" onClick={form.dismissRebuild}>
              Dismiss
            </Button>
          )}
        </div>

        {/* lososd reports 0 for the whole of a rebuild and 100 only at the
            end, so a determinate bar would sit at zero for minutes and read
            as stuck. The indeterminate band is the honest one. */}
        {rebuild.phase === "building" && <Progress label="Applying your changes" />}

        {rebuild.message.length > 0 && <LogView>{rebuild.message}</LogView>}
      </div>
    );
  }

  if (!form.dirty) return null;

  const count = form.changedKeys.length;

  return (
    <div className={cn(BAR, "animate-rise items-center gap-3")}>
      <div className="min-w-0 flex-1">
        <p className="text-[13.5px] leading-snug font-medium">
          {plural(count, "change")} not applied yet
        </p>
        <p className="mt-0.5 text-[12.5px] leading-snug text-muted">
          {form.valid
            ? "This box rebuilds itself to take them on. It stays reachable while it works."
            : "Fix what is marked before this can be applied."}
        </p>
      </div>
      <Button variant="ghost" size="sm" onClick={form.discard} disabled={form.applying}>
        Discard
      </Button>
      <Button size="sm" onClick={form.apply} disabled={!form.valid || form.applying}>
        Apply
      </Button>
    </div>
  );
}

const BAR = cn(
  "sticky bottom-4 z-30 mt-5 flex",
  "rounded-card border border-line bg-surface/95 p-3.5 shadow-pop backdrop-blur-sm",
);
