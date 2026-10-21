/* LosOS Mail: folders, a folder's messages, the open message, and a
 * composer, over lososd's /api/mail (backend/src/mail.rs). Three columns on
 * a wide screen; on a phone one at a time, folders, then messages, then the
 * message, with a way back at the top of each.
 *
 * A message's HTML never enters this page. It is a document of its own at
 * /api/mail/messages/<id>/html, shown in an <iframe sandbox> without
 * allow-scripts or allow-same-origin, under a policy that loads nothing off
 * the message (modules/containers.nix). The plain text, when there is one,
 * is the default view; HTML is one button away. */

import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  ArrowLeft01Icon,
  Attachment01Icon,
  Delete02Icon,
  FileEditIcon,
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
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Spinner } from "@/components/ui/spinner";
import { toast, Toaster } from "@/components/ui/toast";
import { TooltipProvider } from "@/components/ui/tooltip";
import {
  ApiError,
  changeMailMessage,
  deleteMailMessage,
  getMail,
  getMailMessage,
  getMailMessages,
  loadMe,
  mailAttachmentUrl,
  mailHtmlUrl,
  sendMail,
  signInUrl,
  type MailAddress,
  type MailDraft,
  type MailFolder,
  type MailHome,
  type MailMessage,
  type MailPage,
  type MailRole,
  type MailSummary,
} from "@/lib/api";
import { logoFor } from "@/lib/logo";
import { useLocale, useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { cn } from "@/lib/utils";

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

/* Which column a phone shows. */
type Column = "folders" | "list" | "message";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
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

function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} kB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
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
function quoted(m: MailMessage, locale: string, t: ReturnType<typeof useT>): string {
  const head = t("mail.compose.wrote", {
    when: new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(new Date(m.receivedAt)),
    who: who(m.from),
  });
  const body = (m.text !== "" ? m.text : m.preview)
    .split("\n")
    .map((line) => `> ${line}`)
    .join("\n");
  return `\n\n${head}\n${body}`;
}

function withPrefix(prefix: string, subject: string): string {
  return subject.toLowerCase().startsWith(prefix.toLowerCase()) ? subject : `${prefix} ${subject}`.trim();
}

export function MailRoot() {
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

  React.useEffect(() => {
    document.title = t("mail.title");
  }, [t]);

  return (
    <TooltipProvider>
      {state.kind === "ready" ? (
        <Mailbox home={state.home} onHome={(home) => setState({ kind: "ready", home })} />
      ) : (
        <main className="grid min-h-dvh place-items-center bg-ground p-4 text-ink">
          <div className="flex max-w-sm flex-col items-center gap-4 text-center">
            <img src={logoFor().url} alt="" className="size-12" />
            {state.kind === "loading" && <Spinner />}
            {state.kind === "signIn" && (
              <>
                <p>{t("mail.signIn.text")}</p>
                <Button onClick={() => window.location.assign(signInUrl())}>{t("mail.signIn.button")}</Button>
              </>
            )}
            {state.kind === "missing" && (
              <>
                <p>{t("mail.missing")}</p>
                <Button variant="secondary" onClick={() => window.location.assign("/")}>
                  {t("mail.toBox")}
                </Button>
              </>
            )}
            {state.kind === "failed" && (
              <>
                <p>{t("mail.failed", { detail: state.message })}</p>
                <Button variant="secondary" onClick={() => void load()}>
                  {t("mail.retry")}
                </Button>
              </>
            )}
          </div>
        </main>
      )}
      <Toaster />
    </TooltipProvider>
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
  const [draft, setDraft] = React.useState<MailDraft | null>(null);
  const [column, setColumn] = React.useState<Column>("list");

  const folder = home.mailboxes.find((f) => f.id === folderId) ?? inbox;

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

  const reply = (m: MailMessage, all: boolean) => {
    const to = (m.replyTo.length > 0 ? m.replyTo : m.from).map((a) => a.email);
    const cc = all
      ? [...m.to, ...m.cc].map((a) => a.email).filter((e) => e !== home.address && !to.includes(e))
      : [];
    setDraft({
      to,
      cc,
      subject: withPrefix(t("mail.compose.re"), m.subject),
      text: quoted(m, locale, t),
      inReplyTo: m.id,
    });
  };

  const forward = (m: MailMessage) => {
    setDraft({ to: [], cc: [], subject: withPrefix(t("mail.compose.fwd"), m.subject), text: quoted(m, locale, t) });
  };

  const trash = home.mailboxes.find((f) => f.role === "trash");
  const archive = home.mailboxes.find((f) => f.role === "archive");

  return (
    <div className="flex h-dvh flex-col bg-ground text-ink">
      <header className="flex items-center gap-2 border-b border-line bg-surface px-3 py-2">
        <a href="/" className="flex items-center gap-2 rounded-control px-1 py-1 hover:bg-sunk" title={t("mail.toBox")}>
          <img src={logoFor().url} alt="" className="size-7" />
          <span className="hidden font-medium sm:inline">{t("mail.title")}</span>
        </a>
        <form
          className="relative ml-auto flex w-full max-w-md items-center"
          role="search"
          onSubmit={(event) => {
            event.preventDefault();
            setSearch(query);
            setColumn("list");
          }}
        >
          <HugeiconsIcon icon={Search01Icon} className="pointer-events-none absolute left-2.5 size-4 text-muted" />
          <Input
            type="search"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              if (event.target.value === "") setSearch("");
            }}
            placeholder={t("mail.search", { folder: folder !== undefined ? folderName(folder, t) : "" })}
            aria-label={t("mail.searchLabel")}
            className="pl-8"
          />
        </form>
        <Button onClick={() => setDraft({ to: [], cc: [], subject: "", text: "" })} className="shrink-0">
          <HugeiconsIcon icon={PencilEdit02Icon} className="size-4" />
          <span className="hidden sm:inline">{t("mail.compose.new")}</span>
        </Button>
      </header>

      <div className="flex min-h-0 flex-1">
        {/* Folders */}
        <nav
          aria-label={t("mail.folders")}
          className={cn(
            "w-full shrink-0 overflow-y-auto border-r border-line bg-surface p-2 md:block md:w-56",
            column === "folders" ? "block" : "hidden",
          )}
        >
          <p className="truncate px-2 pt-1 pb-3 text-[13px] text-muted" title={home.address}>
            {home.address}
          </p>
          <ul className="flex flex-col gap-0.5">
            {home.mailboxes.map((f) => (
              <li key={f.id}>
                <button
                  type="button"
                  aria-current={f.id === folderId ? "page" : undefined}
                  onClick={() => {
                    setFolderId(f.id);
                    setOpenId(null);
                    setOpen(null);
                    setColumn("list");
                  }}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-control px-2 py-1.5 text-left text-sm",
                    f.id === folderId ? "bg-accent-wash text-accent" : "hover:bg-sunk",
                  )}
                >
                  <HugeiconsIcon icon={f.role !== null ? ROLE_ICON[f.role] : Folder01Icon} className="size-4 shrink-0" />
                  <span className="min-w-0 flex-1 truncate">{folderName(f, t)}</span>
                  {f.unread > 0 && (
                    <span className="rounded-full bg-accent px-1.5 text-[11px] font-medium text-surface tabular-nums">
                      {f.unread}
                    </span>
                  )}
                </button>
              </li>
            ))}
          </ul>
        </nav>

        {/* Messages */}
        <section
          aria-label={folder !== undefined ? folderName(folder, t) : undefined}
          className={cn(
            "min-w-0 flex-1 flex-col border-r border-line md:flex md:max-w-sm lg:max-w-md",
            column === "list" ? "flex" : "hidden",
          )}
        >
          <div className="flex items-center gap-2 border-b border-line px-3 py-2">
            <Button variant="ghost" size="icon-sm" className="md:hidden" onClick={() => setColumn("folders")} aria-label={t("mail.folders")}>
              <HugeiconsIcon icon={ArrowLeft01Icon} className="size-4" />
            </Button>
            <h1 className="min-w-0 flex-1 truncate font-medium">
              {search !== "" ? t("mail.results", { query: search }) : folder !== undefined ? folderName(folder, t) : ""}
            </h1>
            {page?.total != null && <span className="text-[13px] text-muted tabular-nums">{page.total}</span>}
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {listError !== null && <p className="p-4 text-sm text-crit">{listError}</p>}
            {page === null && listError === null && (
              <div className="grid place-items-center p-8">
                <Spinner />
              </div>
            )}
            {page !== null && page.messages.length === 0 && (
              <div className="flex flex-col items-center gap-2 p-10 text-center text-muted">
                <HugeiconsIcon icon={Mail01Icon} className="size-8" />
                <p>{search !== "" ? t("mail.noResults") : t("mail.empty")}</p>
              </div>
            )}
            {page !== null && (
              <ul>
                {page.messages.map((m) => (
                  <MessageRow
                    key={m.id}
                    message={m}
                    sent={folder?.role === "sent" || folder?.role === "drafts"}
                    current={m.id === openId}
                    locale={locale}
                    onOpen={() => void openMessage(m.id)}
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
          className={cn("min-w-0 flex-[2] flex-col bg-surface md:flex", column === "message" ? "flex" : "hidden")}
          aria-label={open?.subject}
        >
          {openId === null ? (
            <div className="m-auto flex flex-col items-center gap-2 p-10 text-center text-muted">
              <HugeiconsIcon icon={MailOpen01Icon} className="size-10" />
              <p>{t("mail.pick")}</p>
            </div>
          ) : open === null ? (
            <div className="m-auto p-10">
              {openError !== null ? <p className="text-sm text-crit">{openError}</p> : <Spinner />}
            </div>
          ) : (
            <Reader
              message={open}
              locale={locale}
              folders={home.mailboxes}
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
      </div>

      {draft !== null && (
        <Composer
          initial={draft}
          from={home.address}
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
    </div>
  );
}

function MessageRow({
  message: m,
  sent,
  current,
  locale,
  onOpen,
}: {
  message: MailSummary;
  sent: boolean;
  current: boolean;
  locale: string;
  onOpen: () => void;
}) {
  const t = useT();
  const people = sent ? m.to : m.from;
  return (
    <li>
      <button
        type="button"
        onClick={onOpen}
        aria-current={current ? "true" : undefined}
        className={cn(
          "flex w-full flex-col gap-0.5 border-b border-hair px-3 py-2.5 text-left",
          current ? "bg-accent-wash" : "hover:bg-sunk",
        )}
      >
        <span className="flex items-center gap-2">
          <span
            aria-label={m.seen ? undefined : t("mail.unread")}
            className={cn("size-2 shrink-0 rounded-full", m.seen ? "bg-transparent" : "bg-accent")}
          />
          <span className={cn("min-w-0 flex-1 truncate text-sm", !m.seen && "font-medium")}>
            {(sent ? t("mail.toPrefix") + " " : "") + (who(people) || t("mail.nobody"))}
          </span>
          {m.hasAttachment && (
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
        <span className="line-clamp-1 pl-4 text-[13px] text-muted">{m.preview}</span>
      </button>
    </li>
  );
}

function Reader({
  message: m,
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
  locale: string;
  folders: readonly MailFolder[];
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
  const others = folders.filter((f) => !m.mailboxIds.includes(f.id) && f.role !== "drafts");

  const tool = (icon: IconSvgElement, label: string, onClick: () => void, pressed?: boolean) => (
    <Button variant="ghost" size="icon-sm" onClick={onClick} aria-label={label} title={label} aria-pressed={pressed}>
      <HugeiconsIcon icon={icon} className="size-4" />
    </Button>
  );

  return (
    <>
      <div className="flex flex-wrap items-center gap-1 border-b border-line px-3 py-2">
        <Button variant="ghost" size="icon-sm" className="md:hidden" onClick={onBack} aria-label={t("mail.back")}>
          <HugeiconsIcon icon={ArrowLeft01Icon} className="size-4" />
        </Button>
        {tool(MailReply01Icon, t("mail.reply"), () => onReply(false))}
        {tool(MailReplyAllIcon, t("mail.replyAll"), () => onReply(true))}
        {tool(ArrowTurnForwardIcon, t("mail.forward"), onForward)}
        <span className="mx-1 h-5 w-px bg-line" />
        {tool(FavouriteIcon, m.flagged ? t("mail.unflag") : t("mail.flag"), onFlag, m.flagged)}
        {tool(Mail01Icon, t("mail.markUnread"), onUnread)}
        {onArchive !== undefined && tool(Archive02Icon, t("mail.archive"), onArchive)}
        {others.length > 0 && (
          <select
            aria-label={t("mail.moveTo")}
            value=""
            onChange={(event) => {
              if (event.target.value !== "") onMove(event.target.value);
            }}
            className="h-8 rounded-control border border-line bg-surface px-2 text-[13px] text-ink"
          >
            <option value="">{t("mail.moveTo")}</option>
            {others.map((f) => (
              <option key={f.id} value={f.id}>
                {folderName(f, t)}
              </option>
            ))}
          </select>
        )}
        <span className="ml-auto" />
        {tool(Delete02Icon, t("mail.delete"), onDelete)}
      </div>
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        <div className="flex flex-col gap-1 px-5 pt-4 pb-3">
          <h2 className="text-lg font-medium break-words">{m.subject || t("mail.noSubject")}</h2>
          <p className="text-sm break-words">
            <span className="text-muted">{t("mail.from")} </span>
            {m.from.map(full).join(", ")}
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
          {m.hasHtml && m.text.trim() !== "" && (
            <div className="pt-1">
              <Button variant="link" size="xs" className="px-0" onClick={() => setHtml(!html)}>
                {html ? t("mail.showText") : t("mail.showHtml")}
              </Button>
            </div>
          )}
        </div>
        {m.attachments.length > 0 && (
          <ul className="flex flex-wrap gap-2 px-5 pb-3" aria-label={t("mail.attachments")}>
            {m.attachments.map((a) => (
              <li key={a.blobId}>
                <a
                  href={mailAttachmentUrl(a)}
                  download={a.name}
                  className="flex items-center gap-2 rounded-control border border-line px-2.5 py-1.5 text-[13px] hover:bg-sunk"
                >
                  <HugeiconsIcon icon={Attachment01Icon} className="size-3.5 text-muted" />
                  <span className="max-w-48 truncate">{a.name}</span>
                  <span className="text-muted tabular-nums">{size(a.size)}</span>
                </a>
              </li>
            ))}
          </ul>
        )}
        {html ? (
          <iframe
            title={t("mail.htmlTitle")}
            src={mailHtmlUrl(m.id)}
            sandbox="allow-popups allow-popups-to-escape-sandbox"
            referrerPolicy="no-referrer"
            className="min-h-[60vh] w-full flex-1 border-t border-line bg-white"
          />
        ) : (
          <pre className="border-t border-line px-5 py-4 font-sans text-sm leading-relaxed break-words whitespace-pre-wrap">
            {m.text !== "" ? m.text : m.preview}
          </pre>
        )}
      </div>
    </>
  );
}

function Composer({
  initial,
  from,
  people,
  onClose,
  onSent,
}: {
  initial: MailDraft;
  from: string;
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
  const list = (value: string) =>
    value
      .split(/[,;]/)
      .map((s) => s.trim())
      .filter((s) => s !== "");

  const send = async (event: React.FormEvent) => {
    event.preventDefault();
    setSending(true);
    setProblem(null);
    try {
      const draft: MailDraft = { to: list(to), cc: list(cc), subject, text };
      if (initial.inReplyTo !== undefined) draft.inReplyTo = initial.inReplyTo;
      await sendMail(draft);
      onSent();
    } catch (error) {
      setProblem(message(error));
      setSending(false);
    }
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
      <form onSubmit={(event) => void send(event)}>
        <DialogHeader>
          <DialogTitle id="mail-compose-title">
            {initial.inReplyTo !== undefined ? t("mail.compose.reply") : t("mail.compose.title")}
          </DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-3">
          <p className="text-[13px] text-muted">{t("mail.compose.from", { address: from })}</p>
          <label className="flex flex-col gap-1 text-sm">
            <span className="flex items-center justify-between">
              {t("mail.to")}
              {!showCc && (
                <Button type="button" variant="link" size="xs" className="px-0" onClick={() => setShowCc(true)}>
                  {t("mail.compose.addCc")}
                </Button>
              )}
            </span>
            <Input
              value={to}
              onChange={(event) => setTo(event.target.value)}
              list="mail-people"
              autoFocus={initial.to.length === 0}
              autoComplete="off"
              placeholder={people[0]?.address ?? ""}
              required
            />
          </label>
          {showCc && (
            <label className="flex flex-col gap-1 text-sm">
              {t("mail.cc")}
              <Input value={cc} onChange={(event) => setCc(event.target.value)} list="mail-people" autoComplete="off" />
            </label>
          )}
          <datalist id="mail-people">
            {people.map((p) => (
              <option key={p.address} value={p.address}>
                {p.label}
              </option>
            ))}
          </datalist>
          <label className="flex flex-col gap-1 text-sm">
            {t("mail.compose.subject")}
            <Input value={subject} onChange={(event) => setSubject(event.target.value)} maxLength={900} />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="sr-only">{t("mail.compose.body")}</span>
            <Textarea
              value={text}
              onChange={(event) => setText(event.target.value)}
              rows={12}
              autoFocus={initial.to.length > 0}
              className="min-h-56 font-sans"
            />
          </label>
          <p className="text-[12px] text-muted">{t("mail.compose.local")}</p>
          {problem !== null && (
            <p role="alert" className="text-sm text-crit">
              {problem}
            </p>
          )}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose} disabled={sending}>
            {t("mail.compose.discard")}
          </Button>
          <Button type="submit" disabled={sending}>
            {sending ? <Spinner className="size-4" /> : <HugeiconsIcon icon={MailSend01Icon} className="size-4" />}
            {t("mail.compose.send")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
