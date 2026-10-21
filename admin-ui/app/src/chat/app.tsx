import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  ArrowLeft01Icon,
  Call02Icon,
  Comment01Icon,
  DashboardSquare01Icon,
  Logout01Icon,
  Notification01Icon,
  Settings01Icon,
  SquareLock01Icon,
  UserAdd01Icon,
  Video01Icon,
} from "@hugeicons/core-free-icons";
import {
  ClientEvent,
  EventType,
  KnownMembership,
  type MatrixClient,
  type MatrixEvent,
  MatrixEventEvent,
  type Room,
  RoomEvent,
  SyncState,
} from "matrix-js-sdk";
import { CryptoEvent, type VerificationRequest } from "matrix-js-sdk/lib/crypto-api";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { Toaster } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { cn } from "@/lib/utils";
import { Avatar } from "./avatar";
import { CallsProvider, useCalls } from "./calls";
import { Composer } from "./composer";
import { ImageViewer, InviteDialog, JoinDialog, NewRoomDialog, RoomSettingsDialog, StartChatDialog } from "./dialogs";
import { directPartner, directRooms, plainBody, ROOM_EVENTS, unread, useEmitted } from "./model";
import { RoomList, SpaceRail } from "./room-list";
import { SecurityDialog, useTrust, VerificationDialog } from "./security";
import { ChatRefused, goSignIn, onSignedOut, type Refusal, type Signed, signIn, signOut, start } from "./session";
import { RoomTimeline, type RowActions } from "./timeline";
import { ThreadPanel } from "./thread-panel";

type Boot = { state: "loading" } | { state: "ready"; signed: Signed } | { state: "refused"; why: Refusal; message: string };

/* The room on screen lives in the address's hash (#/room/!id:server), so a
 * reload or a shared link opens it again without the box routing anything. */
function roomFromHash(): string | null {
  const match = /^#\/room\/(.+)$/.exec(window.location.hash);
  return match?.[1] !== undefined ? decodeURIComponent(match[1]) : null;
}

function useHashRoom(): [string | null, (id: string | null) => void] {
  const [room, setRoom] = React.useState(roomFromHash);
  React.useEffect(() => {
    const onHash = (): void => setRoom(roomFromHash());
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);
  const go = React.useCallback((id: string | null) => {
    window.location.hash = id === null ? "" : `#/room/${encodeURIComponent(id)}`;
    setRoom(id);
  }, []);
  return [room, go];
}

export function ChatRoot() {
  const t = useT();
  const [boot, setBoot] = React.useState<Boot>({ state: "loading" });
  const [attempt, setAttempt] = React.useState(0);

  React.useEffect(() => {
    let live = true;
    setBoot({ state: "loading" });
    (async () => {
      const signed = await signIn();
      onSignedOut(signed.client, () => setAttempt((n) => n + 1));
      await start(signed.client);
      if (live) setBoot({ state: "ready", signed });
    })().catch((error: unknown) => {
      if (!live) return;
      const why: Refusal = error instanceof ChatRefused ? error.why : "broken";
      if (why === "signIn") {
        goSignIn();
        return;
      }
      if (why === "off") {
        // Chat is not on this box now (sharing off): back to the admin page.
        window.location.replace("/");
        return;
      }
      setBoot({ state: "refused", why, message: error instanceof Error ? error.message : String(error) });
    });
    return () => {
      live = false;
    };
  }, [attempt]);

  // A box still starting chat answers again in a few seconds.
  React.useEffect(() => {
    if (boot.state !== "refused" || boot.why !== "starting") return;
    const handle = setTimeout(() => setAttempt((n) => n + 1), 3000);
    return () => clearTimeout(handle);
  }, [boot]);

  if (boot.state === "ready") {
    return (
      <>
        <CallsProvider client={boot.signed.client}>
          <ChatShell signed={boot.signed} />
        </CallsProvider>
        <Toaster />
      </>
    );
  }
  return (
    <div className="flex min-h-dvh flex-col items-center justify-center gap-3 p-6 text-center">
      <img src={logoFor().url} alt="" className="size-12" />
      {boot.state === "loading" || (boot.state === "refused" && boot.why === "starting") ? (
        <p className="flex items-center gap-2 text-[14px] text-muted">
          <Spinner size={16} />
          {boot.state === "loading" ? t("chat.boot.loading") : t("chat.boot.starting")}
        </p>
      ) : boot.why === "notForYou" ? (
        <>
          <p className="max-w-sm text-[14px]">{t("chat.boot.notForYou")}</p>
          <Button onClick={goSignIn}>{t("chat.boot.signIn")}</Button>
        </>
      ) : (
        <>
          <p className="max-w-sm text-[14px]">{t("chat.boot.broken")}</p>
          <p className="max-w-sm text-[12.5px] text-muted">{boot.message}</p>
          <Button onClick={() => setAttempt((n) => n + 1)}>{t("chat.boot.retry")}</Button>
        </>
      )}
    </div>
  );
}

/** Desktop notifications for new messages while the tab is in the background. */
function useNotifications(client: MatrixClient, current: string | null): void {
  const t = useT();
  React.useEffect(() => {
    const me = client.getSafeUserId();
    const tell = (ev: MatrixEvent, room: Room | undefined): void => {
      if (room === undefined || ev.getSender() === me || ev.getType() !== EventType.RoomMessage) return;
      if (!document.hidden && room.roomId === current) return;
      if (typeof Notification === "undefined" || Notification.permission !== "granted") return;
      if (client.getPushActionsForEvent(ev)?.notify !== true) return;
      const body = plainBody(ev.getContent()) || t("chat.reply.attachment");
      const note = new Notification(room.name, { body: `${ev.sender?.name ?? ""}: ${body}`, tag: room.roomId });
      note.onclick = () => {
        window.focus();
        window.location.hash = `#/room/${encodeURIComponent(room.roomId)}`;
      };
    };
    const onTimeline = (ev: MatrixEvent, room: Room | undefined, toStart: boolean | undefined, _removed: boolean, data: { liveEvent?: boolean }): void => {
      if (toStart === true || data.liveEvent !== true) return;
      if (ev.isBeingDecrypted() || ev.getType() === EventType.RoomMessageEncrypted) {
        ev.once(MatrixEventEvent.Decrypted, () => tell(ev, room));
      } else {
        tell(ev, room);
      }
    };
    client.on(RoomEvent.Timeline, onTimeline);
    return () => {
      client.off(RoomEvent.Timeline, onTimeline);
    };
  }, [client, current, t]);
}

/** The page title carries the unread count, so a background tab shows it. */
function useTitle(client: MatrixClient): void {
  const t = useT();
  const tick = useEmitted(client, ROOM_EVENTS);
  React.useEffect(() => {
    const total = client.getRooms().reduce((sum, room) => sum + (room.getMyMembership() === KnownMembership.Join ? unread(room).total : 0), 0);
    document.title = total > 0 ? `(${total}) ${t("chat.title")}` : t("chat.title");
  }, [client, tick, t]);
}

function ChatShell({ signed }: { signed: Signed }) {
  const t = useT();
  const { client } = signed;
  const me = client.getSafeUserId();
  const [current, setCurrent] = useHashRoom();
  const [space, setSpace] = React.useState<string | null>(null);
  const [dialog, setDialog] = React.useState<"room" | "space" | "chat" | "join" | "invite" | "settings" | "security" | null>(null);
  const [verification, setVerification] = React.useState<VerificationRequest | null>(null);
  const [trust, recheck] = useTrust(client);
  const [syncState, setSyncState] = React.useState<SyncState | null>(client.getSyncState());
  const [notify, setNotify] = React.useState(typeof Notification !== "undefined" ? Notification.permission : "denied");
  useEmitted(client, ROOM_EVENTS);
  useNotifications(client, current);
  useTitle(client);

  React.useEffect(() => {
    const onSync = (state: SyncState): void => setSyncState(state);
    const onRequest = (request: VerificationRequest): void => setVerification(request);
    client.on(ClientEvent.Sync, onSync);
    client.on(CryptoEvent.VerificationRequestReceived, onRequest);
    return () => {
      client.off(ClientEvent.Sync, onSync);
      client.off(CryptoEvent.VerificationRequestReceived, onRequest);
    };
  }, [client]);

  const room = current !== null ? (client.getRoom(current) ?? null) : null;
  const tool = "rounded-control p-1.5 text-muted hover:bg-sunk hover:text-ink";

  return (
    <div className="flex h-dvh overflow-hidden bg-ground text-ink">
      <div className={cn("hidden sm:flex", room !== null && "max-sm:hidden")}>
        <SpaceRail client={client} space={space} onSpace={setSpace} onNewSpace={() => setDialog("space")} />
      </div>
      <aside className={cn("flex w-full min-w-0 flex-col border-r border-line bg-surface sm:w-72 sm:shrink-0", room !== null && "max-sm:hidden")} aria-label={t("chat.list.label")}>
        <header className="flex h-14 items-center gap-2 border-b border-line px-3">
          <img src={logoFor().url} alt="" className="size-7" />
          <h1 className="flex-1 text-[15px] font-semibold tracking-tight">{t("chat.title")}</h1>
          <button type="button" className={cn(tool, "relative")} aria-label={t("chat.security.title")} title={t("chat.security.title")} onClick={() => setDialog("security")}>
            <HugeiconsIcon icon={SquareLock01Icon} className="size-4" />
            {trust !== "ready" && trust !== "loading" && <span className="absolute top-1 right-1 size-2 rounded-full bg-warn" />}
          </button>
          {notify === "default" && (
            <button
              type="button"
              className={tool}
              aria-label={t("chat.notify.ask")}
              title={t("chat.notify.ask")}
              onClick={() => void Notification.requestPermission().then(setNotify)}
            >
              <HugeiconsIcon icon={Notification01Icon} className="size-4" />
            </button>
          )}
          <a className={tool} href="/" aria-label={t("chat.toAdmin")} title={t("chat.toAdmin")}>
            <HugeiconsIcon icon={DashboardSquare01Icon} className="size-4" />
          </a>
          <button type="button" className={tool} aria-label={t("chat.signOut")} title={t("chat.signOut")} onClick={() => void signOut(signed)}>
            <HugeiconsIcon icon={Logout01Icon} className="size-4" />
          </button>
        </header>
        {trust !== "ready" && trust !== "loading" && (
          <button type="button" onClick={() => setDialog("security")} className="flex items-center gap-2 border-b border-line bg-accent-wash px-3 py-2 text-left text-[12.5px] text-accent">
            <HugeiconsIcon icon={SquareLock01Icon} className="size-4 shrink-0" />
            {trust === "new" ? t("chat.security.bannerNew") : t("chat.security.bannerUnverified")}
          </button>
        )}
        {syncState === SyncState.Error || syncState === SyncState.Reconnecting ? (
          <p className="border-b border-line bg-sunk px-3 py-1.5 text-[12.5px] text-warn">{t("chat.offline")}</p>
        ) : null}
        <RoomList
          client={client}
          current={current}
          space={space}
          onSpace={setSpace}
          onOpen={setCurrent}
          onNewRoom={() => setDialog("room")}
          onNewChat={() => setDialog("chat")}
          onJoin={() => setDialog("join")}
          onNewSpace={() => setDialog("space")}
        />
        <footer className="flex items-center gap-2 border-t border-line px-3 py-2">
          <Avatar client={client} id={me} name={signed.displayName} mxc={client.getUser(me)?.avatarUrl} size={24} />
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[13px] font-medium">{signed.displayName}</span>
            <span className="block truncate text-[11.5px] text-muted">{me}</span>
          </span>
        </footer>
      </aside>
      {room !== null ? (
        <RoomView key={room.roomId} signed={signed} room={room} onBack={() => setCurrent(null)} onDialog={setDialog} />
      ) : (
        <main className="hidden flex-1 flex-col items-center justify-center gap-3 p-6 text-center sm:flex">
          <HugeiconsIcon icon={Comment01Icon} className="size-10 text-faint" />
          <p className="text-[15px] font-semibold">{t("chat.empty.title")}</p>
          <p className="max-w-sm text-[13.5px] text-muted">{t("chat.empty.lead")}</p>
          <div className="flex gap-2">
            <Button variant="secondary" onClick={() => setDialog("chat")}>
              {t("chat.list.newChat")}
            </Button>
            <Button variant="secondary" onClick={() => setDialog("room")}>
              {t("chat.list.newRoom")}
            </Button>
          </div>
        </main>
      )}

      <NewRoomDialog client={client} open={dialog === "room"} onOpenChange={(o) => setDialog(o ? "room" : null)} space={space} onCreated={setCurrent} />
      <NewRoomDialog
        client={client}
        open={dialog === "space"}
        asSpace
        onOpenChange={(o) => setDialog(o ? "space" : null)}
        space={null}
        onCreated={(id) => {
          setSpace(id);
        }}
      />
      <StartChatDialog client={client} open={dialog === "chat"} onOpenChange={(o) => setDialog(o ? "chat" : null)} onCreated={setCurrent} />
      <JoinDialog client={client} open={dialog === "join"} onOpenChange={(o) => setDialog(o ? "join" : null)} onJoined={setCurrent} space={space} />
      {room !== null && (
        <>
          <InviteDialog client={client} room={room} open={dialog === "invite"} onOpenChange={(o) => setDialog(o ? "invite" : null)} />
          <RoomSettingsDialog
            client={client}
            room={room}
            open={dialog === "settings"}
            onOpenChange={(o) => setDialog(o ? "settings" : null)}
            onLeft={() => setCurrent(null)}
            onInvite={() => setDialog("invite")}
          />
        </>
      )}
      <SecurityDialog
        client={client}
        keys={signed.keys}
        trust={trust}
        open={dialog === "security"}
        onOpenChange={(o) => setDialog(o ? "security" : null)}
        onChanged={recheck}
        onVerify={setVerification}
      />
      <VerificationDialog
        request={verification}
        onClose={() => {
          setVerification(null);
          recheck();
        }}
      />
    </div>
  );
}

function InvitedView({ client, room }: { client: MatrixClient; room: Room }) {
  const t = useT();
  const [busy, setBusy] = React.useState(false);
  const me = client.getSafeUserId();
  const inviter = room.getMember(me)?.events.member?.getSender() ?? "";
  const direct = room.getMember(me)?.events.member?.getContent()["is_direct"] === true;
  const accept = async (): Promise<void> => {
    setBusy(true);
    try {
      await client.joinRoom(room.roomId);
      if (direct && inviter !== "") {
        const content = { ...((client.getAccountData(EventType.Direct)?.getContent() ?? {}) as Record<string, string[]>) };
        content[inviter] = [...new Set([...(content[inviter] ?? []), room.roomId])];
        await client.setAccountData(EventType.Direct, content);
      }
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
      <Avatar client={client} id={room.roomId} name={room.name} mxc={room.getMxcAvatarUrl()} size={64} />
      <p className="text-[15px] font-semibold">{room.name}</p>
      <p className="text-[13.5px] text-muted">{t("chat.invite.youWere", { name: room.getMember(inviter)?.name ?? inviter })}</p>
      <div className="flex gap-2">
        <Button variant="ghost" disabled={busy} onClick={() => void client.leave(room.roomId)}>
          {t("chat.invite.decline")}
        </Button>
        <Button disabled={busy} onClick={() => void accept()}>
          {busy && <Spinner size={14} />}
          {t("chat.invite.accept")}
        </Button>
      </div>
    </div>
  );
}

function RoomView({ signed, room, onBack, onDialog }: { signed: Signed; room: Room; onBack: () => void; onDialog: (d: "invite" | "settings") => void }) {
  const t = useT();
  const { client } = signed;
  const me = client.getSafeUserId();
  useEmitted(room, [RoomEvent.Name, RoomEvent.MyMembership]);
  const calls = useCalls();
  const [replyTo, setReplyTo] = React.useState<MatrixEvent | null>(null);
  const [editing, setEditing] = React.useState<MatrixEvent | null>(null);
  const [panel, setPanel] = React.useState<{ root: string | null } | null>(null);
  const [image, setImage] = React.useState<string | null>(null);
  const partner = directPartner(client, room, directRooms(client));
  const topic = room.currentState.getStateEvents(EventType.RoomTopic, "")?.getContent()["topic"];
  const encrypted = room.hasEncryptionStateEvent();
  const membership = room.getMyMembership();

  const actions: RowActions = {
    onReply: (ev) => {
      setEditing(null);
      setReplyTo(ev);
    },
    onEdit: (ev) => {
      setReplyTo(null);
      setEditing(ev);
    },
    onThread: (root) => setPanel({ root }),
    onOpenImage: setImage,
  };
  const tool = "rounded-control p-2 text-muted hover:bg-sunk hover:text-ink disabled:opacity-45";

  return (
    <>
      <main className={cn("flex min-w-0 flex-1 flex-col", panel !== null && "max-lg:hidden")} aria-label={room.name}>
        <header className="flex h-14 shrink-0 items-center gap-2 border-b border-line bg-surface px-2 sm:px-4">
          <button type="button" className={cn(tool, "sm:hidden")} aria-label={t("chat.back")} onClick={onBack}>
            <HugeiconsIcon icon={ArrowLeft01Icon} className="size-5" />
          </button>
          <Avatar client={client} id={partner ?? room.roomId} name={room.name} mxc={room.getMxcAvatarUrl()} size={32} />
          <div className="min-w-0 flex-1">
            <p className="flex items-center gap-1.5 truncate text-[14.5px] font-semibold">
              {room.name}
              {encrypted && (
                <span title={t("chat.room.encrypted")} aria-label={t("chat.room.encrypted")}>
                  <HugeiconsIcon icon={SquareLock01Icon} className="size-3.5 text-ok" />
                </span>
              )}
            </p>
            {typeof topic === "string" && topic !== "" && <p className="truncate text-[12px] text-muted">{topic}</p>}
          </div>
          {membership === KnownMembership.Join && (
            <>
              {calls.available && (
                <>
                  <button type="button" className={tool} aria-label={t("chat.call.voice")} title={t("chat.call.voice")} disabled={calls.active !== null} onClick={() => calls.start(room, false)}>
                    <HugeiconsIcon icon={Call02Icon} className="size-5" />
                  </button>
                  <button type="button" className={tool} aria-label={t("chat.call.video")} title={t("chat.call.video")} disabled={calls.active !== null} onClick={() => calls.start(room, true)}>
                    <HugeiconsIcon icon={Video01Icon} className="size-5" />
                  </button>
                </>
              )}
              <button
                type="button"
                className={cn(tool, panel !== null && "bg-accent-wash text-accent")}
                aria-label={t("chat.thread.title")}
                title={t("chat.thread.title")}
                aria-pressed={panel !== null}
                onClick={() => setPanel(panel === null ? { root: null } : null)}
              >
                <HugeiconsIcon icon={Comment01Icon} className="size-5" />
              </button>
              <button type="button" className={cn(tool, "max-sm:hidden")} aria-label={t("chat.settings.invite")} title={t("chat.settings.invite")} onClick={() => onDialog("invite")}>
                <HugeiconsIcon icon={UserAdd01Icon} className="size-5" />
              </button>
              <button type="button" className={tool} aria-label={t("chat.settings.open")} title={t("chat.settings.open")} onClick={() => onDialog("settings")}>
                <HugeiconsIcon icon={Settings01Icon} className="size-5" />
              </button>
            </>
          )}
        </header>
        {membership === KnownMembership.Invite ? (
          <InvitedView client={client} room={room} />
        ) : (
          <>
            <RoomTimeline client={client} room={room} me={me} actions={actions} />
            <Composer
              client={client}
              room={room}
              replyTo={replyTo}
              editing={editing}
              onDone={() => {
                setReplyTo(null);
                setEditing(null);
              }}
              placeholder={t("chat.composer.placeholder", { room: room.name })}
            />
          </>
        )}
      </main>
      {panel !== null && (
        <ThreadPanel client={client} room={room} root={panel.root} me={me} actions={actions} onOpen={(root) => setPanel({ root })} onClose={() => setPanel(null)} />
      )}
      <ImageViewer url={image} onClose={() => setImage(null)} />
    </>
  );
}
