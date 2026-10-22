import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Call02Icon, CallEnd01Icon, Mic01Icon, MicOff01Icon, Video01Icon, VideoOffIcon } from "@hugeicons/core-free-icons";
import {
  CallEvent,
  CallFeedEvent,
  createNewMatrixCall,
  type GroupCall,
  GroupCallEvent,
  GroupCallIntent,
  GroupCallType,
  KnownMembership,
  type MatrixCall,
  type MatrixClient,
  type Room,
} from "matrix-js-sdk";
import { CallErrorCode, CallState } from "matrix-js-sdk/lib/webrtc/call";
import type { CallFeed } from "matrix-js-sdk/lib/webrtc/callFeed";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { Avatar } from "./avatar";
import { roomName } from "./model";

/* Voice and video calls. Two people in a room is a 1:1 call (m.call.*,
 * peer to peer over WebRTC); more is a group call, the SDK's full-mesh one,
 * which needs nothing from the homeserver beyond room state and to-device
 * messages. On the box's own network the browsers reach each other
 * directly; calls across the internet need a TURN server, which a box does
 * not run. */

export type Active =
  | { kind: "one"; call: MatrixCall; roomId: string }
  | { kind: "group"; call: GroupCall; roomId: string };

interface CallsState {
  active: Active | null;
  ringing: MatrixCall | null;
  start: (room: Room, video: boolean) => void;
}

const CallsContext = React.createContext<CallsState | null>(null);

export function useCalls(): CallsState {
  const found = React.useContext(CallsContext);
  if (found === null) throw new Error("useCalls outside CallsProvider");
  return found;
}

const joined = (room: Room): number => room.getMembers().filter((m) => m.membership === KnownMembership.Join).length;

export function CallsProvider({ client, children }: { client: MatrixClient; children: React.ReactNode }) {
  const t = useT();
  const [active, setActive] = React.useState<Active | null>(null);
  const [ringing, setRinging] = React.useState<MatrixCall | null>(null);

  React.useEffect(() => {
    const incoming = (call: MatrixCall): void => {
      setRinging(call);
      call.once(CallEvent.Hangup, () => setRinging((current) => (current === call ? null : current)));
    };
    // CallEventHandlerEvent.Incoming; the enum is not exported by name.
    client.on("Call.incoming" as never, incoming as never);
    return () => {
      client.off("Call.incoming" as never, incoming as never);
    };
  }, [client]);

  const endOne = React.useCallback((call: MatrixCall) => {
    call.once(CallEvent.Hangup, () => setActive((current) => (current?.call === call ? null : current)));
  }, []);

  const start = React.useCallback(
    (room: Room, video: boolean) => {
      (async () => {
        if (joined(room) <= 2) {
          const call = createNewMatrixCall(client, room.roomId);
          if (call === null) throw new Error(t("chat.call.unsupported"));
          endOne(call);
          setActive({ kind: "one", call, roomId: room.roomId });
          await (video ? call.placeVideoCall() : call.placeVoiceCall());
        } else {
          await client.waitUntilRoomReadyForGroupCalls(room.roomId);
          const group =
            client.getGroupCallForRoom(room.roomId) ??
            (await client.createGroupCall(room.roomId, video ? GroupCallType.Video : GroupCallType.Voice, false, GroupCallIntent.Room));
          await group.enter();
          setActive({ kind: "group", call: group, roomId: room.roomId });
        }
      })().catch((error: unknown) => {
        setActive(null);
        toast.error(t("chat.call.failed"), error instanceof Error ? error.message : String(error), { help: "chat" });
      });
    },
    [client, endOne, t],
  );

  const answer = (): void => {
    if (ringing === null) return;
    const call = ringing;
    setRinging(null);
    endOne(call);
    setActive({ kind: "one", call, roomId: call.roomId });
    call.answer().catch((error: unknown) => toast.error(t("chat.call.failed"), String(error), { help: "chat" }));
  };

  return (
    <CallsContext.Provider value={{ active, ringing, start }}>
      {children}
      {ringing !== null && active === null && (
        <Ringing
          client={client}
          call={ringing}
          onAnswer={answer}
          onDecline={() => {
            ringing.reject();
            setRinging(null);
          }}
        />
      )}
      {active !== null && <CallView client={client} active={active} onEnd={() => setActive(null)} />}
    </CallsContext.Provider>
  );
}

function Ringing({ client, call, onAnswer, onDecline }: { client: MatrixClient; call: MatrixCall; onAnswer: () => void; onDecline: () => void }) {
  const t = useT();
  const who = call.getOpponentMember();
  const name = who?.name ?? t("chat.call.someone");
  return (
    <div role="alertdialog" aria-label={t("chat.call.incoming", { name })} className="fixed top-4 right-4 z-50 flex w-80 items-center gap-3 rounded-card border border-line bg-surface p-3 shadow-pop">
      <Avatar client={client} id={who?.userId ?? call.roomId} name={name} mxc={who?.getMxcAvatarUrl()} size={40} />
      <div className="min-w-0 flex-1">
        <p className="truncate text-[13.5px] font-semibold">{name}</p>
        <p className="text-[12.5px] text-muted">{call.type === "video" ? t("chat.call.incomingVideo") : t("chat.call.incomingVoice")}</p>
      </div>
      <Button size="icon-sm" variant="destructive" aria-label={t("chat.call.decline")} onClick={onDecline}>
        <HugeiconsIcon icon={CallEnd01Icon} className="size-4" />
      </Button>
      <Button size="icon-sm" aria-label={t("chat.call.answer")} onClick={onAnswer}>
        <HugeiconsIcon icon={Call02Icon} className="size-4" />
      </Button>
    </div>
  );
}

function FeedTile({ client, roomId, feed }: { client: MatrixClient; roomId: string; feed: CallFeed }) {
  const ref = React.useRef<HTMLVideoElement>(null);
  const [, setTick] = React.useState(0);
  React.useEffect(() => {
    const el = ref.current;
    const attach = (): void => {
      if (el !== null && el.srcObject !== feed.stream) el.srcObject = feed.stream;
      setTick((n) => n + 1);
    };
    attach();
    feed.on(CallFeedEvent.NewStream, attach);
    feed.on(CallFeedEvent.MuteStateChanged, attach);
    return () => {
      feed.off(CallFeedEvent.NewStream, attach);
      feed.off(CallFeedEvent.MuteStateChanged, attach);
    };
  }, [feed]);
  const member = client.getRoom(roomId)?.getMember(feed.userId);
  const name = member?.name ?? feed.userId;
  const videoOff = feed.isVideoMuted();
  return (
    <div className="relative flex aspect-video items-center justify-center overflow-hidden rounded-card bg-ink">
      <video ref={ref} autoPlay playsInline muted={feed.isLocal()} className={cn("size-full object-cover", videoOff && "hidden", feed.isLocal() && "-scale-x-100")} />
      {videoOff && <Avatar client={client} id={feed.userId} name={name} mxc={member?.getMxcAvatarUrl()} size={64} />}
      <span className="absolute bottom-2 left-2 rounded bg-ink/70 px-1.5 py-0.5 text-[12px] text-surface">
        {name}
        {feed.isAudioMuted() ? " · 🔇" : ""}
      </span>
    </div>
  );
}

function useFeeds(active: Active): CallFeed[] {
  const [feeds, setFeeds] = React.useState<CallFeed[]>([]);
  React.useEffect(() => {
    if (active.kind === "one") {
      const read = (): void => setFeeds([...active.call.getFeeds()].filter((feed) => feed.purpose === "m.usermedia" || feed.purpose === "m.screenshare"));
      read();
      active.call.on(CallEvent.FeedsChanged, read);
      return () => {
        active.call.off(CallEvent.FeedsChanged, read);
      };
    }
    const group = active.call;
    const read = (): void => setFeeds([...group.userMediaFeeds]);
    read();
    group.on(GroupCallEvent.UserMediaFeedsChanged, read);
    return () => {
      group.off(GroupCallEvent.UserMediaFeedsChanged, read);
    };
  }, [active]);
  return feeds;
}

function CallView({ client, active, onEnd }: { client: MatrixClient; active: Active; onEnd: () => void }) {
  const t = useT();
  const feeds = useFeeds(active);
  const [muted, setMuted] = React.useState(false);
  const [videoOff, setVideoOff] = React.useState(false);
  const [state, setState] = React.useState<string>(active.kind === "one" ? active.call.state : "connected");

  React.useEffect(() => {
    if (active.kind !== "one") return;
    const read = (next: CallState): void => setState(next);
    active.call.on(CallEvent.State, read);
    return () => {
      active.call.off(CallEvent.State, read);
    };
  }, [active]);

  const hangUp = (): void => {
    if (active.kind === "one") active.call.hangup(CallErrorCode.UserHangup, false);
    else active.call.leave();
    onEnd();
  };
  const toggleMic = (): void => {
    void active.call.setMicrophoneMuted(!muted).then(() => setMuted(!muted));
  };
  const toggleVideo = (): void => {
    void active.call.setLocalVideoMuted(!videoOff).then(() => setVideoOff(!videoOff));
  };
  const room = client.getRoom(active.roomId);
  const waiting = active.kind === "one" && state !== CallState.Connected;

  return (
    <div role="dialog" aria-label={t("chat.call.label", { room: room ? roomName(room) : "" })} className="fixed inset-0 z-40 flex flex-col bg-ground/95 p-4">
      <p className="pb-3 text-center text-[14px] font-semibold">
        {room ? roomName(room) : null}
        {waiting && <span className="font-normal text-muted"> · {state === CallState.Ringing || state === CallState.InviteSent ? t("chat.call.ringing") : t("chat.call.connecting")}</span>}
      </p>
      <div className={cn("grid min-h-0 flex-1 content-center gap-3", feeds.length <= 1 ? "grid-cols-1" : feeds.length <= 4 ? "grid-cols-1 sm:grid-cols-2" : "grid-cols-2 lg:grid-cols-3")}>
        {feeds.map((feed) => (
          <FeedTile key={`${feed.userId}:${feed.stream.id}`} client={client} roomId={active.roomId} feed={feed} />
        ))}
      </div>
      <div className="flex justify-center gap-3 pt-4">
        <Button size="icon" variant="secondary" aria-pressed={muted} aria-label={muted ? t("chat.call.unmute") : t("chat.call.mute")} onClick={toggleMic}>
          <HugeiconsIcon icon={muted ? MicOff01Icon : Mic01Icon} className="size-5" />
        </Button>
        <Button size="icon" variant="secondary" aria-pressed={videoOff} aria-label={videoOff ? t("chat.call.cameraOn") : t("chat.call.cameraOff")} onClick={toggleVideo}>
          <HugeiconsIcon icon={videoOff ? VideoOffIcon : Video01Icon} className="size-5" />
        </Button>
        <Button size="icon" variant="destructive" aria-label={t("chat.call.hangUp")} onClick={hangUp}>
          <HugeiconsIcon icon={CallEnd01Icon} className="size-5" />
        </Button>
      </div>
    </div>
  );
}
