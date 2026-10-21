/* What the chat page draws, read off the SDK's live objects.
 *
 * The SDK keeps rooms, members and timelines and fires events when they
 * change; `useEmitted` turns those into re-renders. The timeline the page
 * shows is built here from the room's raw events: an edit, a reaction, a
 * redaction or a thread reply is folded into the message it belongs to
 * rather than drawn on its own. Threads are kept apart from the main
 * timeline on this page because the homeserver predates the SDK's own
 * thread support: thread replies arrive in the room's timeline with an
 * `m.thread` relation, and the thread panel reads the rest with /relations
 * (and lososd's /threads for the list). */

import * as React from "react";
import {
  ClientEvent,
  EventType,
  type MatrixClient,
  type MatrixEvent,
  NotificationCountType,
  type Room,
  RoomEvent,
  RoomStateEvent,
  KnownMembership,
} from "matrix-js-sdk";
import type { RoomMessageEventContent } from "matrix-js-sdk/lib/@types/events";

interface Emitter {
  on(event: string, listener: (...args: never[]) => void): unknown;
  off(event: string, listener: (...args: never[]) => void): unknown;
}

/** Re-render when any of `events` fires on `emitter`. */
export function useEmitted(emitter: Emitter | null | undefined, events: readonly string[]): number {
  const [tick, setTick] = React.useState(0);
  const names = events.join(",");
  React.useEffect(() => {
    if (emitter === null || emitter === undefined) return;
    let queued = false;
    // Sync delivers events in bursts; one re-render per frame is plenty.
    const bump = (): void => {
      if (queued) return;
      queued = true;
      requestAnimationFrame(() => {
        queued = false;
        setTick((n) => n + 1);
      });
    };
    const list = names.split(",");
    for (const name of list) emitter.on(name, bump);
    return () => {
      for (const name of list) emitter.off(name, bump);
    };
  }, [emitter, names]);
  return tick;
}

/** Send a message this page built itself. The SDK types message content as
 *  a union a hand-built object never narrows to. */
export function sendContent(client: MatrixClient, roomId: string, content: Record<string, unknown>): Promise<unknown> {
  return client.sendMessage(roomId, content as unknown as RoomMessageEventContent);
}

// ── Rooms ─────────────────────────────────────────────────────────────────

export const ROOM_EVENTS = [
  ClientEvent.Room,
  ClientEvent.DeleteRoom,
  ClientEvent.AccountData,
  RoomEvent.Timeline,
  RoomEvent.Name,
  RoomEvent.MyMembership,
  RoomEvent.UnreadNotifications,
  RoomEvent.Receipt,
  RoomStateEvent.Events,
] as const;

export function isSpace(room: Room): boolean {
  return room.isSpaceRoom();
}

/** The rooms m.direct lists, by room ID, with the person each one is with. */
export function directRooms(client: MatrixClient): Map<string, string> {
  const content = (client.getAccountData(EventType.Direct)?.getContent() ?? {}) as Record<string, string[]>;
  const out = new Map<string, string>();
  for (const [user, rooms] of Object.entries(content)) for (const id of rooms) out.set(id, user);
  return out;
}

/** Whom a direct room is with: m.direct first, then an invite's flag. */
export function directPartner(client: MatrixClient, room: Room, direct: Map<string, string>): string | null {
  const listed = direct.get(room.roomId);
  if (listed !== undefined) return listed;
  const me = room.getMember(client.getSafeUserId());
  if (me?.membership === KnownMembership.Invite && me.events.member?.getContent()["is_direct"] === true) {
    return me.events.member.getSender() ?? null;
  }
  return null;
}

export function lastActive(room: Room): number {
  return room.getLastActiveTimestamp() || 0;
}

export function unread(room: Room): { total: number; highlight: number } {
  return {
    total: room.getUnreadNotificationCount(NotificationCountType.Total),
    highlight: room.getUnreadNotificationCount(NotificationCountType.Highlight),
  };
}

/** The rooms a space lists as children (m.space.child with a `via`). */
export function spaceChildren(space: Room): string[] {
  return space.currentState
    .getStateEvents(EventType.SpaceChild)
    .filter((ev) => Array.isArray(ev.getContent()["via"]) && (ev.getContent()["via"] as unknown[]).length > 0)
    .map((ev) => ev.getStateKey() ?? "")
    .filter((id) => id !== "");
}

/** Whether `userId` may send state of `type` in `room`. */
export function maySend(room: Room, userId: string, type: string): boolean {
  return room.currentState.maySendStateEvent(type, userId);
}

// ── The timeline ──────────────────────────────────────────────────────────

export interface Reaction {
  key: string;
  count: number;
  senders: string[];
  /** The event of the signed-in person's own reaction, to take it back. */
  mine: MatrixEvent | null;
}

export interface Message {
  kind: "message";
  event: MatrixEvent;
  /** The content to draw: the newest edit's `m.new_content`, else the event's. */
  content: Record<string, unknown>;
  edited: boolean;
  reactions: Reaction[];
  replyTo: string | null;
  thread: { count: number; latest: MatrixEvent | null } | null;
}

export interface Notice {
  kind: "notice";
  event: MatrixEvent;
}

export type Item = Message | Notice;

const NOTICE_TYPES = new Set<string>([
  EventType.RoomMember,
  EventType.RoomName,
  EventType.RoomTopic,
  EventType.RoomEncryption,
  EventType.RoomCreate,
  EventType.RoomAvatar,
  EventType.CallInvite,
  EventType.CallHangup,
]);

const MESSAGE_TYPES = new Set<string>([EventType.RoomMessage, EventType.Sticker, EventType.RoomMessageEncrypted]);

export function relationOf(ev: MatrixEvent): { rel_type?: string; event_id?: string; key?: string } | null {
  const relation = ev.getWireContent()["m.relates_to"] ?? ev.getContent()["m.relates_to"];
  return typeof relation === "object" && relation !== null ? (relation as { rel_type?: string; event_id?: string; key?: string }) : null;
}

export function threadRootOf(ev: MatrixEvent): string | null {
  const relation = relationOf(ev);
  return relation?.rel_type === "m.thread" && typeof relation.event_id === "string" ? relation.event_id : null;
}

export function replyIdOf(ev: MatrixEvent): string | null {
  const content = ev.getContent();
  const relation = (content["m.relates_to"] ?? ev.getWireContent()["m.relates_to"]) as
    | { "m.in_reply_to"?: { event_id?: string }; is_falling_back?: boolean }
    | undefined;
  // A thread reply's fallback reply is not a reply anyone wrote.
  if (relation?.is_falling_back === true) return null;
  const id = relation?.["m.in_reply_to"]?.event_id;
  return typeof id === "string" ? id : null;
}

/** Fold a room's raw events into what the page draws. `inThread` builds a
 *  thread's own timeline (its root first, then its replies); otherwise the
 *  main timeline, with thread replies counted on their roots. */
export function buildItems(events: readonly MatrixEvent[], me: string, inThread: string | null = null): Item[] {
  const edits = new Map<string, MatrixEvent>();
  const reactions = new Map<string, Map<string, Reaction>>();
  const threads = new Map<string, { count: number; latest: MatrixEvent | null }>();
  const senders = new Map<string, string>();
  for (const ev of events) senders.set(ev.getId() ?? "", ev.getSender() ?? "");

  for (const ev of events) {
    if (ev.isRedacted()) continue;
    const relation = relationOf(ev);
    if (relation === null || typeof relation.event_id !== "string") continue;
    const target = relation.event_id;
    if (relation.rel_type === "m.replace") {
      // Only the original sender can edit their message.
      if (senders.get(target) !== undefined && senders.get(target) !== ev.getSender()) continue;
      const known = edits.get(target);
      if (known === undefined || known.getTs() <= ev.getTs()) edits.set(target, ev);
    } else if (relation.rel_type === "m.annotation" && ev.getType() === EventType.Reaction && typeof relation.key === "string") {
      const byKey = reactions.get(target) ?? new Map<string, Reaction>();
      const reaction = byKey.get(relation.key) ?? { key: relation.key, count: 0, senders: [], mine: null };
      const sender = ev.getSender() ?? "";
      if (!reaction.senders.includes(sender)) {
        reaction.count += 1;
        reaction.senders.push(sender);
      }
      if (sender === me) reaction.mine = ev;
      byKey.set(relation.key, reaction);
      reactions.set(target, byKey);
    } else if (relation.rel_type === "m.thread" && inThread === null) {
      const thread = threads.get(target) ?? { count: 0, latest: null };
      thread.count += 1;
      if (thread.latest === null || thread.latest.getTs() <= ev.getTs()) thread.latest = ev;
      threads.set(target, thread);
    }
  }

  const items: Item[] = [];
  for (const ev of events) {
    const type = ev.getType();
    const relation = relationOf(ev);
    if (relation?.rel_type === "m.replace" || type === EventType.Reaction || type === EventType.RoomRedaction) continue;
    const root = threadRootOf(ev);
    if (inThread === null ? root !== null : root !== inThread && ev.getId() !== inThread) continue;
    if (MESSAGE_TYPES.has(type)) {
      const id = ev.getId() ?? "";
      const edit = edits.get(id);
      const newContent = edit?.getContent()["m.new_content"];
      items.push({
        kind: "message",
        event: ev,
        content: !ev.isRedacted() && typeof newContent === "object" && newContent !== null ? (newContent as Record<string, unknown>) : ev.getContent(),
        edited: edit !== undefined && !ev.isRedacted(),
        reactions: [...(reactions.get(id)?.values() ?? [])],
        replyTo: replyIdOf(ev),
        thread: threads.get(id) ?? null,
      });
    } else if (NOTICE_TYPES.has(type) && inThread === null) {
      items.push({ kind: "notice", event: ev });
    }
  }
  return items;
}

/** Whether two messages in a row are one person's run (drawn without a
 *  second name and avatar). */
export function continues(prev: Item | undefined, next: Item): boolean {
  return (
    prev !== undefined &&
    prev.kind === "message" &&
    next.kind === "message" &&
    prev.event.getSender() === next.event.getSender() &&
    next.event.getTs() - prev.event.getTs() < 5 * 60_000
  );
}

/** The body a message reply or notification quotes; an edit quotes its new text, not the "* " fallback. */
export function plainBody(content: Record<string, unknown>): string {
  const edited = content["m.new_content"];
  const body = (edited !== null && typeof edited === "object" ? (edited as Record<string, unknown>) : content)["body"];
  return typeof body === "string" ? body : "";
}

/** A room or person's initial, for an avatar with no picture. */
export function initial(name: string): string {
  const letter = [...name.replace(/^[@#!]/, "")][0];
  return (letter ?? "?").toUpperCase();
}

/** A colour index (0..7) from a user or room ID, the same on every page. */
export function hue(id: string): number {
  let sum = 0;
  for (const char of id) sum = (sum * 31 + char.charCodeAt(0)) >>> 0;
  return sum % 8;
}

/** The localpart of a Matrix user ID, for search and short names. */
export function localpart(userId: string): string {
  return userId.replace(/^@/, "").split(":")[0] ?? userId;
}
