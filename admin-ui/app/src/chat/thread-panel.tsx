import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, Cancel01Icon, Comment01Icon } from "@hugeicons/core-free-icons";
import { Direction, type MatrixClient, MatrixEvent, Method, type Room } from "matrix-js-sdk";
import { Spinner } from "@/components/ui/spinner";
import { useT } from "@/lib/i18n-react";
import { Avatar } from "./avatar";
import { Composer } from "./composer";
import { buildItems, memberName, plainBody, threadRootOf, useEmitted } from "./model";
import { ItemList, type RowActions, TIMELINE_EVENTS, useStickToBottom } from "./timeline";
import { Button } from "@/components/ui/button";

interface ThreadSummary {
  root: MatrixEvent;
  count: number;
  latest: MatrixEvent | null;
}

async function decrypted(client: MatrixClient, raw: object): Promise<MatrixEvent> {
  const ev = new MatrixEvent(raw);
  await client.decryptEventIfNeeded(ev);
  return ev;
}

/* The room's threads, newest activity first. The list is the spec's
 * GET /rooms/{room}/threads, which lososd answers for this homeserver. */
function useThreads(client: MatrixClient, room: Room): { threads: ThreadSummary[] | null; failed: boolean } {
  const tick = useEmitted(room, TIMELINE_EVENTS);
  const [threads, setThreads] = React.useState<ThreadSummary[] | null>(null);
  const [failed, setFailed] = React.useState(false);
  React.useEffect(() => {
    let live = true;
    (async () => {
      const answer = await client.http.authedRequest<{ chunk: Array<Record<string, unknown>> }>(
        Method.Get,
        `/rooms/${encodeURIComponent(room.roomId)}/threads`,
        { limit: "50" },
        undefined,
        { prefix: "/_matrix/client/v1" },
      );
      const out: ThreadSummary[] = [];
      for (const raw of answer.chunk) {
        const summary = ((raw["unsigned"] as Record<string, unknown> | undefined)?.["m.relations"] as Record<string, unknown> | undefined)?.["m.thread"] as
          | { count?: number; latest_event?: object }
          | undefined;
        out.push({
          root: await decrypted(client, raw),
          count: summary?.count ?? 0,
          latest: summary?.latest_event !== undefined ? await decrypted(client, summary.latest_event) : null,
        });
      }
      if (live) {
        setThreads(out);
        setFailed(false);
      }
    })().catch(() => live && setFailed(true));
    return () => {
      live = false;
    };
  }, [client, room, tick]);
  return { threads, failed };
}

function ThreadList({ client, room, onOpen }: { client: MatrixClient; room: Room; onOpen: (root: string) => void }) {
  const t = useT();
  const { threads, failed } = useThreads(client, room);
  if (failed) return <p className="p-4 text-[13px] text-muted">{t("chat.thread.listFailed")}</p>;
  if (threads === null) return <div className="flex justify-center p-6"><Spinner size={18} label={t("chat.timeline.loading")} /></div>;
  if (threads.length === 0) return <p className="p-4 text-[13px] text-muted">{t("chat.thread.none")}</p>;
  return (
    <ul className="flex flex-col gap-1 p-2">
      {threads.map(({ root, count, latest }) => {
        const sender = root.getSender() ?? "";
        const name = memberName(room, sender);
        return (
          <li key={root.getId()}>
            <button type="button" onClick={() => onOpen(root.getId() ?? "")} className="flex w-full gap-2.5 rounded-control p-2 text-left hover:bg-sunk">
              <Avatar client={client} id={sender} name={name} mxc={room.getMember(sender)?.getMxcAvatarUrl()} size={32} />
              <span className="min-w-0 flex-1">
                <span className="block text-[13px] font-semibold">{name}</span>
                <span className="line-clamp-2 text-[13px] text-ink">{plainBody(root.getContent()) || t("chat.reply.attachment")}</span>
                <span className="mt-0.5 flex items-center gap-1 text-[12px] text-accent">
                  <HugeiconsIcon icon={Comment01Icon} className="size-3.5" />
                  {t("chat.thread.replies", { count })}
                  {latest !== null && <span className="truncate text-muted"> · {plainBody(latest.getContent())}</span>}
                </span>
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

function ThreadView({ client, room, root, me, actions }: { client: MatrixClient; room: Room; root: string; me: string; actions: RowActions }) {
  const t = useT();
  const tick = useEmitted(room, TIMELINE_EVENTS);
  const [fetched, setFetched] = React.useState<MatrixEvent[] | null>(null);
  const [replyTo, setReplyTo] = React.useState<MatrixEvent | null>(null);
  const [editing, setEditing] = React.useState<MatrixEvent | null>(null);

  React.useEffect(() => {
    let live = true;
    setFetched(null);
    (async () => {
      const out: MatrixEvent[] = [];
      const first = room.findEventById(root) ?? (await decrypted(client, await client.fetchRoomEvent(room.roomId, root)));
      out.push(first);
      let from: string | null = null;
      for (let page = 0; page < 10; page += 1) {
        const answer = await client.relations(room.roomId, root, "m.thread", null, { dir: Direction.Backward, limit: 50, ...(from !== null ? { from } : {}) });
        out.push(...answer.events);
        from = answer.nextBatch ?? null;
        if (from === null) break;
      }
      if (live) setFetched(out);
    })().catch(() => live && setFetched([]));
    return () => {
      live = false;
    };
  }, [client, room, root]);

  const items = React.useMemo(() => {
    const byId = new Map<string, MatrixEvent>();
    for (const ev of [...(fetched ?? []), ...room.getLiveTimeline().getEvents()]) byId.set(ev.getId() ?? ev.getTxnId() ?? "", ev);
    const all = [...byId.values()].sort((a, b) => a.getTs() - b.getTs());
    return buildItems(all, me, root);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fetched, room, me, root, tick]);

  const latest = React.useMemo(() => {
    const replies = room.getLiveTimeline().getEvents().filter((ev) => threadRootOf(ev) === root);
    return replies.at(-1)?.getId() ?? items.at(-1)?.event.getId() ?? null;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [room, root, items, tick]);

  const box = React.useRef<HTMLDivElement>(null);
  useStickToBottom(box, [items.length]);
  const threadActions: RowActions = { ...actions, onThread: null, onReply: setReplyTo, onEdit: setEditing };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div ref={box} className="min-h-0 flex-1 overflow-y-auto pb-2" role="log" aria-label={t("chat.thread.label")}>
        {fetched === null ? (
          <div className="flex justify-center p-6"><Spinner size={18} label={t("chat.timeline.loading")} /></div>
        ) : (
          <ItemList client={client} room={room} items={items} me={me} actions={threadActions} showReceipts={false} />
        )}
      </div>
      <Composer
        client={client}
        room={room}
        threadRoot={root}
        threadLatest={latest}
        replyTo={replyTo}
        editing={editing}
        onDone={() => {
          setReplyTo(null);
          setEditing(null);
        }}
        placeholder={t("chat.thread.placeholder")}
      />
    </div>
  );
}

export interface ThreadPanelProps {
  client: MatrixClient;
  room: Room;
  root: string | null;
  me: string;
  actions: RowActions;
  onOpen: (root: string | null) => void;
  onClose: () => void;
}

/** The side panel: the room's threads, or one of them. */
export function ThreadPanel({ client, room, root, me, actions, onOpen, onClose }: ThreadPanelProps) {
  const t = useT();
  return (
    <aside aria-label={t("chat.thread.panel")} className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header className="flex h-14 shrink-0 items-center gap-2 border-b border-hair px-3">
        {root !== null && (
          <Button variant="ghost" size="icon-sm" aria-label={t("chat.thread.all")} onClick={() => onOpen(null)}>
            <HugeiconsIcon icon={ArrowLeft01Icon} className="size-4" />
          </Button>
        )}
        <h2 className="flex-1 text-[14px] font-semibold">{root === null ? t("chat.thread.title") : t("chat.thread.one")}</h2>
        <Button variant="ghost" size="icon-sm" aria-label={t("chat.thread.close")} onClick={onClose}>
          <HugeiconsIcon icon={Cancel01Icon} className="size-4" />
        </Button>
      </header>
      {root === null ? (
        <div className="min-h-0 flex-1 overflow-y-auto">
          <ThreadList client={client} room={room} onOpen={onOpen} />
        </div>
      ) : (
        <ThreadView key={root} client={client} room={room} root={root} me={me} actions={actions} />
      )}
    </aside>
  );
}
