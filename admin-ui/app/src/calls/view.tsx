/* What a call looks like: the full-screen view with a tile per member, and
 * the card that rings. Plain components with props; provider.tsx owns the
 * call and passes them what to draw. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { CallEnd01Icon, CallIcon, CameraOff02Icon, CameraVideoIcon, LockKeyIcon, Mic01Icon, MicOff01Icon } from "@hugeicons/core-free-icons";
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

export interface CallViewProps {
  title: string;
  tiles: Tile[];
  status: "connecting" | "waiting" | "live";
  micOff: boolean;
  cameraOff: boolean;
  stats: WorkerStats | null;
  onMic(): void;
  onCamera(): void;
  onLeave(): void;
}

export function CallView(p: CallViewProps) {
  const t = useT();
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={p.title}
      data-call-view=""
      className="fixed inset-0 z-50 flex flex-col gap-3 bg-canvas p-3 sm:p-4"
    >
      <header className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <h2 className="truncate text-base font-medium text-ink">{p.title}</h2>
          <p className="text-[13px] text-muted">
            {p.status === "connecting" ? t("calls.connecting") : p.status === "waiting" ? t("calls.waiting") : t("calls.ongoing")}
          </p>
        </div>
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
      </header>

      <div className={cn("grid min-h-0 flex-1 auto-rows-fr gap-3", grid(p.tiles.length))}>
        {p.tiles.map((tile) => (
          <CallTile key={tile.pid} tile={tile} />
        ))}
      </div>

      <footer className="flex items-center justify-center gap-3 pb-1">
        <Button
          variant="secondary"
          size="lg"
          className="size-12 rounded-full p-0"
          aria-pressed={p.micOff}
          aria-label={p.micOff ? t("calls.unmute") : t("calls.mute")}
          title={p.micOff ? t("calls.unmute") : t("calls.mute")}
          onClick={p.onMic}
        >
          <HugeiconsIcon icon={p.micOff ? MicOff01Icon : Mic01Icon} size={20} />
        </Button>
        <Button
          variant="secondary"
          size="lg"
          className="size-12 rounded-full p-0"
          aria-pressed={p.cameraOff}
          aria-label={p.cameraOff ? t("calls.cameraOn") : t("calls.cameraOff")}
          title={p.cameraOff ? t("calls.cameraOn") : t("calls.cameraOff")}
          onClick={p.onCamera}
        >
          <HugeiconsIcon icon={p.cameraOff ? CameraOff02Icon : CameraVideoIcon} size={20} />
        </Button>
        <Button
          variant="destructive"
          size="lg"
          className="size-12 rounded-full p-0"
          aria-label={t("calls.hangUp")}
          title={t("calls.hangUp")}
          onClick={p.onLeave}
        >
          <HugeiconsIcon icon={CallEnd01Icon} size={20} />
        </Button>
      </footer>
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
      className="fixed right-4 bottom-4 z-50 w-[min(22rem,calc(100vw-2rem))] rounded-card border border-line bg-surface p-4 shadow-lg"
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
