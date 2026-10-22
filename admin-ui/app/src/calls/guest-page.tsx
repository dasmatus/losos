/* The page a guest link opens, at /calls/guest/?call=…&invite=…#secret.
 * It asks for a name, then joins that one call as a guest: no LosOS Login,
 * no chat, no Matrix. The call's keys reach this page only over guest pairs
 * (guests.ts), so the page needs the secret after `#`, which the browser
 * never sends to the box. */

import * as React from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { LanguagePicker } from "@/components/ui/language-picker";
import { Switch } from "@/components/ui/switch";
import { Toaster, toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import type { WorkerStats } from "./e2ee-protocol";
import { parseSecret } from "./guests";
import { LocalMedia, type Background } from "./local";
import { CallSession, canEncrypt, type Member } from "./session";
import { CallView, type Screen, type Tile } from "./view";

export interface GuestLink {
  call: string;
  invite: string;
  secret: string;
}

const HEX32 = /^[0-9a-f]{32}$/;

/* The call, the invite and the secret from this page's address, or null
 * when any is missing or malformed. */
export function readLink(href: string): GuestLink | null {
  const url = new URL(href);
  const call = url.searchParams.get("call") ?? "";
  const invite = url.searchParams.get("invite") ?? "";
  const secret = url.hash.replace(/^#/, "");
  if (!HEX32.test(call) || !HEX32.test(invite) || parseSecret(secret) === null) return null;
  return { call, invite, secret };
}

const socketUrl = (): string => `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/calls/guest/ws`;

/* Refusals from the box that mean this link will not open the call. */
const REFUSED = new Set(["notInvited", "inviteInUse", "noSuchCall", "badName", "full"]);

type Stage =
  | { at: "form" }
  | { at: "call"; name: string; media: LocalMedia }
  | { at: "over"; reason: "ended" | "refused" };

export function GuestPage({ link }: { link: GuestLink | null }) {
  const t = useT();
  const [stage, setStage] = React.useState<Stage>({ at: "form" });

  if (link === null || !canEncrypt()) {
    return (
      <Shell>
        <Alert variant="warn" data-guest-bad="">
          <AlertDescription>{link === null ? t("calls.guestBadLink") : t("calls.noEncryption")}</AlertDescription>
        </Alert>
      </Shell>
    );
  }
  if (stage.at === "call") {
    return (
      <>
        <GuestCall
          link={link}
          name={stage.name}
          media={stage.media}
          onOver={(reason) => {
            stage.media.stop();
            setStage({ at: "over", reason });
          }}
        />
        <Toaster />
      </>
    );
  }
  return (
    <Shell>
      {stage.at === "over" ? (
        <div className="flex flex-col gap-4" data-guest-over={stage.reason}>
          <p className="text-ink">{stage.reason === "ended" ? t("calls.guestEnded") : t("calls.guestRefused")}</p>
          {stage.reason === "ended" ? null : (
            <Button className="self-start" onClick={() => setStage({ at: "form" })}>
              {t("calls.guestRejoin")}
            </Button>
          )}
        </div>
      ) : (
        <JoinForm onJoin={(name, media) => setStage({ at: "call", name, media })} />
      )}
      <Toaster />
    </Shell>
  );
}

function Shell({ children }: { children: React.ReactNode }) {
  const t = useT();
  return (
    <main className="flex min-h-dvh items-center justify-center bg-ground p-4">
      <Card className="w-full max-w-md">
        <CardHeader className="flex flex-row items-start justify-between gap-3">
          <CardTitle>{t("calls.guestTitle")}</CardTitle>
          <LanguagePicker />
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <p className="text-[13px] text-muted">{t("calls.guestIntro")}</p>
          {children}
        </CardContent>
      </Card>
    </main>
  );
}

function JoinForm({ onJoin }: { onJoin(name: string, media: LocalMedia): void }) {
  const t = useT();
  const [name, setName] = React.useState("");
  const [camera, setCamera] = React.useState(true);
  const [busy, setBusy] = React.useState(false);
  const clean = name.trim();

  const join = (e: React.FormEvent): void => {
    e.preventDefault();
    if (clean === "") return;
    setBusy(true);
    LocalMedia.open(camera)
      .then((media) => onJoin(clean, media))
      .catch(() => {
        setBusy(false);
        toast.error(t("calls.failed"), t("calls.noDevices"), { help: "calls" });
      });
  };

  return (
    <form className="flex flex-col gap-4" onSubmit={join} data-guest-form="">
      <FieldGroup>
        <Field>
          <FieldLabel htmlFor="guest-name">{t("calls.guestName")}</FieldLabel>
          <Input
            id="guest-name"
            name="name"
            autoComplete="name"
            maxLength={40}
            required
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value.replace(/[\u0000-\u001f\u007f]/g, ""))}
          />
          <FieldDescription>{t("calls.guestNameHint")}</FieldDescription>
        </Field>
        <div className="flex items-center justify-between gap-3">
          <span className="text-[13px] font-medium">{t("calls.guestCamera")}</span>
          <Switch isSelected={camera} onChange={setCamera} aria-label={t("calls.guestCamera")} />
        </div>
      </FieldGroup>
      <Button type="submit" disabled={busy || clean === ""}>
        {t("calls.guestJoin")}
      </Button>
    </form>
  );
}

function GuestCall({
  link,
  name,
  media,
  onOver,
}: {
  link: GuestLink;
  name: string;
  media: LocalMedia;
  onOver(reason: "ended" | "refused"): void;
}) {
  const t = useT();
  const [state, setState] = React.useState<"connecting" | "connected">("connecting");
  const [members, setMembers] = React.useState<{ list: Member[]; you: number }>({ list: [], you: -1 });
  const [streams, setStreams] = React.useState<Map<number, MediaStream>>(new Map());
  const [screens, setScreens] = React.useState<Map<number, MediaStream>>(new Map());
  const [stats, setStats] = React.useState<WorkerStats | null>(null);
  const [micOff, setMicOff] = React.useState(!media.micOn);
  const [cameraOff, setCameraOff] = React.useState(!media.cameraOn);
  const [background, setBackground] = React.useState<Background>(media.background);
  const [ownScreen, setOwnScreen] = React.useState<MediaStream | null>(null);
  const session = React.useRef<CallSession | null>(null);
  const over = React.useRef(onOver);
  over.current = onOver;

  React.useEffect(() => {
    if (media.backgroundFailed) toast.error(t("calls.failed"), t("calls.backgroundFailed"), { help: "calls" });
    if (media.missing.mic) toast.error(t("calls.noMicTitle"), t("calls.noMic"), { help: "calls" });
    else if (media.missing.camera) toast.error(t("calls.noCameraTitle"), t("calls.noCamera"), { help: "calls" });
    const call = new CallSession({
      socket: socketUrl(),
      call: link.call,
      guest: { invite: link.invite, name, secret: link.secret },
      local: media.stream,
      events: {
        state: (next, reason) => {
          if (next === "connected") setState("connected");
          if (next === "failed") over.current(reason !== undefined && REFUSED.has(reason) ? "refused" : "ended");
        },
        members: (list, you) => {
          setMembers({ list, you });
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
    return () => {
      void call.leave();
      session.current = null;
    };
    // One session per mount: the page joins once per name.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const named = (m: Member): string => m.guest ?? m.user.replace(/^@/, "").replace(/:.*$/, "");

  const stopSharing = (): void => {
    media.stopSharing();
    void session.current?.setScreenTrack(null);
    setOwnScreen(null);
  };

  const tiles: Tile[] = members.list.map((m) => {
    const self = m.pid === members.you;
    const stream = self ? media.stream : streams.get(m.pid);
    return {
      pid: m.pid,
      name: m.guest !== undefined ? t("calls.guestNamed", { name: named(m) }) : named(m),
      self,
      dark: self && cameraOff,
      ...(stream !== undefined ? { stream } : {}),
    };
  });
  const shown: Screen[] = members.list.flatMap((m): Screen[] => {
    const self = m.pid === members.you;
    const stream = self ? ownScreen : m.screen === true ? (screens.get(m.pid) ?? null) : null;
    return stream === null ? [] : [{ pid: m.pid, name: named(m), stream, self }];
  });
  const others = members.list.filter((m) => m.pid !== members.you).length;

  return (
    <div className="h-dvh" data-guest-call="">
      <CallView
        title={t("calls.guestTitle")}
        tiles={tiles}
        screens={shown}
        status={state === "connecting" ? "connecting" : others === 0 ? "waiting" : "live"}
        micOff={micOff}
        cameraOff={cameraOff}
        sharing={ownScreen !== null}
        background={background}
        stats={stats}
        onMic={() => {
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
        }}
        onCamera={() => {
          media
            .setCamera(cameraOff)
            .then(async () => {
              await session.current?.setVideoTrack(media.picture);
              setCameraOff(!media.cameraOn);
            })
            .catch(() => toast.error(t("calls.noCameraTitle"), t("calls.cameraFailed"), { help: "calls" }));
        }}
        onBackground={() => {
          const next: Background = background === "black" ? "real" : "black";
          void media.setBackground(next).then(async () => {
            await session.current?.setVideoTrack(media.picture);
            setBackground(next);
          });
        }}
        onShare={() => {
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
            .catch(() => undefined);
        }}
        onLeave={() => {
          void session.current?.leave();
          over.current("ended");
        }}
      />
    </div>
  );
}
