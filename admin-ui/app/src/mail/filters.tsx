/* A person's mail filters: the list, in the order they run, and the editor
 * for one. Every change saves the whole list (PUT /api/mail/filters), which
 * the mail plugin writes as the person's Sieve script; the mail server runs
 * it as mail arrives, with this page closed (backend/plugins/mail/src/sieve.rs). */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Add01Icon,
  ArrowDown01Icon,
  ArrowUp01Icon,
  Cancel01Icon,
  Delete02Icon,
  FilterIcon,
  PencilEdit02Icon,
} from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia } from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { toast } from "@/components/ui/toast";
import { Group, Row, RowText } from "@/screens/settings/rows";
import { getMailFilters, putMailFilters, type MailFilter, type MailFilterField, type MailFolder } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { tree } from "./folders";

const FIELDS: readonly { value: MailFilterField; label: MessageKey }[] = [
  { value: "from", label: "mail.filters.field.from" },
  { value: "to", label: "mail.filters.field.to" },
  { value: "subject", label: "mail.filters.field.subject" },
];

export function blankFilter(): MailFilter {
  return {
    name: "",
    enabled: true,
    match: "all",
    conditions: [{ field: "from", value: "" }],
    moveTo: null,
    markRead: false,
    flag: false,
  };
}

/* What a filter does, in one line: its conditions, then its actions. */
function summary(f: MailFilter, folderLabel: (id: string) => string | null, t: ReturnType<typeof useT>): string {
  const field = (v: MailFilterField) => t(FIELDS.find((x) => x.value === v)?.label ?? "mail.filters.field.from");
  const conditions = f.conditions
    .map((c) => t("mail.filters.says", { field: field(c.field), value: c.value }))
    .join(` ${t(f.match === "all" ? "mail.filters.and" : "mail.filters.or")} `);
  const actions = [
    f.moveTo !== null ? (folderLabel(f.moveTo) ?? f.moveTo) : null,
    f.markRead ? t("mail.filters.markRead") : null,
    f.flag ? t("mail.filters.flag") : null,
  ].filter((a): a is string => a !== null);
  return actions.length > 0 ? `${conditions} → ${actions.join(", ")}` : conditions;
}

export function FiltersDialog({
  folders,
  label,
  start,
  onClose,
}: {
  folders: readonly MailFolder[];
  label: (f: MailFolder) => string;
  /** A filter to open the editor on, from a message's menu. */
  start: MailFilter | null;
  onClose: () => void;
}) {
  const t = useT();
  const id = React.useId();
  const [filters, setFilters] = React.useState<MailFilter[] | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  /* The filter being edited, and where it goes back: its index, or null for
   * a new one at the end. */
  const [editing, setEditing] = React.useState<{ at: number | null; filter: MailFilter } | null>(
    start !== null ? { at: null, filter: start } : null,
  );

  React.useEffect(() => {
    const controller = new AbortController();
    getMailFilters({ signal: controller.signal }).then(
      (r) => setFilters(r.filters),
      (error: unknown) => {
        if (!controller.signal.aborted) setProblem(error instanceof Error ? error.message : String(error));
      },
    );
    return () => controller.abort();
  }, []);

  const folderLabel = (fid: string) => {
    const f = folders.find((x) => x.id === fid);
    return f !== undefined ? label(f) : null;
  };

  /* Save the whole list; the box answers with what it stored. */
  const save = async (next: MailFilter[]): Promise<boolean> => {
    setBusy(true);
    setProblem(null);
    try {
      const r = await putMailFilters(next);
      setFilters(r.filters);
      toast.done(t("mail.filters.saved"));
      return true;
    } catch (error) {
      setProblem(error instanceof Error ? error.message : String(error));
      return false;
    } finally {
      setBusy(false);
    }
  };

  const move = (from: number, to: number) => {
    if (filters === null || to < 0 || to >= filters.length) return;
    const next = [...filters];
    next.splice(to, 0, ...next.splice(from, 1));
    void save(next);
  };

  if (editing !== null) {
    return (
      <FilterEditor
        initial={editing.filter}
        isNew={editing.at === null}
        folders={folders}
        label={label}
        busy={busy || filters === null}
        problem={problem}
        onCancel={() => {
          setProblem(null);
          if (start !== null && editing.at === null && filters !== null && filters.length === 0) onClose();
          else setEditing(null);
        }}
        onSave={async (filter) => {
          if (filters === null) return;
          const next = editing.at === null ? [...filters, filter] : filters.map((f, i) => (i === editing.at ? filter : f));
          if (await save(next)) setEditing(null);
        }}
      />
    );
  }

  return (
    <Dialog
      open
      onOpenChange={(next) => !next && !busy && onClose()}
      labelledBy={`${id}-title`}
      describedBy={`${id}-text`}
      dialogClassName="w-[min(40rem,calc(100vw-1rem))]"
    >
      <DialogHeader>
        <DialogTitle id={`${id}-title`}>{t("mail.filters")}</DialogTitle>
        <DialogDescription id={`${id}-text`}>{t("mail.filters.text")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        {filters === null && problem === null && (
          <div className="grid place-items-center p-6">
            <Spinner />
          </div>
        )}
        {filters !== null && filters.length === 0 && (
          <Empty className="border-0 p-4">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={FilterIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </EmptyMedia>
              <EmptyDescription>{t("mail.filters.none")}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        )}
        {filters !== null && filters.length > 0 && (
          <Group>
            <ol aria-label={t("mail.filters")}>
              {filters.map((f, i) => (
                <li key={`${i}-${f.name}`} data-filter={f.name}>
                  <Row className="gap-2">
                    <Switch
                      size="sm"
                      aria-label={t("mail.filters.use", { name: f.name })}
                      isSelected={f.enabled}
                      isDisabled={busy}
                      onChange={(on) => void save(filters.map((x, j) => (j === i ? { ...x, enabled: on } : x)))}
                    />
                    <RowText
                      className="ml-1"
                      title={
                        <span className="flex items-center gap-2">
                          <span className="truncate">{f.name}</span>
                          {!f.enabled && <Badge variant="secondary">{t("mail.filters.off")}</Badge>}
                        </span>
                      }
                      detail={summary(f, folderLabel, t)}
                    />
                    <span className="flex shrink-0 items-center">
                      <IconButton icon={ArrowUp01Icon} label={t("mail.filters.up")} disabled={busy || i === 0} onClick={() => move(i, i - 1)} />
                      <IconButton
                        icon={ArrowDown01Icon}
                        label={t("mail.filters.down")}
                        disabled={busy || i === filters.length - 1}
                        onClick={() => move(i, i + 1)}
                      />
                      <IconButton icon={PencilEdit02Icon} label={t("mail.filters.edit")} disabled={busy} onClick={() => setEditing({ at: i, filter: f })} />
                      <IconButton
                        icon={Delete02Icon}
                        label={t("mail.filters.delete")}
                        disabled={busy}
                        onClick={() => void save(filters.filter((_, j) => j !== i))}
                      />
                    </span>
                  </Row>
                </li>
              ))}
            </ol>
          </Group>
        )}
        {problem !== null && (
          <p role="alert" className="text-sm text-crit">
            {problem}
          </p>
        )}
      </DialogBody>
      <DialogFooter>
        <Button variant="secondary" disabled={busy || filters === null} onClick={() => setEditing({ at: null, filter: blankFilter() })}>
          <HugeiconsIcon icon={Add01Icon} className="size-4" aria-hidden="true" />
          {t("mail.filters.new")}
        </Button>
        <Button onClick={onClose} disabled={busy}>
          {t("mail.filters.done")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

function IconButton({
  icon,
  label,
  disabled,
  onClick,
}: {
  icon: typeof Add01Icon;
  label: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <Button variant="ghost" size="icon-sm" aria-label={label} title={label} disabled={disabled} onClick={onClick}>
      <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
    </Button>
  );
}

function FilterEditor({
  initial,
  isNew,
  folders,
  label,
  busy,
  problem,
  onCancel,
  onSave,
}: {
  initial: MailFilter;
  isNew: boolean;
  folders: readonly MailFolder[];
  label: (f: MailFolder) => string;
  busy: boolean;
  problem: string | null;
  onCancel: () => void;
  onSave: (filter: MailFilter) => Promise<void>;
}) {
  const t = useT();
  const id = React.useId();
  const [f, setF] = React.useState<MailFilter>(initial);
  const set = (change: Partial<MailFilter>) => setF((prev) => ({ ...prev, ...change }));
  const setCondition = (i: number, change: Partial<MailFilter["conditions"][number]>) =>
    set({ conditions: f.conditions.map((c, j) => (j === i ? { ...c, ...change } : c)) });

  const nothing = f.moveTo === null && !f.markRead && !f.flag;
  const ready = f.name.trim() !== "" && f.conditions.length > 0 && f.conditions.every((c) => c.value.trim() !== "") && !nothing;
  // The inbox is where a message lands; drafts and sent are not for incoming mail.
  const targets = tree(folders).filter(({ folder }) => folder.role !== "inbox" && folder.role !== "drafts" && folder.role !== "sent");

  return (
    <Dialog
      open
      onOpenChange={(next) => !next && !busy && onCancel()}
      labelledBy={`${id}-title`}
      dialogClassName="w-[min(40rem,calc(100vw-1rem))]"
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (ready && !busy) void onSave({ ...f, name: f.name.trim() });
        }}
      >
        <DialogHeader>
          <DialogTitle id={`${id}-title`}>{isNew ? t("mail.filters.new") : t("mail.filters.editTitle")}</DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          <Field>
            <FieldLabel htmlFor={`${id}-name`}>{t("mail.filters.name")}</FieldLabel>
            <Input id={`${id}-name`} value={f.name} maxLength={200} autoFocus autoComplete="off" onChange={(e) => set({ name: e.target.value })} />
          </Field>

          <fieldset className="flex flex-col gap-2">
            <legend className="mb-1.5 flex flex-wrap items-center gap-2 text-sm font-medium">
              {t("mail.filters.when")}
              <NativeSelect
                aria-label={t("mail.filters.when")}
                value={f.match}
                onChange={(e) => set({ match: e.target.value === "any" ? "any" : "all" })}
                className="[&_select]:h-8 [&_select]:text-[13px]"
              >
                <NativeSelectOption value="all">{t("mail.filters.matchAll")}</NativeSelectOption>
                <NativeSelectOption value="any">{t("mail.filters.matchAny")}</NativeSelectOption>
              </NativeSelect>
            </legend>
            {f.conditions.map((c, i) => (
              <div key={i} className="flex flex-wrap items-center gap-2 sm:flex-nowrap" data-condition={i}>
                <NativeSelect
                  aria-label={t("mail.filters.field.from") + " / " + t("mail.filters.field.to") + " / " + t("mail.filters.field.subject")}
                  value={c.field}
                  onChange={(e) => setCondition(i, { field: e.target.value as MailFilterField })}
                  className="w-36 shrink-0"
                >
                  {FIELDS.map((x) => (
                    <NativeSelectOption key={x.value} value={x.value}>
                      {t(x.label)}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
                <span className="text-sm text-muted">{t("mail.filters.contains")}</span>
                <Input
                  aria-label={t("mail.filters.value")}
                  value={c.value}
                  maxLength={200}
                  autoComplete="off"
                  className="min-w-0 flex-1"
                  onChange={(e) => setCondition(i, { value: e.target.value })}
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t("mail.filters.removeCondition")}
                  title={t("mail.filters.removeCondition")}
                  disabled={f.conditions.length === 1}
                  onClick={() => set({ conditions: f.conditions.filter((_, j) => j !== i) })}
                >
                  <HugeiconsIcon icon={Cancel01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                </Button>
              </div>
            ))}
            {f.conditions.length < 10 && (
              <div>
                <Button
                  type="button"
                  variant="link"
                  size="xs"
                  className="px-0"
                  onClick={() => set({ conditions: [...f.conditions, { field: "subject", value: "" }] })}
                >
                  {t("mail.filters.addCondition")}
                </Button>
              </div>
            )}
          </fieldset>

          <fieldset className="flex flex-col gap-2.5">
            <legend className="mb-1.5 text-sm font-medium">{t("mail.filters.then")}</legend>
            <Field>
              <FieldLabel htmlFor={`${id}-move`}>{t("mail.filters.moveTo")}</FieldLabel>
              <NativeSelect id={`${id}-move`} value={f.moveTo ?? ""} onChange={(e) => set({ moveTo: e.target.value === "" ? null : e.target.value })}>
                <NativeSelectOption value="">{t("mail.filters.stay")}</NativeSelectOption>
                {targets.map(({ folder, depth }) => (
                  <NativeSelectOption key={folder.id} value={folder.id}>
                    {" ".repeat(depth) + label(folder)}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </Field>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={f.markRead} onChange={(e) => set({ markRead: e.target.checked })} className="size-4 accent-accent" />
              {t("mail.filters.markRead")}
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={f.flag} onChange={(e) => set({ flag: e.target.checked })} className="size-4 accent-accent" />
              {t("mail.filters.flag")}
            </label>
            {nothing && <p className="text-[12.5px] text-muted">{t("mail.filters.nothing")}</p>}
          </fieldset>
          {problem !== null && (
            <p role="alert" className="text-sm text-crit">
              {problem}
            </p>
          )}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onCancel} disabled={busy}>
            {t("mail.cancel")}
          </Button>
          <Button type="submit" disabled={!ready || busy}>
            {busy && <Spinner className="size-4" />}
            {t("mail.filters.save")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
