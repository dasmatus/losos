import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  ArrowTurnBackwardIcon,
  CloudUploadIcon,
  Comment01Icon,
  Copy01Icon,
  Delete02Icon,
  PencilEdit01Icon,
  SmileIcon,
} from "@hugeicons/core-free-icons";
import {
  ClientEvent,
  Direction,
  EventStatus,
  EventType,
  type MatrixClient,
  MatrixEvent,
  MatrixEventEvent,
  RelationType,
  type Room,
  RoomEvent,
  RoomMemberEvent,
} from "matrix-js-sdk";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { contextMenu, copyText, type MenuBuilder } from "@/lib/context-menu";
import { getLocale, INTL_TAG } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { Avatar } from "./avatar";
import { callable, callEnded, callEvent, useCalls } from "./calls";
import { saveAttachment, useCloud } from "./cloud";
import { carriesFile, fileNameOf, infoOf, MessageBody, sourceOf } from "./message-body";
import { buildItems, continues, type Item, type Message, memberName, plainBody, shownName, useEmitted } from "./model";

export const QUICK_REACTIONS = ["👍", "❤️", "😂", "🎉", "😮", "😢", "🙏", "👀"] as const;

export const TIMELINE_EVENTS = [
  RoomEvent.Timeline,
  RoomEvent.TimelineReset,
  RoomEvent.Redaction,
  RoomEvent.LocalEchoUpdated,
  RoomEvent.Receipt,
  MatrixEventEvent.Decrypted,
  ClientEvent.Event,
] as const;

export interface RowActions {
  onReply: (event: MatrixEvent) => void;
  onEdit: (event: MatrixEvent) => void;
  onThread: ((rootId: string) => void) | null;
  onOpenImage: (url: string) => void;
}

const clock = (ts: number): string =>
  new Intl.DateTimeFormat(INTL_TAG[getLocale()], { hour: "2-digit", minute: "2-digit" }).format(ts);

const day = (ts: number): string =>
  new Intl.DateTimeFormat(INTL_TAG[getLocale()], { weekday: "long", day: "numeric", month: "long" }).format(ts);

function sameDay(a: number, b: number): boolean {
  return new Date(a).toDateString() === new Date(b).toDateString();
}

/** The message a reply quotes, from the room or fetched once. */
function useQuoted(client: MatrixClient, room: Room, id: string | null): MatrixEvent | null | "missing" {
  const [found, setFound] = React.useState<MatrixEvent | null | "missing">(() => (id === null ? null : (room.findEventById(id) ?? null)));
  React.useEffect(() => {
    if (id === null) return;
    const local = room.findEventById(id);
    if (local !== undefined) {
      setFound(local);
      return;
    }
    let live = true;
    client
      .fetchRoomEvent(room.roomId, id)
      .then(async (raw) => {
        const ev = new MatrixEvent(raw);
        await client.decryptEventIfNeeded(ev);
        if (live) setFound(ev);
      })
      .catch(() => live && setFound("missing"));
    return () => {
      live = false;
    };
  }, [client, room, id]);
  return found;
}

function ReplyQuote({ client, room, id }: { client: MatrixClient; room: Room; id: string }) {
  const t = useT();
  const quoted = useQuoted(client, room, id);
  const text =
    quoted === null
      ? "…"
      : quoted === "missing"
        ? t("chat.reply.missing")
        : quoted.isRedacted()
          ? t("chat.message.deleted")
          : plainBody(quoted.getContent()) || t("chat.reply.attachment");
  const who = quoted !== null && quoted !== "missing" ? (memberName(room, quoted.getSender() ?? "")) : null;
  return (
    <div className="mb-1 max-w-md border-l-2 border-accent pl-2 text-[12.5px] leading-snug text-muted">
      {who !== null && <span className="font-medium text-ink">{who} </span>}
      <span className="line-clamp-2">{text}</span>
    </div>
  );
}

function ReactionBar({ client, room, item }: { client: MatrixClient; room: Room; item: Message }) {
  const t = useT();
  if (item.reactions.length === 0) return null;
  const toggle = (key: string, mine: MatrixEvent | null): void => {
    if (mine !== null) {
      void client.redactEvent(room.roomId, mine.getId() ?? "");
    } else {
      void client.sendEvent(room.roomId, EventType.Reaction, {
        "m.relates_to": { rel_type: RelationType.Annotation, event_id: item.event.getId() ?? "", key },
      });
    }
  };
  return (
    <div className="mt-1 flex flex-wrap gap-1">
      {item.reactions.map((reaction) => {
        const names = reaction.senders.map((id) => memberName(room, id)).join(", ");
        return (
          <button
            key={reaction.key}
            type="button"
            title={t("chat.reaction.who", { names })}
            aria-pressed={reaction.mine !== null}
            onClick={() => toggle(reaction.key, reaction.mine)}
            className={cn(
              "inline-flex h-6 items-center gap-1 rounded-full border px-2 text-[12.5px]",
              reaction.mine !== null ? "border-accent bg-accent-wash text-accent" : "border-line bg-surface text-ink hover:bg-sunk",
            )}
          >
            <span>{reaction.key}</span>
            <span className="tabular-nums">{reaction.count}</span>
          </button>
        );
      })}
    </div>
  );
}

/** What can be done to one message, shared by its hover toolbar and its
 *  right-click menu, so the two never offer different things. */
function useMoves(client: MatrixClient, room: Room, item: Message, me: string) {
  const ev = item.event;
  const mine = ev.getSender() === me;
  const canRedact = mine || room.currentState.hasSufficientPowerLevelFor("redact", room.getMember(me)?.powerLevel ?? 0);
  const isText = typeof item.content["msgtype"] === "string" && ["m.text", "m.emote", "m.notice"].includes(item.content["msgtype"] as string);
  const live = !ev.isRedacted() && ev.status === null;
  const react = (key: string): void => {
    const already = item.reactions.find((r) => r.key === key)?.mine;
    if (already !== null && already !== undefined) return;
    void client.sendEvent(room.roomId, EventType.Reaction, {
      "m.relates_to": { rel_type: RelationType.Annotation, event_id: ev.getId() ?? "", key },
    });
  };
  const remove = (confirmText: string): void => {
    if (window.confirm(confirmText)) void client.redactEvent(room.roomId, ev.getId() ?? "");
  };
  const cloud = useCloud();
  const file = cloud && carriesFile(item.content);
  const save = (t: ReturnType<typeof useT>): void =>
    void saveAttachment(client, t, { eventId: ev.getId() ?? "", source: sourceOf(item.content), mime: infoOf(item.content).mimetype, name: fileNameOf(item.content) });
  return { ev, mine, canRedact, isText, live, react, remove, file, save };
}

function messageMenu(
  t: ReturnType<typeof useT>,
  moves: ReturnType<typeof useMoves>,
  actions: RowActions,
  sender: string,
): MenuBuilder {
  const { ev, mine, canRedact, isText, live, react, remove, file, save } = moves;
  return () => {
    if (!live) return null;
    const text = isText ? plainBody(ev.getContent()) : "";
    const copied = async (): Promise<void> => {
      if (await copyText(text)) toast.done(t("menu.copied"));
      else toast.error(t("menu.copyFailed"), undefined, { help: "index" });
    };
    return {
      label: t("chat.message.menu", { name: sender }),
      sections: [
        [
          { id: "reply", label: t("chat.message.reply"), icon: ArrowTurnBackwardIcon, onSelect: () => actions.onReply(ev) },
          {
            id: "thread",
            label: t("chat.thread.reply"),
            icon: Comment01Icon,
            hidden: actions.onThread === null,
            onSelect: () => actions.onThread?.(ev.getId() ?? ""),
          },
          {
            kind: "submenu",
            id: "react",
            label: t("chat.reaction.add"),
            icon: SmileIcon,
            items: QUICK_REACTIONS.map((key) => ({ id: `react-${key}`, label: key, onSelect: () => react(key) })),
          },
          { id: "copy", label: t("menu.copy"), icon: Copy01Icon, hidden: text === "", onSelect: () => void copied() },
          { id: "save", label: t("together.save.action"), icon: CloudUploadIcon, hidden: !file, onSelect: () => save(t) },
        ],
        [
          { id: "edit", label: t("chat.message.edit"), icon: PencilEdit01Icon, hidden: !(mine && isText), onSelect: () => actions.onEdit(ev) },
          {
            id: "delete",
            label: t("chat.message.delete"),
            icon: Delete02Icon,
            danger: true,
            hidden: !canRedact,
            onSelect: () => remove(t("chat.message.deleteConfirm")),
          },
        ],
      ],
    };
  };
}

function Toolbar({ moves, actions }: { moves: ReturnType<typeof useMoves>; actions: RowActions }) {
  const t = useT();
  const [picking, setPicking] = React.useState(false);
  const { ev, mine, canRedact, isText, live, react, remove } = moves;
  if (!live) return null;
  return (
    <div
      className={cn(
        "absolute -top-4 right-3 z-10 flex items-center gap-0.5 rounded-control border border-line bg-surface p-0.5 shadow-card",
        picking ? "flex" : "hidden group-hover:flex group-focus-within:flex",
      )}
    >
      {picking ? (
        <div role="menu" aria-label={t("chat.reaction.pick")} className="flex items-center gap-0.5">
          {QUICK_REACTIONS.map((key) => (
            <button
              key={key}
              type="button"
              role="menuitem"
              className="rounded-control px-1.5 py-1 text-base hover:bg-sunk"
              onClick={() => {
                setPicking(false);
                react(key);
              }}
            >
              {key}
            </button>
          ))}
        </div>
      ) : (
        <Button variant="ghost" size="icon-sm" aria-label={t("chat.reaction.add")} title={t("chat.reaction.add")} onClick={() => setPicking(true)}>
          <HugeiconsIcon icon={SmileIcon} className="size-4" />
        </Button>
      )}
      <Button variant="ghost" size="icon-sm" aria-label={t("chat.message.reply")} title={t("chat.message.reply")} onClick={() => actions.onReply(ev)}>
        <HugeiconsIcon icon={ArrowTurnBackwardIcon} className="size-4" />
      </Button>
      {actions.onThread !== null && (
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t("chat.thread.reply")}
          title={t("chat.thread.reply")}
          onClick={() => actions.onThread?.(ev.getId() ?? "")}
        >
          <HugeiconsIcon icon={Comment01Icon} className="size-4" />
        </Button>
      )}
      {mine && isText && (
        <Button variant="ghost" size="icon-sm" aria-label={t("chat.message.edit")} title={t("chat.message.edit")} onClick={() => actions.onEdit(ev)}>
          <HugeiconsIcon icon={PencilEdit01Icon} className="size-4" />
        </Button>
      )}
      {canRedact && (
        <Button
          variant="ghost"
          size="icon-sm"
          className="hover:text-crit"
          aria-label={t("chat.message.delete")}
          title={t("chat.message.delete")}
          onClick={() => remove(t("chat.message.deleteConfirm"))}
        >
          <HugeiconsIcon icon={Delete02Icon} className="size-4" />
        </Button>
      )}
    </div>
  );
}

function SendState({ client, room, event }: { client: MatrixClient; room: Room; event: MatrixEvent }) {
  const t = useT();
  if (event.status === EventStatus.NOT_SENT) {
    return (
      <p className="mt-0.5 text-[12px] text-crit">
        {t("chat.message.notSent")}{" "}
        <button type="button" className="underline" onClick={() => void client.resendEvent(event, room)}>
          {t("chat.message.retry")}
        </button>
      </p>
    );
  }
  if (event.status !== null && event.status !== EventStatus.SENT) {
    return <p className="mt-0.5 text-[12px] text-faint">{t("chat.message.sending")}</p>;
  }
  return null;
}

export interface MessageRowProps {
  client: MatrixClient;
  room: Room;
  item: Message;
  grouped: boolean;
  me: string;
  actions: RowActions;
  seenBy: string[];
}

export function MessageRow({ client, room, item, grouped, me, actions, seenBy }: MessageRowProps) {
  const t = useT();
  const ev = item.event;
  const sender = ev.getSender() ?? "";
  const member = room.getMember(sender);
  const name = shownName(member?.name ?? sender);
  const moves = useMoves(client, room, item, me);
  return (
    <div
      ref={contextMenu(messageMenu(t, moves, actions, name))}
      className={cn("group relative flex gap-3 px-4 hover:bg-sunk/60 focus-within:bg-sunk/60", grouped ? "py-0.5" : "pt-3 pb-0.5")}
      data-event-id={ev.getId()}
    >
      <div className="w-8 shrink-0">
        {!grouped && <Avatar client={client} id={sender} name={name} mxc={member?.getMxcAvatarUrl()} />}
      </div>
      <div className="min-w-0 flex-1">
        {!grouped && (
          <p className="flex items-baseline gap-2">
            <span className="text-[13.5px] font-semibold">{name}</span>
            <time className="text-[11.5px] text-faint tabular-nums" dateTime={new Date(ev.getTs()).toISOString()}>
              {clock(ev.getTs())}
            </time>
          </p>
        )}
        {item.replyTo !== null && <ReplyQuote client={client} room={room} id={item.replyTo} />}
        <MessageBody client={client} event={ev} content={item.content} me={me} onOpenImage={actions.onOpenImage} />
        {item.edited && <span className="text-[11.5px] text-faint">{t("chat.message.edited")}</span>}
        <ReactionBar client={client} room={room} item={item} />
        {item.thread !== null && actions.onThread !== null && (
          <button
            type="button"
            onClick={() => actions.onThread?.(ev.getId() ?? "")}
            className="mt-1 inline-flex items-center gap-1.5 rounded-control border border-line bg-surface px-2 py-1 text-[12.5px] text-accent hover:bg-accent-wash"
          >
            <HugeiconsIcon icon={Comment01Icon} className="size-3.5" />
            {t("chat.thread.replies", { count: item.thread.count })}
          </button>
        )}
        <SendState client={client} room={room} event={ev} />
        {seenBy.length > 0 && (
          <div className="mt-1 flex items-center justify-end gap-0.5" title={t("chat.receipt.seen", { names: seenBy.map((id) => memberName(room, id)).join(", ") })}>
            {seenBy.slice(0, 5).map((id) => (
              <Avatar key={id} client={client} id={id} name={memberName(room, id)} mxc={room.getMember(id)?.getMxcAvatarUrl()} size={16} className="size-4 text-[9px]" />
            ))}
          </div>
        )}
      </div>
      <Toolbar moves={moves} actions={actions} />
    </div>
  );
}

/* A call's start, with Join while it lasts, and its end. */
function CallNotice({ room, event, who }: { room: Room; event: MatrixEvent; who: string }) {
  const t = useT();
  const calls = useCalls();
  const said = callEvent(event);
  if (said === null) return null;
  if (said.state === "ended") return <p className="px-4 py-1 pl-15 text-[12.5px] text-faint">{t("calls.ended")}</p>;
  const open = calls.available && callable(room) && calls.active === null && !callEnded(room, said.call);
  return (
    <p className="flex items-center gap-2 px-4 py-1 pl-15 text-[12.5px] text-faint" data-call-notice={said.call}>
      {t("chat.notice.called", { who })}
      {open && (
        <Button size="xs" variant="secondary" onClick={() => calls.join(room.roomId, said.call, said.video)}>
          {t("calls.join")}
        </Button>
      )}
    </p>
  );
}

function NoticeRow({ room, event }: { room: Room; event: MatrixEvent }) {
  const t = useT();
  const who = memberName(room, event.getSender() ?? "");
  const content = event.getContent();
  if (event.getType() === "org.losos.call") return <CallNotice room={room} event={event} who={who} />;
  let text: string | null = null;
  switch (event.getType()) {
    case EventType.RoomMember: {
      const target = (content["displayname"] as string | undefined) ?? shownName(event.getStateKey() ?? "");
      const before = event.getPrevContent()["membership"];
      const membership = content["membership"];
      if (membership === "join" && before === "join") text = null;
      else if (membership === "join") text = t("chat.notice.joined", { who: target });
      else if (membership === "invite") text = t("chat.notice.invited", { who, target });
      else if (membership === "leave" && event.getStateKey() === event.getSender()) text = t("chat.notice.left", { who: target || who });
      else if (membership === "leave") text = t("chat.notice.removed", { who, target: shownName(event.getStateKey() ?? "") });
      else if (membership === "ban") text = t("chat.notice.banned", { who, target: shownName(event.getStateKey() ?? "") });
      break;
    }
    case EventType.RoomName:
      text = t("chat.notice.named", { who, name: String(content["name"] ?? "") });
      break;
    case EventType.RoomTopic:
      text = t("chat.notice.topic", { who, topic: String(content["topic"] ?? "") });
      break;
    case EventType.RoomEncryption:
      text = t("chat.notice.encrypted");
      break;
    case EventType.RoomCreate:
      text = t("chat.notice.created", { who });
      break;
    case EventType.RoomAvatar:
      text = t("chat.notice.avatar", { who });
      break;
    case EventType.CallInvite:
      text = t("chat.notice.called", { who });
      break;
    case EventType.CallHangup:
      text = t("chat.notice.hungUp", { who });
      break;
  }
  if (text === null) return null;
  return <p className="px-4 py-1 pl-15 text-[12.5px] text-faint">{text}</p>;
}

/** Who has read up to each event, for the last message they read. */
function receipts(room: Room, items: Item[], me: string): Map<string, string[]> {
  const out = new Map<string, string[]>();
  const placed = new Set<string>([me]);
  for (let i = items.length - 1; i >= 0; i -= 1) {
    const item = items[i];
    if (item?.kind !== "message") continue;
    const readers = room.getUsersReadUpTo(item.event).filter((id) => !placed.has(id) && id !== item.event.getSender());
    for (const id of readers) placed.add(id);
    if (readers.length > 0) out.set(item.event.getId() ?? "", readers);
  }
  return out;
}

export interface ItemListProps {
  client: MatrixClient;
  room: Room;
  items: Item[];
  me: string;
  actions: RowActions;
  showReceipts?: boolean;
}

/** A run of items, with day lines between days. */
export function ItemList({ client, room, items, me, actions, showReceipts = true }: ItemListProps) {
  const seen = showReceipts ? receipts(room, items, me) : new Map<string, string[]>();
  return (
    <>
      {items.map((item, index) => {
        const prev = items[index - 1];
        const newDay = prev === undefined || !sameDay(prev.event.getTs(), item.event.getTs());
        return (
          <React.Fragment key={item.event.getId() ?? item.event.getTxnId() ?? index}>
            {newDay && (
              <div className="my-3 flex items-center gap-3 px-4 text-[12px] font-medium text-faint">
                <span className="h-px flex-1 bg-hair" />
                {day(item.event.getTs())}
                <span className="h-px flex-1 bg-hair" />
              </div>
            )}
            {item.kind === "notice" ? (
              <NoticeRow room={room} event={item.event} />
            ) : (
              <MessageRow
                client={client}
                room={room}
                item={item}
                grouped={!newDay && continues(prev, item)}
                me={me}
                actions={actions}
                seenBy={seen.get(item.event.getId() ?? "") ?? []}
              />
            )}
          </React.Fragment>
        );
      })}
    </>
  );
}

/** Keep a scroller at the bottom while it is there, and keep the reader's
 *  place when older events load above. */
export function useStickToBottom(ref: React.RefObject<HTMLDivElement | null>, deps: unknown[]): { atBottom: React.RefObject<boolean>; keepPlace: () => () => void } {
  const atBottom = React.useRef(true);
  React.useEffect(() => {
    const box = ref.current;
    if (box === null) return;
    const onScroll = (): void => {
      atBottom.current = box.scrollHeight - box.scrollTop - box.clientHeight < 40;
    };
    box.addEventListener("scroll", onScroll, { passive: true });
    return () => box.removeEventListener("scroll", onScroll);
  }, [ref]);
  React.useLayoutEffect(() => {
    const box = ref.current;
    if (box !== null && atBottom.current) box.scrollTop = box.scrollHeight;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  const keepPlace = (): (() => void) => {
    const box = ref.current;
    const before = box === null ? 0 : box.scrollHeight - box.scrollTop;
    return () => {
      if (box !== null) box.scrollTop = box.scrollHeight - before;
    };
  };
  return { atBottom, keepPlace };
}

export interface RoomTimelineProps {
  client: MatrixClient;
  room: Room;
  me: string;
  actions: RowActions;
}

/** A room's main timeline: back-pagination at the top, read receipts as
 *  the person reads, who is typing at the bottom. */
export function RoomTimeline({ client, room, me, actions }: RoomTimelineProps) {
  const t = useT();
  const tick = useEmitted(room, TIMELINE_EVENTS);
  const tock = useEmitted(client, [MatrixEventEvent.Decrypted]);
  const typing = useEmitted(client, [RoomMemberEvent.Typing]);
  const timeline = room.getLiveTimeline();
  const events = timeline.getEvents();
  const items = React.useMemo(() => buildItems(events, me), [events, events.length, me, tick, tock]);
  const box = React.useRef<HTMLDivElement>(null);
  const { keepPlace } = useStickToBottom(box, [items.length, room.roomId, tick]);
  const [loading, setLoading] = React.useState(false);
  const [start, setStart] = React.useState(false);

  React.useEffect(() => setStart(false), [room.roomId]);

  const loadOlder = React.useCallback(async () => {
    if (loading || start) return;
    setLoading(true);
    const restore = keepPlace();
    try {
      const more = await client.paginateEventTimeline(room.getLiveTimeline(), { backwards: true, limit: 30 });
      if (!more || room.getLiveTimeline().getPaginationToken(Direction.Backward) === null) setStart(true);
    } catch {
      /* try again on the next scroll */
    } finally {
      setLoading(false);
      requestAnimationFrame(restore);
    }
  }, [client, room, loading, start, keepPlace]);

  // A short room fills the view by itself: keep loading until it scrolls.
  React.useEffect(() => {
    const el = box.current;
    if (el !== null && el.scrollHeight <= el.clientHeight + 10 && !start) void loadOlder();
  }, [items.length, start, loadOlder]);

  // Read receipts: the newest event, once the page is in front.
  React.useEffect(() => {
    const last = [...events].reverse().find((ev) => ev.getId()?.startsWith("$") === true);
    if (last === undefined || document.hidden) return;
    if (room.hasUserReadEvent(me, last.getId() ?? "")) return;
    void client.sendReadReceipt(last).catch(() => undefined);
  }, [client, room, me, events, events.length, tick]);
  React.useEffect(() => {
    const onVisible = (): void => {
      const last = room.getLiveTimeline().getEvents().at(-1);
      if (!document.hidden && last !== undefined && last.getId()?.startsWith("$") === true) void client.sendReadReceipt(last).catch(() => undefined);
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  }, [client, room]);

  const typers = React.useMemo(
    () => room.getMembers().filter((m) => m.typing && m.userId !== me).map((m) => m.name),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [room, me, typing],
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        ref={box}
        className="min-h-0 flex-1 overflow-y-auto pb-2"
        onScroll={(event) => {
          if (event.currentTarget.scrollTop < 200) void loadOlder();
        }}
        role="log"
        aria-live="polite"
        aria-label={t("chat.timeline.label")}
      >
        {start ? (
          <p className="px-4 pt-6 pb-2 text-center text-[12.5px] text-faint">{t("chat.timeline.start")}</p>
        ) : (
          <div className="flex justify-center py-3">{loading && <Spinner size={16} label={t("chat.timeline.loading")} />}</div>
        )}
        <ItemList client={client} room={room} items={items} me={me} actions={actions} />
      </div>
      <p className="h-5 truncate px-4 text-[12px] text-muted" aria-live="polite">
        {typers.length === 1
          ? t("chat.typing.one", { name: typers[0] ?? "" })
          : typers.length > 1
            ? t("chat.typing.many", { names: typers.join(", ") })
            : ""}
      </p>
    </div>
  );
}
