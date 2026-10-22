import * as React from "react";
import {
  EventType,
  KnownMembership,
  type MatrixClient,
  Preset,
  type Room,
  Visibility,
} from "matrix-js-sdk";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { Avatar } from "./avatar";
import { directPartner, directRooms, isSpace, localpart, maySend, roomName } from "./model";
import { suggestContacts } from "@/lib/api";
import { boxPeople } from "./session";

const ENCRYPTION = { type: EventType.RoomEncryption, state_key: "", content: { algorithm: "m.megolm.v1.aes-sha2" as const } };

export function failed(t: ReturnType<typeof useT>, error: unknown): void {
  toast.error(t("chat.dialog.failed"), error instanceof Error ? error.message : String(error), { help: "chat" });
}

function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 py-2">
      <div>
        <p className="text-[13.5px] font-medium">{label}</p>
        {hint !== undefined && <p className="text-[12.5px] text-muted">{hint}</p>}
      </div>
      {children}
    </div>
  );
}

/** Put a room in a space, both ways (m.space.child and m.space.parent). */
async function addToSpace(client: MatrixClient, spaceId: string, roomId: string): Promise<void> {
  const via = [client.getDomain() ?? ""];
  await client.sendStateEvent(spaceId, EventType.SpaceChild, { via }, roomId);
  await client.sendStateEvent(roomId, EventType.SpaceParent, { via, canonical: true }, spaceId).catch(() => undefined);
}

export interface NewRoomProps {
  client: MatrixClient;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  space: string | null;
  asSpace?: boolean;
  onCreated: (roomId: string) => void;
}

/** A new room, or a new space: name, topic, who may join, encryption. */
export function NewRoomDialog({ client, open, onOpenChange, space, asSpace = false, onCreated }: NewRoomProps) {
  const t = useT();
  const ids = React.useId();
  const [name, setName] = React.useState("");
  const [topic, setTopic] = React.useState("");
  const [open_, setPublic] = React.useState(false);
  const [encrypted, setEncrypted] = React.useState(true);
  const [busy, setBusy] = React.useState(false);
  React.useEffect(() => {
    if (open) {
      setName("");
      setTopic("");
      setPublic(false);
      setEncrypted(true);
    }
  }, [open]);

  const create = async (): Promise<void> => {
    setBusy(true);
    try {
      const alias = open_ ? name.trim().toLowerCase().replace(/[^a-z0-9._=-]+/g, "-").replace(/^-+|-+$/g, "") : "";
      const { room_id } = await client.createRoom({
        name: name.trim(),
        ...(topic.trim() !== "" ? { topic: topic.trim() } : {}),
        preset: open_ ? Preset.PublicChat : Preset.PrivateChat,
        visibility: open_ ? Visibility.Public : Visibility.Private,
        ...(alias !== "" ? { room_alias_name: alias } : {}),
        ...(asSpace ? { creation_content: { type: "m.space" }, power_level_content_override: { events_default: 100 } } : {}),
        initial_state: !asSpace && encrypted ? [ENCRYPTION] : [],
      });
      if (space !== null) await addToSpace(client, space, room_id);
      onOpenChange(false);
      onCreated(room_id);
    } catch (error) {
      failed(t, error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy="chat-new-room">
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void create();
        }}
      >
        <DialogHeader>
          <DialogTitle id="chat-new-room">{asSpace ? t("chat.newSpace.title") : t("chat.newRoom.title")}</DialogTitle>
          <DialogDescription>{asSpace ? t("chat.newSpace.lead") : t("chat.newRoom.lead")}</DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-3">
          <Field>
            <FieldLabel htmlFor={`${ids}-name`}>{t("chat.newRoom.name")}</FieldLabel>
            <Input id={`${ids}-name`} value={name} onChange={(event) => setName(event.target.value)} required maxLength={120} autoFocus />
          </Field>
          <Field>
            <FieldLabel htmlFor={`${ids}-topic`}>{t("chat.newRoom.topic")}</FieldLabel>
            <Textarea id={`${ids}-topic`} value={topic} onChange={(event) => setTopic(event.target.value)} rows={2} maxLength={500} />
          </Field>
          <Row label={t("chat.newRoom.public")} hint={open_ ? t("chat.newRoom.publicOn") : t("chat.newRoom.publicOff")}>
            <Switch isSelected={open_} onChange={(on) => { setPublic(on); if (on) setEncrypted(false); }} aria-label={t("chat.newRoom.public")} />
          </Row>
          {!asSpace && (
            <Row label={t("chat.newRoom.encrypted")} hint={t("chat.newRoom.encryptedHint")}>
              <Switch isSelected={encrypted} onChange={setEncrypted} aria-label={t("chat.newRoom.encrypted")} />
            </Row>
          )}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
            {t("chat.dialog.cancel")}
          </Button>
          <Button type="submit" disabled={busy || name.trim() === ""}>
            {busy && <Spinner size={14} />}
            {t("chat.dialog.create")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

interface Found {
  user_id: string;
  display_name?: string;
  avatar_url?: string;
}

/** People on this box, then the homeserver's directory (people who share a
 *  room, and other servers' people by full ID), matching a search. */
function PeoplePicker({ client, onPick, exclude, busy }: { client: MatrixClient; onPick: (userId: string) => void; exclude: Set<string>; busy: string | null }) {
  const t = useT();
  const [term, setTerm] = React.useState("");
  const [found, setFound] = React.useState<Found[] | null>(null);
  const [onBox, setOnBox] = React.useState<Found[]>([]);
  React.useEffect(() => {
    let live = true;
    // The person's contacts with a chat address come after the people on
    // this box (LosOS Contacts, src/contacts/).
    const contacts = suggestContacts("", "matrix", { limit: 50 }).catch(() => []);
    void Promise.all([boxPeople().catch(() => []), contacts]).then(([people, book]) => {
      if (!live) return;
      const list: Found[] = people.map((p) => ({ user_id: p.userId, display_name: p.displayName }));
      for (const c of book) {
        if (c.source === "contact" && c.matrixId !== undefined && !list.some((p) => p.user_id === c.matrixId)) {
          list.push({ user_id: c.matrixId, display_name: c.name });
        }
      }
      setOnBox(list);
    });
    return () => {
      live = false;
    };
  }, []);
  React.useEffect(() => {
    let live = true;
    const wanted = term.trim().toLowerCase();
    const handle = setTimeout(() => {
      const local = onBox.filter(
        (p) => !exclude.has(p.user_id) && (wanted === "" || p.user_id.toLowerCase().includes(wanted) || (p.display_name ?? "").toLowerCase().includes(wanted)),
      );
      const directory =
        wanted === ""
          ? Promise.resolve([] as Found[])
          : client.searchUserDirectory({ term: term.trim(), limit: 20 }).then(({ results }) => results as Found[]).catch(() => [] as Found[]);
      void directory.then((results) => {
        if (!live) return;
        const list: Found[] = [...local];
        for (const r of results) if (!exclude.has(r.user_id) && !list.some((p) => p.user_id === r.user_id)) list.push(r);
        // A full Matrix ID is offered even if nothing here knows it.
        if (/^@[^:]+:.+$/.test(term.trim()) && !list.some((r) => r.user_id === term.trim())) list.unshift({ user_id: term.trim() });
        setFound(list);
      });
    }, 150);
    return () => {
      live = false;
      clearTimeout(handle);
    };
  }, [client, term, exclude, onBox]);
  return (
    <div className="flex flex-col gap-2">
      <Input value={term} onChange={(event) => setTerm(event.target.value)} placeholder={t("chat.people.search")} aria-label={t("chat.people.search")} autoFocus />
      <ul className="max-h-72 overflow-y-auto">
        {found === null && (
          <li className="flex justify-center p-4">
            <Spinner size={16} />
          </li>
        )}
        {found?.length === 0 && <li className="p-2 text-[13px] text-muted">{t("chat.people.none")}</li>}
        {found?.map((person) => (
          <li key={person.user_id}>
            <button
              type="button"
              disabled={busy !== null}
              onClick={() => onPick(person.user_id)}
              className="flex w-full items-center gap-2.5 rounded-control p-2 text-left hover:bg-sunk disabled:opacity-45"
            >
              <Avatar client={client} id={person.user_id} name={person.display_name ?? localpart(person.user_id)} mxc={person.avatar_url} />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[13.5px] font-medium">{person.display_name ?? localpart(person.user_id)}</span>
                <span className="block truncate text-[12px] text-muted">{person.user_id}</span>
              </span>
              {busy === person.user_id && <Spinner size={14} />}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** The direct room with `userId`, made if there is none yet. */
export async function openDirect(client: MatrixClient, userId: string): Promise<string> {
  const direct = directRooms(client);
  for (const room of client.getRooms()) {
    if (room.getMyMembership() === KnownMembership.Join && directPartner(client, room, direct) === userId) return room.roomId;
  }
  const { room_id } = await client.createRoom({
    is_direct: true,
    invite: [userId],
    preset: Preset.TrustedPrivateChat,
    initial_state: [ENCRYPTION],
  });
  const content = { ...((client.getAccountData(EventType.Direct)?.getContent() ?? {}) as Record<string, string[]>) };
  content[userId] = [...(content[userId] ?? []), room_id];
  await client.setAccountData(EventType.Direct, content);
  return room_id;
}

export function StartChatDialog({ client, open, onOpenChange, onCreated }: Omit<NewRoomProps, "space">) {
  const t = useT();
  const [busy, setBusy] = React.useState<string | null>(null);
  const exclude = React.useMemo(() => new Set([client.getSafeUserId()]), [client]);
  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy="chat-start">
      <DialogHeader>
        <DialogTitle id="chat-start">{t("chat.start.title")}</DialogTitle>
        <DialogDescription>{t("chat.start.lead")}</DialogDescription>
      </DialogHeader>
      <DialogBody>
        {open && (
          <PeoplePicker
            client={client}
            exclude={exclude}
            busy={busy}
            onPick={(userId) => {
              setBusy(userId);
              openDirect(client, userId)
                .then((roomId) => {
                  onOpenChange(false);
                  onCreated(roomId);
                })
                .catch((error: unknown) => failed(t, error))
                .finally(() => setBusy(null));
            }}
          />
        )}
      </DialogBody>
    </Dialog>
  );
}

export function InviteDialog({ client, room, open, onOpenChange }: { client: MatrixClient; room: Room; open: boolean; onOpenChange: (open: boolean) => void }) {
  const t = useT();
  const [busy, setBusy] = React.useState<string | null>(null);
  const exclude = React.useMemo(
    () => new Set(room.getMembers().filter((m) => m.membership === KnownMembership.Join || m.membership === KnownMembership.Invite).map((m) => m.userId)),
    [room, open],
  );
  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy="chat-invite">
      <DialogHeader>
        <DialogTitle id="chat-invite">{t("chat.invite.title", { room: roomName(room) })}</DialogTitle>
      </DialogHeader>
      <DialogBody>
        {open && (
          <PeoplePicker
            client={client}
            exclude={exclude}
            busy={busy}
            onPick={(userId) => {
              setBusy(userId);
              client
                .invite(room.roomId, userId)
                .then(() => {
                  toast.done(t("chat.invite.sent", { name: localpart(userId) }));
                  onOpenChange(false);
                })
                .catch((error: unknown) => failed(t, error))
                .finally(() => setBusy(null));
            }}
          />
        )}
      </DialogBody>
    </Dialog>
  );
}

interface PublicRoom {
  room_id: string;
  name?: string;
  topic?: string;
  canonical_alias?: string;
  num_joined_members: number;
  avatar_url?: string;
  room_type?: string;
}

/** Join a room: the box's public rooms, or any address or ID. */
export function JoinDialog({ client, open, onOpenChange, onJoined, space }: { client: MatrixClient; open: boolean; onOpenChange: (open: boolean) => void; onJoined: (roomId: string) => void; space: string | null }) {
  const t = useT();
  const [address, setAddress] = React.useState("");
  const [rooms, setRooms] = React.useState<PublicRoom[] | null>(null);
  const [busy, setBusy] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!open) return;
    setRooms(null);
    setAddress("");
    let live = true;
    const load = space !== null
      ? client.getRoomHierarchy(space, 50, 1).then((answer) => answer.rooms.filter((r) => r.room_id !== space) as unknown as PublicRoom[])
      : client.publicRooms({ limit: 50 }).then((answer) => answer.chunk as PublicRoom[]);
    load.then((list) => live && setRooms(list)).catch(() => live && setRooms([]));
    return () => {
      live = false;
    };
  }, [client, open, space]);

  const join = (idOrAlias: string): void => {
    setBusy(idOrAlias);
    client
      .joinRoom(idOrAlias)
      .then((room) => {
        onOpenChange(false);
        onJoined(room.roomId);
      })
      .catch((error: unknown) => failed(t, error))
      .finally(() => setBusy(null));
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy="chat-join" dialogClassName="w-[min(32rem,calc(100vw-2rem))]">
      <DialogHeader>
        <DialogTitle id="chat-join">{space !== null ? t("chat.join.spaceTitle") : t("chat.join.title")}</DialogTitle>
        <DialogDescription>{t("chat.join.lead")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        <form
          className="flex gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            if (address.trim() !== "") join(address.trim());
          }}
        >
          <Input value={address} onChange={(event) => setAddress(event.target.value)} placeholder="#room:server" aria-label={t("chat.join.address")} />
          <Button type="submit" variant="secondary" disabled={address.trim() === "" || busy !== null}>
            {t("chat.join.go")}
          </Button>
        </form>
        <ul className="max-h-80 overflow-y-auto">
          {rooms === null && (
            <li className="flex justify-center p-4">
              <Spinner size={16} />
            </li>
          )}
          {rooms?.length === 0 && <li className="p-2 text-[13px] text-muted">{t("chat.join.none")}</li>}
          {rooms?.map((room) => {
            const joined = client.getRoom(room.room_id)?.getMyMembership() === KnownMembership.Join;
            const label = room.name ?? room.canonical_alias ?? room.room_id;
            return (
              <li key={room.room_id} className="flex items-center gap-2.5 rounded-control p-2 hover:bg-sunk">
                <Avatar client={client} id={room.room_id} name={label} mxc={room.avatar_url} />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[13.5px] font-medium">{label}</span>
                  <span className="block truncate text-[12px] text-muted">
                    {t("chat.join.members", { count: room.num_joined_members })}
                    {room.topic !== undefined && room.topic !== "" ? ` · ${room.topic}` : ""}
                  </span>
                </span>
                <Button size="sm" variant={joined ? "ghost" : "secondary"} disabled={busy !== null} onClick={() => (joined ? (onOpenChange(false), onJoined(room.room_id)) : join(room.room_id))}>
                  {busy === room.room_id && <Spinner size={12} />}
                  {joined ? t("chat.join.open") : t("chat.join.join")}
                </Button>
              </li>
            );
          })}
        </ul>
      </DialogBody>
    </Dialog>
  );
}

/** A room's name, topic, encryption, people and spaces; and leaving it. */
export function RoomSettingsDialog({ client, room, open, onOpenChange, onLeft, onInvite }: { client: MatrixClient; room: Room; open: boolean; onOpenChange: (open: boolean) => void; onLeft: () => void; onInvite: () => void }) {
  const t = useT();
  const ids = React.useId();
  const me = client.getSafeUserId();
  const [name, setName] = React.useState(room.name);
  const [topic, setTopic] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const encrypted = room.hasEncryptionStateEvent();
  React.useEffect(() => {
    if (!open) return;
    setName(room.currentState.getStateEvents(EventType.RoomName, "")?.getContent()["name"] ?? "");
    setTopic(room.currentState.getStateEvents(EventType.RoomTopic, "")?.getContent()["topic"] ?? "");
  }, [open, room]);

  const myLevel = room.getMember(me)?.powerLevel ?? 0;
  const canKick = room.currentState.hasSufficientPowerLevelFor("kick", myLevel);
  const canBan = room.currentState.hasSufficientPowerLevelFor("ban", myLevel);
  const members = room.getMembers().filter((m) => m.membership === KnownMembership.Join || m.membership === KnownMembership.Invite);
  const spaces = client.getRooms().filter((r) => isSpace(r) && r.getMyMembership() === KnownMembership.Join && maySend(r, me, EventType.SpaceChild));

  const run = (job: () => Promise<unknown>, done?: string): void => {
    setBusy(true);
    job()
      .then(() => done !== undefined && toast.done(done))
      .catch((error: unknown) => failed(t, error))
      .finally(() => setBusy(false));
  };

  const save = (): void =>
    run(async () => {
      const oldName = room.currentState.getStateEvents(EventType.RoomName, "")?.getContent()["name"] ?? "";
      const oldTopic = room.currentState.getStateEvents(EventType.RoomTopic, "")?.getContent()["topic"] ?? "";
      if (name.trim() !== oldName) await client.setRoomName(room.roomId, name.trim());
      if (topic.trim() !== oldTopic) await client.setRoomTopic(room.roomId, topic.trim());
    }, t("chat.settings.saved"));

  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy="chat-settings" dialogClassName="w-[min(34rem,calc(100vw-2rem))]">
      <DialogHeader>
        <DialogTitle id="chat-settings">{t("chat.settings.title", { room: roomName(room) })}</DialogTitle>
        <DialogDescription className="font-mono text-[12px] break-all">{room.getCanonicalAlias() ?? room.roomId}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        <Field>
          <FieldLabel htmlFor={`${ids}-name`}>{t("chat.newRoom.name")}</FieldLabel>
          <Input id={`${ids}-name`} value={name} onChange={(event) => setName(event.target.value)} disabled={!maySend(room, me, EventType.RoomName)} />
        </Field>
        <Field>
          <FieldLabel htmlFor={`${ids}-topic`}>{t("chat.newRoom.topic")}</FieldLabel>
          <Textarea id={`${ids}-topic`} value={topic} onChange={(event) => setTopic(event.target.value)} rows={2} disabled={!maySend(room, me, EventType.RoomTopic)} />
        </Field>
        <Row label={t("chat.newRoom.encrypted")} hint={encrypted ? t("chat.settings.encryptedOn") : t("chat.settings.encryptedOff")}>
          <Switch
            isSelected={encrypted}
            isDisabled={encrypted || busy || !maySend(room, me, EventType.RoomEncryption)}
            onChange={(on) => {
              if (on && window.confirm(t("chat.settings.encryptConfirm"))) run(() => client.sendStateEvent(room.roomId, EventType.RoomEncryption, ENCRYPTION.content, ""));
            }}
            aria-label={t("chat.newRoom.encrypted")}
          />
        </Row>
        {spaces.length > 0 && (
          <div>
            <p className="pb-1 text-[13.5px] font-medium">{t("chat.settings.spaces")}</p>
            <div className="flex flex-wrap gap-1.5">
              {spaces.map((space) => {
                const inIt = space.currentState.getStateEvents(EventType.SpaceChild, room.roomId)?.getContent()["via"] !== undefined;
                return (
                  <Button
                    key={space.roomId}
                    size="xs"
                    variant={inIt ? "outline" : "secondary"}
                    disabled={busy}
                    aria-pressed={inIt}
                    onClick={() =>
                      run(() =>
                        inIt ? client.sendStateEvent(space.roomId, EventType.SpaceChild, {}, room.roomId) : addToSpace(client, space.roomId, room.roomId),
                      )
                    }
                  >
                    {space.name}
                  </Button>
                );
              })}
            </div>
          </div>
        )}
        <div>
          <div className="flex items-center justify-between pb-1">
            <p className="text-[13.5px] font-medium">{t("chat.settings.people", { count: members.length })}</p>
            <Button size="xs" variant="secondary" onClick={onInvite}>
              {t("chat.settings.invite")}
            </Button>
          </div>
          <ul className="max-h-56 overflow-y-auto rounded-control border border-hair">
            {members.map((member) => (
              <li key={member.userId} className="flex items-center gap-2.5 px-2 py-1.5">
                <Avatar client={client} id={member.userId} name={member.name} mxc={member.getMxcAvatarUrl()} size={24} />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[13px] font-medium">{member.name}</span>
                  <span className="block truncate text-[11.5px] text-muted">
                    {member.userId}
                    {member.membership === KnownMembership.Invite ? ` · ${t("chat.settings.invited")}` : ""}
                    {member.powerLevel >= 100 ? ` · ${t("chat.settings.admin")}` : member.powerLevel >= 50 ? ` · ${t("chat.settings.moderator")}` : ""}
                  </span>
                </span>
                {member.userId !== me && member.powerLevel < myLevel && (
                  <span className="flex gap-1">
                    {canKick && (
                      <Button size="xs" variant="ghost" disabled={busy} onClick={() => run(() => client.kick(room.roomId, member.userId))}>
                        {t("chat.settings.remove")}
                      </Button>
                    )}
                    {canBan && (
                      <Button size="xs" variant="ghost" className={cn("hover:text-crit")} disabled={busy} onClick={() => window.confirm(t("chat.settings.banConfirm", { name: member.name })) && run(() => client.ban(room.roomId, member.userId))}>
                        {t("chat.settings.ban")}
                      </Button>
                    )}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </div>
      </DialogBody>
      <DialogFooter className="justify-between">
        <Button
          variant="ghost"
          className="hover:text-crit"
          disabled={busy}
          onClick={() => {
            if (!window.confirm(t("chat.settings.leaveConfirm", { room: roomName(room) }))) return;
            run(async () => {
              await client.leave(room.roomId);
              await client.forget(room.roomId).catch(() => undefined);
              onOpenChange(false);
              onLeft();
            });
          }}
        >
          {t("chat.settings.leave")}
        </Button>
        <span className="flex gap-2">
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            {t("chat.dialog.close")}
          </Button>
          <Button onClick={save} disabled={busy}>
            {t("chat.dialog.save")}
          </Button>
        </span>
      </DialogFooter>
    </Dialog>
  );
}

/** A picture at full size. */
export function ImageViewer({ url, onClose }: { url: string | null; onClose: () => void }) {
  const t = useT();
  return (
    <Dialog open={url !== null} onOpenChange={(open) => !open && onClose()} dialogClassName="w-auto max-w-[calc(100vw-2rem)]" aria-label={t("chat.media.viewer")}>
      {url !== null && <img src={url} alt="" className="block max-h-[calc(100dvh-6rem)] max-w-full object-contain" />}
    </Dialog>
  );
}

