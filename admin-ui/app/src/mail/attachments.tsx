/* Mail's files, tied into LosOS Cloud.
 *
 * Writing: a file from this computer, from LosOS Cloud (the shared
 * CloudPicker), or the files of a message being forwarded. They are read
 * here and go to lososd inside POST /api/mail/send, which uploads them to
 * the mail server as the person.
 *
 * Reading: every attachment downloads as before, and can also be saved to
 * the person's Cloud (the Mail folder there) and then opened in the app
 * that edits it: Office for a document, Diagrams for a .canvas, Notes for
 * Markdown (lib/intents.ts openFileHref). */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Attachment01Icon, Cancel01Icon, CloudIcon, CloudUploadIcon, Download04Icon, FolderOpenIcon, LinkSquare02Icon } from "@hugeicons/core-free-icons";
import { CloudPicker } from "@/components/cloud-picker";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { fetchMailAttachment, mailAttachmentUrl, MAIL_ATTACHMENT_BYTES, type MailAttachment } from "@/lib/api";
import { cloudAvailable, INBOX_FOLDERS, readCloudFile, saveToCloud, type CloudEntry } from "@/lib/cloud-files";
import { contextMenu } from "@/lib/context-menu";
import { formatBytes } from "@/lib/apps";
import { useT } from "@/lib/i18n-react";
import { cloudFileHref, openFileHref } from "@/lib/intents";
import { cn } from "@/lib/utils";

/** Whether this box has LosOS Cloud for the person, asked once. */
export function useCloud(): boolean {
  const [has, setHas] = React.useState(false);
  React.useEffect(() => {
    let live = true;
    void cloudAvailable().then((yes) => live && setHas(yes));
    return () => {
      live = false;
    };
  }, []);
  return has;
}

/** A file on the message being written. */
export interface Attached {
  key: string;
  name: string;
  size: number;
  /** Null while it is still being read from Cloud or a forwarded message. */
  file: File | null;
  failed?: boolean;
}

let next = 0;
const key = (): string => `a${++next}`;

/** The files of a new message, and how it starts: Cloud paths from a link
 *  (/mail?attach=…) and a forwarded message's own attachments. */
export function useAttached(start: { cloud?: readonly string[]; carry?: readonly MailAttachment[] }) {
  const t = useT();
  const [files, setFiles] = React.useState<Attached[]>([]);

  const load = React.useCallback((name: string, size: number, read: () => Promise<File>) => {
    const k = key();
    setFiles((list) => [...list, { key: k, name, size, file: null }]);
    read().then(
      (file) => setFiles((list) => list.map((a) => (a.key === k ? { ...a, file, size: file.size } : a))),
      () => {
        setFiles((list) => list.map((a) => (a.key === k ? { ...a, failed: true } : a)));
        toast.error(t("together.mail.reading", { name }), undefined, { help: "mail" });
      },
    );
  }, [t]);

  // Once, for how the composer was opened.
  const started = React.useRef(false);
  React.useEffect(() => {
    if (started.current) return;
    started.current = true;
    for (const path of start.cloud ?? []) load(path.slice(path.lastIndexOf("/") + 1), 0, () => readCloudFile(path));
    for (const a of start.carry ?? []) load(a.name, a.size, () => fetchMailAttachment(a));
  }, [start.cloud, start.carry, load]);

  const addFiles = (list: Iterable<File>) =>
    setFiles((current) => [...current, ...[...list].map((file) => ({ key: key(), name: file.name, size: file.size, file }))]);
  const addCloud = (entries: readonly CloudEntry[]) => {
    for (const e of entries) load(e.name, e.size, () => readCloudFile(e.path));
  };
  const remove = (k: string) => setFiles((list) => list.filter((a) => a.key !== k));
  const total = files.reduce((sum, a) => sum + a.size, 0);
  const ready = files.every((a) => a.file !== null || a.failed === true);
  return { files, addFiles, addCloud, remove, total, ready, tooBig: total > MAIL_ATTACHMENT_BYTES };
}

/** The composer's attach buttons and the files on the message. */
export function ComposeAttachments({ attached }: { attached: ReturnType<typeof useAttached> }) {
  const t = useT();
  const cloud = useCloud();
  const input = React.useRef<HTMLInputElement>(null);
  const [picking, setPicking] = React.useState(false);
  return (
    <div className="flex flex-col gap-2" data-slot="mail-attachments">
      <div className="flex flex-wrap items-center gap-1">
        <input
          ref={input}
          type="file"
          multiple
          hidden
          onChange={(event) => {
            if (event.target.files !== null) attached.addFiles(event.target.files);
            event.target.value = "";
          }}
        />
        <Button type="button" variant="ghost" size="sm" onClick={() => input.current?.click()}>
          <HugeiconsIcon icon={Attachment01Icon} className="size-4" aria-hidden="true" />
          {t("together.mail.attachFile")}
        </Button>
        {cloud && (
          <Button type="button" variant="ghost" size="sm" onClick={() => setPicking(true)} data-action="attach-cloud">
            <HugeiconsIcon icon={CloudIcon} className="size-4" aria-hidden="true" />
            {t("together.mail.attachCloud")}
          </Button>
        )}
      </div>
      {attached.files.length > 0 && (
        <ul className="flex flex-wrap gap-2" aria-label={t("mail.attachments")}>
          {attached.files.map((a) => (
            <li
              key={a.key}
              data-attached={a.name}
              className={cn(
                "flex items-center gap-2 rounded-control border border-line py-1 pr-1 pl-2.5 text-[13px]",
                a.failed === true && "border-crit text-crit",
              )}
            >
              {a.file === null && a.failed !== true ? (
                <Spinner className="size-3.5" label={t("together.mail.reading", { name: a.name })} />
              ) : (
                <HugeiconsIcon icon={Attachment01Icon} className="size-3.5 text-muted" aria-hidden="true" />
              )}
              <span className="max-w-48 truncate">{a.name}</span>
              {a.size > 0 && <span className="text-muted tabular-nums">{formatBytes(a.size)}</span>}
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label={t("together.mail.remove", { name: a.name })}
                onClick={() => attached.remove(a.key)}
              >
                <HugeiconsIcon icon={Cancel01Icon} className="size-3.5" aria-hidden="true" />
              </Button>
            </li>
          ))}
        </ul>
      )}
      {attached.tooBig && (
        <p className="text-[12px] text-crit">{t("together.mail.tooBig", { size: formatBytes(MAIL_ATTACHMENT_BYTES) })}</p>
      )}
      <CloudPicker open={picking} onOpenChange={setPicking} multiple onPick={attached.addCloud} />
    </div>
  );
}

type Saved = { state: "idle" } | { state: "saving" } | { state: "saved"; entry: CloudEntry };

/** A received file: download it, or save it to LosOS Cloud and open it in
 *  the app that edits it. */
export function ReceivedAttachment({ attachment: a, cloud }: { attachment: MailAttachment; cloud: boolean }) {
  const t = useT();
  const [saved, setSaved] = React.useState<Saved>({ state: "idle" });

  const save = async (): Promise<CloudEntry | null> => {
    if (saved.state === "saved") return saved.entry;
    setSaved({ state: "saving" });
    try {
      const entry = await saveToCloud(await fetchMailAttachment(a), { folder: INBOX_FOLDERS.mail, name: a.name });
      setSaved({ state: "saved", entry });
      toast.done(t("together.save.done"), t("together.save.where", { folder: INBOX_FOLDERS.mail }));
      return entry;
    } catch (error) {
      setSaved({ state: "idle" });
      toast.error(t("together.save.failed"), error instanceof Error ? error.message : undefined, { help: "mail" });
      return null;
    }
  };
  const open = async () => {
    const entry = await save();
    if (entry !== null) window.location.assign(openFileHref(entry.id));
  };

  return (
    <li
      data-attachment={a.name}
      className="flex items-center rounded-control border border-line text-[13px]"
      ref={contextMenu(() => ({
        label: a.name,
        sections: [
          [
            ...(cloud
              ? [
                  { id: "open", label: t("together.open"), icon: LinkSquare02Icon, onSelect: () => void open() },
                  saved.state === "saved"
                    ? { id: "show", label: t("together.show"), icon: FolderOpenIcon, onSelect: () => window.location.assign(cloudFileHref(saved.entry.id)) }
                    : { id: "save", label: t("together.save.action"), icon: CloudUploadIcon, onSelect: () => void save() },
                ]
              : []),
            { id: "download", label: t("together.download"), icon: Download04Icon, onSelect: () => window.location.assign(mailAttachmentUrl(a)) },
          ],
        ],
      }))}
    >
      <a href={mailAttachmentUrl(a)} download={a.name} className="flex items-center gap-2 rounded-l-control px-2.5 py-1.5 hover:bg-sunk">
        <HugeiconsIcon icon={Attachment01Icon} className="size-3.5 text-muted" aria-hidden="true" />
        <span className="max-w-48 truncate">{a.name}</span>
        <span className="text-muted tabular-nums">{formatBytes(a.size)}</span>
      </a>
      {cloud && (
        <span className="flex items-center border-l border-hair">
          {saved.state === "saved" ? (
            <a
              href={openFileHref(saved.entry.id)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 text-accent hover:bg-sunk"
              data-action="open"
            >
              <HugeiconsIcon icon={LinkSquare02Icon} className="size-3.5" aria-hidden="true" />
              {t("together.open")}
            </a>
          ) : (
            <button
              type="button"
              className="flex items-center gap-1.5 px-2.5 py-1.5 text-muted hover:bg-sunk hover:text-ink disabled:opacity-60"
              disabled={saved.state === "saving"}
              onClick={() => void save()}
              data-action="save-cloud"
            >
              {saved.state === "saving" ? (
                <Spinner className="size-3.5" label={t("together.save.saving")} />
              ) : (
                <HugeiconsIcon icon={CloudUploadIcon} className="size-3.5" aria-hidden="true" />
              )}
              {t("together.save.action")}
            </button>
          )}
        </span>
      )}
    </li>
  );
}
