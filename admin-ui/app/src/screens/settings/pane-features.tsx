import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { InformationCircleIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { FEATURE_OPTIONS } from "@/lib/option-value";
import { draftOn } from "./pane-advanced";
import { Group, GroupCaption, PaneSection, SwitchRow } from "./rows";
import { slotFor } from "./site-settings";
import type { SettingsForm } from "./use-settings-form";

/* The box's major features, on and off.
 *
 * One switch per feature the box ships: LosOS Cloud, LosOS Chat, LosOS Git,
 * LosOS Mail, Mesh CI, calls, apps, the box's three AI features, the search
 * index, the logs page and the two boot touches. Each row is one line of
 * modules/overrides.nix — the feature's `losos.<name>.enable` — edited
 * through form.extra exactly as the Advanced pane edits it: the same draft,
 * the same Apply, the same gate in lososd, the same commit in the box's
 * history. The Advanced pane shows these rows with a link back here
 * (lib/option-value.ts, ON_PANE) rather than a second editor.
 *
 * Which features the list names is fixed in lib/option-value.ts's
 * FEATURE_OPTIONS; what the pane draws is what the box actually has. A row
 * is drawn only when the box's option document declares its option (an
 * older box would refuse the line at Apply), and never when the document
 * marks one read-only or dangerous — the list is curated, but the box has
 * the last word.
 *
 * An option whose document `needs` another (none of today's list carries
 * one; mail's and CI's "exists only while the box shares" is said in their
 * descriptions) is greyed while that one is off in the draft, with a line
 * saying why — the same rule the Advanced pane applies (draftOn).
 *
 * The switch's off state is the option's default, not always `false`: a
 * feature the box ships on draws on until a line says otherwise, and
 * unsetting the line (Apply drops it) puts the default back, so a default
 * that changes with an update is followed.
 */

export function FeaturesPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const options = form.options;
  const disabled = form.locked || !form.ready;

  if (options === null) {
    return (
      <PaneSection>
        <Group>
          {[0, 1, 2, 3].map((row) => (
            <div key={row} className="flex items-center justify-between gap-4 py-3.5">
              <div className="min-w-0 flex-1 space-y-1.5">
                <Skeleton className="h-4 w-48" />
                <Skeleton className="h-3 w-80 max-w-full" />
              </div>
              <Skeleton className="h-6 w-11 shrink-0 rounded-full" />
            </div>
          ))}
        </Group>
      </PaneSection>
    );
  }

  if (!options.available) {
    return (
      <Alert>
        <HugeiconsIcon icon={InformationCircleIcon} size={17} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        <AlertDescription>{t("advanced.unavailable")}</AlertDescription>
      </Alert>
    );
  }

  /* One entry per feature the box declares and the pane may edit. A name
   * the document does not have simply draws no row: the pane's list is the
   * same on every box, the rows are not. */
  const rows = FEATURE_OPTIONS.flatMap((name) => {
    const slot = slotFor(form, name);
    if (slot === null || slot.option.danger) return [];
    const needs = slot.option.needs ?? null;
    const gated = needs !== null && !draftOn(form, needs);
    const id = featureId(name);
    return [{ name, id, slot, needs, gated }];
  });

  return (
    <>
      <PaneSection data-testid="features-pane">
        <Group>
          {rows.map(({ name, id, slot, needs, gated }, index) => (
            <SwitchRow
              key={name}
              id={`feature-${id}`}
              option={name}
              title={t(`panes.features.${id}.title` as MessageKey)}
              description={
                gated && needs !== null
                  ? t("advanced.needs", { name: needs })
                  : t(`panes.features.${id}.detail` as MessageKey)
              }
              checked={slot.raw === "true"}
              disabled={disabled || gated}
              onCheckedChange={(on) => slot.set(on ? "true" : "false")}
              last={index === rows.length - 1}
            />
          ))}
        </Group>
      </PaneSection>
      <PaneSection>
        <GroupCaption className="pt-0">{t("panes.features.caption")}</GroupCaption>
      </PaneSection>
    </>
  );
}

/** The message-key fragment of one feature option: the first segment of
 *  `losos.<first>.enable`. `cloud.enable` is "cloud", so its title and
 *  detail live under panes.features.cloud.*. */
function featureId(name: string): string {
  return name.slice(0, name.indexOf("."));
}
