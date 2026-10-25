import * as React from "react";
import { ClientEvent, EventType, type MatrixClient, type MatrixEvent, type Room, RoomEvent, RoomStateEvent, KnownMembership } from "matrix-js-sdk";
import { MatrixRTCSession, MatrixRTCSessionEvent } from "matrix-js-sdk/lib/matrixrtc";
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

/* Voice and video calls, started from a room or a DM.
 *
 * MatrixRTC owns per-device room membership, expiry and incoming-call
 * notifications. Its transport descriptor points at LosOS's own SFU; this
 * deliberately does not use LiveKit. losos-calls still carries the media,
 * and the browser still encrypts every frame with the sender keys exchanged
 * between devices over Olm. The legacy org.losos.call event remains only as
 * an end marker and for old timeline history. */

export const CALL_EVENT = "org.losos.call";
const KEY_EVENT = "org.losos.call.key";
const SFU_TRANSPORT_TYPE = "org.losos.sfu";
const SFU_TRANSPORT_KEY = "org.losos.sfu";
/* A start older than this no longer rings. */
const RING_FOR_MS = 60_000;
const MEMBERSHIP_EXPIRY_MS = 5 * 60_000;
const CALL_ID = /^[0-9a-f]{32}$/;

export interface Active {
  roomId: string;
  call: string;
  video: boolean;
  /** The MatrixRTC membership advertises the SFU that owns this call. */
  socket: string;
  /** Only the initiating device asks MatrixRTC to ring the room. */
  notify: boolean;
}

export interface Ringing {
  roomId: string;
  call: string;
  video: boolean;
  sender: string;
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

/** Parse MatrixRTC call notifications and legacy LosOS call timeline events. */
export function callEvent(ev: MatrixEvent): { call: string; video: boolean; state: "started" | "ended" } | null {
  const content = ev.getContent();
  if (ev.getType() === EventType.RTCNotification) {
    const slot = content["slot_id"];
    const prefix = "m.call#";
    if (typeof slot !== "string" || !slot.startsWith(prefix)) return null;
    const call = slot.slice(prefix.length);
    if (!CALL_ID.test(call)) return null;
    return { call, video: content["m.call.intent"] === "video", state: "started" };
  }
  if (ev.getType() !== CALL_EVENT) return null;
  const call = content["call_id"];
  const state = content["state"];
  if (typeof call !== "string" || !CALL_ID.test(call) || (state !== "started" && state !== "ended")) return null;
  return { call, video: content["video"] === true, state };
}

/** Return the active LosOS SFU endpoint announced in MatrixRTC membership. */
export function callTransportUrl(room: Room, call: string): string | null {
  for (const ev of room.currentState.getStateEvents(EventType.GroupCallMemberPrefix)) {
    const content = ev.getContent();
    if (content["application"] !== "m.call" || content["call_id"] !== call) continue;
    const raw = content[SFU_TRANSPORT_KEY];
    if (typeof raw !== "object" || raw === null || Array.isArray(raw)) continue;
    const url = (raw as Record<string, unknown>)["url"];
    if (typeof url !== "string") continue;
    let endpoint: URL;
    try {
      endpoint = new URL(url);
    } catch {
      continue;
    }
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    // Call signaling stays on the same box/edge origin, through its normal
    // authenticated endpoint. Never let a room member redirect a join to an
    // arbitrary WebSocket host or path.
    if (
      endpoint.protocol !== protocol ||
      endpoint.host !== location.host ||
      endpoint.pathname !== "/calls/api/ws" ||
      endpoint.username !== "" ||
      endpoint.password !== "" ||
      endpoint.search !== "" ||
      endpoint.hash !== ""
    ) continue;
    const created = typeof content["created_ts"] === "number" ? content["created_ts"] : ev.getTs();
    const expires = typeof content["expires"] === "number" ? content["expires"] : 0;
    if (!Number.isFinite(created) || !Number.isFinite(expires) || expires <= 0 || Date.now() >= created + expires) continue;
    return url;
  }
  return null;
}

/** True only for LosOS calls, not unrelated MatrixRTC applications in the room. */
export function isLososCall(room: Room, call: string): boolean {
  if (callTransportUrl(room, call) !== null) return true;
  return room.getLiveTimeline().getEvents().some((ev) => {
    if (ev.getType() !== CALL_EVENT) return false;
    const said = callEvent(ev);
    return said?.call === call && said.state === "ended";
  });
}

/** Whether a call ended explicitly or its MatrixRTC membership has expired/left. */
export function callEnded(room: Room, call: string): boolean {
  const events = room.getLiveTimeline().getEvents();
  if (events.some((ev) => {
    if (ev.getType() !== CALL_EVENT) return false;
    const said = callEvent(ev);
    return said?.call === call && said.state === "ended";
  })) return true;

  const wasMatrixRTC = events.some((ev) => ev.getType() === EventType.RTCNotification && callEvent(ev)?.call === call);
  return wasMatrixRTC && callTransportUrl(room, call) === null;
}

const joinedUsers = (room: Room): string[] =>
  room
    .getMembers()
    .filter((m) => m.membership === KnownMembership.Join)
    .map((m) => m.userId);

/* The sender keys, as Olm to-device messages. A key goes only to a device
 * the SFU put in the call AND whose person is in the room, and a key is
 * taken only when Olm says which device sent it. */
/** Whether `room` can hold a call. Call keys go to the members' devices
 *  over Olm, and the crypto keeps device lists only for the members of
 *  encrypted rooms: in any other room a key would go to nobody. */
export function callable(room: Room): boolean {
  return room.hasEncryptionStateEvent();
}

/* How long a key waits for a device that signed in moments ago to reach
 * the device list, which follows a sync behind. */
const DEVICE_WAIT_MS = 20_000;

function olmKeys(client: MatrixClient, roomId: string): KeyChannel {
  return {
    async send(to, msg) {
      const room = client.getRoom(roomId);
      const inRoom = new Set(room === null ? [] : joinedUsers(room));
      const devices = to.filter((d) => inRoom.has(d.user)).map((d) => ({ userId: d.user, deviceId: d.device }));
      const crypto = client.getCrypto();
      if (devices.length === 0 || crypto === undefined) return;
      const users = [...new Set(devices.map((d) => d.userId))];
      // Someone who has just signed in on a new device joins the call
      // before that device is in the list; give the list a few syncs.
      for (const until = Date.now() + DEVICE_WAIT_MS; Date.now() < until; ) {
        const known = await crypto.getUserDeviceInfo(users);
        if (devices.every((d) => known.get(d.userId)?.has(d.deviceId) === true)) break;
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
      await client.encryptAndSendToDevice(KEY_EVENT, devices, msg as unknown as ToDevicePayload);
    },
    subscribe(handler) {
      const on = ({ message, encryptionInfo }: ReceivedToDeviceMessage): void => {
        if (message.type !== KEY_EVENT || encryptionInfo === null) return;
        const device = encryptionInfo.senderDevice;
        if (device === undefined || encryptionInfo.sender !== message.sender) return;
        handler({ user: encryptionInfo.sender, device }, message.content as unknown as KeyMessage);
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

async function createCall(room: Room, video: boolean): Promise<string> {
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
    error?: string;
  };
  if (!response.ok || typeof body.call !== "string") throw new CallError(body.error ?? `HTTP ${response.status}`);
  return body.call;
}

function announce(client: MatrixClient, roomId: string, call: string, video: boolean, state: "ended"): Promise<unknown> {
  // MatrixRTC carries membership and ringing. This small legacy event closes
  // the timeline row cleanly and remains readable by older LosOS clients.
  return client.sendEvent(roomId, CALL_EVENT as never, { call_id: call, video, state } as never);
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
      void client.decryptEventIfNeeded(ev).then(() => {
        const said = callEvent(ev);
        if (said === null) return;
        if (ev.getType() === EventType.RTCNotification && !isLososCall(room, said.call)) return;
        if (said.state === "ended" || callEnded(room, said.call)) {
          setRinging((current) => (current?.call === said.call ? null : current));
          return;
        }
        if (ev.getSender() === me || Date.now() - ev.getTs() > RING_FOR_MS || activeCall.current === said.call) return;
        setRinging({
          roomId: room.roomId,
          call: said.call,
          video: said.video,
          sender: ev.getSender() ?? "",
        });
      }).catch(() => undefined);
    };
    client.on(RoomEvent.Timeline, onTimeline);
    return () => {
      client.off(RoomEvent.Timeline, onTimeline);
    };
  }, [available, client, me]);

  // Stop ringing when the last LosOS MatrixRTC membership leaves or expires.
  React.useEffect(() => {
    if (ringing === null) return;
    const room = client.getRoom(ringing.roomId);
    if (room === null) return;
    const onRoomState = (ev: MatrixEvent): void => {
      if (ev.getType() !== EventType.GroupCallMemberPrefix) return;
      if (callEnded(room, ringing.call)) {
        setRinging((current) => (current?.call === ringing.call ? null : current));
      }
    };
    room.on(RoomStateEvent.Events, onRoomState);
    return () => {
      room.off(RoomStateEvent.Events, onRoomState);
    };
  }, [client, ringing]);

  // A call that was not answered stops ringing on its own.
  React.useEffect(() => {
    if (ringing === null) return;
    const timer = setTimeout(() => setRinging((current) => (current === ringing ? null : current)), RING_FOR_MS);
    return () => clearTimeout(timer);
  }, [ringing]);

  const failed = React.useCallback(
    (error: unknown) => {
      const key = error instanceof CallError ? error.message : "";
      const detail =
        key === "calls.noDevices" || key === "calls.noEncryption"
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
    (roomId: string, video: boolean, make: () => Promise<string>, notify: boolean, socket: string) => {
      if (activeCall.current !== null) return;
      (async () => {
        if (!canEncrypt()) throw new CallError("calls.noEncryption");
        const local = await openMedia(video);
        try {
          const call = await make();
          setRinging((current) => (current?.call === call ? null : current));
          setActive({ roomId, call, video, socket, notify, local });
        } catch (error) {
          local.stop();
          throw error;
        }
      })().catch(failed);
    },
    [failed],
  );

  const start = React.useCallback(
    (room: Room, video: boolean) =>
      enter(room.roomId, video, () => createCall(room, video), true, socketUrl("/calls/api/ws")),
    [enter],
  );

  const join = React.useCallback(
    (roomId: string, call: string, video: boolean) => {
      const room = client.getRoom(roomId);
      const socket = room === null ? socketUrl("/calls/api/ws") : callTransportUrl(room, call) ?? socketUrl("/calls/api/ws");
      enter(roomId, video, async () => call, false, socket);
    },
    [client, enter],
  );

  const end = React.useCallback(
    (last: boolean | Promise<boolean>) => {
      if (active === null) return;
      active.local.stop();
      void Promise.resolve(last)
        .then((was) => (was ? announce(client, active.roomId, active.call, active.video, "ended") : undefined))
        .catch(() => undefined);
      setActive(null);
    },
    [active, client],
  );

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
          onDecline={() => setRinging(null)}
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
      if (rtc !== null) void rtc.leave(5000).finally(() => void rtc.stop());
      callbacks.current.onFail(error);
      callbacks.current.onEnd(false);
    };
    warnBackground();
    try {
      const call = new CallSession({
        socket: active.socket,
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
      const userId = client.getUserId();
      const deviceId = client.getDeviceId();
      if (userId === null || deviceId === null) throw new CallError("calls.noEncryption");

      const transport = { type: SFU_TRANSPORT_TYPE, url: active.socket };
      const rtc = MatrixRTCSession.sessionForSlot(client, room, { application: "m.call", id: active.call });
      rtcSession.current = rtc;
      const membershipError = (error: unknown): void => {
        stop(new CallError(`MatrixRTC membership failed: ${String(error)}`));
      };
      rtc.on(MatrixRTCSessionEvent.MembershipManagerError, membershipError);
      rtc.join(
        { userId, deviceId, memberId: crypto.randomUUID() },
        [transport],
        {
          callIntent: active.video ? "video" : "audio",
          callCapabilities: active.video ? ["m.render_audio", "m.render_video"] : ["m.render_audio"],
          applicationData: { [SFU_TRANSPORT_KEY]: transport },
          notificationType: active.notify ? "ring" : undefined,
          notificationLifetimeMs: RING_FOR_MS,
          membershipEventExpiryMs: MEMBERSHIP_EXPIRY_MS,
          manageMediaKeys: false,
        },
      );
    } catch (error) {
      stop(error);
    }
    return () => {
      if (!finished.current) void session.current?.leave();
      session.current = null;
      const rtc = rtcSession.current;
      rtcSession.current = null;
      if (rtc !== null) void rtc.leave(5000).finally(() => void rtc.stop());
    };
  }, [client, active.call, active.roomId, active.socket, active.video, active.notify, media, warnBackground, room]);

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
