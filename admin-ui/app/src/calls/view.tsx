/* What a call looks like: the full-screen view with a tile per member (or,
 * while someone shares a screen, the screen in the middle and the members
 * beside it), and the card that rings. Plain components with props; provider.tsx owns the
 * call and passes them what to draw. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  BackgroundIcon,
  CallEnd01Icon,
  CallIcon,
  CameraOff02Icon,
  CameraVideoIcon,
  ComputerRemoveIcon,
  ComputerScreenShareIcon,
  LockKeyIcon,
  Mic01Icon,
  MicOff01Icon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import type { WorkerStats } from "./e2ee-protocol";

export interface Tile {
  pid: number;
  name: string;
  /* Undefined until the first track arrives. */
  stream?: MediaStream;
  self?: boolean;
  /* This browser's own camera is off. A remote tile goes dark by itself
   * while no picture comes in. */
  dark?: boolean;
  /* Drawn while there is no picture; initials otherwise. */
  avatar?: React.ReactNode;
}

function Video({ stream, muted, mirrored }: { stream: MediaStream; muted: boolean; mirrored?: boolean }) {
  const ref = React.useRef<HTMLVideoElement>(null);
  React.useEffect(() => {
    if (ref.current && ref.current.srcObject !== stream) ref.current.srcObject = stream;
  }, [stream]);
  return <video ref={ref} autoPlay playsInline muted={muted} className={cn("size-full object-cover", mirrored && "-scale-x-100")} />;
}

function Initials({ name }: { name: string }) {
  const letters = name
    .replace(/^@/, "")
    .split(/[\s:._-]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase())
    .join("");
  return <div className="grid size-20 place-items-center rounded-full bg-accent-wash text-2xl font-medium text-accent">{letters || "?"}</div>;
}

/* Whether a received picture is coming in. A sender whose camera is off
 * sends nothing, and the track goes quiet. */
function useLive(stream: MediaStream | undefined): boolean {
  const [live, setLive] = React.useState(false);
  React.useEffect(() => {
    const track = stream?.getVideoTracks()[0];
    if (track === undefined) {
      setLive(false);
      return;
    }
    const read = (): void => setLive(!track.muted && track.readyState === "live");
    read();
    for (const name of ["mute", "unmute", "ended"]) track.addEventListener(name, read);
    return () => {
      for (const name of ["mute", "unmute", "ended"]) track.removeEventListener(name, read);
    };
  }, [stream]);
  return live;
}

function CallTile({ tile }: { tile: Tile }) {
  const t = useT();
  const live = useLive(tile.self ? undefined : tile.stream);
  const picture = tile.stream !== undefined && !tile.dark && (tile.self || live);
  return (
    <div data-pid={tile.pid} className="relative grid min-h-0 place-items-center overflow-hidden rounded-card border border-line bg-sunk">
      {picture && tile.stream ? (
        <Video stream={tile.stream} muted={!!tile.self} mirrored={tile.self} />
      ) : (
        (tile.avatar ?? <Initials name={tile.name} />)
      )}
      {/* Remote audio plays even while the picture is off. */}
      {tile.stream && !picture && !tile.self ? (
        <div className="hidden">
          <Video stream={tile.stream} muted={false} />
        </div>
      ) : null}
      <span className="absolute bottom-2 left-2 rounded-control bg-surface/85 px-2 py-0.5 text-[13px] text-ink">
        {tile.self ? t("calls.you") : tile.name}
      </span>
    </div>
  );
}

function grid(n: number): string {
  if (n <= 1) return "grid-cols-1";
  if (n === 2) return "grid-cols-1 sm:grid-cols-2";
  if (n <= 4) return "grid-cols-2";
  return "grid-cols-2 lg:grid-cols-3";
}

/* A member's screen, as the call carries it. */
export interface Screen {
  pid: number;
  name: string;
  stream: MediaStream;
  self?: boolean;
}

/* Tells its parent when a screen's picture starts and stops coming in. */
function LiveProbe({ screen, onChange }: { screen: Screen; onChange(pid: number, live: boolean): void }) {
  const live = useLive(screen.stream);
  React.useEffect(() => onChange(screen.pid, live), [screen.pid, live, onChange]);
  return null;
}

function Stage({ screen }: { screen: Screen }) {
  const t = useT();
  return (
    <div data-share={screen.pid} className="relative grid min-h-0 min-w-0 flex-1 place-items-center overflow-hidden rounded-card border border-line bg-sunk">
      <video
        ref={(el) => {
          if (el && el.srcObject !== screen.stream) el.srcObject = screen.stream;
        }}
        autoPlay
        playsInline
        muted
        className="size-full object-contain"
      />
      <span className="absolute bottom-2 left-2 rounded-control bg-surface/85 px-2 py-0.5 text-[13px] text-ink">
        {screen.self ? t("calls.yourScreen") : t("calls.theirScreen", { name: screen.name })}
      </span>
    </div>
  );
}

export interface CallViewProps {
  title: string;
  tiles: Tile[];
  /* Everyone's screen tracks; the one coming in is drawn in the middle. */
  screens: Screen[];
  status: "connecting" | "waiting" | "live";
  micOff: boolean;
  cameraOff: boolean;
  sharing: boolean;
  /* The background behind the person on this browser's camera. */
  background: "black" | "real";
  stats: WorkerStats | null;
  onMic(): void;
  onCamera(): void;
  onShare(): void;
  onBackground(): void;
  onLeave(): void;
  /* More controls, after the built-in ones. */
  actions?: React.ReactNode;
  /* Next to the encryption label, such as the live-stream badge. */
  badges?: React.ReactNode;
  /* Drawn over the call, such as a panel a control opened. */
  children?: React.ReactNode;
}

function Control({
  label,
  pressed,
  icon,
  onClick,
  destructive,
}: {
  label: string;
  pressed?: boolean;
  icon: React.ComponentProps<typeof HugeiconsIcon>["icon"];
  onClick(): void;
  destructive?: boolean;
}) {
  return (
    <Button
      variant={destructive ? "destructive" : "secondary"}
      size="lg"
      className="size-12 rounded-full p-0"
      {...(pressed !== undefined ? { "aria-pressed": pressed } : {})}
      aria-label={label}
      title={label}
      onClick={onClick}
    >
      <HugeiconsIcon icon={icon} size={20} />
    </Button>
  );
}

export { Control as CallControl };

export function CallView(p: CallViewProps) {
  const t = useT();
  const [live, setLive] = React.useState<ReadonlySet<number>>(new Set());
  const onLive = React.useCallback((pid: number, on: boolean) => {
    setLive((current) => {
      if (current.has(pid) === on) return current;
      const next = new Set(current);
      if (on) next.add(pid);
      else next.delete(pid);
      return next;
    });
  }, []);
  const share = p.screens.find((s) => (s.self ? p.sharing : live.has(s.pid))) ?? null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={p.title}
      data-call-view=""
      data-sharing={share?.pid ?? ""}
      className="fixed inset-0 z-50 flex flex-col gap-3 bg-ground p-3 sm:p-4"
    >
      {p.screens
        .filter((s) => !s.self)
        .map((s) => (
          <LiveProbe key={s.pid} screen={s} onChange={onLive} />
        ))}
      <header className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <h2 className="truncate text-base font-medium text-ink">{p.title}</h2>
          <p className="text-[13px] text-muted">
            {p.status === "connecting" ? t("calls.connecting") : p.status === "waiting" ? t("calls.waiting") : t("calls.ongoing")}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {p.badges}
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger
                render={
                  <span
                    data-e2ee=""
                    /* The worker's counts, read by the call checks. */
                    data-sealed={p.stats?.sealed ?? 0}
                    data-opened={p.stats?.opened ?? 0}
                    data-refused={p.stats?.refused ?? 0}
                    className="inline-flex items-center gap-1.5 rounded-control border border-line bg-surface px-2.5 py-1 text-[13px] text-ink"
                  />
                }
              >
                <HugeiconsIcon icon={LockKeyIcon} size={15} className="text-accent" />
                {t("calls.encrypted")}
              </TooltipTrigger>
              <TooltipContent className="max-w-xs">
                <p>{t("calls.encryptedDetail")}</p>
                {p.stats ? (
                  <p className="mt-1 text-muted">
                    {t("calls.frames", {
                      sealed: p.stats.sealed,
                      opened: p.stats.opened,
                    })}
                  </p>
                ) : null}
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      </header>

      {share === null ? (
        <div className={cn("grid min-h-0 flex-1 auto-rows-fr gap-3", grid(p.tiles.length))}>
          {p.tiles.map((tile) => (
            <CallTile key={tile.pid} tile={tile} />
          ))}
        </div>
      ) : (
        /* A screen in the middle, the people beside it: a column on a
         * wide window, a row under it on a narrow one. */
        <div className="flex min-h-0 flex-1 flex-col gap-3 lg:flex-row">
          <Stage screen={share} />
          <aside
            data-call-strip=""
            className="flex shrink-0 gap-3 overflow-auto max-lg:h-32 lg:w-60 lg:flex-col"
          >
            {p.tiles.map((tile) => (
              <div key={tile.pid} className="grid aspect-video shrink-0 max-lg:h-full lg:w-full">
                <CallTile tile={tile} />
              </div>
            ))}
          </aside>
        </div>
      )}

      <footer className="flex flex-wrap items-center justify-center gap-3 pb-1">
        <Control
          label={p.micOff ? t("calls.unmute") : t("calls.mute")}
          pressed={p.micOff}
          icon={p.micOff ? MicOff01Icon : Mic01Icon}
          onClick={p.onMic}
        />
        <Control
          label={p.cameraOff ? t("calls.cameraOn") : t("calls.cameraOff")}
          pressed={p.cameraOff}
          icon={p.cameraOff ? CameraOff02Icon : CameraVideoIcon}
          onClick={p.onCamera}
        />
        <Control
          label={p.background === "black" ? t("calls.backgroundShow") : t("calls.backgroundHide")}
          pressed={p.background === "black"}
          icon={BackgroundIcon}
          onClick={p.onBackground}
        />
        <Control
          label={p.sharing ? t("calls.shareStop") : t("calls.share")}
          pressed={p.sharing}
          icon={p.sharing ? ComputerRemoveIcon : ComputerScreenShareIcon}
          onClick={p.onShare}
        />
        {p.actions}
        <Control label={t("calls.hangUp")} icon={CallEnd01Icon} onClick={p.onLeave} destructive />
      </footer>
      {p.children}
    </div>
  );
}

export interface RingingProps {
  caller: string;
  avatar?: React.ReactNode;
  room?: string;
  video: boolean;
  onAccept(video: boolean): void;
  onDecline(): void;
}

export function RingingCard(p: RingingProps) {
  const t = useT();
  return (
    <div
      role="alertdialog"
      aria-live="assertive"
      data-ringing=""
      className="fixed right-4 bottom-4 z-50 w-[min(22rem,calc(100vw-2rem))] rounded-card border border-line bg-surface p-4 shadow-pop"
    >
      <div className="flex items-center gap-3">
        {p.avatar ?? <Initials name={p.caller} />}
        <div className="min-w-0">
          <p className="font-medium text-ink">
            {t(p.video ? "calls.incomingVideo" : "calls.incomingVoice", {
              name: p.caller,
            })}
          </p>
          {p.room ? <p className="truncate text-[13px] text-muted">{t("calls.inRoom", { room: p.room })}</p> : null}
          <p className="mt-1 inline-flex items-center gap-1 text-[12px] text-muted">
            <HugeiconsIcon icon={LockKeyIcon} size={13} />
            {t("calls.encrypted")}
          </p>
        </div>
      </div>
      <div className="mt-4 flex flex-wrap justify-end gap-2">
        <Button variant="ghost" size="sm" onClick={p.onDecline}>
          {t("calls.decline")}
        </Button>
        {p.video ? (
          <Button variant="secondary" size="sm" onClick={() => p.onAccept(false)}>
            <HugeiconsIcon icon={CallIcon} size={16} />
            {t("calls.acceptVoice")}
          </Button>
        ) : null}
        <Button size="sm" onClick={() => p.onAccept(p.video)}>
          <HugeiconsIcon icon={p.video ? CameraVideoIcon : CallIcon} size={16} />
          {t("calls.accept")}
        </Button>
      </div>
    </div>
  );
}
