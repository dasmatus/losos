import * as React from "react";
import { Link, useInRouterContext } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon, Cancel01Icon, InformationCircleIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { FieldError, Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import {
  nixString,
  type OptionDoc,
  type OptionEditor,
  type OptionValue,
  type SettingsResponse,
} from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import {
  nixList,
  nixStringList,
  nixUnquote,
  OWNED,
  ownedLiteral,
  ownedValue,
  toLiteral,
} from "@/lib/option-value";
import { paneHref } from "@/lib/routes";
import { cn } from "@/lib/utils";
import { paneById } from "./panes";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue } from "./rows";
import type { SettingsForm } from "./use-settings-form";

/* Every option this box declares, as rows with a typed editor each.
 *
 * The rows are not written here. lososd serves GET /api/options, the
 * document flake/options-doc.nix generates from the losos.* declarations
 * (name, type, default, description, the value the box runs with), joined
 * with what modules/overrides.nix currently assigns. So an option added to
 * modules/options.nix appears on this pane on the next update without anyone
 * listing it, and the pane cannot describe one differently from the module
 * that declares it. What the pane adds is the editor per kind
 * (lib/option-value.ts), and tests/advanced.browser.mjs renders the real
 * document and fails on any row it cannot draw.
 *
 * Three kinds of row:
 *   - an option one of the other panes owns (lib/option-value.ts, OWNED)
 *     shows its value and links to that pane, so there is one editor per
 *     setting and the two cannot disagree. When the owning pane is planned
 *     (Market), the row edits the form's own field instead.
 *   - a read-only option (set by the installer at normal priority, or a
 *     package the build chose) shows its value and says who owns it.
 *   - everything else edits one line of overrides.nix through form.extra.
 *     Unset means the default; "Use default" removes the line rather than
 *     writing the default's value, so a default that changes with an update
 *     is followed.
 *
 * The options marked dangerous (flake/options-doc.nix) ask once per option
 * per visit before the first change goes into the draft: a wrong value for
 * one of them leaves a box with no shell to recover from. Text fields commit
 * on blur or Enter, not per keystroke, so the question is asked about a
 * value and not about its first character.
 */

export function AdvancedPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const [search, setSearch] = React.useState("");
  const searchId = React.useId();
  const options = form.options;
  const disabled = form.locked || !form.ready;

  /* The dangerous options the owner has already said yes to this visit, and
   * the change waiting on the dialog. `revision` bumps when a change is
   * declined so a text editor holding the declined text goes back to the
   * draft's value. */
  const [confirmed, setConfirmed] = React.useState<ReadonlySet<string>>(() => new Set());
  const [pending, setPending] = React.useState<{ name: string; commit: () => void } | null>(null);
  const [revision, setRevision] = React.useState(0);
  const dialogTitleId = React.useId();
  const dialogBodyId = React.useId();

  const guard = React.useCallback(
    (option: OptionDoc, commit: () => void) => {
      if (!option.danger || confirmed.has(option.name)) {
        commit();
        return;
      }
      setPending({ name: option.name, commit });
    },
    [confirmed],
  );

  const decline = () => {
    setPending(null);
    setRevision((n) => n + 1);
  };
  const accept = () => {
    if (pending === null) return;
    setConfirmed((current) => new Set(current).add(pending.name));
    pending.commit();
    setPending(null);
  };

  if (options === null) {
    return (
      <PaneSection>
        <Group>
          {[0, 1, 2, 3, 4].map((row) => (
            <Row key={row} last={row === 4}>
              <Skeleton className="h-4 w-56" />
              <Skeleton className="h-6 w-20" />
            </Row>
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

  const query = fold(search.trim());
  const shown = options.options.filter((option) => matches(option, query));
  const groups = groupBy(shown);
  const strays = options.stray.filter((stray) => stray.key in form.extra);

  return (
    <>
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <label htmlFor={searchId} className="sr-only">
          {t("advanced.search.label")}
        </label>
        <Input
          id={searchId}
          type="search"
          value={search}
          placeholder={t("advanced.search.placeholder")}
          autoComplete="off"
          spellCheck={false}
          className="max-w-xs"
          onChange={(event) => setSearch(event.target.value)}
        />
        <span className="text-[12.5px] text-muted tabular-nums" data-testid="advanced-count">
          {t("advanced.search.count", { count: shown.length })}
        </span>
      </div>

      {shown.length === 0 && (
        <p className="px-1.5 py-6 text-center text-[13px] text-muted" role="status">
          {t("advanced.search.noMatch", { query: search.trim() })}
        </p>
      )}

      {groups.map(([group, rows]) => (
        <PaneSection key={group}>
          <GroupTitle>
            <span className="numeric">{group === "general" ? t("advanced.group.general") : `losos.${group}`}</span>
          </GroupTitle>
          <Group>
            {rows.map((option, index) => (
              <OptionRow
                key={`${option.name}:${revision}`}
                option={option}
                form={form}
                disabled={disabled}
                guard={guard}
                last={index === rows.length - 1}
              />
            ))}
          </Group>
        </PaneSection>
      ))}

      {strays.length > 0 && query.length === 0 && (
        <PaneSection>
          <GroupTitle>{t("advanced.stray.title")}</GroupTitle>
          <Group className="border-crit/35">
            {strays.map((stray, index) => (
              <Row key={stray.key} last={index === strays.length - 1} data-option={stray.key} data-kind="stray">
                <RowText
                  title={
                    <span className="numeric text-[13px]">
                      losos.{stray.key} = {stray.value};
                    </span>
                  }
                  detail={<Badge variant="destructive">{t("advanced.badge.stray")}</Badge>}
                />
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={disabled}
                  onClick={() => form.setExtra(stray.key, null)}
                >
                  {t("advanced.stray.remove")}
                </Button>
              </Row>
            ))}
          </Group>
          <GroupCaption>{t("advanced.stray.caption")}</GroupCaption>
        </PaneSection>
      )}

      {query.length === 0 && (
        <PaneSection>
          <GroupCaption className="pt-0">{t("advanced.caption")}</GroupCaption>
          <GroupCaption>
            {t("advanced.commits")}{" "}
            <PaneLink pane="history" className="text-accent underline-offset-4 hover:underline">
              {paneById("history").label}
            </PaneLink>
          </GroupCaption>
          {Object.entries(options.excluded).map(([prefix, reason]) => (
            <GroupCaption key={prefix}>{t("advanced.excluded", { prefix, reason })}</GroupCaption>
          ))}
        </PaneSection>
      )}

      <Dialog
        open={pending !== null}
        onOpenChange={(open) => {
          if (!open) decline();
        }}
        labelledBy={dialogTitleId}
        describedBy={dialogBodyId}
      >
        <DialogHeader>
          <DialogTitle id={dialogTitleId} className="flex items-center gap-2">
            <HugeiconsIcon
              icon={Alert02Icon}
              size={19}
              strokeWidth={1.5}
              color="currentColor"
              className="text-warn"
              aria-hidden="true"
            />
            <span className="numeric">{t("advanced.danger.title", { name: pending?.name ?? "" })}</span>
          </DialogTitle>
          <DialogDescription id={dialogBodyId}>{t("advanced.danger.body")}</DialogDescription>
        </DialogHeader>
        <DialogBody />
        <DialogFooter>
          <Button variant="ghost" onClick={decline}>
            {t("advanced.danger.keep")}
          </Button>
          <Button variant="destructive" onClick={accept}>
            {t("advanced.danger.confirm")}
          </Button>
        </DialogFooter>
      </Dialog>
    </>
  );
}

// ── Rows ──────────────────────────────────────────────────────────────────

type Guard = (option: OptionDoc, commit: () => void) => void;

function OptionRow({
  option,
  form,
  disabled,
  guard,
  last,
}: {
  option: OptionDoc;
  form: SettingsForm;
  disabled: boolean;
  guard: Guard;
  last: boolean;
}) {
  const t = useT();
  const controlId = React.useId();
  const errorId = React.useId();
  const owned = OWNED[option.name];
  const ownerPlanned = owned !== undefined && paneById(owned.pane).planned;
  const extraRaw = form.extra[option.name] ?? null;
  const problem = form.extraProblems[option.name] ?? null;
  const defaultRaw = toLiteral(option.editor, option.default);

  /* What the row edits: the form's own field for an owned option whose pane
   * is planned, the extra line for everything editable, nothing otherwise. */
  let raw: string | null = extraRaw;
  let isSet = extraRaw !== null;
  if (owned !== undefined && form.draft !== null) {
    raw = ownedLiteral(option.editor, form.draft[owned.key]);
    isSet = true;
  }

  const commit = (next: string | null) => {
    /* A value that is the default, on an option the box had unset, stays
     * unset: toggling a switch off and on again is no change, and the file
     * does not gain a line restating a default it already follows. */
    if (next !== null && next === defaultRaw && !(option.name in form.savedExtra)) next = null;
    if (owned !== undefined) {
      if (next === null) return;
      const value = ownedValue(option.editor, next);
      if (value !== null) form.set(owned.key, value as never);
      return;
    }
    form.setExtra(option.name, next);
  };

  const editable = !option.readOnly && (owned === undefined || ownerPlanned);

  const badges = (
    <span className="mt-1 flex flex-wrap gap-1.5">
      {option.danger && <Badge variant="warn">{t("advanced.badge.danger")}</Badge>}
      {option.fixed !== null && (
        <Badge variant="outline">
          {option.fixed === "installer"
            ? t("advanced.badge.installer")
            : t("advanced.badge.fixed", { owner: option.fixed })}
        </Badge>
      )}
      {option.editor.kind === "opaque" && <Badge variant="outline">{t("advanced.badge.flake")}</Badge>}
      {editable && isSet && owned === undefined && <Badge variant="local">{t("advanced.badge.set")}</Badge>}
    </span>
  );

  const detail = (
    <>
      <span className="block whitespace-pre-line">{option.description.trim()}</span>
      <span className="numeric mt-1 block text-[12px] text-faint">
        {option.defaultText === null
          ? t("advanced.row.noDefault")
          : t("advanced.row.default", { text: option.defaultText })}
        {" · "}
        {t("advanced.row.running", { text: describe(option.editor, option.current, t) })}
      </span>
      {badges}
    </>
  );

  return (
    <Row
      last={last}
      className="max-sm:flex-col max-sm:items-stretch"
      data-option={option.name}
      data-kind={option.editor.kind}
      data-set={isSet ? "true" : "false"}
    >
      <RowText
        htmlFor={editable ? controlId : undefined}
        title={<span className="numeric text-[13px]">losos.{option.name}</span>}
        detail={detail}
      />
      <div className="flex shrink-0 flex-col items-end gap-1.5 max-sm:items-stretch">
        {option.readOnly ? (
          <RowValue className="max-w-[18rem] text-right whitespace-normal break-all">
            {describe(option.editor, option.current, t)}
          </RowValue>
        ) : owned !== undefined && !ownerPlanned ? (
          <>
            <RowValue className="text-ink">{describe(option.editor, draftValue(form, owned.key), t)}</RowValue>
            <PaneLink pane={owned.pane} className="text-[12.5px] text-accent underline-offset-4 hover:underline">
              {t("advanced.owned.changeIn", { pane: paneById(owned.pane).label })}
            </PaneLink>
          </>
        ) : (
          <>
            <ValueEditor
              id={controlId}
              name={option.name}
              editor={option.editor}
              raw={raw}
              defaultRaw={defaultRaw}
              disabled={disabled}
              invalid={problem !== null}
              errorId={errorId}
              onChange={(next) => guard(option, () => commit(next))}
            />
            {owned === undefined && isSet && (
              <Button
                variant="ghost"
                size="xs"
                disabled={disabled}
                onClick={() => guard(option, () => commit(null))}
              >
                <HugeiconsIcon icon={Cancel01Icon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("advanced.useDefault")}
              </Button>
            )}
            <FieldError id={errorId}>{problem}</FieldError>
          </>
        )}
      </div>
    </Row>
  );
}

function draftValue(form: SettingsForm, key: keyof SettingsResponse): OptionValue {
  const draft = form.draft;
  if (draft === null) return null;
  const value = draft[key];
  return typeof value === "number" && Number.isNaN(value) ? null : value;
}

// ── Editors ───────────────────────────────────────────────────────────────

interface EditorProps {
  id: string;
  name: string;
  editor: OptionEditor;
  /** The draft's literal, or null for "unset: the default". */
  raw: string | null;
  defaultRaw: string | null;
  disabled: boolean;
  invalid: boolean;
  errorId: string;
  /** A new literal, or null to go back to the default. */
  onChange: (raw: string | null) => void;
}

function ValueEditor(props: EditorProps) {
  const { editor } = props;
  switch (editor.kind) {
    case "bool":
      return <BoolEditor {...props} />;
    case "enum":
      return <EnumEditor {...props} values={editor.values} />;
    case "int":
      return <TextEditor {...props} inputMode="numeric" width="w-[8rem]" />;
    case "float":
      return <TextEditor {...props} inputMode="decimal" width="w-[8rem]" />;
    case "str":
      return <TextEditor {...props} quoted width={editor.form === "path" ? "w-[20rem]" : "w-[16rem]"} />;
    case "list":
      return <ListEditor {...props} />;
    case "nullable":
      return <NullableEditor {...props} inner={editor.inner} />;
    case "opaque":
      // Read-only rows never reach an editor; see OptionRow.
      return null;
    default: {
      const exhaustive: never = editor;
      return exhaustive;
    }
  }
}

function BoolEditor({ id, name, raw, defaultRaw, disabled, onChange }: EditorProps) {
  const t = useT();
  const on = (raw ?? defaultRaw) === "true";
  return (
    <Switch
      id={id}
      aria-label={t("advanced.value.label", { name })}
      isSelected={on}
      isDisabled={disabled}
      onChange={(next) => onChange(next ? "true" : "false")}
    />
  );
}

function EnumEditor({ id, name, raw, defaultRaw, disabled, invalid, errorId, values, onChange }: EditorProps & { values: string[] }) {
  const t = useT();
  const value = nixUnquote(raw ?? defaultRaw ?? '""') ?? "";
  return (
    <NativeSelect
      id={id}
      aria-label={t("advanced.value.label", { name })}
      aria-invalid={invalid}
      aria-describedby={invalid ? errorId : undefined}
      value={value}
      disabled={disabled}
      onChange={(event) => onChange(nixString(event.target.value))}
    >
      {values.map((option) => (
        <NativeSelectOption key={option} value={option}>
          {option}
        </NativeSelectOption>
      ))}
    </NativeSelect>
  );
}

/* A number or a string, committed on blur or Enter. `quoted` editors show
 * the string inside the literal and write it back quoted; the others show
 * the literal itself. Empty goes back to the default. */
function TextEditor({
  id,
  name,
  raw,
  defaultRaw,
  disabled,
  invalid,
  errorId,
  onChange,
  quoted = false,
  inputMode,
  width,
  hint,
}: EditorProps & {
  quoted?: boolean;
  inputMode?: "numeric" | "decimal";
  width: string;
  hint?: string;
}) {
  const t = useT();
  const [text, setText] = React.useState(() => showLiteral(raw, quoted));
  // The draft moved under us (discard, a reload): follow it.
  React.useEffect(() => setText(showLiteral(raw, quoted)), [raw, quoted]);

  const commit = () => {
    const trimmed = text.trim();
    if (trimmed === showLiteral(raw, quoted)) return;
    if (trimmed.length === 0) {
      onChange(null);
      return;
    }
    onChange(quoted ? nixString(trimmed) : trimmed);
  };

  return (
    <>
      <Input
        id={id}
        aria-label={t("advanced.value.label", { name })}
        aria-invalid={invalid}
        aria-describedby={invalid ? errorId : undefined}
        value={text}
        placeholder={showLiteral(defaultRaw, quoted)}
        inputMode={inputMode}
        autoComplete="off"
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
        disabled={disabled}
        className={cn("numeric", width, "max-w-full")}
        onChange={(event) => setText(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            commit();
          }
        }}
      />
      {hint !== undefined && <span className="text-[11.5px] text-faint">{hint}</span>}
    </>
  );
}

/** The text a field shows for a literal: the string inside a quoted one. */
function showLiteral(literal: string | null, quoted: boolean): string {
  if (literal === null) return "";
  return quoted ? (nixUnquote(literal) ?? literal) : literal;
}

function ListEditor({ id, name, raw, defaultRaw, disabled, invalid, errorId, onChange }: EditorProps) {
  const t = useT();
  const [text, setText] = React.useState(() => showLines(raw));
  React.useEffect(() => setText(showLines(raw)), [raw]);

  const commit = () => {
    if (text === showLines(raw)) return;
    const items = text
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0);
    onChange(items.length === 0 && raw === null ? null : nixList(items));
  };

  return (
    <>
      <Textarea
        id={id}
        aria-label={t("advanced.value.label", { name })}
        aria-invalid={invalid}
        aria-describedby={invalid ? errorId : undefined}
        value={text}
        placeholder={showLines(defaultRaw)}
        rows={3}
        spellCheck={false}
        disabled={disabled}
        className="numeric w-[20rem] max-w-full"
        onChange={(event) => setText(event.target.value)}
        onBlur={commit}
      />
      <span className="text-[11.5px] text-faint">{t("advanced.list.hint")}</span>
    </>
  );
}

function showLines(literal: string | null): string {
  if (literal === null) return "";
  return (nixStringList(literal) ?? [literal]).join("\n");
}

/* `null` or the inner kind. An empty field is null (unset, which for every
 * nullable option on this box is also the default). */
function NullableEditor({ inner, ...props }: EditorProps & { inner: OptionEditor }) {
  const t = useT();
  const raw = props.raw === "null" ? null : props.raw;
  const defaultRaw = props.defaultRaw === "null" ? null : props.defaultRaw;
  const onChange = (next: string | null) => props.onChange(next === null ? (props.defaultRaw === "null" ? null : "null") : next);
  if (inner.kind === "str") {
    return (
      <TextEditor
        {...props}
        raw={raw}
        defaultRaw={defaultRaw}
        onChange={onChange}
        quoted
        width={inner.form === "path" ? "w-[20rem]" : "w-[16rem]"}
        hint={t("advanced.nullable.hint")}
      />
    );
  }
  return <ValueEditor {...props} editor={inner} raw={raw} defaultRaw={defaultRaw} onChange={onChange} />;
}

// ── Helpers ───────────────────────────────────────────────────────────────

/* The value a document carries, as a sentence fragment for the row: on/off
 * for a switch, the text for a string, a package's name, a list joined. */
function describe(editor: OptionEditor, value: OptionValue, t: ReturnType<typeof useT>): string {
  if (value === null) return t("advanced.none");
  if (typeof value === "boolean") return value ? t("advanced.on") : t("advanced.off");
  if (typeof value === "number") return editor.kind === "float" && Number.isInteger(value) ? value.toFixed(1) : String(value);
  if (typeof value === "string") return value.length === 0 ? "“”" : value;
  if (Array.isArray(value)) return value.length === 0 ? t("advanced.none") : value.map((v) => describe(editor, v, t)).join(", ");
  if ("package" in value) return value.package;
  return value.nix;
}

/** Lower-case and strip diacritics, as the sidebar's search does. */
function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");
}

function matches(option: OptionDoc, query: string): boolean {
  if (query.length === 0) return true;
  const haystack = fold(`losos.${option.name} ${option.group} ${option.description} ${option.nixType}`);
  return query.split(/\s+/).every((term) => haystack.includes(term));
}

/* Rows by group, in the document's order (sorted by name, so the groups are
 * alphabetical) with the top-level options first. */
function groupBy(options: readonly OptionDoc[]): [string, OptionDoc[]][] {
  const groups = new Map<string, OptionDoc[]>();
  for (const option of options) {
    const rows = groups.get(option.group);
    if (rows === undefined) groups.set(option.group, [option]);
    else rows.push(option);
  }
  const general = groups.get("general");
  groups.delete("general");
  const rest = [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
  return general === undefined ? rest : [["general", general], ...rest];
}

/* A link to another pane: the router's Link when there is a router above
 * (the app), a plain anchor otherwise (a test rendering the screen alone). */
function PaneLink({
  pane,
  className,
  children,
}: {
  pane: Parameters<typeof paneHref>[0];
  className?: string;
  children: React.ReactNode;
}) {
  const href = paneHref(pane);
  const routed = useInRouterContext();
  return routed ? (
    <Link to={href} className={className}>
      {children}
    </Link>
  ) : (
    <a href={href} className={className}>
      {children}
    </a>
  );
}
