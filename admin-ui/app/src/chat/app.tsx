import * as React from "react";
import { useNavigate } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Add01Icon,
  ArrowLeft01Icon,
  Call02Icon,
  Comment01Icon,
  Home01Icon,
  Notification01Icon,
  Search01Icon,
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
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Spinner } from "@/components/ui/spinner";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { PaneHeader } from "@/screens/settings/rows";
import { Avatar } from "./avatar";
import { useCalls } from "./calls";
import { chatDocument } from "./document";
import { Composer } from "./composer";
import { ImageViewer, InviteDialog, JoinDialog, NewRoomDialog, RoomSettingsDialog, StartChatDialog } from "./dialogs";
import { directPartner, directRooms, isSpace, plainBody, ROOM_EVENTS, roomName, unread, useEmitted } from "./model";
import { RoomList } from "./room-list";
import { SecurityDialog, useTrust, VerificationDialog } from "./security";
import { ChatRefused, goSignIn, type Refusal, type Signed, session, signedNow, signedOutListeners } from "./session";
import { RoomTimeline, type RowActions } from "./timeline";
import { ThreadPanel } from "./thread-panel";

/* LosOS Chat, a screen of the admin pages at /chat/ (App.tsx), in their top
 * bar and sidebar like Mail: a header, a row of spaces and a search, then
 * one card with the conversations beside the open one. On a phone the card
 * shows one column at a time.
 *
 * The document has to be loaded at /chat/: nginx serves it there under
 * chat's own policy (modules/containers.nix), which adds 'wasm-unsafe-eval'
 * for the end-to-end crypto and blob: for decrypted media. The sidebar and
 * the palette link to it as a real navigation; anything that routes here
 * inside the page instead loads it again (chatDocument). */

type Boot = { state: "loading" } | { state: "ready"; signed: Signed } | { state: "refused"; why: Refusal; message: string };

/* The room on screen lives in the address's hash (#/room/!id:server), so a
 * reload or a shared link opens it again. */
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

export default function ChatScreen() {
  const t = useT();
  const navigate = useNavigate();
  const here = chatDocument();
  const [boot, setBoot] = React.useState<Boot>(() => {
    const now = signedNow();
    return now !== null ? { state: "ready", signed: now } : { state: "loading" };
  });
  const [attempt, setAttempt] = React.useState(0);
  // When each recent device was signed out, so a homeserver that refuses
  // every new device too does not get asked for one again and again.
  const signedOut = React.useRef<number[]>([]);

  React.useEffect(() => {
    if (!here) window.location.reload();
  }, [here]);

  React.useEffect(() => {
    const again = (): void => {
      const now = Date.now();
      const recent = [...signedOut.current.filter((at) => now - at < 60_000), now];
      signedOut.current = recent;
      if (recent.length >= 3) {
        signedOut.current = [];
        setBoot({ state: "refused", why: "broken", message: t("chat.boot.refusedAgain") });
        return;
      }
      setAttempt((n) => n + 1);
    };
    signedOutListeners.add(again);
    return () => {
      signedOutListeners.delete(again);
    };
  }, [t]);

  React.useEffect(() => {
    if (!here) return;
    let live = true;
    if (signedNow() === null) setBoot({ state: "loading" });
    session()
      .then((signed) => {
        if (live) setBoot({ state: "ready", signed });
      })
      .catch((error: unknown) => {
        if (!live) return;
        const why: Refusal = error instanceof ChatRefused ? error.why : "broken";
        if (why === "signIn") {
          goSignIn();
          return;
        }
        if (why === "off") {
          // Chat is not on this box now (sharing off): back to the overview.
          navigate("/", { replace: true });
          return;
        }
        setBoot({ state: "refused", why, message: error instanceof Error ? error.message : String(error) });
      });
    return () => {
      live = false;
    };
  }, [attempt, here, navigate]);

  // A box still starting chat answers again in a few seconds.
  React.useEffect(() => {
    if (boot.state !== "refused" || boot.why !== "starting") return;
    const handle = setTimeout(() => setAttempt((n) => n + 1), 3000);
    return () => clearTimeout(handle);
  }, [boot]);

  if (boot.state === "ready") {
    return (
      // Calls are held by the shell (calls-shell.tsx), so one outlives this screen.
      <ChatHome signed={boot.signed} />
    );
  }
  if (boot.state === "loading" || boot.why === "starting") {
    return (
      <div className="grid min-h-48 place-items-center text-muted">
        <p className="flex items-center gap-2 text-[13.5px]">
          <Spinner size={16} />
          {boot.state === "loading" ? t("chat.boot.loading") : t("chat.boot.starting")}
        </p>
      </div>
    );
  }
  return (
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
          <Button variant="secondary" onClick={() => setAttempt((n) => n + 1)}>
            {t("chat.boot.retry")}
          </Button>
        )}
      </EmptyContent>
    </Empty>
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

/** The page title carries the unread count while chat is on screen, so a
 *  background tab shows it. */
function useTitle(client: MatrixClient): void {
  const t = useT();
  const tick = useEmitted(client, ROOM_EVENTS);
  React.useEffect(() => {
    const before = document.title;
    return () => {
      document.title = before;
    };
  }, []);
  React.useEffect(() => {
    const total = client.getRooms().reduce((sum, room) => sum + (room.getMyMembership() === KnownMembership.Join ? unread(room).total : 0), 0);
    document.title = total > 0 ? `(${total}) ${t("chat.title")}` : t("chat.title");
  }, [client, tick, t]);
}

/** Spaces as one row of buttons, as Mail's folders are. */
function SpaceRow({ client, space, onSpace, onNewSpace }: { client: MatrixClient; space: string | null; onSpace: (id: string | null) => void; onNewSpace: () => void }) {
  const t = useT();
  useEmitted(client, ROOM_EVENTS);
  const spaces = client.getRooms().filter((room) => isSpace(room) && room.getMyMembership() === KnownMembership.Join);
  const chip = (id: string | null, label: string, icon: React.ReactNode) => {
    const current = space === id;
    return (
      <li key={id ?? "all"} className="shrink-0">
        <Button
          variant={current ? "secondary" : "ghost"}
          size="sm"
          aria-current={current ? "page" : undefined}
          onClick={() => onSpace(id)}
          className={cn("gap-1.5", current && "text-ink")}
        >
          {icon}
          {label}
        </Button>
      </li>
    );
  };
  return (
    <nav aria-label={t("chat.spaces.label")} className="-mx-1 min-w-0 max-w-full overflow-x-auto px-1">
      <ul className="flex gap-1">
        {chip(null, t("chat.spaces.all"), <HugeiconsIcon icon={Home01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />)}
        {spaces.map((room) =>
          chip(room.roomId, roomName(room), <Avatar client={client} id={room.roomId} name={roomName(room)} mxc={room.getMxcAvatarUrl()} size={16} className="size-4 text-[8px]" />),
        )}
        <li className="shrink-0">
          <Button variant="ghost" size="sm" className="gap-1.5 text-muted" onClick={onNewSpace}>
            <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("chat.spaces.new")}
          </Button>
        </li>
      </ul>
    </nav>
  );
}

function ChatHome({ signed }: { signed: Signed }) {
  const t = useT();
  const { client } = signed;
  const me = client.getSafeUserId();
  const [current, setCurrent] = useHashRoom();
  const [space, setSpace] = React.useState<string | null>(null);
  const [filter, setFilter] = React.useState("");
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
  const spaceRoom = space !== null ? client.getRoom(space) : null;
  const unsafe = trust !== "ready" && trust !== "loading";

  return (
    <div className="flex flex-col">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <PaneHeader title={t("shell.nav.chat")} summary={t("chat.summary", { id: me })} />
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {notify === "default" && (
            <Button
              variant="ghost"
              size="icon"
              aria-label={t("chat.notify.ask")}
              title={t("chat.notify.ask")}
              onClick={() => void Notification.requestPermission().then(setNotify)}
            >
              <HugeiconsIcon icon={Notification01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </Button>
          )}
          <Button variant="secondary" className="relative max-sm:px-2.5" aria-label={t("chat.security.title")} onClick={() => setDialog("security")}>
            <HugeiconsIcon icon={SquareLock01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:hidden">{t("chat.security.title")}</span>
            {unsafe && <span className="absolute -top-0.5 -right-0.5 size-2.5 rounded-full border-2 border-surface bg-warn" />}
          </Button>
          <Button onClick={() => setDialog("chat")}>
            <HugeiconsIcon icon={UserAdd01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("chat.list.newChat")}
          </Button>
        </div>
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-2">
        <SpaceRow client={client} space={space} onSpace={setSpace} onNewSpace={() => setDialog("space")} />
        <form className="w-full sm:ml-auto sm:w-64" role="search" onSubmit={(event) => event.preventDefault()}>
          <InputGroup>
            <InputGroupAddon>
              <HugeiconsIcon icon={Search01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </InputGroupAddon>
            <InputGroupInput type="search" value={filter} onChange={(event) => setFilter(event.target.value)} placeholder={t("chat.list.filter")} aria-label={t("chat.list.filter")} />
          </InputGroup>
        </form>
      </div>

      {unsafe && (
        <Alert className="mb-3">
          <HugeiconsIcon icon={SquareLock01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertDescription className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
            <span className="min-w-0 flex-1">{trust === "new" ? t("chat.security.whyNew") : t("chat.security.whyUnverified")}</span>
            <Button variant="secondary" size="sm" onClick={() => setDialog("security")}>
              {trust === "new" ? t("chat.security.bannerNew") : t("chat.security.bannerUnverified")}
            </Button>
          </AlertDescription>
        </Alert>
      )}

      <Card className="flex h-[min(46rem,calc(100dvh-15rem))] min-h-[26rem] overflow-hidden">
        <section
          aria-label={t("chat.list.label")}
          className={cn("min-w-0 flex-1 flex-col md:flex md:w-72 md:flex-none md:border-r md:border-line lg:w-80", room !== null ? "hidden" : "flex")}
        >
          <div className="flex items-center gap-2 border-b border-hair px-4 py-2.5">
            <h2 className="min-w-0 flex-1 truncate text-[15px] font-semibold tracking-tight">{spaceRoom !== null ? roomName(spaceRoom) : t("chat.list.label")}</h2>
          </div>
          {syncState === SyncState.Error || syncState === SyncState.Reconnecting ? (
            <p className="border-b border-hair bg-sunk px-4 py-1.5 text-[12.5px] text-warn">{t("chat.offline")}</p>
          ) : null}
          <RoomList
            client={client}
            current={current}
            space={space}
            filter={filter}
            onOpen={setCurrent}
            onNewRoom={() => setDialog("room")}
            onNewChat={() => setDialog("chat")}
            onJoin={() => setDialog("join")}
          />
        </section>
        {room !== null ? (
          <RoomView key={room.roomId} signed={signed} room={room} onBack={() => setCurrent(null)} onDialog={setDialog} />
        ) : (
          <section className="hidden min-w-0 flex-1 items-center justify-center p-6 md:flex">
            <Empty className="border-0">
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
          </section>
        )}
      </Card>

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
      <section className={cn("flex min-w-0 flex-1 flex-col", panel !== null && "hidden")} aria-label={roomName(room)}>
        <header className="flex h-14 shrink-0 items-center gap-2 border-b border-hair px-2 sm:px-4">
          <Button variant="ghost" size="icon-sm" className="md:hidden" aria-label={t("chat.back")} onClick={onBack}>
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
              {calls.available && (
                <>
                  <Button variant="ghost" size="icon-sm" aria-label={t("chat.call.voice")} title={t("chat.call.voice")} disabled={calls.active !== null} onClick={() => calls.start(room, false)}>
                    <HugeiconsIcon icon={Call02Icon} className="size-4" />
                  </Button>
                  <Button variant="ghost" size="icon-sm" aria-label={t("chat.call.video")} title={t("chat.call.video")} disabled={calls.active !== null} onClick={() => calls.start(room, true)}>
                    <HugeiconsIcon icon={Video01Icon} className="size-4" />
                  </Button>
                </>
              )}
              <Button
                variant="ghost"
                size="icon-sm"
                className={cn(panel !== null && "bg-accent-wash text-accent")}
                aria-label={t("chat.thread.title")}
                title={t("chat.thread.title")}
                aria-pressed={panel !== null}
                onClick={() => setPanel(panel === null ? { root: null } : null)}
              >
                <HugeiconsIcon icon={Comment01Icon} className="size-4" />
              </Button>
              <Button variant="ghost" size="icon-sm" className="max-sm:hidden" aria-label={t("chat.settings.invite")} title={t("chat.settings.invite")} onClick={() => onDialog("invite")}>
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
      </section>
      {panel !== null && (
        <ThreadPanel client={client} room={room} root={panel.root} me={me} actions={actions} onOpen={(root) => setPanel({ root })} onClose={() => setPanel(null)} />
      )}
      <ImageViewer url={image} onClose={() => setImage(null)} />
    </>
  );
}
