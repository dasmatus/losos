/* LosOS Cloud inside chat: files sent from it, files that arrive saved
 * into it (the Chat folder), and the share links Cloud makes,
 * `/chat/?share=<path>&text=…` (lib/intents.ts), which ask for a
 * conversation and send the files there. */

import * as React from "react";
import { KnownMembership, type MatrixClient } from "matrix-js-sdk";
import { Dialog, DialogBody, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { toast } from "@/components/ui/toast";
import { cloudAvailable, INBOX_FOLDERS, readCloudFile, saveToCloud, type CloudEntry } from "@/lib/cloud-files";
import { useT } from "@/lib/i18n-react";
import { Avatar } from "./avatar";
import { sendFile, mediaFile, type MediaSource } from "./media";
import { isSpace, lastActive, roomName } from "./model";

/** Whether the person has LosOS Cloud on this box; false until known. */
export function useCloud(): boolean {
  const [on, setOn] = React.useState(false);
  React.useEffect(() => {
    let live = true;
    void cloudAvailable().then((ok) => {
      if (live) setOn(ok);
    });
    return () => {
      live = false;
    };
  }, []);
  return on;
}

/* What was saved from which message on this page, so the file card turns
 * its save button into an Open link wherever the save started. */
const saved = new Map<string, CloudEntry>();
const listeners = new Set<() => void>();

export function useSaved(eventId: string): CloudEntry | undefined {
  return React.useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => saved.get(eventId),
  );
}

/** A message's file, decrypted, into the Chat folder of LosOS Cloud. */
export async function saveAttachment(
  client: MatrixClient,
  t: ReturnType<typeof useT>,
  file: { eventId: string; source: MediaSource; mime: string; name: string },
): Promise<void> {
  const id = `chat-save-${file.eventId}`;
  toast.status(t("together.save.saving"), file.name, { id, duration: Infinity });
  try {
    const bytes = await mediaFile(client, file.source, file.mime, file.name);
    const entry = await saveToCloud(bytes, { folder: INBOX_FOLDERS.chat, name: file.name });
    saved.set(file.eventId, entry);
    for (const listener of listeners) listener();
    toast.done(t("together.save.done"), t("together.save.where", { folder: INBOX_FOLDERS.chat }), { settles: id });
  } catch (error) {
    toast.error(t("together.save.failed"), error instanceof Error ? error.message : String(error), { help: "chat", settles: id });
  }
}

/** Cloud files into a room, one message each, as the composer sends a file. */
export async function sendCloudFiles(client: MatrixClient, roomId: string, paths: readonly string[], threadRoot?: string): Promise<void> {
  for (const path of paths) {
    const file = await readCloudFile(path);
    await sendFile(client, roomId, file, file.name, threadRoot !== undefined ? { threadRoot } : {});
  }
}

/* Composer drafts handed over by a share link: the room's composer takes
 * its text once, when it mounts. */
const drafts = new Map<string, string>();

export function takeDraft(roomId: string): string {
  const text = drafts.get(roomId) ?? "";
  drafts.delete(roomId);
  return text;
}

/** What a share link carries, or null when it carries nothing. */
export function readShare(search: URLSearchParams): { paths: string[]; text: string } | null {
  const paths = search
    .getAll("share")
    .map((p) => p.trim())
    .filter((p) => p !== "");
  const text = search.get("text") ?? "";
  return paths.length === 0 && text.trim() === "" ? null : { paths, text };
}

/** "Send to…": the conversations a share link's files can go to. */
export function ShareDialog({
  client,
  share,
  onClose,
  onSent,
}: {
  client: MatrixClient;
  share: { paths: string[]; text: string } | null;
  onClose: () => void;
  onSent: (roomId: string) => void;
}) {
  const t = useT();
  const [filter, setFilter] = React.useState("");
  const [busy, setBusy] = React.useState<string | null>(null);
  const rooms = React.useMemo(() => {
    const wanted = filter.trim().toLowerCase();
    return client
      .getRooms()
      .filter((room) => !isSpace(room) && room.getMyMembership() === KnownMembership.Join)
      .filter((room) => wanted === "" || roomName(room).toLowerCase().includes(wanted))
      .sort((a, b) => lastActive(b) - lastActive(a));
  }, [client, filter]);
  if (share === null) return null;
  const count = share.paths.length;
  const pick = (roomId: string): void => {
    setBusy(roomId);
    if (share.text.trim() !== "") drafts.set(roomId, share.text);
    sendCloudFiles(client, roomId, share.paths)
      .then(() => onSent(roomId))
      .catch((error: unknown) => toast.error(t("chat.share.failed"), error instanceof Error ? error.message : String(error), { help: "chat" }))
      .finally(() => setBusy(null));
  };
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()} labelledBy="chat-share">
      <DialogHeader>
        <DialogTitle id="chat-share">{count > 0 ? t("together.share.many", { count }) : t("together.send.chat")}</DialogTitle>
        <DialogDescription>{t("chat.share.lead")}</DialogDescription>
      </DialogHeader>
      <DialogBody>
        <Input value={filter} onChange={(event) => setFilter(event.target.value)} placeholder={t("chat.list.filter")} aria-label={t("chat.list.filter")} autoFocus />
        <ul className="mt-2 flex max-h-[min(22rem,55dvh)] flex-col gap-0.5 overflow-y-auto">
          {rooms.map((room) => (
            <li key={room.roomId}>
              <button
                type="button"
                disabled={busy !== null}
                onClick={() => pick(room.roomId)}
                className="flex w-full items-center gap-2.5 rounded-control px-2 py-1.5 text-left text-ink hover:bg-sunk disabled:opacity-60"
              >
                <Avatar client={client} id={room.roomId} name={roomName(room)} mxc={room.getMxcAvatarUrl()} size={28} />
                <span className="min-w-0 flex-1 truncate text-[13.5px] font-medium">{roomName(room)}</span>
                {busy === room.roomId && <span className="text-[12px] text-muted">{t("chat.share.sending")}</span>}
              </button>
            </li>
          ))}
          {rooms.length === 0 && <li className="px-2 py-1 text-[12.5px] text-faint">{t("chat.list.noRooms")}</li>}
        </ul>
      </DialogBody>
    </Dialog>
  );
}
