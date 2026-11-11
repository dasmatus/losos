import { HugeiconsIcon } from "@hugeicons/react";
import { InformationCircleIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import { MESSAGES } from "@/i18n";
import { t, type MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import type { OptionDoc } from "@/lib/api";
import { featurePaneOwns } from "@/lib/option-value";
import { draftOn } from "./pane-advanced";
import { Group, GroupCaption, PaneSection, SwitchRow } from "./rows";
import { slotFor, type Slot } from "./site-settings";
import type { SettingsForm } from "./use-settings-form";

/* The box's features, on and off.
 *
 * A view over the option document, not a list: one switch per bool
 * `losos.<name>.enable` the box declares that no other pane owns, less the
 * ones the document holds back — read-only (the installer's), dangerous
 * (the Advanced pane asks about those) and hidden (`visible = false`). The
 * rule lives in lib/option-value.ts (featurePaneOwns), where the Advanced
 * pane uses it to link here rather than draw a second editor. Each row is
 * one line of modules/overrides.nix, edited through form.extra exactly as
 * the Advanced pane edits it: the same draft, the same Apply, the same
 * gate in lososd, the same commit in the box's history.
 *
 * A row's text is a catalogue override under panes.features.<the name
 * without .enable>.* when one exists (three languages, checked by tsc),
 * else the prettified name and the description the declaring module
 * carries — so a feature added to modules/options.nix needs no UI change
 * to be switchable, and gets its polished line when someone writes one.
 *
 * Unset is the default. A switch whose feature the box ships on draws on
 * until a line says otherwise, and returning a switch to its default
 * removes the line, so the file never restates a default that an update
 * may change and the bar is not armed by it.
 *
 * An option whose document `needs` another is greyed while that one is off
 * in the draft, with a line saying why — the same rule the Advanced pane
 * applies (draftOn).
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

  /* What the box declares, in the document's order (sorted by name). A
   * feature the box does not declare draws no row. */
  const rows: { option: OptionDoc; slot: Slot; needs: string | null; gated: boolean }[] = [];
  for (const option of options.options) {
    if (!featurePaneOwns(option)) continue;
    const slot = slotFor(form, option.name);
    if (slot === null) continue;
    const needs = option.needs ?? null;
    rows.push({ option, slot, needs, gated: needs !== null && !draftOn(form, needs) });
  }

  return (
    <>
      <PaneSection data-testid="features-pane">
        <Group>
          {rows.map(({ option, slot, needs, gated }, index) => {
            const id = option.name.replace(/\.enable$/, "");
            return (
              <SwitchRow
                key={option.name}
                id={`feature-${cssId(id)}`}
                option={option.name}
                title={featureText(id, "title") ?? prettify(id)}
                description={
                  gated && needs !== null
                    ? t("advanced.needs", { name: needs })
                    : (featureText(id, "detail") ?? option.description.trim())
                }
                checked={slot.raw === "true"}
                disabled={disabled || gated}
                onCheckedChange={(on) => slot.set(on ? "true" : "false")}
                last={index === rows.length - 1}
              />
            );
          })}
        </Group>
      </PaneSection>
      <PaneSection>
        <GroupCaption className="pt-0">{t("panes.features.caption")}</GroupCaption>
      </PaneSection>
    </>
  );
}

/* The catalogue override for one feature's title or detail, or null when
 * none exists — the fallback is the prettified name and the description
 * the declaring module carries. The same shape the Credentials page's
 * optional name catalogue takes (pane-credentials-words.ts). */
function featureText(id: string, part: "title" | "detail"): string | null {
  const key = `panes.features.${id}.${part}`;
  return key in MESSAGES ? t(key as MessageKey) : null;
}

/* The name as the row's title when no override exists: "configRepo" reads
 * "Config repo", "lab.libvirt" reads "Lab libvirt". */
function prettify(id: string): string {
  return id
    .replace(/[.]/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .split(" ")
    .map((word) => (word.length === 0 ? word : word.charAt(0).toUpperCase() + word.slice(1)))
    .join(" ");
}

/* The option's name in a DOM id: dots out, since querySelector would read
 * them as class separators. */
function cssId(id: string): string {
  return id.replace(/[.]/g, "-");
}
