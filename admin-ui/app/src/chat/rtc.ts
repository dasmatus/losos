import { EventType, KnownMembership, type MatrixEvent, type Room } from "matrix-js-sdk";
import { RTC_SLOT_ENCRYPTION_PER_MEMBER } from "matrix-js-sdk/lib/matrixrtc";

/* Calls as a MatrixRTC application (MSC4143), read and written here so
 * the call screen, the timeline and room creation agree.
 *
 * The application is LosOS's own, `org.losos.call`, in one slot per room,
 * `org.losos.call#room`. MSC4196's `m.call` is defined for the LiveKit
 * transport only, and these calls go through the box's SFU: other clients
 * ignore an application they do not know instead of offering a Join that
 * cannot work. Membership is the state-event form matrix-js-sdk sends
 * (`org.matrix.msc3401.call.member`), because Dendrite has no sticky
 * events; each one publishes an `org.losos.sfu` transport naming the SFU's
 * call. */

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
  /** When this membership began, by the homeserver's clock unless the
   *  member's client carried it over from an earlier event. */
  since: number;
  video: boolean;
  transport: SfuTransport;
  event: MatrixEvent;
}

/** Whether `content` is a joined membership of this slot, and its parts.
 *  Expired memberships are left: a device that vanished without leaving
 *  stops counting once its `expires` runs out. */
function joinedMember(room: Room, ev: MatrixEvent, now: number): RtcMember | null {
  const content = ev.getContent();
  if (content["application"] !== RTC_APPLICATION || content["call_id"] !== RTC_SLOT.id) return null;
  const user = ev.getSender();
  const device = content["device_id"];
  if (user === undefined || typeof device !== "string" || !room.hasMembershipState(user, KnownMembership.Join)) return null;
  const since = typeof content["created_ts"] === "number" ? content["created_ts"] : ev.getTs();
  const expires = content["expires"];
  if (typeof expires !== "number" || !Number.isFinite(since) || now >= since + expires) return null;
  const preferred = content["foci_preferred"];
  const transport = Array.isArray(preferred) ? preferred.map(sfuTransport).find((t) => t !== null) : undefined;
  if (transport === undefined) return null;
  return { user, device, since, video: content["m.call.intent"] === "video", transport, event: ev };
}

/** The slot's state event, if the room has one and it is open for calls. */
export function slotOpen(room: Room): boolean {
  const content = room.currentState.getStateEvents(EventType.RTCSlot, RTC_SLOT_ID)?.getContent();
  const application = content?.["application"] as { type?: unknown } | undefined;
  return content?.["status"] === "open" && application?.type === RTC_APPLICATION;
}

/** Everyone in the room's call, oldest first. MSC4143 counts a member only
 *  while the slot is open. */
export function rtcMembers(room: Room, now = Date.now()): RtcMember[] {
  if (!slotOpen(room)) return [];
  return room.currentState
    .getStateEvents(EventType.GroupCallMemberPrefix)
    .map((ev) => joinedMember(room, ev, now))
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
  const ev =
    room.currentState.getStateEvents(EventType.GroupCallMemberPrefix).find((e) => e.getId() === eventId) ?? room.findEventById(eventId);
  if (ev === undefined || ev.getType() !== EventType.GroupCallMemberPrefix) return null;
  const content = ev.getContent();
  if (content["application"] !== RTC_APPLICATION) return null;
  const preferred = content["foci_preferred"];
  const transport = Array.isArray(preferred) ? preferred.map(sfuTransport).find((t) => t !== null) : undefined;
  return transport === undefined ? null : { call: transport.call, video: content["m.call.intent"] === "video" };
}

/** What `user` needs to take part: the open slot, and the right to send a
 *  membership. */
export function mayCall(room: Room, user: string): boolean {
  return slotOpen(room) && room.currentState.maySendStateEvent(EventType.GroupCallMemberPrefix, user);
}

/** Whether `user` can open the slot and let everyone in the room send a
 *  membership, as an admin of an older room. */
export function mayEnableCalls(room: Room, user: string): boolean {
  const state = room.currentState;
  return state.maySendStateEvent(EventType.RoomPowerLevels, user) && state.maySendStateEvent(EventType.RTCSlot, user);
}

/** The slot event for an encrypted room (MSC4143 asks for encryption in
 *  the slot whenever the room is encrypted). */
export const SLOT_CONTENT = {
  status: "open",
  application: { type: RTC_APPLICATION },
  encryption: { type: RTC_SLOT_ENCRYPTION_PER_MEMBER },
} as const;

export const slotState = { type: EventType.RTCSlot, state_key: RTC_SLOT_ID, content: SLOT_CONTENT };

/* The event levels a new room has anyway, restated because a homeserver
 * may take `events` in an override as the whole map; and a membership any
 * member may send. */
export const CALL_POWER_LEVELS = {
  events: {
    [EventType.RoomName]: 50,
    [EventType.RoomAvatar]: 50,
    [EventType.RoomCanonicalAlias]: 50,
    [EventType.RoomTopic]: 50,
    [EventType.RoomPowerLevels]: 100,
    [EventType.RoomHistoryVisibility]: 100,
    [EventType.RoomEncryption]: 100,
    [EventType.RoomServerAcl]: 100,
    [EventType.RoomTombstone]: 100,
    [EventType.RTCSlot]: 100,
    [EventType.GroupCallMemberPrefix]: 0,
  },
};

/** The plain notice a new call also puts in the room, for Matrix apps that
 *  do not know these calls (calls.tsx, `announceStart`). LosOS Chat draws
 *  the call from its MatrixRTC events and leaves this one out. */
export const isStartNotice = (content: Record<string, unknown>): boolean =>
  content["msgtype"] === "m.notice" && typeof content[RTC_APPLICATION] === "object" && content[RTC_APPLICATION] !== null;
