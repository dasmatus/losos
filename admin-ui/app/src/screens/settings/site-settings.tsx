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

/* The box-wide settings of LosOS cloud and LosOS Git, on the Apps pane.
 *
 * What an administrator would otherwise change in Nextcloud's
 * Administration settings and Forgejo's Site administration. Each row is
 * one `losos.<app>.site.*` option (modules/options.nix), edited as a line of
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

/* The languages LosOS cloud can be set to here, by the name each one uses
 * for itself. The same list as the option's enum. */
const LANGUAGES: readonly (readonly [string, string])[] = [
  ["en", "English"],
  ["en_GB", "English (UK)"],
  ["sk", "Slovenčina"],
  ["cs", "Čeština"],
  ["de", "Deutsch (du)"],
  ["de_DE", "Deutsch (Sie)"],
  ["fr", "Français"],
  ["es", "Español"],
  ["it", "Italiano"],
  ["nl", "Nederlands"],
  ["pl", "Polski"],
  ["hu", "Magyar"],
  ["uk", "Українська"],
];

const LANDING: readonly (readonly [string, MessageKey])[] = [
  ["home", "panes.apps.site.landing.home"],
  ["explore", "panes.apps.site.landing.explore"],
  ["organizations", "panes.apps.site.landing.organizations"],
  ["login", "panes.apps.site.landing.login"],
];

const PRIVATE: readonly (readonly [string, MessageKey])[] = [
  ["last", "panes.apps.site.private.last"],
  ["private", "panes.apps.site.private.private"],
  ["public", "panes.apps.site.private.public"],
];

const PHONE_REGION = /^[A-Z]{2}$/;

/** One option's row state: the literal the draft holds, else the default's. */
interface Slot {
  option: OptionDoc;
  /** The literal in force: the draft's line, else the default's. */
  raw: string | null;
  disabled: boolean;
  problem: string | null;
  set: (next: string | null) => void;
}

function slotFor(form: SettingsForm, name: string): Slot | null {
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

export function SiteSettings({ form }: { form: SettingsForm }) {
  const t = useT();
  const cloud = {
    language: slotFor(form, "nextcloud.site.defaultLanguage"),
    phone: slotFor(form, "nextcloud.site.phoneRegion"),
    links: slotFor(form, "nextcloud.site.publicLinks"),
    password: slotFor(form, "nextcloud.site.linkPassword"),
    expiry: slotFor(form, "nextcloud.site.linkExpiryDays"),
    quota: slotFor(form, "nextcloud.site.defaultQuotaGB"),
    trash: slotFor(form, "nextcloud.site.trashDays"),
    versions: slotFor(form, "nextcloud.site.versionDays"),
  };
  const git = {
    signIn: slotFor(form, "forgejo.site.requireSignIn"),
    landing: slotFor(form, "forgejo.site.landingPage"),
    private: slotFor(form, "forgejo.site.defaultPrivate"),
    email: slotFor(form, "forgejo.site.keepEmailPrivate"),
    push: slotFor(form, "forgejo.site.pushCreate"),
  };
  const ids = {
    language: React.useId(),
    phone: React.useId(),
    links: React.useId(),
    password: React.useId(),
    expiry: React.useId(),
    quota: React.useId(),
    trash: React.useId(),
    versions: React.useId(),
    signIn: React.useId(),
    landing: React.useId(),
    private: React.useId(),
    email: React.useId(),
    push: React.useId(),
  };

  const linksOff = cloud.links !== null && cloud.links.raw === "false";
  const cloudRows = Object.values(cloud).some((slot) => slot !== null);
  const gitRows = Object.values(git).some((slot) => slot !== null);

  return (
    <>
      {cloudRows && (
        <PaneSection data-testid="site-cloud">
          <GroupTitle>LosOS cloud</GroupTitle>
          <Group>
            {cloud.language !== null && (
              <SelectRow
                id={ids.language}
                slot={cloud.language}
                title={t("panes.apps.site.language")}
                detail={t("panes.apps.site.languageDetail")}
                choices={[
                  ["", t("panes.apps.site.languageBrowser")],
                  ...LANGUAGES,
                ]}
                nullable
              />
            )}
            {cloud.phone !== null && (
              <PhoneRegionRow
                id={ids.phone}
                slot={cloud.phone}
                title={t("panes.apps.site.phone")}
                detail={t("panes.apps.site.phoneDetail")}
                invalid={t("panes.apps.site.phoneInvalid")}
                empty={t("panes.apps.site.noRegion")}
              />
            )}
            {cloud.links !== null && (
              <BoolRow id={ids.links} slot={cloud.links} title={t("panes.apps.site.links")} description={t("panes.apps.site.linksDetail")} />
            )}
            {cloud.password !== null && (
              <BoolRow id={ids.password} slot={cloud.password} title={t("panes.apps.site.password")} off={linksOff} />
            )}
            {cloud.expiry !== null && (
              <NumberRow
                id={ids.expiry}
                slot={cloud.expiry}
                title={t("panes.apps.site.expiry")}
                unit={t("panes.apps.site.days")}
                empty={t("panes.apps.site.never")}
                off={linksOff}
              />
            )}
            {cloud.quota !== null && (
              <NumberRow
                id={ids.quota}
                slot={cloud.quota}
                title={t("panes.apps.site.quota")}
                unit="GB"
                empty={t("panes.apps.site.noLimit")}
              />
            )}
            {cloud.trash !== null && (
              <NumberRow
                id={ids.trash}
                slot={cloud.trash}
                title={t("panes.apps.site.trash")}
                unit={t("panes.apps.site.days")}
                empty={t("panes.apps.site.automatic")}
              />
            )}
            {cloud.versions !== null && (
              <NumberRow
                id={ids.versions}
                slot={cloud.versions}
                title={t("panes.apps.site.versions")}
                unit={t("panes.apps.site.days")}
                empty={t("panes.apps.site.automatic")}
                last
              />
            )}
          </Group>
          <GroupCaption>{t("panes.apps.site.cloudCaption")}</GroupCaption>
        </PaneSection>
      )}

      {gitRows && (
        <PaneSection data-testid="site-git">
          <GroupTitle>LosOS Git</GroupTitle>
          <Group>
            {git.signIn !== null && (
              <BoolRow
                id={ids.signIn}
                slot={git.signIn}
                title={t("panes.apps.site.signIn")}
                description={t("panes.apps.site.signInDetail")}
              />
            )}
            {git.landing !== null && (
              <SelectRow
                id={ids.landing}
                slot={git.landing}
                title={t("panes.apps.site.landing")}
                choices={LANDING.map(([value, key]) => [value, t(key)] as const)}
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
            {git.email !== null && (
              <BoolRow id={ids.email} slot={git.email} title={t("panes.apps.site.email")} description={t("panes.apps.site.emailDetail")} />
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

function BoolRow({
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
function SelectRow({
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
  /** What an empty field means: "Never", "No limit". */
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
    slot.set(trimmed.length === 0 ? "null" : trimmed);
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

/* Two capital letters or nothing. The option's type is a pattern Nix checks
 * when the box rebuilds and lososd's gate does not, so a value that does
 * not fit is held here, with a line saying why, and never reaches the
 * draft. */
function PhoneRegionRow({
  id,
  slot,
  title,
  detail,
  invalid,
  empty,
}: {
  id: string;
  slot: Slot;
  title: string;
  detail: string;
  invalid: string;
  empty: string;
}) {
  const errorId = React.useId();
  const shown = slot.raw === null || slot.raw === "null" ? "" : (nixUnquote(slot.raw) ?? "");
  const [text, setText] = React.useState(shown);
  React.useEffect(() => setText(shown), [shown]);
  const bad = text.length > 0 && !PHONE_REGION.test(text);

  const commit = () => {
    if (text === shown || bad) return;
    slot.set(text.length === 0 ? "null" : nixString(text));
  };

  return (
    <Row data-option={slot.option.name} className="max-sm:flex-col max-sm:items-stretch">
      <RowText htmlFor={id} title={title} detail={detail} />
      <div className="flex shrink-0 flex-col items-end gap-1 max-sm:items-stretch">
        <Input
          id={id}
          value={text}
          placeholder={empty}
          maxLength={2}
          autoComplete="off"
          autoCapitalize="characters"
          spellCheck={false}
          disabled={slot.disabled}
          aria-invalid={bad}
          aria-describedby={bad ? errorId : undefined}
          className="numeric w-[5rem] text-center uppercase placeholder:normal-case"
          onChange={(event) => setText(event.target.value.toUpperCase())}
          onBlur={commit}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              commit();
            }
          }}
        />
        <FieldError id={errorId}>{bad ? invalid : null}</FieldError>
      </div>
    </Row>
  );
}
