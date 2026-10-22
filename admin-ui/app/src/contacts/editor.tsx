/* A contact's form: a new one, or one from the book. The fields people
 * fill in most come first (name, email, phone, chat); the rest open under
 * "More fields", and open by themselves when the contact has any. The box
 * cleans every field again when it saves (backend/src/vcard.rs), so this
 * form only has to be pleasant, not strict. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon, Camera01Icon, Cancel01Icon, Delete02Icon } from "@hugeicons/core-free-icons";
import { PersonAvatar } from "@/components/person";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import {
  removeContactPhoto,
  saveContact,
  setContactPhoto,
  type Contact,
  type ContactAddress,
  type ContactFields,
  type ContactLabelled,
} from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { PeopleField } from "./suggest";

const LABELS = ["", "home", "work", "mobile", "main", "fax", "pager", "other"] as const;

export function labelText(label: string, t: ReturnType<typeof useT>): string {
  return (LABELS as readonly string[]).includes(label) && label !== ""
    ? t(`contacts.label.${label}` as MessageKey)
    : label;
}

function LabelSelect({ value, onChange, phone }: { value: string; onChange: (v: string) => void; phone?: boolean }) {
  const t = useT();
  const options = LABELS.filter((l) => phone === true || (l !== "mobile" && l !== "fax" && l !== "pager"));
  return (
    <NativeSelect
      aria-label={t("contacts.field.label")}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      className="w-32 shrink-0"
    >
      {options.map((l) => (
        <NativeSelectOption key={l} value={l}>
          {l === "" ? t("contacts.label.none") : t(`contacts.label.${l}` as MessageKey)}
        </NativeSelectOption>
      ))}
      {!(LABELS as readonly string[]).includes(value) && <NativeSelectOption value={value}>{value}</NativeSelectOption>}
    </NativeSelect>
  );
}

/* A list of labelled values: emails, phones, websites. */
function Rows({
  id,
  label,
  rows,
  onChange,
  type,
  phone,
}: {
  id: string;
  label: string;
  rows: ContactLabelled[];
  onChange: (rows: ContactLabelled[]) => void;
  type: "email" | "tel" | "url";
  phone?: boolean;
}) {
  const t = useT();
  const shown = rows.length > 0 ? rows : [{ value: "", label: "" }];
  const set = (i: number, row: ContactLabelled) => onChange(shown.map((r, j) => (j === i ? row : r)));
  return (
    <fieldset className="flex flex-col gap-1.5">
      <legend className="mb-1.5 text-[13px] font-medium">{label}</legend>
      {shown.map((row, i) => (
        <div key={i} className="flex items-center gap-2">
          <Input
            id={i === 0 ? id : `${id}-${i}`}
            aria-label={label}
            type={type}
            value={row.value}
            onChange={(event) => set(i, { ...row, value: event.target.value })}
            className="min-w-0 flex-1"
          />
          <LabelSelect value={row.label} onChange={(l) => set(i, { ...row, label: l })} phone={phone ?? false} />
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label={t("contacts.removeRow")}
            onClick={() => onChange(shown.filter((_, j) => j !== i))}
            disabled={shown.length === 1 && row.value === ""}
          >
            <HugeiconsIcon icon={Cancel01Icon} size={16} strokeWidth={1.5} aria-hidden="true" />
          </Button>
        </div>
      ))}
      <Button
        type="button"
        variant="link"
        size="xs"
        className="self-start px-0"
        onClick={() => onChange([...shown, { value: "", label: "" }])}
      >
        <HugeiconsIcon icon={Add01Icon} size={14} strokeWidth={1.5} aria-hidden="true" />
        {t("contacts.addRow", { what: label.toLowerCase() })}
      </Button>
    </fieldset>
  );
}

const EMPTY_ADDRESS: ContactAddress = { street: "", locality: "", region: "", code: "", country: "", label: "" };

function hasMore(c: ContactFields): boolean {
  return (
    (c.nickname ?? "") !== "" ||
    (c.title ?? "") !== "" ||
    (c.addresses ?? []).length > 0 ||
    (c.urls ?? []).length > 0 ||
    (c.birthday ?? "") !== "" ||
    (c.note ?? "") !== "" ||
    (c.mesh ?? "") !== ""
  );
}

export function ContactEditor({
  contact,
  initial,
  onClose,
  onSaved,
}: {
  /** The contact being edited; absent for a new one. */
  contact?: Contact;
  /** Fields to start a new contact from (someone on this box). */
  initial?: ContactFields;
  onClose: () => void;
  onSaved: (saved: Contact) => void;
}) {
  const t = useT();
  const start: ContactFields = contact ?? initial ?? { name: "" };
  const [fields, setFields] = React.useState<ContactFields>(start);
  const [more, setMore] = React.useState(hasMore(start));
  const [photo, setPhoto] = React.useState<{ file: File; url: string } | "remove" | null>(null);
  const [busy, setBusy] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);
  const fileInput = React.useRef<HTMLInputElement>(null);
  const set = (patch: Partial<ContactFields>) => setFields((f) => ({ ...f, ...patch }));

  React.useEffect(
    () => () => {
      if (photo !== null && photo !== "remove") URL.revokeObjectURL(photo.url);
    },
    [photo],
  );

  const shownPicture =
    photo === "remove" ? null : photo !== null ? photo.url : (contact?.photo ?? null);
  const addresses = fields.addresses ?? [];

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setProblem(null);
    try {
      const clean = (rows: ContactLabelled[] | undefined) => (rows ?? []).filter((r) => r.value.trim() !== "");
      let saved = await saveContact(contact?.id ?? null, {
        ...fields,
        emails: clean(fields.emails),
        phones: clean(fields.phones),
        urls: clean(fields.urls),
        addresses: addresses.filter((a) => Object.entries(a).some(([k, v]) => k !== "label" && v.trim() !== "")),
      });
      if (photo === "remove" && saved.photo !== undefined) saved = await removeContactPhoto(saved.id);
      else if (photo !== null && photo !== "remove") saved = await setContactPhoto(saved.id, photo.file);
      onSaved(saved);
    } catch (error) {
      setProblem(error instanceof Error ? error.message : String(error));
      setBusy(false);
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(next) => {
        if (!next && !busy) onClose();
      }}
      labelledBy="contact-editor-title"
      dialogClassName="w-[min(36rem,calc(100vw-1rem))]"
    >
      <form onSubmit={(event) => void save(event)} noValidate>
        <DialogHeader>
          <DialogTitle id="contact-editor-title">
            {contact === undefined ? t("contacts.edit.newTitle") : t("contacts.edit.title")}
          </DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          <div className="flex items-center gap-4">
            <PersonAvatar
              person={{ name: fields.name || "?", displayName: fields.name, picture: shownPicture }}
              size="lg"
            />
            <div className="flex flex-wrap gap-2">
              <input
                ref={fileInput}
                type="file"
                accept="image/png,image/jpeg,image/gif,image/webp"
                hidden
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  if (file !== undefined) setPhoto({ file, url: URL.createObjectURL(file) });
                  event.target.value = "";
                }}
              />
              <Button type="button" variant="secondary" size="sm" onClick={() => fileInput.current?.click()}>
                <HugeiconsIcon icon={Camera01Icon} size={16} strokeWidth={1.5} aria-hidden="true" />
                {shownPicture === null ? t("contacts.edit.photoAdd") : t("contacts.edit.photoChange")}
              </Button>
              {shownPicture !== null && (
                <Button type="button" variant="ghost" size="sm" onClick={() => setPhoto("remove")}>
                  <HugeiconsIcon icon={Delete02Icon} size={16} strokeWidth={1.5} aria-hidden="true" />
                  {t("contacts.edit.photoRemove")}
                </Button>
              )}
            </div>
          </div>

          <Field>
            <FieldLabel htmlFor="contact-name">{t("contacts.field.name")}</FieldLabel>
            <Input
              id="contact-name"
              value={fields.name}
              onChange={(event) => set({ name: event.target.value })}
              autoFocus
              required
              maxLength={500}
            />
          </Field>
          <Field>
            <FieldLabel htmlFor="contact-org">{t("contacts.field.org")}</FieldLabel>
            <Input id="contact-org" value={fields.org ?? ""} onChange={(event) => set({ org: event.target.value })} />
          </Field>
          <Rows
            id="contact-email"
            label={t("contacts.field.emails")}
            rows={fields.emails ?? []}
            onChange={(emails) => set({ emails })}
            type="email"
          />
          <Rows
            id="contact-phone"
            label={t("contacts.field.phones")}
            rows={fields.phones ?? []}
            onChange={(phones) => set({ phones })}
            type="tel"
            phone
          />
          <Field>
            <FieldLabel htmlFor="contact-matrix">{t("contacts.field.matrix")}</FieldLabel>
            <PeopleField
              id="contact-matrix"
              want="matrix"
              value={fields.matrix ?? ""}
              onChange={(matrix) => set({ matrix })}
              placeholder="@name:box.local"
            />
            <FieldDescription>{t("contacts.matrixHint")}</FieldDescription>
          </Field>

          {!more ? (
            <Button type="button" variant="link" size="xs" className="self-start px-0" onClick={() => setMore(true)}>
              <HugeiconsIcon icon={Add01Icon} size={14} strokeWidth={1.5} aria-hidden="true" />
              {t("contacts.edit.more")}
            </Button>
          ) : (
            <>
              <div className="grid gap-4 sm:grid-cols-2">
                <Field>
                  <FieldLabel htmlFor="contact-nickname">{t("contacts.field.nickname")}</FieldLabel>
                  <Input
                    id="contact-nickname"
                    value={fields.nickname ?? ""}
                    onChange={(event) => set({ nickname: event.target.value })}
                  />
                </Field>
                <Field>
                  <FieldLabel htmlFor="contact-title">{t("contacts.field.title")}</FieldLabel>
                  <Input id="contact-title" value={fields.title ?? ""} onChange={(event) => set({ title: event.target.value })} />
                </Field>
              </div>
              <fieldset className="flex flex-col gap-2">
                <legend className="mb-1.5 text-[13px] font-medium">{t("contacts.field.addresses")}</legend>
                {(addresses.length > 0 ? addresses : [EMPTY_ADDRESS]).map((a, i, all) => {
                  const put = (patch: Partial<ContactAddress>) =>
                    set({ addresses: all.map((x, j) => (j === i ? { ...x, ...patch } : x)) });
                  return (
                    <div key={i} className="grid gap-2 rounded-control border border-hair p-2.5 sm:grid-cols-6">
                      <Textarea
                        aria-label={t("contacts.field.street")}
                        placeholder={t("contacts.field.street")}
                        value={a.street}
                        onChange={(event) => put({ street: event.target.value })}
                        rows={2}
                        className="sm:col-span-6"
                      />
                      <Input
                        aria-label={t("contacts.field.code")}
                        placeholder={t("contacts.field.code")}
                        value={a.code}
                        onChange={(event) => put({ code: event.target.value })}
                        className="sm:col-span-2"
                      />
                      <Input
                        aria-label={t("contacts.field.locality")}
                        placeholder={t("contacts.field.locality")}
                        value={a.locality}
                        onChange={(event) => put({ locality: event.target.value })}
                        className="sm:col-span-4"
                      />
                      <Input
                        aria-label={t("contacts.field.region")}
                        placeholder={t("contacts.field.region")}
                        value={a.region}
                        onChange={(event) => put({ region: event.target.value })}
                        className="sm:col-span-3"
                      />
                      <Input
                        aria-label={t("contacts.field.country")}
                        placeholder={t("contacts.field.country")}
                        value={a.country}
                        onChange={(event) => put({ country: event.target.value })}
                        className="sm:col-span-3"
                      />
                      <div className="flex items-center justify-between gap-2 sm:col-span-6">
                        <LabelSelect value={a.label} onChange={(label) => put({ label })} />
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          onClick={() => set({ addresses: all.filter((_, j) => j !== i) })}
                        >
                          {t("contacts.removeRow")}
                        </Button>
                      </div>
                    </div>
                  );
                })}
                <Button
                  type="button"
                  variant="link"
                  size="xs"
                  className="self-start px-0"
                  onClick={() => set({ addresses: [...(addresses.length > 0 ? addresses : [EMPTY_ADDRESS]), EMPTY_ADDRESS] })}
                >
                  <HugeiconsIcon icon={Add01Icon} size={14} strokeWidth={1.5} aria-hidden="true" />
                  {t("contacts.addRow", { what: t("contacts.field.addresses").toLowerCase() })}
                </Button>
              </fieldset>
              <Rows
                id="contact-url"
                label={t("contacts.field.urls")}
                rows={fields.urls ?? []}
                onChange={(urls) => set({ urls })}
                type="url"
              />
              <Field>
                <FieldLabel htmlFor="contact-birthday">{t("contacts.field.birthday")}</FieldLabel>
                <Input
                  id="contact-birthday"
                  value={fields.birthday ?? ""}
                  onChange={(event) => set({ birthday: event.target.value })}
                  placeholder="1990-04-12"
                  inputMode="numeric"
                  className="w-44"
                />
                <FieldDescription>{t("contacts.birthdayHint")}</FieldDescription>
              </Field>
              <Field>
                <FieldLabel htmlFor="contact-mesh">{t("contacts.field.mesh")}</FieldLabel>
                <Input
                  id="contact-mesh"
                  value={fields.mesh ?? ""}
                  onChange={(event) => set({ mesh: event.target.value })}
                  placeholder="eva@kitchenbox.local"
                />
                <FieldDescription>{t("contacts.meshHint")}</FieldDescription>
              </Field>
              <Field>
                <FieldLabel htmlFor="contact-note">{t("contacts.field.note")}</FieldLabel>
                <Textarea
                  id="contact-note"
                  value={fields.note ?? ""}
                  onChange={(event) => set({ note: event.target.value })}
                  rows={3}
                  maxLength={4000}
                />
              </Field>
            </>
          )}
          {problem !== null && (
            <Alert variant="crit" role="alert">
              <AlertDescription>{problem}</AlertDescription>
            </Alert>
          )}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t("contacts.edit.cancel")}
          </Button>
          <Button type="submit" disabled={busy || fields.name.trim() === ""}>
            {busy && <Spinner className="size-4" />}
            {t("contacts.edit.save")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
