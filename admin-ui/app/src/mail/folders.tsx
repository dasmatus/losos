/* A person's own folders in LosOS Mail: the tree they make, and the two
 * dialogs that make, rename and delete one. The routes are the mail
 * plugin's (backend/plugins/mail); LosOS Mail's own folders, the ones with
 * a role, keep their names and places and come first, as lososd lists
 * them. */

import * as React from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { ApiError, type MailFolder } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";

export interface Placed {
  folder: MailFolder;
  /** 0 at the top. */
  depth: number;
}

/* The folders as a tree, depth first: the top level in lososd's order (the
 * inbox first, the bin last), each folder's own under it. A folder whose
 * parent is not listed sits at the top. */
export function tree(folders: readonly MailFolder[]): Placed[] {
  const ids = new Set(folders.map((f) => f.id));
  const kids = new Map<string | null, MailFolder[]>();
  for (const f of folders) {
    const parent = f.parentId !== null && ids.has(f.parentId) ? f.parentId : null;
    kids.set(parent, [...(kids.get(parent) ?? []), f]);
  }
  const out: Placed[] = [];
  const seen = new Set<string>();
  const walk = (parent: string | null, depth: number) => {
    for (const folder of kids.get(parent) ?? []) {
      if (seen.has(folder.id)) continue;
      seen.add(folder.id);
      out.push({ folder, depth });
      walk(folder.id, depth + 1);
    }
  };
  walk(null, 0);
  return out;
}

/* Whether `id` is `ancestor` or sits somewhere inside it. */
export function within(folders: readonly MailFolder[], id: string, ancestor: string): boolean {
  let at: string | null = id;
  for (let i = 0; at !== null && i <= folders.length; i++) {
    if (at === ancestor) return true;
    const here: string = at;
    at = folders.find((f) => f.id === here)?.parentId ?? null;
  }
  return false;
}

const FOLDER_ERRORS: Record<string, MessageKey> = {
  exists: "mail.folder.error.exists",
  badName: "mail.folder.error.badName",
  hasFolders: "mail.folder.error.hasFolders",
  loop: "mail.folder.error.loop",
};

/* The plugin's refusal in the reader's language, where it has a word for
 * it; the box's own sentence otherwise. */
export function explainFolder(error: unknown, t: ReturnType<typeof useT>): string {
  const key = error instanceof ApiError && error.code !== null ? FOLDER_ERRORS[error.code] : undefined;
  if (key !== undefined) return t(key);
  return error instanceof Error ? error.message : String(error);
}

/* A new folder, with where it goes, or a new name for one. */
export function FolderDialog({
  mode,
  folders,
  label,
  onClose,
  onSubmit,
}: {
  /* What is asked, and of which folder. */
  mode: { kind: "new"; parentId: string | null } | { kind: "rename"; folder: MailFolder };
  folders: readonly MailFolder[];
  label: (f: MailFolder) => string;
  onClose: () => void;
  /** Resolves when done; a throw is shown under the field. */
  onSubmit: (name: string, parentId: string | null) => Promise<void>;
}) {
  const t = useT();
  const id = React.useId();
  const initial = mode.kind === "rename" ? mode.folder.name : "";
  const [name, setName] = React.useState(initial);
  const [parent, setParent] = React.useState(mode.kind === "new" ? mode.parentId : null);
  const [error, setError] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  const trimmed = name.trim();
  const bad = trimmed.includes("/");

  return (
    <Dialog open onOpenChange={(next) => !next && !busy && onClose()} labelledBy={`${id}-title`}>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (trimmed === "" || bad || busy) return;
          setBusy(true);
          onSubmit(trimmed, parent).then(onClose, (err: unknown) => {
            setBusy(false);
            setError(explainFolder(err, t));
          });
        }}
      >
        <DialogHeader>
          <DialogTitle id={`${id}-title`}>
            {mode.kind === "new" ? t("mail.folder.new") : t("mail.folder.renameTitle", { name: mode.folder.name })}
          </DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-3">
          <Field invalid={error !== null || bad}>
            <FieldLabel htmlFor={`${id}-name`}>{t("mail.folder.name")}</FieldLabel>
            <Input
              id={`${id}-name`}
              value={name}
              autoFocus
              autoComplete="off"
              maxLength={100}
              aria-invalid={error !== null || bad}
              onChange={(event) => {
                setName(event.target.value);
                setError(null);
              }}
            />
            <FieldError>{bad ? t("mail.folder.error.badName") : error}</FieldError>
          </Field>
          {mode.kind === "new" && (
            <Field>
              <FieldLabel htmlFor={`${id}-where`}>{t("mail.folder.where")}</FieldLabel>
              <NativeSelect
                id={`${id}-where`}
                value={parent ?? ""}
                onChange={(event) => setParent(event.target.value === "" ? null : event.target.value)}
              >
                <NativeSelectOption value="">{t("mail.folder.top")}</NativeSelectOption>
                {tree(folders).map(({ folder, depth }) => (
                  <NativeSelectOption key={folder.id} value={folder.id}>
                    {" ".repeat(depth) + label(folder)}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </Field>
          )}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t("mail.cancel")}
          </Button>
          <Button type="submit" disabled={trimmed === "" || bad || busy || trimmed === initial}>
            {busy && <Spinner className="size-4" />}
            {mode.kind === "new" ? t("mail.folder.create") : t("mail.folder.save")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

/* Delete one: its mail goes to the bin first, so the question is light. */
export function DeleteFolderDialog({
  folder,
  onClose,
  onDelete,
}: {
  folder: MailFolder;
  onClose: () => void;
  onDelete: () => Promise<void>;
}) {
  const t = useT();
  const id = React.useId();
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  return (
    <Dialog
      open
      onOpenChange={(next) => !next && !busy && onClose()}
      labelledBy={`${id}-title`}
      describedBy={`${id}-text`}
    >
      <DialogHeader>
        <DialogTitle id={`${id}-title`}>{t("mail.folder.deleteTitle", { name: folder.name })}</DialogTitle>
        <DialogDescription id={`${id}-text`}>{t("mail.folder.deleteText")}</DialogDescription>
      </DialogHeader>
      {error !== null && (
        <DialogBody>
          <p role="alert" className="text-sm text-crit">
            {error}
          </p>
        </DialogBody>
      )}
      <DialogFooter>
        <Button variant="ghost" onClick={onClose} disabled={busy}>
          {t("mail.cancel")}
        </Button>
        {/* Not the destructive colour: nothing is lost, the mail is in the bin. */}
        <Button
          disabled={busy}
          onClick={() => {
            setBusy(true);
            onDelete().then(onClose, (err: unknown) => {
              setBusy(false);
              setError(explainFolder(err, t));
            });
          }}
        >
          {busy && <Spinner className="size-4" />}
          {t("mail.folder.delete")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
