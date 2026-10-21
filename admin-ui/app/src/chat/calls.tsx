import * as React from "react";
import { ClientEvent, type MatrixClient, type MatrixEvent, type Room, RoomEvent, KnownMembership } from "matrix-js-sdk";
import type { ReceivedToDeviceMessage } from "matrix-js-sdk/lib/sync-accumulator";
import type { ToDevicePayload } from "matrix-js-sdk/lib/models/ToDeviceMessage";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { CallSession, canEncrypt, type KeyChannel, type KeyMessage, type Member, type OpenIdToken } from "@/calls/session";
import type { WorkerStats } from "@/calls/e2ee-protocol";
import { CallView, RingingCard, type Tile } from "@/calls/view";
import { Avatar } from "./avatar";

/* Voice and video calls, started from a room or a DM.
 *
 * The media goes through the box's own SFU (losos-calls, backend/src/calls/),
 * which forwards frames it cannot read: every frame is encrypted in the
 * sender's browser (src/calls/e2ee.ts) with a key that only the people in
 * the call hold. Those keys travel between devices as Olm-encrypted
 * to-device messages, so the box carries them without being able to open
 * them either. The room gets one `org.losos.call` event when a call starts
 * and one when it ends, which is what rings the others and what the
 * timeline draws with a Join button. */

export const CALL_EVENT = "org.losos.call";
const KEY_EVENT = "org.losos.call.key";
/* A start older than this no longer rings. */
const RING_FOR_MS = 60_000;
const CALL_ID = /^[0-9a-f]{32}$/;

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

export function useCalls(): CallsState {
  const found = React.useContext(CallsContext);
  if (found === null) throw new Error("useCalls outside CallsProvider");
  return found;
}

/** What an `org.losos.call` event says, or null when it is not one. */
export function callEvent(ev: MatrixEvent): { call: string; video: boolean; state: "started" | "ended" } | null {
  if (ev.getType() !== CALL_EVENT) return null;
  const content = ev.getContent();
  const call = content["call_id"];
  const state = content["state"];
  if (typeof call !== "string" || !CALL_ID.test(call) || (state !== "started" && state !== "ended")) return null;
  return { call, video: content["video"] === true, state };
}

/** Whether the room's live timeline already holds the end of `call`. */
export function callEnded(room: Room, call: string): boolean {
  return room
    .getLiveTimeline()
    .getEvents()
    .some((ev) => {
      const said = callEvent(ev);
      return said?.call === call && said.state === "ended";
    });
}

const joinedUsers = (room: Room): string[] =>
  room
    .getMembers()
    .filter((m) => m.membership === KnownMembership.Join)
    .map((m) => m.userId);

/* The sender keys, as Olm to-device messages. A key goes only to a device
 * the SFU put in the call AND whose person is in the room, and a key is
 * taken only when Olm says which device sent it. */
function olmKeys(client: MatrixClient, roomId: string): KeyChannel {
  return {
    async send(to, msg) {
      const room = client.getRoom(roomId);
      const inRoom = new Set(room === null ? [] : joinedUsers(room));
      const devices = to.filter((d) => inRoom.has(d.user)).map((d) => ({ userId: d.user, deviceId: d.device }));
      if (devices.length === 0) return;
      // Fetches device lists the client does not track yet (someone it
      // shares no encrypted room with).
      await client.getCrypto()?.getUserDeviceInfo([...new Set(devices.map((d) => d.userId))], true);
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

async function openMedia(video: boolean): Promise<MediaStream> {
  try {
    return await navigator.mediaDevices.getUserMedia({ audio: true, video });
  } catch {
    throw new CallError("calls.noDevices");
  }
}

async function openId(client: MatrixClient): Promise<OpenIdToken> {
  const token = await client.getOpenIdToken();
  return {
    access_token: token.access_token,
    matrix_server_name: token.matrix_server_name,
  };
}

async function createCall(client: MatrixClient, room: Room, video: boolean): Promise<string> {
  const response = await fetch("/calls/api/calls", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({
      openid: await openId(client),
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

function announce(client: MatrixClient, roomId: string, call: string, video: boolean, state: "started" | "ended"): Promise<unknown> {
  // The SDK types a custom event's content as never; this one is ours.
  return client.sendEvent(roomId, CALL_EVENT as never, { call_id: call, video, state } as never);
}

export function CallsProvider({ client, children }: { client: MatrixClient; children: React.ReactNode }) {
  const t = useT();
  const [active, setActive] = React.useState<(Active & { local: MediaStream }) | null>(null);
  const [ringing, setRinging] = React.useState<Ringing | null>(null);
  const activeCall = React.useRef<string | null>(null);
  activeCall.current = active?.call ?? null;
  const me = client.getSafeUserId();
  const [available, setAvailable] = React.useState(false);

  React.useEffect(() => {
    let live = true;
    fetch("/calls/api/health", { credentials: "same-origin" })
      .then((response) => {
        if (live) setAvailable(response.ok && canEncrypt());
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
        if (said.state === "ended") {
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
      });
    };
    client.on(RoomEvent.Timeline, onTimeline);
    return () => {
      client.off(RoomEvent.Timeline, onTimeline);
    };
  }, [available, client, me]);

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
    (roomId: string, video: boolean, make: (local: MediaStream) => Promise<string>) => {
      if (activeCall.current !== null) return;
      (async () => {
        if (!canEncrypt()) throw new CallError("calls.noEncryption");
        const local = await openMedia(video);
        try {
          const call = await make(local);
          setRinging((current) => (current?.call === call ? null : current));
          setActive({ roomId, call, video, local });
        } catch (error) {
          for (const track of local.getTracks()) track.stop();
          throw error;
        }
      })().catch(failed);
    },
    [failed],
  );

  const start = React.useCallback(
    (room: Room, video: boolean) =>
      enter(room.roomId, video, async () => {
        const call = await createCall(client, room, video);
        await announce(client, room.roomId, call, video, "started");
        return call;
      }),
    [client, enter],
  );

  const join = React.useCallback((roomId: string, call: string, video: boolean) => enter(roomId, video, async () => call), [enter]);

  const end = React.useCallback(
    (last: boolean) => {
      if (active === null) return;
      for (const track of active.local.getTracks()) track.stop();
      if (last) void announce(client, active.roomId, active.call, active.video, "ended").catch(() => undefined);
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

function CallScreen({
  client,
  active,
  onEnd,
  onFail,
}: {
  client: MatrixClient;
  active: Active & { local: MediaStream };
  onEnd: (last: boolean) => void;
  onFail: (error: unknown) => void;
}) {
  const room = client.getRoom(active.roomId);
  const [state, setState] = React.useState<"connecting" | "connected">("connecting");
  const [members, setMembers] = React.useState<{ list: Member[]; you: number }>({ list: [], you: -1 });
  const [streams, setStreams] = React.useState<Map<number, MediaStream>>(new Map());
  const [stats, setStats] = React.useState<WorkerStats | null>(null);
  const [micOff, setMicOff] = React.useState(false);
  const [cameraOff, setCameraOff] = React.useState(!active.video);
  const session = React.useRef<CallSession | null>(null);
  const others = React.useRef(0);
  const finished = React.useRef(false);
  const callbacks = React.useRef({ onEnd, onFail });
  callbacks.current = { onEnd, onFail };

  React.useEffect(() => {
    let live = true;
    const stop = (error: unknown): void => {
      if (finished.current) return;
      finished.current = true;
      callbacks.current.onFail(error);
      callbacks.current.onEnd(false);
    };
    // The token is fetched as the call opens rather than kept: it is good
    // for a short while only, and a page left ringing can outlast it.
    openId(client)
      .then((openid) => {
        if (!live) return;
        const call = new CallSession({
          socket: `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/calls/api/ws`,
          call: active.call,
          openid,
          device: client.getDeviceId() ?? "",
          local: active.local,
          keys: olmKeys(client, active.roomId),
          events: {
            state: (next, reason) => {
              if (next === "connected") setState("connected");
              if (next === "failed") stop(new CallError(reason ?? "failed"));
            },
            members: (list, you) => {
              others.current = list.filter((m) => m.pid !== you).length;
              setMembers({ list, you });
              setStreams((current) => new Map([...current].filter(([pid]) => list.some((m) => m.pid === pid))));
            },
            track: (pid, stream) => setStreams((current) => new Map(current).set(pid, stream)),
            stats: setStats,
          },
        });
        session.current = call;
        call.start();
      })
      .catch(stop);
    return () => {
      live = false;
      if (!finished.current) session.current?.leave();
      session.current = null;
    };
  }, [client, active.call, active.local, active.roomId]);

  const leave = (): void => {
    finished.current = true;
    session.current?.leave();
    onEnd(others.current === 0);
  };

  const toggleMic = (): void => {
    session.current?.setMuted("audio", !micOff);
    setMicOff(!micOff);
  };

  // Off stops the camera, so its light goes out, and sends nothing; on
  // asks for it again.
  const toggleCamera = (): void => {
    const local = active.local;
    if (!cameraOff) {
      for (const track of local.getVideoTracks()) {
        track.stop();
        local.removeTrack(track);
      }
      void session.current?.setVideoTrack(null);
      setCameraOff(true);
      return;
    }
    navigator.mediaDevices
      .getUserMedia({ video: true })
      .then(async (camera) => {
        const [track] = camera.getVideoTracks();
        if (track === undefined) return;
        local.addTrack(track);
        await session.current?.setVideoTrack(track);
        setCameraOff(false);
      })
      .catch(() => onFail(new CallError("calls.noDevices")));
  };

  const tiles: Tile[] = members.list.map((m) => {
    const self = m.pid === members.you;
    const person = room?.getMember(m.user);
    const name = person?.name ?? m.user;
    const stream = self ? active.local : streams.get(m.pid);
    return {
      pid: m.pid,
      name,
      self,
      dark: self && cameraOff,
      avatar: <Avatar client={client} id={m.user} name={name} mxc={person?.getMxcAvatarUrl()} size={64} />,
      ...(stream !== undefined ? { stream } : {}),
    };
  });

  return (
    <CallView
      title={room?.name ?? ""}
      tiles={tiles}
      status={state === "connecting" ? "connecting" : others.current === 0 ? "waiting" : "live"}
      micOff={micOff}
      cameraOff={cameraOff}
      stats={stats}
      onMic={toggleMic}
      onCamera={toggleCamera}
      onLeave={leave}
    />
  );
}
