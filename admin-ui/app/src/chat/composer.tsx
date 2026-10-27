import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Attachment01Icon, Cancel01Icon, CloudIcon, Mic01Icon, SentIcon } from "@hugeicons/core-free-icons";
import type { MatrixClient, MatrixEvent, Room } from "matrix-js-sdk";
import { CloudPicker } from "@/components/cloud-picker";
import { Masked, useDetailsCheck } from "@/components/details";
import { useSendCheck } from "@/components/send-check";
import { toast } from "@/components/ui/toast";
import { readCloudFile } from "@/lib/cloud-files";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { takeDraft, useCloud } from "./cloud";
import { sendFile } from "./media";
import { plainBody, sendContent } from "./model";
import { Button } from "@/components/ui/button";

const MENTION = /@[a-z0-9._=/+-]+:[a-z0-9.-]+(?::\d+)?/gi;

export interface ComposerProps {
  client: MatrixClient;
  room: Room;
  /** Write into a thread: its root event. */
  threadRoot?: string | null;
  /** The newest event in that thread, for the fallback reply. */
  threadLatest?: string | null;
  replyTo: MatrixEvent | null;
  editing: MatrixEvent | null;
  onDone: () => void;
  placeholder: string;
}

/** Where a new message points: a reply, a thread, both, or nowhere. */
function relationFor(threadRoot: string | null, threadLatest: string | null, replyTo: MatrixEvent | null): Record<string, unknown> | null {
  if (threadRoot !== null) {
    const reply = replyTo?.getId() ?? threadLatest ?? threadRoot;
    return {
      rel_type: "m.thread",
      event_id: threadRoot,
      is_falling_back: replyTo === null,
      "m.in_reply_to": { event_id: reply },
    };
  }
  if (replyTo !== null) return { "m.in_reply_to": { event_id: replyTo.getId() } };
  return null;
}

/** Type, attach and record a message. Enter sends, Shift+Enter breaks a line. */
export function Composer({ client, room, threadRoot = null, threadLatest = null, replyTo, editing, onDone, placeholder }: ComposerProps) {
  const t = useT();
  // A share link from LosOS Cloud may leave words for this room's composer.
  const [text, setText] = React.useState(() => (threadRoot === null ? takeDraft(room.roomId) : ""));
  const cloud = useCloud();
  const [picking, setPicking] = React.useState(false);
  const area = React.useRef<HTMLTextAreaElement>(null);
  const files = React.useRef<HTMLInputElement>(null);
  const typingSent = React.useRef(0);
  const [recording, setRecording] = React.useState<{ recorder: MediaRecorder; started: number } | null>(null);
  const [uploading, setUploading] = React.useState(0);
  const sendCheck = useSendCheck();
  const detailsCheck = useDetailsCheck();

  React.useEffect(() => {
    if (editing !== null) {
      // Start from the newest edit, so a second edit does not undo the first.
      const newest = room
        .getLiveTimeline()
        .getEvents()
        .filter((ev) => ev.getSender() === editing.getSender() && ev.getContent()["m.relates_to"]?.rel_type === "m.replace" && ev.getContent()["m.relates_to"]?.event_id === editing.getId())
        .at(-1);
      setText(plainBody((newest ?? editing).getContent()));
      area.current?.focus();
    }
  }, [editing]);
  React.useEffect(() => {
    if (replyTo !== null) area.current?.focus();
  }, [replyTo]);

  // The box grows with what is typed, up to a third of the window.
  React.useLayoutEffect(() => {
    const el = area.current;
    if (el === null) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, Math.round(window.innerHeight / 3))}px`;
  }, [text]);

  const typing = (on: boolean): void => {
    const now = Date.now();
    if (on && now - typingSent.current < 3000) return;
    typingSent.current = on ? now : 0;
    void client.sendTyping(room.roomId, on, on ? 5000 : 0).catch(() => undefined);
  };

  const send = async (): Promise<void> => {
    const body = text.trim();
    if (body === "") return;
    // Card and ID numbers are asked about first; "Don't send" keeps the text.
    if (!(await detailsCheck.check(body))) return;
    setText("");
    typing(false);
    const mentions = [...new Set(body.match(MENTION) ?? [])];
    try {
      if (editing !== null) {
        await sendContent(client, room.roomId, {
          msgtype: "m.text",
          body: `* ${body}`,
          "m.new_content": { msgtype: "m.text", body, "m.mentions": { user_ids: mentions } },
          "m.relates_to": { rel_type: "m.replace", event_id: editing.getId() },
          "m.mentions": {},
        });
      } else {
        const content: Record<string, unknown> = {
          msgtype: body.startsWith("/me ") ? "m.emote" : "m.text",
          body: body.startsWith("/me ") ? body.slice(4) : body,
          "m.mentions": { user_ids: [...mentions, ...(replyTo?.getSender() !== undefined && replyTo.getSender() !== client.getUserId() ? [replyTo.getSender() as string] : [])] },
        };
        const relation = relationFor(threadRoot, threadLatest, replyTo);
        if (relation !== null) content["m.relates_to"] = relation;
        await sendContent(client, room.roomId, content);
      }
    } catch (error) {
      // A message that did not go stays in the timeline with a retry.
      if (editing !== null) toast.error(t("chat.composer.editFailed"), String(error), { help: "chat" });
    }
    onDone();
  };

  const attach = async (list: FileList | File[]): Promise<void> => {
    const chosen = [...list];
    setUploading((n) => n + chosen.length);
    for (const file of chosen) {
      try {
        if (!(await sendCheck.check(file))) continue;
        await sendFile(client, room.roomId, file, file.name, threadRoot !== null ? { threadRoot } : {});
      } catch (error) {
        toast.error(t("chat.composer.uploadFailed", { name: file.name }), String(error), { help: "chat" });
      } finally {
        setUploading((n) => n - 1);
      }
    }
  };

  const record = async (): Promise<void> => {
    if (recording !== null) {
      recording.recorder.stop();
      return;
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const type = MediaRecorder.isTypeSupported("audio/ogg;codecs=opus") ? "audio/ogg;codecs=opus" : "audio/webm";
      const recorder = new MediaRecorder(stream, { mimeType: type });
      const chunks: Blob[] = [];
      const started = Date.now();
      recorder.ondataavailable = (event) => chunks.push(event.data);
      recorder.onstop = () => {
        for (const track of stream.getTracks()) track.stop();
        setRecording(null);
        const duration = Date.now() - started;
        if (duration < 700) return;
        const blob = new Blob(chunks, { type: type.split(";")[0] });
        setUploading((n) => n + 1);
        sendFile(client, room.roomId, blob, t("chat.composer.voiceName"), {
          durationMs: duration,
          extra: {
            "org.matrix.msc1767.audio": { duration },
            "org.matrix.msc3245.voice": {},
          },
          ...(threadRoot !== null ? { threadRoot } : {}),
        })
          .catch((error: unknown) => toast.error(t("chat.composer.uploadFailed", { name: t("chat.composer.voiceName") }), String(error), { help: "chat" }))
          .finally(() => setUploading((n) => n - 1));
      };
      recorder.start();
      setRecording({ recorder, started });
    } catch {
      toast.error(t("chat.composer.noMic"), undefined, { help: "chat" });
    }
  };

  const banner =
    editing !== null ? t("chat.composer.editing") : replyTo !== null ? t("chat.composer.replyingTo", { name: room.getMember(replyTo.getSender() ?? "")?.name ?? replyTo.getSender() ?? "" }) : null;

  return (
    <div
      className="border-t border-line bg-surface px-3 pt-2 pb-3"
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault();
        if (event.dataTransfer.files.length > 0) void attach(event.dataTransfer.files);
      }}
    >
      {banner !== null && (
        <div className="mb-2 flex items-center gap-2 rounded-control bg-sunk px-3 py-1.5 text-[12.5px] text-muted">
          <span className="min-w-0 flex-1 truncate">
            <span className="font-medium text-ink">{banner}</span>
            {replyTo !== null && editing === null && <> · <Masked text={plainBody(replyTo.getContent())} still /></>}
          </span>
          <button type="button" aria-label={t("chat.composer.cancel")} className="rounded p-0.5 hover:bg-surface" onClick={() => { setText(""); onDone(); }}>
            <HugeiconsIcon icon={Cancel01Icon} className="size-3.5" />
          </button>
        </div>
      )}
      {uploading > 0 && <p className="mb-1 text-[12px] text-muted">{t("chat.composer.uploading", { count: uploading })}</p>}
      <div className="flex items-end gap-1">
        <input
          ref={files}
          type="file"
          multiple
          hidden
          onChange={(event) => {
            if (event.currentTarget.files !== null) void attach(event.currentTarget.files);
            event.currentTarget.value = "";
          }}
        />
        <Button variant="ghost" size="icon-sm" aria-label={t("chat.composer.attach")} title={t("chat.composer.attach")} onClick={() => files.current?.click()} disabled={editing !== null}>
          <HugeiconsIcon icon={Attachment01Icon} className="size-4" />
        </Button>
        {cloud && (
          <Button variant="ghost" size="icon-sm" aria-label={t("chat.composer.fromCloud")} title={t("chat.composer.fromCloud")} onClick={() => setPicking(true)} disabled={editing !== null}>
            <HugeiconsIcon icon={CloudIcon} className="size-4" />
          </Button>
        )}
        <CloudPicker
          open={picking}
          onOpenChange={setPicking}
          multiple
          action={t("chat.composer.send")}
          onPick={(entries) => {
            Promise.all(entries.map((entry) => readCloudFile(entry.path)))
              .then((list) => attach(list))
              .catch((error: unknown) => toast.error(t("chat.share.failed"), error instanceof Error ? error.message : String(error), { help: "chat" }));
          }}
        />
        <textarea
          ref={area}
          rows={1}
          value={text}
          placeholder={recording !== null ? t("chat.composer.recording") : placeholder}
          aria-label={placeholder}
          disabled={recording !== null}
          className="max-h-[33vh] min-h-9 flex-1 resize-none rounded-control border border-line bg-ground px-3 py-2 text-[14.5px] leading-snug outline-none placeholder:text-faint focus:border-accent"
          onChange={(event) => {
            setText(event.target.value);
            typing(event.target.value !== "");
          }}
          onBlur={() => typing(false)}
          onPaste={(event) => {
            const pasted = [...event.clipboardData.files];
            if (pasted.length > 0) {
              event.preventDefault();
              void attach(pasted);
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
              event.preventDefault();
              void send();
            } else if (event.key === "Escape" && (editing !== null || replyTo !== null)) {
              setText("");
              onDone();
            }
          }}
        />
        {text.trim() === "" && editing === null ? (
          <Button variant="ghost" size="icon-sm"
           
            className={cn(recording !== null && "bg-crit text-surface hover:bg-crit hover:text-surface")}
            aria-label={recording !== null ? t("chat.composer.stopRecording") : t("chat.composer.record")}
            title={recording !== null ? t("chat.composer.stopRecording") : t("chat.composer.record")}
            onClick={() => void record()}
          >
            <HugeiconsIcon icon={Mic01Icon} className="size-4" />
          </Button>
        ) : (
          <Button variant="ghost" size="icon-sm" className={cn("text-accent")} aria-label={t("chat.composer.send")} title={t("chat.composer.send")} onClick={() => void send()}>
            <HugeiconsIcon icon={SentIcon} className="size-4" />
          </Button>
        )}
      </div>
      {sendCheck.dialog}
      {detailsCheck.dialog}
    </div>
  );
}
