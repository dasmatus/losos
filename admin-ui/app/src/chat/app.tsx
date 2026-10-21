import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  ArrowLeft01Icon,
  Call02Icon,
  Comment01Icon,
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
import { AppBar, type AppBarPerson } from "@/components/app-bar";
import { ContextMenuHost, usePageMenu } from "@/components/context-menu";
import { PersonAvatar } from "@/components/person";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { loadMe, useMe } from "@/lib/api";
import { Spinner } from "@/components/ui/spinner";
import { Toaster } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { Avatar } from "./avatar";
import { CallsProvider, useCalls } from "./calls";
import { Composer } from "./composer";
import { ImageViewer, InviteDialog, JoinDialog, NewRoomDialog, RoomSettingsDialog, StartChatDialog } from "./dialogs";
import { directPartner, directRooms, plainBody, ROOM_EVENTS, roomName, unread, useEmitted } from "./model";
import { RoomList, SpaceRail } from "./room-list";
import { SecurityDialog, useTrust, VerificationDialog } from "./security";
import { ChatRefused, goSignIn, onSignedOut, type Refusal, type Signed, signedHere, signIn, signOutHere, start } from "./session";
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
  // When each recent device was signed out, so a homeserver that refuses
  // every new device too does not get asked for one again and again.
  const signedOut = React.useRef<number[]>([]);
  const pageMenu = usePageMenu();

  React.useEffect(() => {
    let live = true;
    setBoot({ state: "loading" });
    (async () => {
      const signed = await signIn();
      onSignedOut(signed.client, () => {
        const now = Date.now();
        const recent = [...signedOut.current.filter((at) => now - at < 60_000), now];
        signedOut.current = recent;
        if (recent.length >= 3) {
          signedOut.current = [];
          if (live) setBoot({ state: "refused", why: "broken", message: t("chat.boot.refusedAgain") });
          return;
        }
        setAttempt((n) => n + 1);
      });
      await start(signed.client);
      if (live) {
        signedHere(signed);
        setBoot({ state: "ready", signed });
      }
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
      signedHere(null);
    };
  }, [attempt]);

  // A box still starting chat answers again in a few seconds.
  React.useEffect(() => {
    if (boot.state !== "refused" || boot.why !== "starting") return;
    const handle = setTimeout(() => setAttempt((n) => n + 1), 3000);
    return () => clearTimeout(handle);
  }, [boot]);

  // One right-click menu for the whole page: a message answers with its
  // own (timeline.tsx), anywhere else gets the page's.
  const menu = <ContextMenuHost page={pageMenu} />;
  if (boot.state === "ready") {
    return (
      <>
        <CallsProvider client={boot.signed.client}>
          <ChatShell signed={boot.signed} />
        </CallsProvider>
        {menu}
        <Toaster />
      </>
    );
  }
  return (
    <div className="flex h-dvh flex-col bg-ground text-ink">
      {menu}
      <ChatBar />
      <main className="flex flex-1 items-center justify-center p-6">
        {boot.state === "loading" || (boot.state === "refused" && boot.why === "starting") ? (
          <p className="flex items-center gap-2 text-[13.5px] text-muted">
            <Spinner size={16} />
            {boot.state === "loading" ? t("chat.boot.loading") : t("chat.boot.starting")}
          </p>
        ) : (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={Comment01Icon} strokeWidth={1.5} aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>{t(boot.why === "notForYou" ? "chat.boot.notForYouTitle" : "chat.boot.broken")}</EmptyTitle>
              {boot.why === "notForYou" && <EmptyDescription>{t("chat.boot.notForYou")}</EmptyDescription>}
            </EmptyHeader>
            {boot.why !== "notForYou" && (
              <Alert variant="crit" className="text-left">
                <AlertDescription>{boot.message}</AlertDescription>
              </Alert>
            )}
            <EmptyContent>
              {boot.why === "notForYou" ? (
                <Button onClick={goSignIn}>{t("chat.boot.signIn")}</Button>
              ) : (
                <Button onClick={() => setAttempt((n) => n + 1)}>{t("chat.boot.retry")}</Button>
              )}
            </EmptyContent>
          </Empty>
        )}
      </main>
    </div>
  );
}

/** The web UI's top bar (components/app-bar.tsx) at full width, as over every
 *  app laid out in panes: the box's logo, LosOS Chat, chat's own buttons,
 *  then the language, the theme, who is signed in and Sign out. */
function ChatBar({ children }: { children?: React.ReactNode }) {
  const t = useT();
  const me = useMe();
  React.useEffect(() => {
    void loadMe().catch(() => undefined);
  }, []);
  const person: AppBarPerson | null =
    me === null || me.spareKey || !me.name
      ? null
      : {
          label: me.displayName ?? me.name,
          tip: `${t("shell.me.signedIn", { name: me.name })} ${t(me.admin ? "shell.me.admin" : "shell.me.limited")}`,
          avatar: (
            <PersonAvatar
              size="xs"
              person={{ name: me.name, displayName: me.displayName ?? me.name, picture: me.picture, accent: me.accent }}
            />
          ),
          accountLink: <a href="/settings/account" />,
        };
  return (
    <AppBar width="full" name={t("chat.title")} nameLink={<a href="/chat/" />} person={person} onSignOut={signOutHere}>
      {children}
    </AppBar>
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
      const note = new Notification(roomName(room), { body: `${ev.sender?.name ?? ""}: ${body}`, tag: room.roomId });
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

  return (
    <div className="flex h-dvh flex-col bg-ground text-ink">
      <ChatBar>
        <div className="flex-1" />
        <Button variant="ghost" size="icon-sm" className="relative" aria-label={t("chat.security.title")} title={t("chat.security.title")} onClick={() => setDialog("security")}>
          <HugeiconsIcon icon={SquareLock01Icon} className="size-4" />
          {trust !== "ready" && trust !== "loading" && <span className="absolute top-1 right-1 size-2 rounded-full bg-warn" />}
        </Button>
        {notify === "default" && (
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={t("chat.notify.ask")}
            title={t("chat.notify.ask")}
            onClick={() => void Notification.requestPermission().then(setNotify)}
          >
            <HugeiconsIcon icon={Notification01Icon} className="size-4" />
          </Button>
        )}
      </ChatBar>
      <div className="flex min-h-0 flex-1 overflow-hidden">
      <div className={cn("hidden sm:flex", room !== null && "max-sm:hidden")}>
        <SpaceRail client={client} space={space} onSpace={setSpace} onNewSpace={() => setDialog("space")} />
      </div>
      <aside className={cn("flex w-full min-w-0 flex-col border-r border-line bg-surface sm:w-72 sm:shrink-0", room !== null && "max-sm:hidden")} aria-label={t("chat.list.label")}>
        <header className="flex h-12 items-center border-b border-line px-3">
          <h1 className="text-[15px] font-semibold tracking-tight">{t("chat.list.label")}</h1>
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
        <main className="hidden flex-1 items-center justify-center p-6 sm:flex">
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={Comment01Icon} strokeWidth={1.5} aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>{t("chat.empty.title")}</EmptyTitle>
              <EmptyDescription>{t("chat.empty.lead")}</EmptyDescription>
            </EmptyHeader>
            <EmptyContent className="flex-row justify-center">
              <Button variant="secondary" onClick={() => setDialog("chat")}>
                {t("chat.list.newChat")}
              </Button>
              <Button variant="secondary" onClick={() => setDialog("room")}>
                {t("chat.list.newRoom")}
              </Button>
            </EmptyContent>
          </Empty>
        </main>
      )}
      </div>

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
    <Empty className="flex-1">
      <EmptyHeader>
        <EmptyMedia>
          <Avatar client={client} id={room.roomId} name={roomName(room)} mxc={room.getMxcAvatarUrl()} size={64} />
        </EmptyMedia>
        <EmptyTitle>{roomName(room)}</EmptyTitle>
        <EmptyDescription>{t("chat.invite.youWere", { name: room.getMember(inviter)?.name ?? inviter })}</EmptyDescription>
      </EmptyHeader>
      <EmptyContent className="flex-row justify-center">
        <Button variant="ghost" disabled={busy} onClick={() => void client.leave(room.roomId)}>
          {t("chat.invite.decline")}
        </Button>
        <Button disabled={busy} onClick={() => void accept()}>
          {busy && <Spinner size={14} />}
          {t("chat.invite.accept")}
        </Button>
      </EmptyContent>
    </Empty>
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

  return (
    <>
      <main className={cn("flex min-w-0 flex-1 flex-col", panel !== null && "max-lg:hidden")} aria-label={roomName(room)}>
        <header className="flex h-14 shrink-0 items-center gap-2 border-b border-line bg-surface px-2 sm:px-4">
          <Button variant="ghost" size="icon-sm" className={cn("sm:hidden")} aria-label={t("chat.back")} onClick={onBack}>
            <HugeiconsIcon icon={ArrowLeft01Icon} className="size-4" />
          </Button>
          <Avatar client={client} id={partner ?? room.roomId} name={roomName(room)} mxc={room.getMxcAvatarUrl()} size={32} />
          <div className="min-w-0 flex-1">
            <p className="flex items-center gap-1.5 truncate text-[14.5px] font-semibold">
              {roomName(room)}
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
              <Button variant="ghost" size="icon-sm" aria-label={t("chat.call.voice")} title={t("chat.call.voice")} disabled={calls.active !== null} onClick={() => calls.start(room, false)}>
                <HugeiconsIcon icon={Call02Icon} className="size-4" />
              </Button>
              <Button variant="ghost" size="icon-sm" aria-label={t("chat.call.video")} title={t("chat.call.video")} disabled={calls.active !== null} onClick={() => calls.start(room, true)}>
                <HugeiconsIcon icon={Video01Icon} className="size-4" />
              </Button>
              <Button variant="ghost" size="icon-sm"
               
                className={cn(panel !== null && "bg-accent-wash text-accent")}
                aria-label={t("chat.thread.title")}
                title={t("chat.thread.title")}
                aria-pressed={panel !== null}
                onClick={() => setPanel(panel === null ? { root: null } : null)}
              >
                <HugeiconsIcon icon={Comment01Icon} className="size-4" />
              </Button>
              <Button variant="ghost" size="icon-sm" className={cn("max-sm:hidden")} aria-label={t("chat.settings.invite")} title={t("chat.settings.invite")} onClick={() => onDialog("invite")}>
                <HugeiconsIcon icon={UserAdd01Icon} className="size-4" />
              </Button>
              <Button variant="ghost" size="icon-sm" aria-label={t("chat.settings.open")} title={t("chat.settings.open")} onClick={() => onDialog("settings")}>
                <HugeiconsIcon icon={Settings01Icon} className="size-4" />
              </Button>
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
              placeholder={t("chat.composer.placeholder", { room: roomName(room) })}
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
