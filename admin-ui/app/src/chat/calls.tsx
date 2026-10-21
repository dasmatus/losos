import * as React from "react";
import { ClientEvent, type MatrixClient, type MatrixEvent, type Room, RoomEvent, KnownMembership } from "matrix-js-sdk";
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

/* Before chat's session is up there is no provider yet (calls-shell.tsx):
 * no calls, and no buttons for them. */
const NO_CALLS: CallsState = { available: false, active: null, ringing: null, start: () => {}, join: () => {} };

export function useCalls(): CallsState {
  return React.useContext(CallsContext) ?? NO_CALLS;
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

function announce(client: MatrixClient, roomId: string, call: string, video: boolean, state: "started" | "ended"): Promise<unknown> {
  // The SDK types a custom event's content as never; this one is ours.
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
    (roomId: string, video: boolean, make: () => Promise<string>) => {
      if (activeCall.current !== null) return;
      (async () => {
        if (!canEncrypt()) throw new CallError("calls.noEncryption");
        const local = await openMedia(video);
        try {
          const call = await make();
          setRinging((current) => (current?.call === call ? null : current));
          setActive({ roomId, call, video, local });
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
      enter(room.roomId, video, async () => {
        const call = await createCall(room, video);
        await announce(client, room.roomId, call, video, "started");
        return call;
      }),
    [client, enter],
  );

  const join = React.useCallback((roomId: string, call: string, video: boolean) => enter(roomId, video, async () => call), [enter]);

  const end = React.useCallback(
    (last: boolean) => {
      if (active === null) return;
      active.local.stop();
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

const EMPTY_FIELDS: PodcastFields = { rtmp: "", title: "", subtitle: "", overlay: null };

function CallScreen({
  client,
  active,
  onEnd,
  onFail,
}: {
  client: MatrixClient;
  active: Active & { local: LocalMedia };
  onEnd: (last: boolean) => void;
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
  const [micOff, setMicOff] = React.useState(false);
  const [cameraOff, setCameraOff] = React.useState(!media.cameraOn);
  const [background, setBackground] = React.useState<Background>(media.background);
  const [ownScreen, setOwnScreen] = React.useState<MediaStream | null>(null);
  const [podcastOpen, setPodcastOpen] = React.useState(false);
  const [fields, setFields] = React.useState<PodcastFields>(EMPTY_FIELDS);
  const [streaming, setStreaming] = React.useState<"off" | "starting" | "live">("off");
  const [podcast, setPodcast] = React.useState<Podcast | null>(null);
  const [invite, setInvite] = React.useState<{ link: string | null } | null>(null);
  const session = React.useRef<CallSession | null>(null);
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

  const warnBackground = React.useCallback(() => {
    if (media.backgroundFailed) toast.error(t("calls.failed"), t("calls.backgroundFailed"), { help: "calls" });
  }, [media, t]);

  React.useEffect(() => {
    const stop = (error: unknown): void => {
      if (finished.current) return;
      finished.current = true;
      callbacks.current.onFail(error);
      callbacks.current.onEnd(false);
    };
    warnBackground();
    try {
      const call = new CallSession({
        socket: socketUrl("/calls/api/ws"),
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
    } catch (error) {
      stop(error);
    }
    return () => {
      if (!finished.current) session.current?.leave();
      session.current = null;
    };
  }, [client, active.call, active.roomId, media, warnBackground]);

  React.useEffect(() => () => podcast?.dispose(), [podcast]);

  const leave = (): void => {
    if (finished.current) return;
    finished.current = true;
    podcast?.dispose();
    session.current?.leave();
    onEnd(others.current === 0);
  };
  const leaveRef = React.useRef(leave);
  leaveRef.current = leave;
  React.useEffect(() => {
    if (deserted) leaveRef.current();
  }, [deserted]);

  const toggleMic = (): void => {
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
      .catch(() => onFail(new CallError("calls.noDevices")));
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
