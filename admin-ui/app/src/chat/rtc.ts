import { EventType, KnownMembership, type MatrixEvent, type Room } from "matrix-js-sdk";
import { RTC_SLOT_ENCRYPTION_PER_MEMBER } from "matrix-js-sdk/lib/matrixrtc";

/* Calls as a MatrixRTC application (MSC4143), read and written here so
 * the call screen, the timeline and room creation agree.
 *
 * The application is LosOS's own, `org.losos.call`, in one slot per room,
 * `org.losos.call#room`. MSC4196's `m.call` is defined for the LiveKit
 * transport only, and these calls go through the box's SFU: other clients
 * ignore an application they do not know instead of offering a Join that
 * cannot work. A membership is a sticky `org.matrix.msc4143.rtc.member`
 * event (MSC4354) that publishes an `org.losos.sfu` transport naming the
 * SFU's call; the homeserver holds a delayed leave (MSC4140) that ends it
 * when the page stops answering. */

export const RTC_APPLICATION = "org.losos.call";
export const RTC_SLOT = { application: RTC_APPLICATION, id: "room" } as const;
export const RTC_SLOT_ID = `${RTC_SLOT.application}#${RTC_SLOT.id}`;
export const SFU_TRANSPORT = "org.losos.sfu";
const SFU_PATH = "/calls/api/ws";
export const CALL_ID = /^[0-9a-f]{32}$/;

/** The transport a member publishes: the SFU's websocket and its call. */
export interface SfuTransport {
  type: typeof SFU_TRANSPORT;
  url: string;
  call: string;
}

export const sfuUrl = (): string => `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}${SFU_PATH}`;

/** The SFU transport in `raw`, if it is one. Its `url` says where the
 *  member reached the SFU, which may be the box under another name; this
 *  page always connects through its own origin (`sfuUrl`), so a room member
 *  cannot point a join at another host. */
export function sfuTransport(raw: unknown): SfuTransport | null {
  if (typeof raw !== "object" || raw === null) return null;
  const { type, url, call } = raw as Record<string, unknown>;
  if (type !== SFU_TRANSPORT || typeof url !== "string" || typeof call !== "string" || !CALL_ID.test(call)) return null;
  let at: URL;
  try {
    at = new URL(url);
  } catch {
    return null;
  }
  if ((at.protocol !== "wss:" && at.protocol !== "ws:") || at.pathname !== SFU_PATH) return null;
  return { type, url, call };
}

/** One device in the slot. */
export interface RtcMember {
  user: string;
  device: string;
  /** When this membership was sent, by the homeserver's clock. */
  since: number;
  video: boolean;
  transport: SfuTransport;
  event: MatrixEvent;
}

const record = (raw: unknown): Record<string, unknown> | null =>
  typeof raw === "object" && raw !== null && !Array.isArray(raw) ? (raw as Record<string, unknown>) : null;

/** The call and kind a membership of this slot names, read from its
 *  content alone. */
function membershipCall(ev: MatrixEvent): { member: Record<string, unknown>; transport: SfuTransport; video: boolean } | null {
  if (ev.getType() !== EventType.RTCMembership) return null;
  const content = ev.getContent();
  const application = record(content["application"]);
  const member = record(content["member"]);
  if (content["slot_id"] !== RTC_SLOT_ID || application?.["type"] !== RTC_APPLICATION || member === null) return null;
  const published = record(content["transports"])?.["published"];
  const transport = Array.isArray(published) ? published.map(sfuTransport).find((t) => t !== null) : undefined;
  if (transport === undefined) return null;
  return { member, transport, video: application["m.call.intent"] === "video" };
}

/** Whether `ev` is a joined membership of this slot, and its parts. A
 *  withdrawn one (the sticky key alone) names nothing and is left, and the
 *  member must be its sender, a device of theirs, and in the room. */
function joinedMember(room: Room, ev: MatrixEvent): RtcMember | null {
  const said = membershipCall(ev);
  const user = ev.getSender();
  if (said === null || user === undefined) return null;
  const device = said.member["device_id"];
  if (said.member["user_id"] !== user || typeof device !== "string" || typeof said.member["id"] !== "string") return null;
  if (!room.hasMembershipState(user, KnownMembership.Join)) return null;
  return { user, device, since: ev.getTs(), video: said.video, transport: said.transport, event: ev };
}

/** The slot's state event, if the room has one and it is open for calls. */
export function slotOpen(room: Room): boolean {
  const content = room.currentState.getStateEvents(EventType.RTCSlot, RTC_SLOT_ID)?.getContent();
  const application = content?.["application"] as { type?: unknown } | undefined;
  return content?.["status"] === "open" && application?.type === RTC_APPLICATION;
}

/** Everyone in the room's call, oldest first. MSC4143 counts a member only
 *  while the slot is open; the room keeps a sticky event only until it
 *  runs out. */
export function rtcMembers(room: Room): RtcMember[] {
  if (!slotOpen(room)) return [];
  return [...room._unstable_getStickyEvents()]
    .map((ev) => joinedMember(room, ev))
    .filter((m): m is RtcMember => m !== null)
    .sort((a, b) => a.since - b.since);
}

/** The call going on in the room, if any: the oldest member's. Two people
 *  who start at the same moment both end up in this one. */
export function liveCall(room: Room): { call: string; video: boolean } | null {
  const members = rtcMembers(room);
  const first = members[0];
  if (first === undefined) return null;
  return { call: first.transport.call, video: members.some((m) => m.video) };
}

/** Whether anyone is in `call` right now. */
export const callLive = (room: Room, call: string): boolean => rtcMembers(room).some((m) => m.transport.call === call);

/** The call a membership event names, for a notification that refers to it. */
export function callOfMembership(room: Room, eventId: string): { call: string; video: boolean } | null {
  const ev = [...room._unstable_getStickyEvents()].find((e) => e.getId() === eventId) ?? room.findEventById(eventId);
  const said = ev === undefined ? null : membershipCall(ev);
  return said === null ? null : { call: said.transport.call, video: said.video };
}

/** What `user` needs to take part: the open slot, and the right to send a
 *  membership, which any member has by default. */
export function mayCall(room: Room, user: string): boolean {
  return slotOpen(room) && room.currentState.maySendEvent(EventType.RTCMembership, user);
}

/** Whether `user` can open the slot of an older room. */
export const mayOpenSlot = (room: Room, user: string): boolean => room.currentState.maySendStateEvent(EventType.RTCSlot, user);

/** The slot event for an encrypted room (MSC4143 asks for encryption in
 *  the slot whenever the room is encrypted). */
export const SLOT_CONTENT = {
  status: "open",
  application: { type: RTC_APPLICATION },
  encryption: { type: RTC_SLOT_ENCRYPTION_PER_MEMBER },
} as const;

export const slotState = { type: EventType.RTCSlot, state_key: RTC_SLOT_ID, content: SLOT_CONTENT };

/** The plain notice a new call also puts in the room, for Matrix apps that
 *  do not know these calls (calls.tsx, `announceStart`). LosOS Chat draws
 *  the call from its MatrixRTC events and leaves this one out. */
export const isStartNotice = (content: Record<string, unknown>): boolean =>
  content["msgtype"] === "m.notice" && typeof content[RTC_APPLICATION] === "object" && content[RTC_APPLICATION] !== null;
