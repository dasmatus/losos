import * as React from "react";
import { ClientEvent, EventType, type MatrixClient, type MatrixEvent, type Room, RoomEvent, RoomStickyEventsEvent, KnownMembership } from "matrix-js-sdk";
import {
  getCallNotificationExpiry,
  MatrixRTCSession,
  MatrixRTCSessionEvent,
  parseCallNotificationContent,
} from "matrix-js-sdk/lib/matrixrtc";
import type { ReceivedToDeviceMessage } from "matrix-js-sdk/lib/sync-accumulator";
import type { ToDevicePayload } from "matrix-js-sdk/lib/models/ToDeviceMessage";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { HugeiconsIcon } from "@hugeicons/react";
import { LiveStreaming02Icon, PodcastIcon, UserAdd01Icon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { CallSession, canEncrypt, type KeyChannel, type KeyMessage, type Member } from "@/calls/session";
import type { WorkerStats } from "@/calls/e2ee-protocol";
import { LocalMedia, type Background } from "@/calls/local";
import { Podcast, type PodcastSource } from "@/calls/podcast";
import { PodcastPanel, type PodcastFields } from "@/calls/podcast-panel";
import { InvitePanel } from "@/calls/invite-panel";
import { newSecret } from "@/calls/guests";
import { CallControl, CallView, RingingCard, type Screen, type Tile } from "@/calls/view";
import { Avatar } from "./avatar";
import { roomName } from "./model";
import {
  CALL_ID,
  callLive,
  callOfMembership,
  mayCall,
  mayOpenSlot,
  RTC_APPLICATION,
  RTC_SLOT,
  RTC_SLOT_ID,
  rtcMembers,
  SFU_TRANSPORT,
  sfuUrl,
  SLOT_CONTENT,
  slotOpen,
  type SfuTransport,
} from "./rtc";

/* Voice and video calls, started from a room or a DM.
 *
 * Who is in a call, the ringing and the declines are MatrixRTC (MSC4143,
 * MSC4075, MSC4310; the identifiers are in rtc.ts). The media goes through
 * the box's own SFU (losos-calls, backend/src/calls/), which forwards frames
 * it cannot read: every frame is encrypted in the sender's browser
 * (src/calls/e2ee.ts) with a key that only the people in the call hold.
 * The keys travel as MatrixRTC's per-member keys, Olm-encrypted to-device
 * messages, so the box carries them without being able to open them either.
 * When the last person leaves, the room gets one `org.losos.call` event,
 * which the timeline draws as the end of the call. */

export const CALL_EVENT = "org.losos.call";
/* MSC4143's `m.rtc.encryption_key` under its unstable name. */
const KEY_EVENT = EventType.CallEncryptionKeysPrefix;
/* How long a ring lasts, as MSC4075 recommends; it caps any at two minutes. */
const RING_FOR_MS = 90_000;

export interface Active {
  roomId: string;
  call: string;
  video: boolean;
}

export interface Ringing {
  roomId: string;
  call: string;
  video: boolean;
  sender: string;
  /** The notification, which a decline refers to. */
  eventId: string;
  /** When the notification's lifetime runs out. */
  until: number;
}

interface CallsState {
  /* The box runs calls: it shares its storage in a mesh it joined and has
   * losos.calls.enable. Otherwise /calls/api/ does not exist, and nothing
   * here draws a button. */
  available: boolean;
  active: Active | null;
  ringing: Ringing | null;
  start: (room: Room, video: boolean) => void;
  join: (roomId: string, call: string, video: boolean) => void;
}

const CallsContext = React.createContext<CallsState | null>(null);

/* Before chat's session is up there is no provider yet (calls-shell.tsx):
 * no calls, and no buttons for them. */
const NO_CALLS: CallsState = { available: false, active: null, ringing: null, start: () => {}, join: () => {} };

export function useCalls(): CallsState {
  return React.useContext(CallsContext) ?? NO_CALLS;
}

/** What a timeline event says about a call: a MatrixRTC notification
 *  starts one (its call is the one the sender's membership names, `""` when
 *  that membership is not loaded), and an `org.losos.call` event ends one,
 *  or in older history starts one. */
export function callEvent(room: Room, ev: MatrixEvent): { call: string; video: boolean; state: "started" | "ended" } | null {
  const content = ev.getContent();
  if (ev.getType() === EventType.RTCNotification) {
    if (content["slot_id"] !== RTC_SLOT_ID) return null;
    const relation = content["m.relates_to"] as { event_id?: unknown } | undefined;
    const said = typeof relation?.event_id === "string" ? callOfMembership(room, relation.event_id) : null;
    return { call: said?.call ?? "", video: content["m.call.intent"] === "video", state: "started" };
  }
  if (ev.getType() !== CALL_EVENT) return null;
  const call = content["call_id"];
  const state = content["state"];
  if (typeof call !== "string" || !CALL_ID.test(call) || (state !== "started" && state !== "ended")) return null;
  return { call, video: content["video"] === true, state };
}

/** Whether `call` is over: nobody is in it, or the room was told it ended. */
export function callEnded(room: Room, call: string): boolean {
  if (call === "" || !callLive(room, call)) return true;
  return room
    .getLiveTimeline()
    .getEvents()
    .some((ev) => {
      if (ev.getType() !== CALL_EVENT) return false;
      const said = callEvent(room, ev);
      return said?.call === call && said.state === "ended";
    });
}

/* Sticky events reach the room's store after the timeline has been told
 * about them, and an encrypted one only once it is decrypted. A
 * notification waits a little for the store to catch up: for itself, which
 * MSC4075 checks is its sender's current one, and for the membership it
 * refers to. */
function whenSticky<T>(room: Room, found: () => T | null, waitMs = 10_000): Promise<T | null> {
  const now = found();
  if (now !== null) return Promise.resolve(now);
  return new Promise((resolve) => {
    const done = (said: T | null): void => {
      clearTimeout(timer);
      room.off(RoomStickyEventsEvent.Update, check);
      resolve(said);
    };
    const check = (): void => {
      const said = found();
      if (said !== null) done(said);
    };
    const timer = setTimeout(() => done(null), waitMs);
    room.on(RoomStickyEventsEvent.Update, check);
  });
}

const joinedUsers = (room: Room): string[] =>
  room
    .getMembers()
    .filter((m) => m.membership === KnownMembership.Join)
    .map((m) => m.userId);

/** Whether this person can have a call in `room`. Call keys go to the
 *  members' devices over Olm, and the crypto keeps device lists only for
 *  the members of encrypted rooms: in any other room a key would go to
 *  nobody. Then the slot has to be open, or the person able to open it as
 *  an admin of an older room. */
export function callable(room: Room): boolean {
  if (!room.hasEncryptionStateEvent()) return false;
  const me = room.myUserId;
  return slotOpen(room) ? mayCall(room, me) : mayOpenSlot(room, me);
}

/* An older room has no slot. An admin who starts a call there opens it,
 * as a room made now has from the start. */
async function enableCalls(client: MatrixClient, room: Room): Promise<void> {
  const me = client.getSafeUserId();
  if (!slotOpen(room)) {
    if (!mayOpenSlot(room, me)) throw new CallError("calls.notAllowed");
    await client.sendStateEvent(room.roomId, EventType.RTCSlot, SLOT_CONTENT as never, RTC_SLOT_ID);
  }
  if (!mayCall(room, me)) throw new CallError("calls.notAllowed");
}

/* The sender keys, as MatrixRTC's `m.per_member` keys (MSC4143): an
 * Olm-encrypted to-device message per device. A key goes only to a device
 * the SFU put in the call whose person is in the room; a key is taken only
 * when Olm says which
 * device sent it and the message names that same member. What the SFU
 * needs on top (the call, the sender's place in it, and the guests' invite
 * secrets) rides under `org.losos.call`. */
interface WireKey {
  room_id: string;
  member_id: string;
  media_key: { index: number; key: string };
  /* The same again, as matrix-js-sdk sends and reads it. */
  keys: { index: number; key: string };
  member: { id: string; claimed_device_id: string };
  session: { application: string; call_id: string; scope: "m.room" };
  sent_ts: number;
  [RTC_APPLICATION]: { call: string; pid: number; guests?: string[] };
}

/* How long a key waits for a device that signed in moments ago to reach
 * the device list, which follows a sync behind, and for its membership. */
const DEVICE_WAIT_MS = 20_000;

const memberId = (user: string, device: string): string => `${user}:${device}`;
const unpadded = (b64: string): string => b64.replace(/=+$/, "");

function olmKeys(client: MatrixClient, roomId: string): KeyChannel {
  const me = client.getSafeUserId();
  const myDevice = client.getDeviceId() ?? "";
  return {
    async send(to, msg) {
      const crypto = client.getCrypto();
      if (crypto === undefined) return;
      const room = client.getRoom(roomId);
      if (room === null) return;
      const inRoom = new Set(joinedUsers(room));
      const devices = to.filter((d) => inRoom.has(d.user)).map((d) => ({ userId: d.user, deviceId: d.device }));
      if (devices.length === 0) return;
      const users = [...new Set(devices.map((d) => d.userId))];
      // Someone who has just signed in on a new device joins the call
      // before that device is in the list; give it a few syncs. The SFU's
      // list decides who is in the call: a membership can reach this page
      // later than the person does.
      for (const until = Date.now() + DEVICE_WAIT_MS; Date.now() < until; ) {
        const known = await crypto.getUserDeviceInfo(users);
        if (devices.every((d) => known.get(d.userId)?.has(d.deviceId) === true)) break;
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
      const key = { index: msg.index, key: unpadded(msg.key) };
      const content: WireKey = {
        room_id: roomId,
        member_id: memberId(me, myDevice),
        media_key: key,
        keys: key,
        member: { id: memberId(me, myDevice), claimed_device_id: myDevice },
        session: { application: RTC_APPLICATION, call_id: RTC_SLOT.id, scope: "m.room" },
        sent_ts: Date.now(),
        [RTC_APPLICATION]: { call: msg.call, pid: msg.pid, ...(msg.guests !== undefined ? { guests: msg.guests } : {}) },
      };
      await client.encryptAndSendToDevice(KEY_EVENT, devices, content as unknown as ToDevicePayload);
    },
    subscribe(handler) {
      const on = ({ message, encryptionInfo }: ReceivedToDeviceMessage): void => {
        // A key that came in the clear is dropped, as MSC4143 asks.
        if (message.type !== KEY_EVENT || encryptionInfo === null) return;
        const device = encryptionInfo.senderDevice;
        if (device === undefined || encryptionInfo.sender !== message.sender) return;
        const content = message.content as Partial<WireKey>;
        const ours = content[RTC_APPLICATION];
        const key = content.media_key;
        if (content.room_id !== roomId || typeof ours !== "object" || ours === null || typeof key !== "object" || key === null) return;
        // The member the message names must be the device Olm says sent it.
        if (content.member_id !== memberId(encryptionInfo.sender, device)) return;
        if (typeof key.key !== "string" || !Number.isInteger(key.index) || key.index < 0 || key.index > 255) return;
        handler(
          { user: encryptionInfo.sender, device },
          {
            call: ours.call,
            pid: ours.pid,
            index: key.index,
            key: key.key.padEnd(Math.ceil(key.key.length / 4) * 4, "="),
            ...(Array.isArray(ours.guests) ? { guests: ours.guests } : {}),
          } as KeyMessage,
        );
      };
      client.on(ClientEvent.ReceivedToDeviceMessage, on);
      return () => {
        client.off(ClientEvent.ReceivedToDeviceMessage, on);
      };
    },
  };
}

class CallError extends Error {}

async function openMedia(video: boolean): Promise<LocalMedia> {
  try {
    return await LocalMedia.open(video);
  } catch {
    throw new CallError("calls.noDevices");
  }
}

const socketUrl = (path: string): string => `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}${path}`;

/* The room's call, from the SFU: a new one, or the one already going on
 * there when someone else started first. */
async function createCall(room: Room, video: boolean): Promise<{ call: string; created: boolean }> {
  const response = await fetch("/calls/api/calls", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({
      room: room.roomId,
      invited: joinedUsers(room),
      video,
    }),
  });
  const body = (await response.json().catch(() => ({}))) as {
    call?: string;
    created?: boolean;
    error?: string;
  };
  if (!response.ok || typeof body.call !== "string") throw new CallError(body.error ?? `HTTP ${response.status}`);
  return { call: body.call, created: body.created === true };
}

/** The link in the start notice: this room in LosOS Chat, joining its call. */
export const joinLink = (roomId: string, video: boolean): string =>
  `${location.origin}/chat/?join=${encodeURIComponent(roomId)}&call=${video ? "video" : "voice"}`;

/* Other Matrix apps do not know these calls, so a new one also gets a
 * plain notice they all show, with a link that opens it here, signed in as
 * whoever follows it. LosOS Chat draws the MatrixRTC notification instead
 * and leaves this out (`org.losos.call` in its content). */
function announceStart(client: MatrixClient, room: Room, call: string, video: boolean, body: string): Promise<unknown> {
  return client.sendEvent(room.roomId, EventType.RoomMessage, {
    msgtype: "m.notice",
    body,
    [CALL_EVENT]: { call_id: call, video },
  } as never);
}

/* A device leaving a call and joining again sends its membership twice
 * under one state key; the join waits for the leave, so the leave cannot
 * land after it and take the device out. */
let leaving: Promise<unknown> = Promise.resolve();

/* The end of a call, for the timeline: MatrixRTC has no event for it. */
function announceEnd(client: MatrixClient, roomId: string, call: string, video: boolean): Promise<unknown> {
  return client.sendEvent(roomId, CALL_EVENT as never, { call_id: call, video, state: "ended" } as never);
}

export function CallsProvider({ client, children }: { client: MatrixClient; children: React.ReactNode }) {
  const t = useT();
  const [active, setActive] = React.useState<(Active & { local: LocalMedia }) | null>(null);
  const [ringing, setRinging] = React.useState<Ringing | null>(null);
  const activeCall = React.useRef<string | null>(null);
  activeCall.current = active?.call ?? null;
  const me = client.getSafeUserId();
  const [available, setAvailable] = React.useState(false);

  React.useEffect(() => {
    let live = true;
    fetch("/calls/api/health", { credentials: "same-origin" })
      // Without calls the path falls through to the admin page's own
      // index.html, which answers 200, so only the SFU's "ok" counts.
      .then(async (response) => {
        const ok = response.ok && (await response.text()) === "ok";
        if (live) setAvailable(ok && canEncrypt());
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);

  // A notification rings when MSC4075 says it is meant for this person:
  // not their own, within its lifetime, mentioning them or the room by
  // someone allowed to, and not yet answered or declined. It has to be for
  // this slot and for a call that is still going on.
  React.useEffect(() => {
    if (!available) return;
    const onTimeline = (
      ev: MatrixEvent,
      room: Room | undefined,
      toStart: boolean | undefined,
      _removed: boolean,
      data: { liveEvent?: boolean },
    ): void => {
      if (toStart === true || data.liveEvent !== true || room === undefined) return;
      void client
        .decryptEventIfNeeded(ev)
        .then(async () => {
          if (ev.getType() === EventType.RTCDecline) {
            // Declined on another of this person's devices.
            const declined = (ev.getContent()["m.relates_to"] as { event_id?: unknown } | undefined)?.event_id;
            if (ev.getSender() === me) setRinging((current) => (current?.eventId === declined ? null : current));
            return;
          }
          if (ev.getType() !== EventType.RTCNotification || ev.getSender() === me) return;
          const key = ev.getContent()["msc4354_sticky_key"];
          if (ev.unstableStickyInfo !== undefined && typeof key === "string") {
            await whenSticky(room, () => (room._unstable_getKeyedStickyEvent(ev.getSender() ?? "", ev.getType(), key)?.getId() === ev.getId() ? true : null));
          }
          const content = await parseCallNotificationContent(ev, { room, userId: me });
          const eventId = ev.getId();
          if (content.slot_id !== RTC_SLOT_ID || callEvent(room, ev) === null || eventId === undefined) throw new Error("not a call in this room's slot");
          const said = await whenSticky(room, () => {
            const said = callEvent(room, ev);
            return said !== null && !callEnded(room, said.call) ? { call: said.call, video: said.video } : null;
          });
          if (said === null || activeCall.current === said.call) return;
          setRinging({
            roomId: room.roomId,
            call: said.call,
            video: said.video,
            sender: ev.getSender() ?? "",
            eventId,
            until: getCallNotificationExpiry(content, ev.getTs()),
          });
        })
        .catch((error: unknown) => {
          // Why a call notification from someone else does not ring here.
          if (ev.getType() === EventType.RTCNotification) console.warn(`calls: ${ev.getId() ?? ""} does not ring:`, error instanceof Error ? error.message : error);
        });
    };
    client.on(RoomEvent.Timeline, onTimeline);
    return () => {
      client.off(RoomEvent.Timeline, onTimeline);
    };
  }, [available, client, me]);

  // It stops when its lifetime runs out, when the call is over, or when
  // this person joined it from another device.
  React.useEffect(() => {
    if (ringing === null) return;
    const stop = (): void => setRinging((current) => (current === ringing ? null : current));
    const room = client.getRoom(ringing.roomId);
    if (room === null) return;
    const check = (): void => {
      if (callEnded(room, ringing.call) || rtcMembers(room).some((m) => m.user === me)) stop();
    };
    room.on(RoomStickyEventsEvent.Update, check);
    const timer = setTimeout(stop, Math.max(0, ringing.until - Date.now()));
    return () => {
      room.off(RoomStickyEventsEvent.Update, check);
      clearTimeout(timer);
    };
  }, [client, me, ringing]);

  const failed = React.useCallback(
    (error: unknown) => {
      const key = error instanceof CallError ? error.message : "";
      const detail =
        key === "calls.noDevices" || key === "calls.noEncryption" || key === "calls.notAllowed"
          ? t(key)
          : key === "noSuchCall"
            ? t("calls.ended")
            : error instanceof Error
              ? error.message
              : String(error);
      toast.error(t("calls.failed"), detail, { help: "calls" });
    },
    [t],
  );

  const enter = React.useCallback(
    (room: Room, video: boolean, pick: () => Promise<string>) => {
      if (activeCall.current !== null) return;
      (async () => {
        if (!canEncrypt()) throw new CallError("calls.noEncryption");
        const local = await openMedia(video);
        try {
          await enableCalls(client, room);
          const call = await pick();
          setRinging((current) => (current?.roomId === room.roomId ? null : current));
          setActive({ roomId: room.roomId, call, video, local });
        } catch (error) {
          local.stop();
          throw error;
        }
      })().catch(failed);
    },
    [client, failed],
  );

  // The SFU hands back the call the room already has, if it has one: a
  // slot holds one call at a time.
  const start = React.useCallback(
    (room: Room, video: boolean) =>
      enter(room, video, async () => {
        const { call, created } = await createCall(room, video);
        if (created) {
          const who = room.getMember(me)?.name ?? me;
          void announceStart(client, room, call, video, t("calls.startNotice", { who, link: joinLink(room.roomId, video) })).catch(
            () => undefined,
          );
        }
        return call;
      }),
    [client, enter, me, t],
  );

  const join = React.useCallback(
    (roomId: string, call: string, video: boolean) => {
      const room = client.getRoom(roomId);
      if (room !== null) enter(room, video, async () => call);
    },
    [client, enter],
  );

  const end = React.useCallback(
    (last: boolean | Promise<boolean>) => {
      if (active === null) return;
      active.local.stop();
      void Promise.resolve(last)
        .then((was) => (was ? announceEnd(client, active.roomId, active.call, active.video) : undefined))
        .catch(() => undefined);
      setActive(null);
    },
    [active, client],
  );

  const decline = React.useCallback(() => {
    if (ringing === null) return;
    void client.sendRtcDecline(ringing.roomId, ringing.eventId).catch(() => undefined);
    setRinging(null);
  }, [client, ringing]);

  const ringingRoom = ringing === null ? null : client.getRoom(ringing.roomId);
  const caller = ringing === null ? null : ringingRoom?.getMember(ringing.sender);
  const callerName = caller?.name ?? ringing?.sender ?? "";

  return (
    <CallsContext.Provider value={{ available, active, ringing, start, join }}>
      {children}
      {ringing !== null && active === null && (
        <RingingCard
          caller={callerName}
          avatar={<Avatar client={client} id={ringing.sender} name={callerName} mxc={caller?.getMxcAvatarUrl()} size={40} />}
          {...(ringingRoom !== null && ringingRoom.getJoinedMemberCount() > 2 ? { room: ringingRoom.name } : {})}
          video={ringing.video}
          onAccept={(video) => join(ringing.roomId, ringing.call, video)}
          onDecline={decline}
        />
      )}
      {active !== null && <CallScreen key={active.call} client={client} active={active} onEnd={end} onFail={failed} />}
    </CallsContext.Provider>
  );
}

const EMPTY_FIELDS: PodcastFields = { rtmp: "", title: "", subtitle: "", overlay: null };

function CallScreen({
  client,
  active,
  onEnd,
  onFail,
}: {
  client: MatrixClient;
  active: Active & { local: LocalMedia };
  /* `last`: whether this page was the last of the room's people in the
   * call, and so tells the room it ended. */
  onEnd: (last: boolean | Promise<boolean>) => void;
  onFail: (error: unknown) => void;
}) {
  const t = useT();
  const media = active.local;
  const room = client.getRoom(active.roomId);
  const [state, setState] = React.useState<"connecting" | "connected">("connecting");
  const [members, setMembers] = React.useState<{ list: Member[]; you: number; live: number | null }>({
    list: [],
    you: -1,
    live: null,
  });
  const [streams, setStreams] = React.useState<Map<number, MediaStream>>(new Map());
  const [screens, setScreens] = React.useState<Map<number, MediaStream>>(new Map());
  const [stats, setStats] = React.useState<WorkerStats | null>(null);
  const [micOff, setMicOff] = React.useState(!media.micOn);
  const [cameraOff, setCameraOff] = React.useState(!media.cameraOn);
  const [background, setBackground] = React.useState<Background>(media.background);
  const [ownScreen, setOwnScreen] = React.useState<MediaStream | null>(null);
  const [podcastOpen, setPodcastOpen] = React.useState(false);
  const [fields, setFields] = React.useState<PodcastFields>(EMPTY_FIELDS);
  const [streaming, setStreaming] = React.useState<"off" | "starting" | "live">("off");
  const [podcast, setPodcast] = React.useState<Podcast | null>(null);
  const [invite, setInvite] = React.useState<{ link: string | null } | null>(null);
  const session = React.useRef<CallSession | null>(null);
  const rtcSession = React.useRef<MatrixRTCSession | null>(null);
  /* The ring this device sent, if it was the first in the call. */
  const notification = React.useRef<string | null>(null);
  const others = React.useRef(0);
  const [deserted, setDeserted] = React.useState(false);
  const finished = React.useRef(false);
  const callbacks = React.useRef({ onEnd, onFail });
  callbacks.current = { onEnd, onFail };

  const nameOf = React.useCallback((user: string) => room?.getMember(user)?.name ?? user, [room]);
  const memberName = (m: Member): string => (m.guest !== undefined ? t("calls.guestNamed", { name: m.guest }) : nameOf(m.user));

  // What podcast mode draws, read on every frame it draws.
  const drawn = React.useRef<{ people: PodcastSource[]; screen: { name: string; stream: MediaStream } | null; fields: PodcastFields }>({
    people: [],
    screen: null,
    fields: EMPTY_FIELDS,
  });

  // A call that opened without a device says so once, and goes on.
  React.useEffect(() => {
    if (media.missing.mic) toast.error(t("calls.noMicTitle"), t("calls.noMic"), { help: "calls" });
    else if (media.missing.camera) toast.error(t("calls.noCameraTitle"), t("calls.noCamera"), { help: "calls" });
  }, [media, t]);

  const warnBackground = React.useCallback(() => {
    if (media.backgroundFailed) toast.error(t("calls.failed"), t("calls.backgroundFailed"), { help: "calls" });
  }, [media, t]);

  React.useEffect(() => {
    const stop = (error: unknown): void => {
      if (finished.current) return;
      finished.current = true;
      void session.current?.leave();
      const rtc = rtcSession.current;
      rtcSession.current = null;
      if (rtc !== null) leaving = rtc.leave(5000).finally(() => rtc.stop());
      callbacks.current.onFail(error);
      callbacks.current.onEnd(false);
    };
    warnBackground();
    try {
      const call = new CallSession({
        socket: sfuUrl(),
        call: active.call,
        device: client.getDeviceId() ?? "",
        local: media.stream,
        keys: olmKeys(client, active.roomId),
        events: {
          state: (next, reason) => {
            if (next === "connected") setState("connected");
            if (next === "failed") stop(new CallError(reason ?? "failed"));
          },
          members: (list, you, liveBy) => {
            const before = others.current;
            others.current = list.filter((m) => m.pid !== you).length;
            // Once everyone else has hung up the call is over, as on a
            // phone; whoever is left does not sit in it alone.
            if (before > 0 && others.current === 0) setDeserted(true);
            setMembers({ list, you, live: liveBy });
            const keep = (current: Map<number, MediaStream>): Map<number, MediaStream> =>
              new Map([...current].filter(([pid]) => list.some((m) => m.pid === pid)));
            setStreams(keep);
            setScreens(keep);
          },
          track: (pid, stream, screen) => (screen ? setScreens : setStreams)((current) => new Map(current).set(pid, stream)),
          stats: setStats,
        },
      });
      session.current = call;
      call.start();

      if (room === null) throw new CallError("calls.ended");
      const deviceId = client.getDeviceId();
      if (deviceId === null) throw new CallError("calls.noEncryption");
      // This device's membership in the room's slot, published with the
      // SFU and the call it is in. It rings the room when it is the first.
      const transport: SfuTransport & Record<string, unknown> = { type: SFU_TRANSPORT, url: sfuUrl(), call: active.call };
      const rtc = MatrixRTCSession.sessionForSlot(client, room, RTC_SLOT, {
        listenForStickyEvents: true,
        listenForMemberStateEvents: false,
      });
      rtcSession.current = rtc;
      rtc.on(MatrixRTCSessionEvent.MembershipManagerError, (error: unknown) => {
        stop(new CallError(`MatrixRTC membership failed: ${String(error)}`));
      });
      rtc.on(MatrixRTCSessionEvent.DidSendCallNotification, (sent: { event_id?: string }) => {
        notification.current = sent.event_id ?? null;
      });
      // Join once the session knows who is already in the call: a session
      // that still counts nobody takes this device for the first and rings
      // the room a second time.
      void leaving.then(() => rtc.initialMembershipCalculated).then(() => {
        if (rtcSession.current !== rtc) return;
        const me = client.getSafeUserId();
        rtc.join({ userId: me, deviceId, memberId: memberId(me, deviceId) }, [transport], {
          // A sticky membership (MSC4354) and a delayed leave (MSC4140): the
          // homeserver withdraws it half a minute after this page stops answering.
          unstableSendStickyEvents: true,
          // A small box's homeserver can take seconds to answer, and a
          // restart given up after the default 2 s let the leave go out
          // under a page still in the call, which then joined again in a
          // loop. The leave waits 30 s for a page gone quiet instead of 8.
          delayedLeaveEventDelayMs: 30_000,
          delayedLeaveEventRestartMs: 5_000,
          delayedLeaveEventRestartLocalTimeoutMs: 10_000,
          callIntent: active.video ? "video" : "audio",
          callCapabilities: ["m.render_audio", "m.render_video"],
          // A direct chat rings; a room gets a notification, as Element does.
          notificationType: room.getJoinedMemberCount() <= 2 ? "ring" : "notification",
          notificationLifetimeMs: RING_FOR_MS,
          // The keys are this page's own (olmKeys), in MatrixRTC's format.
          manageMediaKeys: false,
        });
      });
    } catch (error) {
      stop(error);
    }
    return () => {
      if (!finished.current) void session.current?.leave();
      session.current = null;
      const rtc = rtcSession.current;
      rtcSession.current = null;
      if (rtc !== null) leaving = rtc.leave(5000).finally(() => rtc.stop());
    };
  }, [client, active.call, active.roomId, active.video, media, warnBackground, room]);

  // In a direct chat, the other person declining ends the call that rang
  // them (MSC4310).
  React.useEffect(() => {
    if (room === null) return;
    const onTimeline = (ev: MatrixEvent, at: Room | undefined): void => {
      if (at?.roomId !== room.roomId || ev.getSender() === client.getSafeUserId()) return;
      void client
        .decryptEventIfNeeded(ev)
        .then(() => {
          if (ev.getType() !== EventType.RTCDecline) return;
          const declined = (ev.getContent()["m.relates_to"] as { event_id?: unknown } | undefined)?.event_id;
          if (declined === undefined || declined !== notification.current) return;
          if (room.getJoinedMemberCount() > 2 || others.current > 0) return;
          toast.done(t("calls.declined", { who: room.getMember(ev.getSender() ?? "")?.name ?? ev.getSender() ?? "" }));
          leaveRef.current();
        })
        .catch(() => undefined);
    };
    client.on(RoomEvent.Timeline, onTimeline);
    return () => {
      client.off(RoomEvent.Timeline, onTimeline);
    };
  }, [client, room, t]);

  React.useEffect(() => () => podcast?.dispose(), [podcast]);

  const leave = (): void => {
    if (finished.current) return;
    finished.current = true;
    podcast?.dispose();
    // The SFU says who hung up last: two people who leave at once each
    // still see the other in their own list.
    const guess = others.current === 0;
    onEnd(session.current?.leave().then((said) => said ?? guess) ?? guess);
  };
  const leaveRef = React.useRef(leave);
  leaveRef.current = leave;
  React.useEffect(() => {
    if (deserted) leaveRef.current();
  }, [deserted]);

  // A call that started without a microphone asks for it on the first
  // press; after that the button mutes and unmutes.
  const toggleMic = (): void => {
    if (!media.micOn) {
      media
        .openMic()
        .then(async (track) => {
          await session.current?.setAudioTrack(track);
          setMicOff(false);
        })
        .catch(() => toast.error(t("calls.noMicTitle"), t("calls.noMic"), { help: "calls" }));
      return;
    }
    media.setMuted(!micOff);
    setMicOff(!micOff);
  };

  // Off stops the camera, so its light goes out, and sends nothing; on
  // asks for it again, through the chosen background.
  const toggleCamera = (): void => {
    media
      .setCamera(cameraOff)
      .then(async () => {
        await session.current?.setVideoTrack(media.picture);
        setCameraOff(!media.cameraOn);
        warnBackground();
      })
      .catch(() => toast.error(t("calls.noCameraTitle"), t("calls.cameraFailed"), { help: "calls" }));
  };

  const toggleBackground = (): void => {
    const next: Background = background === "black" ? "real" : "black";
    void media.setBackground(next).then(async () => {
      await session.current?.setVideoTrack(media.picture);
      setBackground(next);
      warnBackground();
    });
  };

  const stopSharing = React.useCallback((): void => {
    media.stopSharing();
    void session.current?.setScreenTrack(null);
    setOwnScreen(null);
  }, [media]);

  const toggleShare = (): void => {
    if (ownScreen !== null) {
      stopSharing();
      return;
    }
    media
      .share(stopSharing)
      .then(async (track) => {
        await session.current?.setScreenTrack(track);
        setOwnScreen(new MediaStream([track]));
      })
      // Closing the browser's picker is no failure.
      .catch(() => undefined);
  };

  const openPodcast = (): void => {
    if (podcastOpen) {
      setPodcastOpen(false);
      return;
    }
    if (podcast === null) {
      setPodcast(
        new Podcast(
          () => drawn.current.people,
          () => ({ ...drawn.current.fields, screen: drawn.current.screen }),
          {
            live: () => setStreaming("live"),
            ended: (error) => {
              setStreaming("off");
              if (error !== undefined) toast.error(t("calls.liveFailed"), error, { help: "calls" });
            },
          },
        ),
      );
    }
    setPodcastOpen(true);
  };

  const goLive = (): void => {
    const call = session.current;
    if (podcast === null || call === null) return;
    setStreaming("starting");
    try {
      podcast.start(socketUrl("/calls/api/stream"), { call: active.call, pid: call.pid, rtmp: fields.rtmp });
    } catch (error) {
      setStreaming("off");
      onFail(error);
    }
  };

  /* A link from the edge with the call's guest secret after `#`. The
   * secret is made here, once per call, and goes to the others over Olm
   * before the link is shown. */
  const makeInvite = (): void => {
    const call = session.current;
    if (call === null) return;
    setPodcastOpen(false);
    setInvite({ link: null });
    (async () => {
      const response = await fetch("/calls/api/invites", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "same-origin",
        body: JSON.stringify({ call: active.call, pid: call.pid }),
      });
      const body = (await response.json().catch(() => ({}))) as { url?: string; error?: string };
      if (!response.ok || typeof body.url !== "string") throw new CallError(body.error ?? `HTTP ${response.status}`);
      const secret = call.guestSecret ?? newSecret();
      await call.addGuestSecret(secret);
      setInvite({ link: `${body.url}#${secret}` });
    })()
      .catch((error: unknown) => {
        setInvite(null);
        const code = error instanceof Error ? error.message : "";
        const detail =
          code === "noEdge" ? t("calls.inviteNoEdge") : code === "tooManyInvites" ? t("calls.inviteTooMany") : code;
        toast.error(t("calls.inviteFailed"), detail, { help: "calls" });
      });
  };

  const tiles: Tile[] = members.list.map((m) => {
    const self = m.pid === members.you;
    const person = m.guest === undefined ? room?.getMember(m.user) : undefined;
    const name = memberName(m);
    const stream = self ? media.stream : streams.get(m.pid);
    return {
      pid: m.pid,
      name,
      self,
      dark: self && cameraOff,
      avatar: <Avatar client={client} id={m.user} name={name} mxc={person?.getMxcAvatarUrl()} size={64} />,
      ...(stream !== undefined ? { stream } : {}),
    };
  });

  const shown: Screen[] = members.list.flatMap((m): Screen[] => {
    const self = m.pid === members.you;
    const stream = self ? ownScreen : m.screen === true ? (screens.get(m.pid) ?? null) : null;
    return stream === null ? [] : [{ pid: m.pid, name: memberName(m), stream, self }];
  });

  drawn.current = {
    people: tiles.map((tile) => ({ pid: tile.pid, name: tile.self ? nameOf(client.getSafeUserId()) : tile.name, ...(tile.stream ? { stream: tile.stream } : {}) })),
    // The podcast shows a screen the way the call does: the one coming in.
    screen: (() => {
      const s = shown.find((x) => (x.self ? ownScreen !== null : !x.stream.getVideoTracks()[0]?.muted));
      return s === undefined ? null : { name: s.name, stream: s.stream };
    })(),
    fields,
  };

  const liveBy = members.list.find((m) => m.pid === members.live);

  return (
    <CallView
      title={room ? roomName(room) : ""}
      tiles={tiles}
      screens={shown}
      status={state === "connecting" ? "connecting" : others.current === 0 ? "waiting" : "live"}
      micOff={micOff}
      cameraOff={cameraOff}
      sharing={ownScreen !== null}
      background={background}
      stats={stats}
      onMic={toggleMic}
      onCamera={toggleCamera}
      onShare={toggleShare}
      onBackground={toggleBackground}
      onLeave={leave}
      badges={
        liveBy !== undefined ? (
          <Badge variant="crit" data-live={liveBy.pid} title={t("calls.liveBy", { name: nameOf(liveBy.user) })}>
            <HugeiconsIcon icon={LiveStreaming02Icon} size={14} />
            {t("calls.live")}
          </Badge>
        ) : null
      }
      actions={
        <>
          <CallControl
            label={t("calls.invite")}
            pressed={invite !== null}
            icon={UserAdd01Icon}
            onClick={() => (invite !== null ? setInvite(null) : makeInvite())}
          />
          <CallControl
            label={t("calls.podcast")}
            pressed={podcastOpen}
            icon={PodcastIcon}
            onClick={() => {
              setInvite(null);
              openPodcast();
            }}
          />
        </>
      }
    >
      {invite !== null ? <InvitePanel link={invite.link} onAnother={makeInvite} onClose={() => setInvite(null)} /> : null}
      {podcastOpen && podcast !== null ? (
        <PodcastPanel
          podcast={podcast}
          fields={fields}
          live={streaming === "live"}
          starting={streaming === "starting"}
          onChange={setFields}
          onGoLive={goLive}
          onStop={() => podcast.stop()}
          onClose={() => setPodcastOpen(false)}
        />
      ) : null}
    </CallView>
  );
}
