import * as React from "react";
import { FieldError, Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { nixString, type OptionDoc } from "@/lib/api";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { nixUnquote, toLiteral } from "@/lib/option-value";
import { cn } from "@/lib/utils";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, SwitchRow } from "./rows";
import type { SettingsForm } from "./use-settings-form";

/* The box-wide settings of LosOS Cloud and LosOS Git, on the Apps pane,
 * each inside that app's accordion.
 *
 * What an administrator would otherwise change in an app's own admin
 * pages: `losos.cloud.trashDays` for LosOS Cloud and the
 * `losos.git.site.*` options for LosOS Git (modules/options.nix), edited as a line of
 * overrides.nix through form.extra, exactly as the Advanced pane edits it:
 * the same draft, the same Apply, the same gate in lososd, the same commit
 * in the box's history. The Advanced pane shows these rows with a link back
 * here (lib/option-value.ts, ON_PANE) rather than a second editor.
 *
 * A row is drawn only when the box's option document declares its option:
 * an older box would refuse the line at Apply, so it gets no row to write
 * one with.
 *
 * Unset is the default. Choosing the default value, or clearing a number
 * field, removes the option's line, so the file never carries a line that
 * only restates a default.
 */

const PRIVATE: readonly (readonly [string, MessageKey])[] = [
  ["last", "panes.apps.site.private.last"],
  ["private", "panes.apps.site.private.private"],
  ["public", "panes.apps.site.private.public"],
];

/** One option's row state: the literal the draft holds, else the default's. */
export interface Slot {
  option: OptionDoc;
  /** The literal in force: the draft's line, else the default's. */
  raw: string | null;
  disabled: boolean;
  problem: string | null;
  set: (next: string | null) => void;
}

/** What the rows need from a form: the box's own settings form, or the
 *  edge's form over GET /api/site (src/manage/), whose draft is null. */
export type SiteForm = Pick<
  SettingsForm,
  "draft" | "options" | "extra" | "extraProblems" | "locked" | "ready" | "setExtra"
>;

export function slotFor(form: SiteForm, name: string): Slot | null {
  const option = form.options?.options.find((candidate) => candidate.name === name);
  if (option === undefined || option.readOnly) return null;
  const defaultRaw = toLiteral(option.editor, option.default);
  return {
    option,
    raw: form.extra[name] ?? defaultRaw,
    disabled: form.locked || !form.ready,
    problem: form.extraProblems[name] ?? null,
    // The default is no line at all, so a default that changes with an
    // update is followed.
    set: (next) => form.setExtra(name, next === defaultRaw ? null : next),
  };
}

/** Whether the box declares any of `app`'s settings, so the pane can leave
 *  out an accordion that would hold nothing. */
export function hasSiteSettings(form: SiteForm, app: "cloud" | "git"): boolean {
  const own = (name: string) =>
    app === "cloud"
      ? name === "cloud.trashDays"
      : name.startsWith("git.site.");
  return form.options?.options.some((o) => !o.readOnly && own(o.name)) ?? false;
}

/* One app's settings, under the title the accordion it sits in leaves room
 * for: the app's name is on the accordion. A page with no accordion (the
 * edge's, src/manage/) names the app in `title` instead. */
export function SiteSettings({
  form,
  app,
  title,
}: {
  form: SiteForm;
  app: "cloud" | "git";
  title?: string;
}) {
  const t = useT();
  const cloud = {
    trash: slotFor(form, "cloud.trashDays"),
  };
  const git = {
    signIn: slotFor(form, "git.site.requireSignIn"),
    private: slotFor(form, "git.site.defaultPrivate"),
    push: slotFor(form, "git.site.pushCreate"),
  };
  const ids = {
    trash: React.useId(),
    signIn: React.useId(),
    private: React.useId(),
    push: React.useId(),
  };

  const cloudRows = cloud.trash !== null;
  const gitRows = Object.values(git).some((slot) => slot !== null);

  return (
    <>
      {app === "cloud" && cloudRows && (
        <PaneSection data-testid="site-cloud">
          <GroupTitle>{title ?? t("cloudApps.settings.title")}</GroupTitle>
          <Group>
            {cloud.trash !== null && (
              <NumberRow
                id={ids.trash}
                slot={cloud.trash}
                title={t("panes.apps.site.trash")}
                unit={t("panes.apps.site.days")}
                empty={t("panes.apps.site.trashDefault")}
                last
              />
            )}
          </Group>
          <GroupCaption>{t("panes.apps.site.cloudCaption")}</GroupCaption>
        </PaneSection>
      )}

      {app === "git" && gitRows && (
        <PaneSection data-testid="site-git">
          <GroupTitle>{title ?? t("cloudApps.settings.title")}</GroupTitle>
          <Group>
            {git.signIn !== null && (
              <BoolRow
                id={ids.signIn}
                slot={git.signIn}
                title={t("panes.apps.site.signIn")}
                description={t("panes.apps.site.signInDetail")}
              />
            )}
            {git.private !== null && (
              <SelectRow
                id={ids.private}
                slot={git.private}
                title={t("panes.apps.site.private")}
                choices={PRIVATE.map(([value, key]) => [value, t(key)] as const)}
              />
            )}
            {git.push !== null && (
              <BoolRow id={ids.push} slot={git.push} title={t("panes.apps.site.push")} last />
            )}
          </Group>
          <GroupCaption>{t("panes.apps.site.gitCaption")}</GroupCaption>
        </PaneSection>
      )}
    </>
  );
}

// ── Rows ──────────────────────────────────────────────────────────────────

export function BoolRow({
  id,
  slot,
  title,
  description,
  off = false,
  last,
}: {
  id: string;
  slot: Slot;
  title: string;
  description?: string;
  /** Greyed because another row makes this one moot. */
  off?: boolean;
  last?: boolean;
}) {
  return (
    <SwitchRow
      id={id}
      option={slot.option.name}
      title={title}
      description={description}
      checked={slot.raw === "true"}
      disabled={slot.disabled || off}
      onCheckedChange={(on) => slot.set(on ? "true" : "false")}
      last={last}
    />
  );
}

/* A choice from a fixed list. `nullable`: the first choice, "", is null. */
export function SelectRow({
  id,
  slot,
  title,
  detail,
  choices,
  nullable = false,
  last,
}: {
  id: string;
  slot: Slot;
  title: string;
  detail?: string;
  choices: readonly (readonly [string, string])[];
  nullable?: boolean;
  last?: boolean;
}) {
  const value = slot.raw === null || slot.raw === "null" ? "" : (nixUnquote(slot.raw) ?? "");
  return (
    <Row last={last} data-option={slot.option.name} className="max-sm:flex-col max-sm:items-stretch">
      <RowText htmlFor={id} title={title} detail={detail} />
      <NativeSelect
        id={id}
        value={value}
        disabled={slot.disabled}
        aria-invalid={slot.problem !== null}
        onChange={(event) => {
          const next = event.target.value;
          slot.set(nullable && next === "" ? "null" : nixString(next));
        }}
      >
        {choices.map(([choice, label]) => (
          <NativeSelectOption key={choice} value={choice}>
            {label}
          </NativeSelectOption>
        ))}
      </NativeSelect>
    </Row>
  );
}

/* A whole number of something, or nothing (null, which the placeholder
 * names). Commits on blur or Enter, as the Advanced pane's fields do. */
function NumberRow({
  id,
  slot,
  title,
  unit,
  empty,
  off = false,
  last,
}: {
  id: string;
  slot: Slot;
  title: string;
  unit: string;
  /** What an empty field means: "Never", "No limit", "30 by default". */
  empty: string;
  off?: boolean;
  last?: boolean;
}) {
  const errorId = React.useId();
  const shown = slot.raw === null || slot.raw === "null" ? "" : slot.raw;
  const [text, setText] = React.useState(shown);
  // The draft moved under us (discard, a reload): follow it.
  React.useEffect(() => setText(shown), [shown]);

  const commit = () => {
    const trimmed = text.trim();
    if (trimmed === shown) return;
    // An empty field is null where the option takes one, else the default:
    // no line at all.
    if (trimmed.length === 0) slot.set(slot.option.editor.kind === "nullable" ? "null" : null);
    else slot.set(trimmed);
  };

  return (
    <Row last={last} data-option={slot.option.name} className="max-sm:flex-col max-sm:items-stretch">
      <RowText htmlFor={id} title={title} />
      <div className="flex shrink-0 flex-col items-end gap-1 max-sm:items-stretch">
        <span className="flex items-center gap-2">
          <Input
            id={id}
            value={text}
            placeholder={empty}
            inputMode="numeric"
            autoComplete="off"
            disabled={slot.disabled || off}
            aria-invalid={slot.problem !== null}
            aria-describedby={slot.problem !== null ? errorId : undefined}
            className={cn("numeric w-[7.5rem] text-right")}
            onChange={(event) => setText(event.target.value)}
            onBlur={commit}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                commit();
              }
            }}
          />
          <span className="w-9 text-[13px] text-muted">{unit}</span>
        </span>
        <FieldError id={errorId}>{slot.problem}</FieldError>
      </div>
    </Row>
  );
}

