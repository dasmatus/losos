import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { CloudSavingDone01Icon, CloudUploadIcon, Download01Icon, File01Icon, SquareLock01Icon } from "@hugeicons/core-free-icons";
import { EventType, type MatrixClient, type MatrixEvent } from "matrix-js-sdk";
import { useT } from "@/lib/i18n-react";
import { openFileHref } from "@/lib/intents";
import { cn } from "@/lib/utils";
import { saveAttachment, useCloud, useSaved } from "./cloud";
import { download, mediaUrl, type MediaSource } from "./media";

/* A message's body. Only the plain `body` is drawn, never `formatted_body`:
 * HTML from other servers would need a sanitiser, and the plain text is
 * always there. Links and Matrix IDs are picked out of it. */

const LINK = /(https?:\/\/[^\s<>"']+[^\s<>"'.,;:!?)\]])|(@[a-z0-9._=/+-]+:[a-z0-9.-]+(?::\d+)?)/gi;

export function RichText({ text, me }: { text: string; me: string }) {
  const parts: React.ReactNode[] = [];
  let last = 0;
  for (const match of text.matchAll(LINK)) {
    const at = match.index;
    if (at > last) parts.push(text.slice(last, at));
    const [whole, url, user] = match;
    if (url !== undefined) {
      parts.push(
        <a key={at} href={url} target="_blank" rel="noreferrer noopener" className="text-accent underline underline-offset-2 break-all">
          {url}
        </a>,
      );
    } else if (user !== undefined) {
      parts.push(
        <span key={at} className={cn("rounded px-1 font-medium", user === me ? "bg-accent text-surface" : "bg-accent-wash text-accent")}>
          {user}
        </span>,
      );
    } else {
      parts.push(whole);
    }
    last = at + whole.length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return <>{parts}</>;
}

export function sourceOf(content: Record<string, unknown>): MediaSource {
  const source: MediaSource = {};
  if (typeof content["url"] === "string") source.url = content["url"];
  if (typeof content["file"] === "object" && content["file"] !== null) source.file = content["file"] as MediaSource["file"];
  return source;
}

export function infoOf(content: Record<string, unknown>): { mimetype: string; size?: number; w?: number; h?: number; duration?: number } {
  const info = (typeof content["info"] === "object" && content["info"] !== null ? content["info"] : {}) as Record<string, unknown>;
  const mimetype = typeof info["mimetype"] === "string" ? info["mimetype"] : "application/octet-stream";
  const out: { mimetype: string; size?: number; w?: number; h?: number; duration?: number } = { mimetype };
  if (typeof info["size"] === "number") out.size = info["size"];
  if (typeof info["w"] === "number") out.w = info["w"];
  if (typeof info["h"] === "number") out.h = info["h"];
  if (typeof info["duration"] === "number") out.duration = info["duration"];
  return out;
}

function useMedia(client: MatrixClient, source: MediaSource, mime: string, thumb?: number): string | null | "failed" {
  const [url, setUrl] = React.useState<string | null | "failed">(null);
  const key = source.file?.url ?? source.url ?? "";
  React.useEffect(() => {
    let live = true;
    setUrl(null);
    mediaUrl(client, source, mime, thumb)
      .then((found) => live && setUrl(found))
      .catch(() => live && setUrl("failed"));
    return () => {
      live = false;
    };
    // The source object is rebuilt each render; its URL is what matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client, key, mime, thumb]);
  return url;
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} kB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function Picture({ client, content, onOpen }: { client: MatrixClient; content: Record<string, unknown>; onOpen: (url: string) => void }) {
  const t = useT();
  const info = infoOf(content);
  const url = useMedia(client, sourceOf(content), info.mimetype, 640);
  const ratio = info.w !== undefined && info.h !== undefined && info.w > 0 ? Math.min(info.h / info.w, 1.5) : 0.66;
  if (url === "failed") return <p className="text-[13px] text-muted">{t("chat.media.failed")}</p>;
  return (
    <button
      type="button"
      onClick={() => url !== null && onOpen(url)}
      className="block overflow-hidden rounded-control border border-hair bg-sunk"
      aria-label={t("chat.media.open")}
    >
      {url === null ? (
        <span className={cn("block w-72 max-w-full animate-breathe bg-sunk", ratio > 1 ? "h-96" : ratio > 0.8 ? "h-64" : "h-44")} />
      ) : (
        <img src={url} alt={typeof content["body"] === "string" ? content["body"] : ""} className="block max-h-96 max-w-full object-contain sm:max-w-sm" />
      )}
    </button>
  );
}

function Playable({ client, content, video }: { client: MatrixClient; content: Record<string, unknown>; video: boolean }) {
  const t = useT();
  const info = infoOf(content);
  const url = useMedia(client, sourceOf(content), info.mimetype);
  if (url === "failed") return <p className="text-[13px] text-muted">{t("chat.media.failed")}</p>;
  if (url === null) return <span className="block h-10 w-64 animate-breathe rounded-control bg-sunk" />;
  return video ? (
    <video src={url} controls preload="metadata" className="max-h-96 max-w-full rounded-control border border-hair sm:max-w-sm" />
  ) : (
    <audio src={url} controls preload="metadata" className="w-72 max-w-full" />
  );
}

/** The name a file was sent under. */
export function fileNameOf(content: Record<string, unknown>): string {
  return typeof content["filename"] === "string" ? content["filename"] : typeof content["body"] === "string" ? content["body"] : "file";
}

/** Whether a message carries a file (a picture, video, voice or any file). */
export function carriesFile(content: Record<string, unknown>): boolean {
  return ["m.image", "m.video", "m.audio", "m.file"].includes(content["msgtype"] as string) && (typeof content["url"] === "string" || typeof content["file"] === "object");
}

function FileCard({ client, event, content }: { client: MatrixClient; event: MatrixEvent; content: Record<string, unknown> }) {
  const t = useT();
  const info = infoOf(content);
  const name = fileNameOf(content);
  const cloud = useCloud();
  const eventId = event.getId() ?? "";
  const kept = useSaved(eventId);
  const [busy, setBusy] = React.useState(false);
  const [failed, setFailed] = React.useState(false);
  return (
    <div className="flex w-72 max-w-full items-center gap-3 rounded-control border border-line bg-surface px-3 py-2">
      <HugeiconsIcon icon={File01Icon} className="size-5 text-muted" />
      <div className="min-w-0 flex-1">
        <p className="truncate text-[13.5px] font-medium">{name}</p>
        <p className="text-[12px] text-muted">{failed ? t("chat.media.failed") : info.size !== undefined ? formatSize(info.size) : info.mimetype}</p>
      </div>
      <button
        type="button"
        disabled={busy}
        aria-label={t("chat.media.download", { name })}
        className="rounded-control p-1.5 text-muted hover:bg-sunk hover:text-ink disabled:opacity-45"
        onClick={() => {
          setBusy(true);
          setFailed(false);
          download(client, sourceOf(content), info.mimetype, name)
            .catch(() => setFailed(true))
            .finally(() => setBusy(false));
        }}
      >
        <HugeiconsIcon icon={Download01Icon} className="size-4" />
      </button>
      {cloud && kept !== undefined && (
        <a
          href={openFileHref(kept.id)}
          aria-label={t("chat.media.openSaved", { name })}
          title={t("chat.media.openSaved", { name })}
          className="rounded-control p-1.5 text-accent hover:bg-sunk"
        >
          <HugeiconsIcon icon={CloudSavingDone01Icon} className="size-4" />
        </a>
      )}
      {cloud && kept === undefined && (
        <button
          type="button"
          aria-label={t("chat.media.save", { name })}
          title={t("chat.media.save", { name })}
          className="rounded-control p-1.5 text-muted hover:bg-sunk hover:text-ink"
          onClick={() => void saveAttachment(client, t, { eventId, source: sourceOf(content), mime: info.mimetype, name })}
        >
          <HugeiconsIcon icon={CloudUploadIcon} className="size-4" />
        </button>
      )}
    </div>
  );
}

export interface BodyProps {
  client: MatrixClient;
  event: MatrixEvent;
  content: Record<string, unknown>;
  me: string;
  onOpenImage: (url: string) => void;
}

/** What a message says or carries. */
export function MessageBody({ client, event, content, me, onOpenImage }: BodyProps) {
  const t = useT();
  if (event.isRedacted()) return <p className="text-[13.5px] text-faint italic">{t("chat.message.deleted")}</p>;
  if (event.isDecryptionFailure()) {
    return (
      <p className="flex items-center gap-1.5 text-[13.5px] text-muted italic">
        <HugeiconsIcon icon={SquareLock01Icon} className="size-4" />
        {t("chat.message.undecryptable")}
      </p>
    );
  }
  if (event.getType() === EventType.RoomMessageEncrypted || event.isBeingDecrypted()) {
    return <p className="text-[13.5px] text-faint italic">{t("chat.message.decrypting")}</p>;
  }
  if (event.getType() === EventType.Sticker) return <Picture client={client} content={content} onOpen={onOpenImage} />;
  const msgtype = content["msgtype"];
  const body = typeof content["body"] === "string" ? content["body"] : "";
  switch (msgtype) {
    case "m.image":
      return <Picture client={client} content={content} onOpen={onOpenImage} />;
    case "m.video":
      return <Playable client={client} content={content} video />;
    case "m.audio":
      return <Playable client={client} content={content} video={false} />;
    case "m.file":
      return <FileCard client={client} event={event} content={content} />;
    case "m.emote":
      return (
        <p className="text-[14.5px] leading-relaxed break-words whitespace-pre-wrap italic">
          * {event.sender?.name ?? event.getSender()} <RichText text={body} me={me} />
        </p>
      );
    case "m.notice":
      return (
        <p className="text-[14.5px] leading-relaxed break-words whitespace-pre-wrap text-muted">
          <RichText text={body} me={me} />
        </p>
      );
    case "m.location": {
      const geo = typeof content["geo_uri"] === "string" ? content["geo_uri"] : "";
      const [lat, lon] = geo.replace(/^geo:/, "").split(/[,;]/);
      const href = lat !== undefined && lon !== undefined ? `https://www.openstreetmap.org/?mlat=${lat}&mlon=${lon}#map=16/${lat}/${lon}` : null;
      return (
        <p className="text-[14.5px]">
          {t("chat.message.location")}{" "}
          {href !== null && (
            <a href={href} target="_blank" rel="noreferrer noopener" className="text-accent underline">
              {lat}, {lon}
            </a>
          )}
        </p>
      );
    }
    default:
      return (
        <p className="text-[14.5px] leading-relaxed break-words whitespace-pre-wrap">
          <RichText text={body} me={me} />
        </p>
      );
  }
}
