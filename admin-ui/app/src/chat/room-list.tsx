import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon, UserAdd01Icon, UserGroupIcon } from "@hugeicons/core-free-icons";
import { KnownMembership, type MatrixClient, type Room } from "matrix-js-sdk";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { Avatar } from "./avatar";
import { directPartner, directRooms, isSpace, lastActive, memberName, plainBody, ROOM_EVENTS, roomName, spaceChildren, unread, useEmitted } from "./model";

export interface RoomListProps {
  client: MatrixClient;
  current: string | null;
  space: string | null;
  /** What is typed in the screen's search. */
  filter: string;
  onOpen: (roomId: string) => void;
  onNewRoom: () => void;
  onNewChat: () => void;
  onJoin: () => void;
}

const TEXT_TYPES = new Set(["m.text", "m.emote", "m.notice"]);

/** The last message's line under the room's name. A picture, file or
 *  voice message says so rather than showing its file name. */
function preview(room: Room, attachment: string): string {
  const last = [...room.getLiveTimeline().getEvents()].reverse().find((ev) => ev.getType() === "m.room.message" || ev.getType() === "m.room.encrypted");
  if (last === undefined) return "";
  const content = last.getContent();
  const text = typeof content["msgtype"] !== "string" || TEXT_TYPES.has(content["msgtype"]);
  const body = last.isRedacted() ? "" : text ? plainBody(content) : attachment;
  const who = last.getSender() === room.client.getUserId() ? "" : `${room.getMember(last.getSender() ?? "")?.name ?? ""}: `;
  return body === "" ? "" : `${who}${body}`;
}

function RoomRow({ client, room, current, onOpen, invite }: { client: MatrixClient; room: Room; current: string | null; onOpen: (id: string) => void; invite?: boolean }) {
  const t = useT();
  const counts = unread(room);
  const active = room.roomId === current;
  const line = invite === true ? t("chat.list.invitedBy", { name: memberName(room, room.getMember(client.getSafeUserId())?.events.member?.getSender() ?? "") }) : preview(room, t("chat.reply.attachment"));
  return (
    <li>
      <button
        type="button"
        onClick={() => onOpen(room.roomId)}
        aria-current={active ? "page" : undefined}
        className={cn(
          "flex w-full items-center gap-2.5 rounded-control px-2 py-1.5 text-left",
          active ? "bg-accent-wash text-accent" : "text-ink hover:bg-sunk",
        )}
      >
        <Avatar client={client} id={room.roomId} name={roomName(room)} mxc={room.getMxcAvatarUrl()} size={32} />
        <span className="min-w-0 flex-1">
          <span className={cn("block truncate text-[13.5px]", counts.total > 0 ? "font-semibold" : "font-medium")}>{roomName(room)}</span>
          {line !== "" && <span className="block truncate text-[12px] text-muted">{line}</span>}
        </span>
        {counts.total > 0 && (
          <span
            className={cn(
              "min-w-5 rounded-full px-1.5 text-center text-[11px] leading-5 font-semibold tabular-nums",
              counts.highlight > 0 ? "bg-crit text-surface" : "bg-accent text-surface",
            )}
            aria-label={t("chat.list.unread", { count: counts.total })}
          >
            {counts.total}
          </span>
        )}
      </button>
    </li>
  );
}

function Section({ title, children, action }: { title: string; children: React.ReactNode; action?: React.ReactNode }) {
  return (
    <section className="mt-3 first:mt-1">
      <div className="flex items-center justify-between px-2 pb-1">
        <h2 className="text-[11.5px] font-semibold tracking-wide text-faint uppercase">{title}</h2>
        {action}
      </div>
      <ul className="flex flex-col gap-0.5">{children}</ul>
    </section>
  );
}

/** Invites, people and rooms, newest first; narrowed to a space when one
 *  is picked, and by what is typed in the search. */
export function RoomList({ client, current, space, filter, onOpen, onNewRoom, onNewChat, onJoin }: RoomListProps) {
  const t = useT();
  const tick = useEmitted(client, ROOM_EVENTS);

  const { invites, people, rooms } = React.useMemo(() => {
    const direct = directRooms(client);
    const all = client.getRooms().filter((room) => !isSpace(room));
    const spaceRoom = space === null ? null : client.getRoom(space);
    const inSpace = space === null ? null : new Set(spaceRoom === null ? [] : spaceChildren(spaceRoom));
    const wanted = filter.trim().toLowerCase();
    const shown = all
      .filter((room) => inSpace === null || inSpace.has(room.roomId))
      .filter((room) => wanted === "" || roomName(room).toLowerCase().includes(wanted))
      .sort((a, b) => lastActive(b) - lastActive(a));
    return {
      invites: shown.filter((room) => room.getMyMembership() === KnownMembership.Invite),
      people: shown.filter((room) => room.getMyMembership() === KnownMembership.Join && directPartner(client, room, direct) !== null),
      rooms: shown.filter((room) => room.getMyMembership() === KnownMembership.Join && directPartner(client, room, direct) === null),
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client, space, filter, tick]);

  const plus = (label: string, onClick: () => void, icon = Add01Icon) => (
    <Button variant="ghost" size="icon-sm" aria-label={label} title={label} onClick={onClick}>
      <HugeiconsIcon icon={icon} className="size-4" />
    </Button>
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
        {invites.length > 0 && (
          <Section title={t("chat.list.invites")}>
            {invites.map((room) => (
              <RoomRow key={room.roomId} client={client} room={room} current={current} onOpen={onOpen} invite />
            ))}
          </Section>
        )}
        <Section title={t("chat.list.people")} action={plus(t("chat.list.newChat"), onNewChat, UserAdd01Icon)}>
          {people.map((room) => (
            <RoomRow key={room.roomId} client={client} room={room} current={current} onOpen={onOpen} />
          ))}
          {people.length === 0 && <li className="px-2 py-1 text-[12.5px] text-faint">{t("chat.list.noPeople")}</li>}
        </Section>
        <Section title={t("chat.list.rooms")} action={plus(t("chat.list.newRoom"), onNewRoom)}>
          {rooms.map((room) => (
            <RoomRow key={room.roomId} client={client} room={room} current={current} onOpen={onOpen} />
          ))}
          {rooms.length === 0 && <li className="px-2 py-1 text-[12.5px] text-faint">{t("chat.list.noRooms")}</li>}
        </Section>
        <div className="mt-4 px-2">
          <Button variant="secondary" size="sm" className="w-full" onClick={onJoin}>
            <HugeiconsIcon icon={UserGroupIcon} className="size-4" />
            {t("chat.list.join")}
          </Button>
        </div>
      </div>
    </div>
  );
}
