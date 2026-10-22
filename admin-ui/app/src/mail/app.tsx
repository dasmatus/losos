/* LosOS Mail: a screen of the admin UI at /mail, inside its top bar and
 * sidebar like every other pane, over lososd's /api/mail (backend/src/mail.rs).
 * On a wide screen one card holds three columns: the folders as a tree, the
 * open folder's messages, and the open message. Narrower, the folders are a
 * row of buttons above the card; on a phone the card shows the list or the
 * message, with a way back to the list. A person's own folders (made,
 * renamed, nested, deleted) and their filters are the mail plugin's
 * (backend/plugins/mail), in ./folders.tsx and ./filters.tsx. Messages and
 * folders drag onto folders; every folder and message has a right-click
 * menu.
 *
 * A message's HTML never enters this page. It is a document of its own at
 * /api/mail/messages/<id>/html, shown in an <iframe sandbox> without
 * allow-scripts or allow-same-origin, under a policy that loads nothing off
 * the message (modules/containers.nix). The plain text, when there is one,
 * is the default view; HTML is one button away.
 *
 * OpenPGP mail is the exception: the box cannot read it, so the page fetches
 * the raw message, opens it in the browser (./pgp.ts, ./encryption.tsx) and
 * shows its text and attachments, never its HTML. */

import { Masked, useDetailsCheck } from "@/components/details";
import { openReport } from "@/components/report-dialog";
import { MailHtml } from "./pictures";
import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Alert01Icon,
  ArrowLeft01Icon,
  Attachment01Icon,
  Delete02Icon,
  FileEditIcon,
  Flag02Icon,
  FavouriteIcon,
  Folder01Icon,
  InboxIcon,
  Mail01Icon,
  MailOpen01Icon,
  MailReply01Icon,
  MailReplyAllIcon,
  MailSend01Icon,
  PencilEdit02Icon,
  Search01Icon,
  SentIcon,
  SpamIcon,
  ArrowTurnForwardIcon,
  Archive02Icon,
  CheckmarkBadge01Icon,
  Key01Icon,
  SquareLock02Icon,
  FilterIcon,
  FolderAddIcon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { PersonAvatar, type PersonLike } from "@/components/person";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { Dialog, DialogBody, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { toast } from "@/components/ui/toast";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { PaneHeader } from "@/screens/settings/rows";
import {
  ApiError,
  changeMailMessage,
  createMailFolder,
  deleteMailFolder,
  deleteMailMessage,
  findMailKeys,
  getContacts,
  getMail,
  getMailMessage,
  getMailMessages,
  getMailRaw,
  getProfiles,
  loadMe,
  mailFile,
  sendMail,
  signInUrl,
  updateMailFolder,
  type MailAddress,
  type MailAttachment,
  type MailDraft,
  type MailFilter,
  type MailFolder,
  type MailHome,
  type MailMessage,
  type MailPage,
  type MailRole,
  type MailSummary,
  sharedApp,
} from "@/lib/api";
import { useLocale, useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { contextMenu, type MenuBuilder, type MenuItem } from "@/lib/context-menu";
import { PeopleField } from "@/contacts/suggest";
import { readMailIntent, withoutMailIntent } from "@/lib/intents";
import { cn } from "@/lib/utils";
import {
  KeyDialog,
  SecurityNote,
  sourceName,
  UnlockDialog,
  useOwnKey,
  useRecipients,
  useUnlocked,
  type OwnKey,
  type Reading,
} from "./encryption";
import { formatBytes } from "@/lib/apps";
import { CannotOpen, openMail, seal, unlockedKey, type Opened } from "./pgp";
import { InviteCard, isCalendarFile } from "@/calendar/invite";
import { ComposeAttachments, ReceivedAttachment, useAttached, useCloud } from "./attachments";
import { DeleteFolderDialog, FolderDialog, explainFolder, tree, within, type Placed } from "./folders";
import { FiltersDialog, blankFilter } from "./filters";
import { PersonLinks } from "./together";

/* How often the open folder and the counts are read again. */
const REFRESH_MS = 30_000;

const ROLE_ICON: Record<MailRole, IconSvgElement> = {
  inbox: InboxIcon,
  drafts: FileEditIcon,
  sent: SentIcon,
  archive: Archive02Icon,
  junk: SpamIcon,
  trash: Delete02Icon,
};

const ROLE_NAME: Record<MailRole, MessageKey> = {
  inbox: "mail.folder.inbox",
  drafts: "mail.folder.drafts",
  sent: "mail.folder.sent",
  archive: "mail.folder.archive",
  junk: "mail.folder.junk",
  trash: "mail.folder.trash",
};

type Loading =
  | { kind: "loading" }
  | { kind: "signIn" }
  | { kind: "missing" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; home: MailHome };

/* Which half of the card a phone shows. */
type Column = "list" | "message";

/* How the composer opens: a draft, and the files it starts with (LosOS
 * Cloud paths from another app's link, a forwarded message's own). */
type ComposeStart = MailDraft & { cloud?: string[]; carry?: MailAttachment[] };

/* What a drag carries: a message, or one of the person's own folders. */
const DRAG_MESSAGE = "text/x-losos-mail-message";
const DRAG_FOLDER = "text/x-losos-mail-folder";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/* A request that failed, said in the page's alert, with the one button that
 * tries it again. */
function Failed({ text, onRetry, className }: { text: string; onRetry?: () => void; className?: string }) {
  const t = useT();
  return (
    <Alert variant="crit" role="alert" className={className}>
      <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <AlertDescription>
        <p>{text}</p>
        {onRetry !== undefined && (
          <Button size="sm" variant="secondary" className="mt-2" onClick={onRetry}>
            {t("mail.retry")}
          </Button>
        )}
      </AlertDescription>
    </Alert>
  );
}

/* Whom an address draws as: a person on the box with their picture and
 * colour (GET /api/profiles), anyone else by their initials. */
type Faces = (address: MailAddress) => PersonLike;

function useFaces(home: MailHome): Faces {
  const [byAddress, setByAddress] = React.useState<ReadonlyMap<string, PersonLike>>(new Map());
  React.useEffect(() => {
    const controller = new AbortController();
    // The person's contacts name and picture whoever is in them; people on
    // the box, with their own profiles, come after and win.
    const book = getContacts({ signal: controller.signal })
      .then(({ contacts }) => contacts)
      .catch(() => []);
    void Promise.all([getProfiles({ signal: controller.signal }), book])
      .then(([{ people }, contacts]) => {
        const byName = new Map(people.map((p) => [p.name, p]));
        const next = new Map<string, PersonLike>();
        for (const c of contacts) {
          for (const e of c.emails ?? []) {
            next.set(e.value.toLowerCase(), { name: `contact:${c.id}`, displayName: c.name, picture: c.photo ?? null });
          }
        }
        for (const p of home.people) {
          const profile = byName.get(p.name);
          if (profile !== undefined) next.set(p.address.toLowerCase(), profile);
        }
        setByAddress(next);
      })
      .catch(() => {
        /* initials, then */
      });
    return () => controller.abort();
  }, [home.people]);
  return React.useCallback(
    (a: MailAddress) => byAddress.get(a.email.toLowerCase()) ?? { name: a.email, displayName: a.name ?? a.email },
    [byAddress],
  );
}

function folderName(folder: MailFolder, t: ReturnType<typeof useT>): string {
  return folder.role !== null ? t(ROLE_NAME[folder.role]) : folder.name;
}

function who(addresses: readonly MailAddress[]): string {
  return addresses.map((a) => a.name ?? a.email).join(", ");
}

function full(address: MailAddress): string {
  return address.name !== null ? `${address.name} <${address.email}>` : address.email;
}

/* Today: the time. This year: day and month. Before: the date. */
function when(iso: string, locale: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const now = new Date();
  if (date.toDateString() === now.toDateString()) {
    return new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(date);
  }
  if (date.getFullYear() === now.getFullYear()) {
    return new Intl.DateTimeFormat(locale, { day: "numeric", month: "short" }).format(date);
  }
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(date);
}

/* The quoted text under a reply or a forward. */
function quoted(m: MailMessage, text: string, locale: string, t: ReturnType<typeof useT>): string {
  const head = t("mail.compose.wrote", {
    when: new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(new Date(m.receivedAt)),
    who: who(m.from),
  });
  const body = (text !== "" ? text : m.preview)
    .split("\n")
    .map((line) => `> ${line}`)
    .join("\n");
  return `\n\n${head}\n${body}`;
}

function withPrefix(prefix: string, subject: string): string {
  return subject.toLowerCase().startsWith(prefix.toLowerCase()) ? subject : `${prefix} ${subject}`.trim();
}

export default function MailScreen() {
  const t = useT();
  const [state, setState] = React.useState<Loading>({ kind: "loading" });

  const load = React.useCallback(async () => {
    try {
      const me = await loadMe();
      if (me === null) {
        setState({ kind: "signIn" });
        return;
      }
      setState({ kind: "ready", home: await getMail() });
    } catch (error) {
      if (error instanceof ApiError && error.status === 404) setState({ kind: "missing" });
      else if (error instanceof ApiError && error.status === 409) setState({ kind: "signIn" });
      else setState({ kind: "failed", message: message(error) });
    }
  }, []);

  React.useEffect(() => {
    void load();
  }, [load]);

  if (state.kind === "ready") {
    return <Mailbox home={state.home} onHome={(home) => setState({ kind: "ready", home })} />;
  }
  if (state.kind === "loading") {
    return (
      <div className="grid min-h-48 place-items-center text-muted">
        <Spinner />
      </div>
    );
  }
  return (
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <HugeiconsIcon icon={Mail01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </EmptyMedia>
        <EmptyTitle>{t("mail.title")}</EmptyTitle>
        <EmptyDescription>
          {state.kind === "signIn"
            ? t("mail.signIn.text")
            : state.kind === "missing"
              ? t("mail.missing")
              : t("mail.failed", { detail: state.message })}
        </EmptyDescription>
      </EmptyHeader>
      {state.kind === "signIn" && (
        <Button onClick={() => window.location.assign(signInUrl())}>{t("mail.signIn.button")}</Button>
      )}
      {state.kind === "failed" && (
        <Button variant="secondary" onClick={() => void load()}>
          {t("mail.retry")}
        </Button>
      )}
    </Empty>
  );
}

function Mailbox({ home, onHome }: { home: MailHome; onHome: (home: MailHome) => void }) {
  const t = useT();
  const locale = useLocale();
  const inbox = home.mailboxes.find((f) => f.role === "inbox") ?? home.mailboxes[0];
  const [folderId, setFolderId] = React.useState(inbox?.id ?? "");
  const [query, setQuery] = React.useState("");
  const [search, setSearch] = React.useState("");
  const [page, setPage] = React.useState<MailPage | null>(null);
  const [listError, setListError] = React.useState<string | null>(null);
  const [openId, setOpenId] = React.useState<string | null>(null);
  const [open, setOpen] = React.useState<MailMessage | null>(null);
  const [openError, setOpenError] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<ComposeStart | null>(null);
  const [column, setColumn] = React.useState<Column>("list");
  const [own, setOwn] = useOwnKey();
  const [keys, setKeys] = React.useState(false);
  // What an opened OpenPGP message said, for quoting it in a reply.
  const [opened, setOpened] = React.useState<{ id: string; text: string } | null>(null);
  const [folderDialog, setFolderDialog] = React.useState<
    { kind: "new"; parentId: string | null } | { kind: "rename"; folder: MailFolder } | null
  >(null);
  const [deleting, setDeleting] = React.useState<MailFolder | null>(null);
  const [filters, setFilters] = React.useState<{ start: MailFilter | null } | null>(null);
  /* The folder a drag is over ("top" for the tree's empty space), and the
   * folder being dragged, which a dragover cannot read from the drag. */
  const [dropAt, setDropAt] = React.useState<string | null>(null);
  const draggedFolder = React.useRef<string | null>(null);

  // Another app's link (lib/intents.ts composeMailHref): the composer,
  // filled in, once; the address loses the parameters so a reload does
  // not open it again.
  React.useEffect(() => {
    const search = new URLSearchParams(window.location.search);
    const intent = readMailIntent(search);
    if (intent === null) return;
    setDraft({ to: intent.to, cc: intent.cc, subject: intent.subject, text: intent.body, cloud: intent.attach });
    window.history.replaceState(window.history.state, "", window.location.pathname + withoutMailIntent(search) + window.location.hash);
  }, []);

  const folder = home.mailboxes.find((f) => f.id === folderId) ?? inbox;
  const placed = React.useMemo(() => tree(home.mailboxes), [home.mailboxes]);

  const refreshHome = React.useCallback(async () => {
    try {
      onHome(await getMail());
    } catch {
      // The next tick tries again; the page keeps what it has.
    }
  }, [onHome]);

  const refreshList = React.useCallback(
    async (more = false) => {
      if (folderId === "") return;
      try {
        const next = await getMailMessages(folderId, search, more && page !== null ? page.messages.length : 0);
        setPage((prev) =>
          more && prev !== null ? { ...next, messages: [...prev.messages, ...next.messages] } : next,
        );
        setListError(null);
      } catch (error) {
        setListError(message(error));
      }
    },
    [folderId, search, page],
  );

  // A new folder or search: the first page, nothing open.
  React.useEffect(() => {
    setPage(null);
    void (async () => {
      try {
        setPage(await getMailMessages(folderId, search, 0));
        setListError(null);
      } catch (error) {
        setListError(message(error));
      }
    })();
  }, [folderId, search]);

  React.useEffect(() => {
    const timer = window.setInterval(() => {
      void refreshHome();
      void refreshList();
    }, REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [refreshHome, refreshList]);

  const openMessage = async (id: string) => {
    setOpenId(id);
    setOpen(null);
    setOpenError(null);
    setColumn("message");
    try {
      const m = await getMailMessage(id);
      setOpen(m);
      setPage((prev) =>
        prev === null
          ? prev
          : { ...prev, messages: prev.messages.map((s) => (s.id === id ? { ...s, seen: true } : s)) },
      );
      void refreshHome();
    } catch (error) {
      setOpenError(message(error));
    }
  };

  const act = async (work: () => Promise<unknown>, done: MessageKey, close = false) => {
    try {
      await work();
      toast.done(t(done));
      if (close) {
        setOpenId(null);
        setOpen(null);
        setColumn("list");
      }
      await Promise.all([refreshHome(), refreshList()]);
    } catch (error) {
      toast.error(t("mail.actionFailed"), message(error), { help: "mail" });
    }
  };

  const textOf = (m: MailMessage) => (opened?.id === m.id ? opened.text : m.pgp === "encrypted" ? "" : m.text);

  const openFolder = (id: string) => {
    setFolderId(id);
    setOpenId(null);
    setOpen(null);
    setColumn("list");
  };

  /* A folder change: said in a toast, then the folders read again. */
  const folderAct = async (work: () => Promise<unknown>, done: string) => {
    try {
      await work();
      toast.done(done);
    } catch (error) {
      toast.error(t("mail.actionFailed"), explainFolder(error, t), { help: "mail" });
    }
    await refreshHome();
  };

  const moveFolder = (id: string, parentId: string | null) =>
    void folderAct(() => updateMailFolder(id, { parentId }), t("mail.done.folderMoved"));

  const moveMessage = (id: string, to: string) =>
    void act(() => changeMailMessage(id, { mailbox: to }), "mail.done.moved", id === openId);

  /* Where a drag may land: a message on any folder but its own and Drafts;
   * a folder on the top level or on any folder outside itself. */
  const canDrop = (target: MailFolder | null, types: readonly string[]): boolean => {
    if (types.includes(DRAG_MESSAGE)) return target !== null && target.id !== folder?.id && target.role !== "drafts";
    const dragged = draggedFolder.current;
    if (!types.includes(DRAG_FOLDER) || dragged === null) return false;
    const parent = home.mailboxes.find((f) => f.id === dragged)?.parentId ?? null;
    if (target === null) return parent !== null;
    return target.id !== parent && !within(home.mailboxes, target.id, dragged);
  };

  const dropProps = (target: MailFolder | null) => ({
    onDragOver: (event: React.DragEvent) => {
      if (!canDrop(target, [...event.dataTransfer.types])) return;
      event.preventDefault();
      event.stopPropagation();
      event.dataTransfer.dropEffect = "move";
      setDropAt(target?.id ?? "top");
    },
    onDragLeave: (event: React.DragEvent) => {
      if (!(event.currentTarget as Node).contains(event.relatedTarget as Node | null)) setDropAt(null);
    },
    onDrop: (event: React.DragEvent) => {
      if (!canDrop(target, [...event.dataTransfer.types])) return;
      event.preventDefault();
      event.stopPropagation();
      setDropAt(null);
      const message = event.dataTransfer.getData(DRAG_MESSAGE);
      const dragged = event.dataTransfer.getData(DRAG_FOLDER);
      if (message !== "" && target !== null) moveMessage(message, target.id);
      else if (dragged !== "") moveFolder(dragged, target?.id ?? null);
    },
  });

  const folderDrag = (f: MailFolder) =>
    f.role !== null
      ? {}
      : {
          draggable: true,
          onDragStart: (event: React.DragEvent) => {
            draggedFolder.current = f.id;
            event.dataTransfer.setData(DRAG_FOLDER, f.id);
            event.dataTransfer.effectAllowed = "move";
          },
          onDragEnd: () => {
            draggedFolder.current = null;
            setDropAt(null);
          },
        };

  /* A right click on a folder: open it, make one inside it, and for the
   * person's own folders rename, move and delete. */
  const folderMenu =
    (f: MailFolder): MenuBuilder =>
    () => {
      const own = f.role === null;
      const moveTargets: MenuItem[] = [
        {
          id: "move-top",
          label: t("mail.folder.top"),
          hidden: f.parentId === null,
          onSelect: () => moveFolder(f.id, null),
        },
        ...placed
          .filter(({ folder: x }) => x.id !== f.parentId && !within(home.mailboxes, x.id, f.id))
          .map(({ folder: x }) => ({
            id: `move-${x.id}`,
            label: folderName(x, t),
            icon: x.role !== null ? ROLE_ICON[x.role] : Folder01Icon,
            onSelect: () => moveFolder(f.id, x.id),
          })),
      ];
      return {
        label: folderName(f, t),
        sections: [
          [
            { id: "open", label: t("menu.open"), icon: MailOpen01Icon, onSelect: () => openFolder(f.id) },
            {
              id: "new-inside",
              label: t("mail.folder.newInside"),
              icon: FolderAddIcon,
              onSelect: () => setFolderDialog({ kind: "new", parentId: f.id }),
            },
          ],
          [
            {
              id: "rename",
              label: t("mail.folder.rename"),
              icon: PencilEdit02Icon,
              hidden: !own,
              onSelect: () => setFolderDialog({ kind: "rename", folder: f }),
            },
            {
              kind: "submenu",
              id: "move",
              label: t("mail.folder.moveTo"),
              icon: Folder01Icon,
              hidden: !own,
              items: moveTargets,
            },
          ],
          [
            {
              id: "delete",
              label: t("mail.folder.delete"),
              icon: Delete02Icon,
              danger: true,
              hidden: !own,
              onSelect: () => setDeleting(f),
            },
          ],
        ],
      };
    };

  /* One folder, in the tree or in the row. */
  const folderItem = ({ folder: f, depth }: Placed, layout: "tree" | "row") => {
    const current = f.id === folderId;
    const icon = f.role !== null ? ROLE_ICON[f.role] : Folder01Icon;
    return (
      <li key={f.id} ref={contextMenu(folderMenu(f))} className={layout === "row" ? "shrink-0" : undefined}>
        <button
          type="button"
          aria-current={current ? "page" : undefined}
          data-folder={f.name}
          onClick={() => openFolder(f.id)}
          {...folderDrag(f)}
          {...dropProps(f)}
          // A CSSOM write, which the admin CSP allows.
          style={layout === "tree" ? { paddingInlineStart: `${0.5 + depth * 0.875}rem` } : undefined}
          className={cn(
            "flex items-center gap-2 rounded-control text-left text-[13px] outline-none transition-colors duration-150",
            "focus-visible:ring-2 focus-visible:ring-accent/40",
            layout === "tree" ? "w-full py-1.5 pr-2" : "h-8 px-2.5",
            current ? "bg-accent-wash font-medium text-ink" : "text-muted hover:bg-sunk hover:text-ink",
            dropAt === f.id && "bg-accent-wash text-ink ring-2 ring-accent/50",
          )}
        >
          <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="shrink-0" />
          <span className="min-w-0 flex-1 truncate">{folderName(f, t)}</span>
          {f.unread > 0 && (
            <Badge variant="default" className="tabular-nums">
              {f.unread}
            </Badge>
          )}
        </button>
      </li>
    );
  };

  const reply = (m: MailMessage, all: boolean) => {
    const to = (m.replyTo.length > 0 ? m.replyTo : m.from).map((a) => a.email);
    const cc = all
      ? [...m.to, ...m.cc].map((a) => a.email).filter((e) => e !== home.address && !to.includes(e))
      : [];
    setDraft({
      to,
      cc,
      subject: withPrefix(t("mail.compose.re"), m.subject),
      text: quoted(m, textOf(m), locale, t),
      inReplyTo: m.id,
    });
  };

  const forward = (m: MailMessage) => {
    setDraft({
      to: [],
      cc: [],
      subject: withPrefix(t("mail.compose.fwd"), m.subject),
      text: quoted(m, textOf(m), locale, t),
      // An encrypted message's files are inside it; the reader offers them.
      carry: isPgp(m) ? undefined : m.attachments,
    });
  };

  const faces = useFaces(home);
  const trash = home.mailboxes.find((f) => f.role === "trash");
  const archive = home.mailboxes.find((f) => f.role === "archive");

  /* A right click on a message in the list: what the reader's buttons do,
   * without opening it first. Replying needs the whole message, so it is
   * fetched then. */
  const withMessage = (id: string, then: (m: MailMessage) => void) =>
    void getMailMessage(id).then(then, (error: unknown) =>
      toast.error(t("mail.actionFailed"), message(error), { help: "mail" }),
    );
  const rowMenu =
    (m: MailSummary): MenuBuilder =>
    () => {
      const close = m.id === openId;
      const elsewhere = placed.map((p) => p.folder).filter((f) => f.id !== folder?.id && f.role !== "drafts");
      return {
        label: m.subject || t("mail.noSubject"),
        sections: [
          [
            { id: "open", label: t("menu.open"), icon: MailOpen01Icon, onSelect: () => void openMessage(m.id) },
            { id: "reply", label: t("mail.reply"), icon: MailReply01Icon, onSelect: () => withMessage(m.id, (full) => reply(full, false)) },
            { id: "reply-all", label: t("mail.replyAll"), icon: MailReplyAllIcon, onSelect: () => withMessage(m.id, (full) => reply(full, true)) },
            { id: "forward", label: t("mail.forward"), icon: ArrowTurnForwardIcon, onSelect: () => withMessage(m.id, forward) },
          ],
          [
            {
              id: "seen",
              label: t(m.seen ? "mail.markUnread" : "mail.markRead"),
              icon: m.seen ? Mail01Icon : MailOpen01Icon,
              onSelect: () =>
                void act(() => changeMailMessage(m.id, { seen: !m.seen }), m.seen ? "mail.done.unread" : "mail.done.read", close && m.seen),
            },
            {
              id: "flag",
              label: t(m.flagged ? "mail.unflag" : "mail.flag"),
              icon: FavouriteIcon,
              onSelect: () =>
                void act(async () => {
                  await changeMailMessage(m.id, { flagged: !m.flagged });
                  if (open?.id === m.id) setOpen({ ...open, flagged: !m.flagged });
                }, m.flagged ? "mail.done.unflagged" : "mail.done.flagged"),
            },
          ],
          [
            {
              id: "archive",
              label: t("mail.archive"),
              icon: Archive02Icon,
              hidden: archive === undefined || folder?.id === archive.id,
              onSelect: () => {
                if (archive !== undefined) void act(() => changeMailMessage(m.id, { mailbox: archive.id }), "mail.done.archived", close);
              },
            },
            {
              kind: "submenu",
              id: "move",
              // A submenu names where it leads, without the select's dots.
              label: t("mail.moveTo").replace(/…$/, ""),
              icon: Folder01Icon,
              items: elsewhere.map((f) => ({
                id: `move-${f.id}`,
                label: folderName(f, t),
                icon: f.role !== null ? ROLE_ICON[f.role] : Folder01Icon,
                onSelect: () => void act(() => changeMailMessage(m.id, { mailbox: f.id }), "mail.done.moved", close),
              })),
            },
          ],
          [
            {
              id: "filter",
              label: t("mail.filters.fromThis"),
              icon: FilterIcon,
              hidden: m.from[0] === undefined || folder?.role === "sent" || folder?.role === "drafts",
              onSelect: () => {
                const sender = m.from[0];
                if (sender === undefined) return;
                setFilters({
                  start: {
                    ...blankFilter(),
                    name: sender.name ?? sender.email,
                    conditions: [{ field: "from", value: sender.email }],
                    moveTo: folder !== undefined && folder.role === null ? folder.id : null,
                  },
                });
              },
            },
          ],
          [
            {
              id: "delete",
              label: t("mail.delete"),
              icon: Delete02Icon,
              danger: true,
              onSelect: () =>
                void act(
                  () => deleteMailMessage(m.id),
                  trash !== undefined && folder?.id === trash.id ? "mail.done.deleted" : "mail.done.binned",
                  close,
                ),
            },
          ],
        ],
      };
    };

  return (
    <TooltipProvider>
    <div className="flex flex-col">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <PaneHeader title={t("mail.title")} summary={t("mail.summary", { address: home.address })} />
        </div>
        <div className="flex shrink-0 gap-2">
          <Button variant="secondary" onClick={() => setKeys(true)} data-testid="mail-encryption">
            <HugeiconsIcon
              icon={own.state === "set" ? SquareLock02Icon : Key01Icon}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
            />
            {t("mail.pgp.button")}
          </Button>
          <Button variant="secondary" onClick={() => setFilters({ start: null })}>
            <HugeiconsIcon icon={FilterIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("mail.filters")}
          </Button>
          <Button onClick={() => setDraft({ to: [], cc: [], subject: "", text: "" })}>
            <HugeiconsIcon icon={PencilEdit02Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("mail.compose.new")}
          </Button>
        </div>
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-2">
        {/* Folders, narrower than the tree: one row, each with its unread count. */}
        <nav aria-label={t("mail.folders")} className="-mx-1 min-w-0 max-w-full overflow-x-auto px-1 lg:hidden">
          <ul className="flex gap-1">
            {placed.map((p) => folderItem(p, "row"))}
            <li className="shrink-0">
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={t("mail.folder.new")}
                title={t("mail.folder.new")}
                onClick={() => setFolderDialog({ kind: "new", parentId: null })}
              >
                <HugeiconsIcon icon={FolderAddIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </Button>
            </li>
          </ul>
        </nav>
        <form
          className="w-full sm:ml-auto sm:w-64"
          role="search"
          onSubmit={(event) => {
            event.preventDefault();
            setSearch(query);
            setColumn("list");
          }}
        >
          <InputGroup>
            <InputGroupAddon>
              <HugeiconsIcon icon={Search01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </InputGroupAddon>
            <InputGroupInput
              type="search"
              value={query}
              onChange={(event) => {
                setQuery(event.target.value);
                if (event.target.value === "") setSearch("");
              }}
              placeholder={t("mail.search", { folder: folder !== undefined ? folderName(folder, t) : "" })}
              aria-label={t("mail.searchLabel")}
            />
          </InputGroup>
        </form>
      </div>

      <Card className="flex h-[min(46rem,calc(100dvh-15rem))] min-h-[26rem] overflow-hidden">
        {/* Folders, as a tree, on a wide screen */}
        <nav aria-label={t("mail.folders")} className="hidden w-52 shrink-0 flex-col border-r border-line lg:flex xl:w-56">
          <div className="flex items-center gap-2 border-b border-hair py-1.5 pr-1.5 pl-4">
            <h2 className="min-w-0 flex-1 truncate text-[15px] font-semibold tracking-tight">{t("mail.folders")}</h2>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t("mail.folder.new")}
                    onClick={() => setFolderDialog({ kind: "new", parentId: null })}
                  />
                }
              >
                <HugeiconsIcon icon={FolderAddIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </TooltipTrigger>
              <TooltipContent side="bottom">{t("mail.folder.new")}</TooltipContent>
            </Tooltip>
          </div>
          {/* The empty space under the tree takes a folder to the top level. */}
          <div
            className={cn("min-h-0 flex-1 overflow-y-auto p-1.5", dropAt === "top" && "bg-accent-wash")}
            {...dropProps(null)}
          >
            <ul className="flex flex-col gap-px">{placed.map((p) => folderItem(p, "tree"))}</ul>
          </div>
        </nav>

        {/* Messages */}
        <section
          aria-label={folder !== undefined ? folderName(folder, t) : undefined}
          className={cn(
            "min-w-0 flex-1 flex-col md:flex md:w-72 md:flex-none md:border-r md:border-line lg:w-80",
            column === "list" ? "flex" : "hidden",
          )}
        >
          <div className="flex items-center gap-2 border-b border-hair px-4 py-2.5">
            <h2 className="min-w-0 flex-1 truncate text-[15px] font-semibold tracking-tight">
              {search !== "" ? t("mail.results", { query: search }) : folder !== undefined ? folderName(folder, t) : ""}
            </h2>
            {page?.total != null && <span className="text-[13px] text-muted tabular-nums">{page.total}</span>}
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {listError !== null && <Failed className="m-3" text={listError} onRetry={() => void refreshList()} />}
            {page === null && listError === null && (
              <div className="grid place-items-center p-8">
                <Spinner />
              </div>
            )}
            {page !== null && page.messages.length === 0 && (
              <Empty className="m-3 border-0">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <HugeiconsIcon icon={Mail01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  </EmptyMedia>
                  <EmptyDescription>{search !== "" ? t("mail.noResults") : t("mail.empty")}</EmptyDescription>
                </EmptyHeader>
              </Empty>
            )}
            {page !== null && (
              <ul>
                {page.messages.map((m) => (
                  <MessageRow
                    key={m.id}
                    faces={faces}
                    message={m}
                    sent={folder?.role === "sent" || folder?.role === "drafts"}
                    current={m.id === openId}
                    locale={locale}
                    onOpen={() => void openMessage(m.id)}
                    menu={rowMenu(m)}
                    onDragEnd={() => setDropAt(null)}
                  />
                ))}
              </ul>
            )}
            {page !== null && page.total !== null && page.messages.length < page.total && (
              <div className="p-3">
                <Button variant="secondary" className="w-full" onClick={() => void refreshList(true)}>
                  {t("mail.more")}
                </Button>
              </div>
            )}
          </div>
        </section>

        {/* The open message */}
        <article
          className={cn("min-w-0 flex-1 flex-col md:flex", column === "message" ? "flex" : "hidden")}
          aria-label={open?.subject}
        >
          {openId === null ? (
            <Empty className="m-4 border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <HugeiconsIcon icon={MailOpen01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                </EmptyMedia>
                <EmptyDescription>{t("mail.pick")}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          ) : open === null ? (
            <div className="m-auto p-6">
              {openError !== null ? <Failed text={openError} onRetry={() => void openMessage(openId)} /> : <Spinner />}
            </div>
          ) : (
            <Reader
              message={open}
              own={own}
              onSetUp={() => setKeys(true)}
              onOpened={(text) => setOpened({ id: open.id, text })}
              home={home}
              faces={faces}
              locale={locale}
              folders={placed}
              onBack={() => setColumn("list")}
              onReply={(all) => reply(open, all)}
              onForward={() => forward(open)}
              onFlag={() =>
                void act(async () => {
                  await changeMailMessage(open.id, { flagged: !open.flagged });
                  setOpen({ ...open, flagged: !open.flagged });
                }, open.flagged ? "mail.done.unflagged" : "mail.done.flagged")
              }
              onUnread={() => void act(() => changeMailMessage(open.id, { seen: false }), "mail.done.unread", true)}
              onArchive={
                archive !== undefined && !open.mailboxIds.includes(archive.id)
                  ? () => void act(() => changeMailMessage(open.id, { mailbox: archive.id }), "mail.done.archived", true)
                  : undefined
              }
              onMove={(to) => void act(() => changeMailMessage(open.id, { mailbox: to }), "mail.done.moved", true)}
              onDelete={() =>
                void act(
                  () => deleteMailMessage(open.id),
                  trash !== undefined && open.mailboxIds.includes(trash.id) ? "mail.done.deleted" : "mail.done.binned",
                  true,
                )
              }
            />
          )}
        </article>
      </Card>
    </div>
      {folderDialog !== null && (
        <FolderDialog
          mode={folderDialog}
          folders={home.mailboxes}
          label={(f) => folderName(f, t)}
          onClose={() => setFolderDialog(null)}
          onSubmit={async (name, parentId) => {
            if (folderDialog.kind === "new") {
              const made = await createMailFolder(name, parentId);
              toast.done(t("mail.done.folderMade"));
              await refreshHome();
              openFolder(made.id);
            } else {
              await updateMailFolder(folderDialog.folder.id, { name });
              toast.done(t("mail.done.folderRenamed"));
              await refreshHome();
            }
          }}
        />
      )}
      {deleting !== null && (
        <DeleteFolderDialog
          folder={deleting}
          onClose={() => setDeleting(null)}
          onDelete={async () => {
            const { moved } = await deleteMailFolder(deleting.id);
            toast.done(
              t("mail.done.folderDeleted"),
              moved > 0 ? t("mail.done.folderBinned", { count: moved }) : undefined,
            );
            if (deleting.id === folderId && inbox !== undefined) openFolder(inbox.id);
            await refreshHome();
          }}
        />
      )}
      {filters !== null && (
        <FiltersDialog
          folders={home.mailboxes}
          label={(f) => folderName(f, t)}
          start={filters.start}
          onClose={() => setFilters(null)}
        />
      )}
      {draft !== null && (
        <Composer
          initial={draft}
          from={home.address}
          own={own}
          onSetUp={() => setKeys(true)}
          people={home.people.map((p) => ({ address: p.address, label: p.displayName || p.name }))}
          onClose={() => setDraft(null)}
          onSent={() => {
            setDraft(null);
            toast.done(t("mail.done.sent"));
            void refreshHome();
            void refreshList();
          }}
        />
      )}
      {keys && (
        <KeyDialog
          own={own}
          address={home.address}
          name={home.people.find((p) => p.address === home.address)?.displayName ?? home.name}
          onOwn={setOwn}
          onClose={() => setKeys(false)}
        />
      )}
    </TooltipProvider>
  );
}

function MessageRow({
  message: m,
  faces,
  sent,
  current,
  locale,
  onOpen,
  menu,
  onDragEnd,
}: {
  message: MailSummary;
  faces: Faces;
  sent: boolean;
  current: boolean;
  locale: string;
  onOpen: () => void;
  menu: MenuBuilder;
  onDragEnd: () => void;
}) {
  const t = useT();
  const people = sent ? m.to : m.from;
  return (
    <li ref={contextMenu(menu)}>
      <button
        type="button"
        onClick={onOpen}
        draggable
        onDragStart={(event) => {
          event.dataTransfer.setData(DRAG_MESSAGE, m.id);
          event.dataTransfer.effectAllowed = "move";
        }}
        onDragEnd={onDragEnd}
        aria-current={current ? "true" : undefined}
        className={cn(
          "flex w-full items-start gap-3 border-b border-hair px-3 py-2.5 text-left",
          current ? "bg-accent-wash" : "hover:bg-sunk",
        )}
      >
        {people[0] !== undefined ? (
          <PersonAvatar person={faces(people[0])} size="sm" className="mt-0.5" />
        ) : (
          <span className="size-8 shrink-0" />
        )}
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="flex items-center gap-2">
          <span
            aria-label={m.seen ? undefined : t("mail.unread")}
            className={cn("size-2 shrink-0 rounded-full", m.seen ? "bg-transparent" : "bg-accent")}
          />
          <span className={cn("min-w-0 flex-1 truncate text-sm", !m.seen && "font-medium")}>
            {(sent ? t("mail.toPrefix") + " " : "") +
              (people.map((a) => a.name ?? faces(a).displayName).join(", ") || t("mail.nobody"))}
          </span>
          {m.pgp === "encrypted" && (
            <HugeiconsIcon icon={SquareLock02Icon} className="size-3.5 shrink-0 text-ok" aria-label={t("mail.pgp.list.encrypted")} />
          )}
          {m.pgp === "signed" && (
            <HugeiconsIcon icon={CheckmarkBadge01Icon} className="size-3.5 shrink-0 text-muted" aria-label={t("mail.pgp.list.signed")} />
          )}
          {m.hasAttachment && m.pgp !== "encrypted" && (
            <HugeiconsIcon icon={Attachment01Icon} className="size-3.5 shrink-0 text-muted" aria-label={t("mail.attachment")} />
          )}
          {m.flagged && (
            <HugeiconsIcon icon={FavouriteIcon} className="size-3.5 shrink-0 text-warn" aria-label={t("mail.flagged")} />
          )}
          <time dateTime={m.receivedAt} className="shrink-0 text-[12px] text-muted tabular-nums">
            {when(m.receivedAt, locale)}
          </time>
        </span>
        <span className={cn("truncate pl-4 text-sm", !m.seen && "font-medium")}>
          {m.subject || t("mail.noSubject")}
        </span>
        <span className="line-clamp-1 pl-4 text-[13px] text-muted">
          {m.pgp === "encrypted" ? t("mail.pgp.list.encrypted") : <Masked text={m.preview} still />}
        </span>
        </span>
      </button>
    </li>
  );
}

function isPgp(m: MailSummary): boolean {
  return m.pgp === "encrypted" || m.pgp === "signed";
}

/* An OpenPGP message, opened in the browser: fetched raw, decrypted with the
 * person's unlocked key, its signature checked against the sender's keys.
 * Opened again when the key is unlocked or locked. */
function useOpened(m: MailMessage, own: OwnKey): { reading: Reading | null; opened: Opened | null } {
  const unlocked = useUnlocked(own);
  const [state, setState] = React.useState<{ reading: Reading | null; opened: Opened | null }>({
    reading: isPgp(m) ? { state: "opening" } : null,
    opened: null,
  });
  const ownState = own.state;
  React.useEffect(() => {
    if (!isPgp(m)) {
      setState({ reading: null, opened: null });
      return;
    }
    if (ownState === "loading") return;
    const controller = new AbortController();
    const from = m.from[0]?.email ?? "";
    setState({ reading: { state: "opening" }, opened: null });
    void (async () => {
      try {
        const [raw, found] = await Promise.all([
          getMailRaw(m.id, { signal: controller.signal }),
          from !== ""
            ? findMailKeys(from, { signal: controller.signal }).catch(() => ({ keys: [] }))
            : Promise.resolve({ keys: [] }),
        ]);
        const opened = await openMail(
          raw,
          unlocked,
          found.keys.map((k) => k.publicKey),
        );
        if (controller.signal.aborted) return;
        setState({
          reading:
            opened.kind === "plain"
              ? null
              : { state: "open", kind: opened.kind, inline: opened.inline, signature: opened.signature, from },
          opened,
        });
      } catch (error) {
        if (controller.signal.aborted) return;
        if (error instanceof CannotOpen) {
          const reading: Reading =
            error.kind === "locked"
              ? ownState === "set"
                ? { state: "locked" }
                : { state: "noKey" }
              : { state: error.kind };
          setState({ reading, opened: null });
        } else {
          setState({ reading: { state: "broken" }, opened: null });
        }
      }
    })();
    return () => controller.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [m.id, m.pgp, m.from, unlocked, ownState]);
  return state;
}

/* A decrypted message's HTML, as text: it never becomes a document here,
 * so nothing in it loads or runs. */
function htmlText(html: string): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  for (const el of doc.querySelectorAll("br")) el.replaceWith("\n");
  for (const el of doc.querySelectorAll("p, div, li, tr, h1, h2, h3, h4, h5, h6")) el.append("\n");
  return (doc.body.textContent ?? "").replace(/\n{3,}/g, "\n\n").trim();
}

/* Decrypted attachments, as downloads from blob: URLs that live as long as
 * the message is open. */
function useBlobUrls(files: Opened["attachments"] | undefined): string[] {
  const urls = React.useMemo(
    () => (files ?? []).map((f) => URL.createObjectURL(new Blob([f.bytes as BlobPart], { type: f.type }))),
    [files],
  );
  React.useEffect(() => () => urls.forEach((u) => URL.revokeObjectURL(u)), [urls]);
  return urls;
}

function Reader({
  message: m,
  own,
  onSetUp,
  onOpened,
  home,
  faces,
  locale,
  folders,
  onBack,
  onReply,
  onForward,
  onFlag,
  onUnread,
  onArchive,
  onMove,
  onDelete,
}: {
  message: MailMessage;
  own: OwnKey;
  onSetUp: () => void;
  onOpened: (text: string) => void;
  home: MailHome;
  faces: Faces;
  locale: string;
  folders: readonly Placed[];
  onBack: () => void;
  onReply: (all: boolean) => void;
  onForward: () => void;
  onFlag: () => void;
  onUnread: () => void;
  onArchive: (() => void) | undefined;
  onMove: (folder: string) => void;
  onDelete: () => void;
}) {
  const t = useT();
  // Plain text first when there is any: it is what the sender wrote, and it
  // loads nothing.
  const [html, setHtml] = React.useState(m.text.trim() === "" && m.hasHtml);
  React.useEffect(() => setHtml(m.text.trim() === "" && m.hasHtml), [m.id, m.text, m.hasHtml]);
  const others = folders.filter(({ folder: f }) => !m.mailboxIds.includes(f.id) && f.role !== "drafts");
  const cloud = useCloud();
  const { reading, opened } = useOpened(m, own);
  const [unlocking, setUnlocking] = React.useState(false);
  const pgp = isPgp(m);
  const openedText = opened === null ? "" : opened.text.trim() !== "" ? opened.text : opened.html !== null ? htmlText(opened.html) : "";
  React.useEffect(() => {
    if (opened !== null) onOpened(openedText);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [opened]);
  const blobs = useBlobUrls(opened?.attachments);

  // Icon buttons name themselves in a tooltip, as the sidebar's rail does.
  const tool = (icon: IconSvgElement, label: string, onClick: () => void, pressed?: boolean) => (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button variant="ghost" size="icon-sm" onClick={onClick} aria-label={label} aria-pressed={pressed} />
        }
      >
        <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );

  return (
    <>
      <div className="flex flex-wrap items-center gap-1 border-b border-hair px-3 py-2">
        <Button variant="ghost" size="icon-sm" className="md:hidden" onClick={onBack} aria-label={t("mail.back")}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        {tool(MailReply01Icon, t("mail.reply"), () => onReply(false))}
        {tool(MailReplyAllIcon, t("mail.replyAll"), () => onReply(true))}
        {tool(ArrowTurnForwardIcon, t("mail.forward"), onForward)}
        <span aria-hidden="true" className="mx-1 h-5 w-px bg-line" />
        {tool(FavouriteIcon, m.flagged ? t("mail.unflag") : t("mail.flag"), onFlag, m.flagged)}
        {tool(Mail01Icon, t("mail.markUnread"), onUnread)}
        {onArchive !== undefined && tool(Archive02Icon, t("mail.archive"), onArchive)}
        {others.length > 0 && (
          <NativeSelect
            aria-label={t("mail.moveTo")}
            value=""
            onChange={(event) => {
              if (event.target.value !== "") onMove(event.target.value);
            }}
            className="ml-1 [&_select]:h-8 [&_select]:text-[13px]"
          >
            <NativeSelectOption value="">{t("mail.moveTo")}</NativeSelectOption>
            {others.map(({ folder: f, depth }) => (
              <NativeSelectOption key={f.id} value={f.id}>
                {"\u2003".repeat(depth) + folderName(f, t)}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        )}
        <PersonLinks message={m} home={home} />
        <span className="ml-auto" />
        {!folders.some(({ folder: f }) => f.role === "sent" && m.mailboxIds.includes(f.id)) &&
          tool(Flag02Icon, t("report.action"), () => openReport({ app: "mail", about: null, ref: m.id }))}
        {tool(Delete02Icon, t("mail.delete"), onDelete)}
      </div>
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        <div className="flex flex-col gap-1 px-5 pt-4 pb-3">
          <h2 className="text-[17px] leading-tight font-semibold tracking-tight break-words">
            {opened?.subject || m.subject || t("mail.noSubject")}
          </h2>
          <p className="flex items-center gap-2 pt-1 text-sm break-words">
            {m.from[0] !== undefined && <PersonAvatar person={faces(m.from[0])} size="sm" />}
            <span className="min-w-0">
              <span className="text-muted">{t("mail.from")} </span>
              {m.from.map(full).join(", ")}
            </span>
          </p>
          {m.to.length > 0 && (
            <p className="text-sm break-words">
              <span className="text-muted">{t("mail.to")} </span>
              {m.to.map(full).join(", ")}
            </p>
          )}
          {m.cc.length > 0 && (
            <p className="text-sm break-words">
              <span className="text-muted">{t("mail.cc")} </span>
              {m.cc.map(full).join(", ")}
            </p>
          )}
          <p className="text-[13px] text-muted">
            {new Intl.DateTimeFormat(locale, { dateStyle: "full", timeStyle: "short" }).format(new Date(m.receivedAt))}
          </p>
          {!pgp && m.hasHtml && m.text.trim() !== "" && (
            <div className="pt-1">
              <Button variant="link" size="xs" className="px-0" onClick={() => setHtml(!html)}>
                {html ? t("mail.showText") : t("mail.showHtml")}
              </Button>
            </div>
          )}
          {reading !== null && (
            <div className="pt-2">
              <SecurityNote
                reading={reading}
                onUnlock={() => setUnlocking(true)}
                onSetUp={onSetUp}
              />
            </div>
          )}
        </div>
        {/* An invitation, a guest's answer or a cancellation, as LosOS
            Calendar reads it (src/calendar/invite.tsx). */}
        {sharedApp("calendar") &&
          m.attachments
            .filter(isCalendarFile)
            .slice(0, 1)
            .map((a) => <InviteCard key={a.blobId} attachment={a} address={home.address} subject={m.subject} />)}
        {!pgp && m.attachments.length > 0 && (
          <ul className="flex flex-wrap gap-2 px-5 pb-3" aria-label={t("mail.attachments")}>
            {m.attachments.map((a) => (
              <ReceivedAttachment key={a.blobId} attachment={a} cloud={cloud} />
            ))}
          </ul>
        )}
        {pgp && opened !== null && opened.attachments.length > 0 && (
          <ul className="flex flex-wrap gap-2 px-5 pb-3" aria-label={t("mail.attachments")}>
            {opened.attachments.map((a, i) => (
              <li key={`${i}-${a.name}`}>
                <a
                  href={blobs[i]}
                  download={a.name}
                  className="flex items-center gap-2 rounded-control border border-line px-2.5 py-1.5 text-[13px] hover:bg-sunk"
                >
                  <HugeiconsIcon icon={Attachment01Icon} className="size-3.5 text-muted" />
                  <span className="max-w-48 truncate">{a.name}</span>
                  <span className="text-muted tabular-nums">{formatBytes(a.bytes.length)}</span>
                </a>
              </li>
            ))}
          </ul>
        )}
        {pgp ? (
          opened !== null && (
            <pre
              data-testid="mail-opened"
              className="border-t border-hair px-5 py-4 font-sans text-sm leading-relaxed break-words whitespace-pre-wrap animate-fade-in"
            >
              {openedText}
            </pre>
          )
        ) : html ? (
          <MailHtml
            id={m.id}
            from={m.from[0]}
            title={t("mail.htmlTitle")}
            own={folders.some(({ folder: f }) => f.role === "sent" && m.mailboxIds.includes(f.id))}
          />
        ) : (
          <pre className="border-t border-hair px-5 py-4 font-sans text-sm leading-relaxed break-words whitespace-pre-wrap">
            <Masked text={m.text !== "" ? m.text : m.preview} />
          </pre>
        )}
      </div>
      {unlocking && own.state === "set" && <UnlockDialog own={own.key} onClose={() => setUnlocking(false)} />}
    </>
  );
}

/* Something that looks like an address, for the key lookup while typing. */
function addressLike(s: string): boolean {
  return /^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/.test(s);
}

function Composer({
  initial,
  from,
  own,
  onSetUp,
  people,
  onClose,
  onSent,
}: {
  initial: ComposeStart;
  from: string;
  own: OwnKey;
  onSetUp: () => void;
  people: readonly { address: string; label: string }[];
  onClose: () => void;
  onSent: () => void;
}) {
  const t = useT();
  const [to, setTo] = React.useState(initial.to.join(", "));
  const [cc, setCc] = React.useState(initial.cc.join(", "));
  const [subject, setSubject] = React.useState(initial.subject);
  const [text, setText] = React.useState(initial.text);
  const [sending, setSending] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [showCc, setShowCc] = React.useState(initial.cc.length > 0);
  const detailsCheck = useDetailsCheck();
  const attached = useAttached({ cloud: initial.cloud, carry: initial.carry });
  const [unlocking, setUnlocking] = React.useState(false);
  const list = (value: string) =>
    value
      .split(/[,;]/)
      .map((s) => s.trim())
      .filter((s) => s !== "");

  // OpenPGP: encrypt by itself once every recipient has a key, until the
  // person decides; sign whenever there is a key to sign with.
  const hasKey = own.state === "set";
  const unlocked = useUnlocked(own);
  const recipients = React.useMemo(
    () => [...new Set([...list(to), ...list(cc)].map((a) => a.toLowerCase()))].filter(addressLike),
    [to, cc],
  );
  const keys = useRecipients(recipients);
  const allKeys =
    recipients.length > 0 && recipients.every((a) => keys.get(a)?.state === "found");
  const [encryptChoice, setEncryptChoice] = React.useState<boolean | null>(null);
  const [signChoice, setSignChoice] = React.useState<boolean | null>(null);
  const [inline, setInline] = React.useState(false);
  const encrypt = hasKey && (encryptChoice ?? allKeys);
  const sign = hasKey && (signChoice ?? (encrypt || unlocked !== null));
  const missing = encrypt ? recipients.filter((a) => keys.get(a)?.state === "none") : [];
  const looking = encrypt && recipients.some((a) => keys.get(a) === undefined || keys.get(a)?.state === "looking");

  const deliver = async () => {
    setSending(true);
    setProblem(null);
    // Card and ID numbers are asked about first; "Don't send" keeps the draft.
    if (!(await detailsCheck.check(`${subject}\n${text}`))) {
      setSending(false);
      return;
    }
    try {
      let made: { text: string; pgp?: MailDraft["pgp"] } = { text };
      const files = attached.files.flatMap((a) => (a.file === null ? [] : [a.file]));
      const sealing = own.state === "set" && (encrypt || sign);
      if (sealing && own.state === "set") {
        const encryptTo: string[] = [];
        if (encrypt) {
          for (const a of recipients) {
            const r = keys.get(a);
            if (r?.state !== "found") throw new Error(t("mail.pgp.compose.missing", { who: a }));
            encryptTo.push(r.key.publicKey);
          }
          // And to the sender, so the copy in Sent can be read again.
          encryptTo.push(own.key.publicKey);
        }
        try {
          const inside = await Promise.all(
            files.map(async (f) => ({ name: f.name, type: f.type, bytes: new Uint8Array(await f.arrayBuffer()) })),
          );
          // Read now: an unlock that just happened is not in this render's `unlocked`.
          const signWith = sign ? unlockedKey(own.key.fingerprint) : null;
          if (sign && signWith === null) throw new Error(t("mail.pgp.unlockFailed"));
          made = await seal({ text, files: inside, encryptTo, signWith, inline });
        } catch (error) {
          throw new Error(t("mail.pgp.compose.failed", { detail: message(error) }));
        }
      }
      const draft: MailDraft = { to: list(to), cc: list(cc), subject, text: made.text };
      if (made.pgp !== undefined) draft.pgp = made.pgp;
      if (initial.inReplyTo !== undefined) draft.inReplyTo = initial.inReplyTo;
      // A sealed message carries its files inside; the box refuses them beside it.
      if (!sealing && files.length > 0) draft.attachments = await Promise.all(files.map(mailFile));
      await sendMail(draft);
      onSent();
    } catch (error) {
      setProblem(message(error));
      setSending(false);
    }
  };

  const send = (event: React.FormEvent) => {
    event.preventDefault();
    if (sign && unlocked === null) {
      setUnlocking(true);
      return;
    }
    void deliver();
  };

  return (
    <Dialog
      open
      onOpenChange={(next) => {
        if (!next && !sending) onClose();
      }}
      labelledBy="mail-compose-title"
      dialogClassName="w-[min(40rem,calc(100vw-1rem))]"
    >
      <form onSubmit={send}>
        <DialogHeader>
          <DialogTitle id="mail-compose-title">
            {initial.inReplyTo !== undefined ? t("mail.compose.reply") : t("mail.compose.title")}
          </DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-3">
          <p className="text-[13px] text-muted">{t("mail.compose.from", { address: from })}</p>
          <Field>
            <span className="flex items-center justify-between">
              <FieldLabel htmlFor="mail-to">{t("mail.to")}</FieldLabel>
              {!showCc && (
                <Button type="button" variant="link" size="xs" className="px-0" onClick={() => setShowCc(true)}>
                  {t("mail.compose.addCc")}
                </Button>
              )}
            </span>
            {/* The person's contacts and the people on this box, as they
                type (src/contacts/suggest.tsx). */}
            <PeopleField
              id="mail-to"
              want="email"
              multiple
              value={to}
              onChange={setTo}
              autoFocus={initial.to.length === 0}
              placeholder={people[0]?.address ?? ""}
              required
            />
          </Field>
          {showCc && (
            <Field>
              <FieldLabel htmlFor="mail-cc">{t("mail.cc")}</FieldLabel>
              <PeopleField id="mail-cc" want="email" multiple value={cc} onChange={setCc} />
            </Field>
          )}
          {hasKey && recipients.length > 0 && (
            <ul className="flex flex-wrap gap-1.5" aria-live="polite">
              {recipients.map((a) => {
                const r = keys.get(a);
                const state = r?.state ?? "looking";
                return (
                  <li
                    key={a}
                    data-testid="mail-recipient-key"
                    data-state={state}
                    className={cn(
                      "flex max-w-full items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-[12px]",
                      state === "found" ? "border-ok/25 bg-ok/10" : "border-line bg-sunk text-muted",
                    )}
                  >
                    {state === "looking" ? (
                      <Spinner className="size-3" />
                    ) : (
                      <HugeiconsIcon
                        icon={state === "found" ? SquareLock02Icon : Key01Icon}
                        size={13}
                        strokeWidth={1.5}
                        aria-hidden="true"
                        className={state === "found" ? "text-ok" : undefined}
                      />
                    )}
                    <span className="truncate">{a}</span>
                    <span className="shrink-0 text-muted">
                      {r?.state === "found"
                        ? t("mail.pgp.compose.found", { where: sourceName(r.key.source, t) })
                        : state === "none"
                          ? t("mail.pgp.compose.none")
                          : t("mail.pgp.compose.looking")}
                    </span>
                  </li>
                );
              })}
            </ul>
          )}
          <Field>
            <FieldLabel htmlFor="mail-subject">{t("mail.compose.subject")}</FieldLabel>
            <Input id="mail-subject" value={subject} onChange={(event) => setSubject(event.target.value)} maxLength={900} />
          </Field>
          <Field>
            <FieldLabel htmlFor="mail-body" className="sr-only">
              {t("mail.compose.body")}
            </FieldLabel>
            <Textarea
              id="mail-body"
              value={text}
              onChange={(event) => setText(event.target.value)}
              rows={12}
              autoFocus={initial.to.length > 0}
              className="min-h-56 font-sans"
            />
          </Field>
          <ComposeAttachments attached={attached} />
          {hasKey ? (
            <div className="flex flex-wrap items-center gap-x-5 gap-y-2 text-[13px]">
              <span className="flex items-center gap-2">
                <Switch
                  id="mail-encrypt"
                  size="sm"
                  isSelected={encrypt}
                  onChange={(on) => setEncryptChoice(on)}
                  data-testid="mail-encrypt"
                />
                <label htmlFor="mail-encrypt" className="flex items-center gap-1">
                  <HugeiconsIcon icon={SquareLock02Icon} size={14} strokeWidth={1.5} aria-hidden="true" />
                  {t("mail.pgp.compose.encrypt")}
                </label>
              </span>
              <span className="flex items-center gap-2">
                <Switch
                  id="mail-sign"
                  size="sm"
                  isSelected={sign}
                  onChange={(on) => setSignChoice(on)}
                  data-testid="mail-sign"
                />
                <label htmlFor="mail-sign" className="flex items-center gap-1">
                  <HugeiconsIcon icon={CheckmarkBadge01Icon} size={14} strokeWidth={1.5} aria-hidden="true" />
                  {t("mail.pgp.compose.sign")}
                </label>
              </span>
              {(encrypt || sign) && attached.files.length === 0 && (
                <span className="flex items-center gap-2 text-muted">
                  <Switch id="mail-inline" size="sm" isSelected={inline} onChange={setInline} />
                  <label htmlFor="mail-inline">{t("mail.pgp.compose.inline")}</label>
                </span>
              )}
            </div>
          ) : (
            <p className="text-[12px] text-muted">
              <Button type="button" variant="link" size="xs" className="px-0" onClick={onSetUp}>
                {t("mail.pgp.compose.noKeyYet")}
              </Button>
            </p>
          )}
          {missing.length > 0 && (
            <Alert variant="warn" role="status">
              <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} aria-hidden="true" />
              <AlertDescription>{t("mail.pgp.compose.missing", { who: missing.join(", ") })}</AlertDescription>
            </Alert>
          )}
          <p className="text-[12px] text-muted">
            {t("mail.compose.local")}
            {encrypt && ` ${t("mail.pgp.compose.subjectOpen")}`}
          </p>
          {problem !== null && <Failed text={problem} />}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose} disabled={sending}>
            {t("mail.compose.discard")}
          </Button>
          <Button
            type="submit"
            disabled={sending || missing.length > 0 || looking || !attached.ready || attached.tooBig}
            data-encrypted={encrypt || undefined}
          >
            {sending ? (
              <Spinner className="size-4" />
            ) : (
              <HugeiconsIcon icon={encrypt ? SquareLock02Icon : MailSend01Icon} className="size-4" />
            )}
            {t("mail.compose.send")}
          </Button>
        </DialogFooter>
      </form>
      {unlocking && own.state === "set" && (
        <UnlockDialog
          own={own.key}
          onClose={(ok) => {
            setUnlocking(false);
            if (ok) void deliver();
          }}
        />
      )}
      {detailsCheck.dialog}
    </Dialog>
  );
}
