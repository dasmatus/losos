/* LosOS Contacts: a screen of the admin UI at /contacts, inside its top bar
 * and sidebar like Mail, over LosOS Cloud's /cloud/api/contacts
 * (backend/src/cloud/contacts.rs). The book is the person's address books
 * in LosOS Cloud, the same cards their phone syncs over CardDAV, so
 * Contacts exists wherever LosOS Cloud does.
 *
 * Two views share one card: the person's own contacts, and the people
 * with an account on this box (who need no entry to be reached). A picked
 * person opens beside the list on a wide screen and over it on a phone,
 * with buttons that go straight to Mail, a chat, or a call in LosOS Chat,
 * where the box runs them.
 * Files come and go as .vcf (vCard 4.0, 3.0, 2.1). */

import * as React from "react";
import { useNavigate } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Alert01Icon,
  ArrowLeft01Icon,
  BubbleChatIcon,
  Building03Icon,
  Calendar03Icon,
  Call02Icon,
  ContactBookIcon,
  Copy01Icon,
  Delete02Icon,
  Download04Icon,
  Globe02Icon,
  Location01Icon,
  Mail01Icon,
  Note01Icon,
  PencilEdit02Icon,
  Search01Icon,
  Share08Icon,
  SmartPhone01Icon,
  Upload04Icon,
  UserAdd01Icon,
  UserGroupIcon,
  Video01Icon,
} from "@hugeicons/core-free-icons";
import { PersonAvatar } from "@/components/person";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { PaneHeader } from "@/screens/settings/rows";
import {
  ApiError,
  contactsExportUrl,
  deleteContact,
  getContacts,
  importContacts,
  loadMe,
  sharedApp,
  signInUrl,
  suggestContacts,
  type Contact,
  type ContactFields,
  type ContactSuggestion,
  type ContactsImport,
} from "@/lib/api";
import { useLocale, useT } from "@/lib/i18n-react";
import { chatWithHref, composeMailHref } from "@/lib/intents";
import { contextMenu, type MenuBuilder, type MenuItem } from "@/lib/context-menu";
import { cn } from "@/lib/utils";
import { ContactEditor, labelText } from "./editor";

type Loading =
  | { kind: "loading" }
  | { kind: "signIn" }
  | { kind: "missing" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; contacts: Contact[] };

type View = "all" | "box";

/* Someone the page can reach: a contact of the person's, or someone with
 * an account on this box. */
type Someone = { kind: "contact"; contact: Contact } | { kind: "box"; person: ContactSuggestion };

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function nameOf(s: Someone): string {
  return s.kind === "contact" ? s.contact.name : s.person.name;
}

function keyOf(s: Someone): string {
  return s.kind === "contact" ? `contact:${s.contact.id}` : s.person.key;
}

function emailsOf(s: Someone): string[] {
  return s.kind === "contact" ? (s.contact.emails ?? []).map((e) => e.value) : s.person.emails;
}

function matrixOf(s: Someone): string | null {
  const m = s.kind === "contact" ? s.contact.matrix : s.person.matrixId;
  return m !== undefined && m !== "" ? m : null;
}

function pictureOf(s: Someone): string | null {
  return (s.kind === "contact" ? s.contact.photo : s.person.picture) ?? null;
}

/* The line under a name in the list. */
function detailOf(s: Someone): string {
  if (s.kind === "box") return s.person.matrixId ?? s.person.emails[0] ?? "";
  const c = s.contact;
  return [c.org, c.emails?.[0]?.value, c.phones?.[0]?.value, c.matrix].find((x) => x !== undefined && x !== "") ?? "";
}

/* Accents off, lower case: "stefan" finds "Štefan", as the box's own
 * suggestions do. */
function folded(s: string): string {
  return s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

function matches(s: Someone, q: string): boolean {
  if (q === "") return true;
  const c = s.kind === "contact" ? s.contact : null;
  const hay = [
    nameOf(s),
    ...emailsOf(s),
    matrixOf(s) ?? "",
    c?.org ?? "",
    c?.nickname ?? "",
    ...(c?.phones ?? []).map((p) => p.value),
  ];
  return hay.some((h) => folded(h).includes(q));
}

/* The first letter a list groups by: A to Z after accents, "#" for the rest. */
function letterOf(name: string): string {
  const first = folded(name).charAt(0).toUpperCase();
  return /[A-Z]/.test(first) ? first : "#";
}

function download(url: string): void {
  const a = document.createElement("a");
  a.href = url;
  a.download = "contacts.vcf";
  document.body.append(a);
  a.click();
  a.remove();
}

function Failed({ text, onRetry, className }: { text: string; onRetry?: () => void; className?: string }) {
  const t = useT();
  return (
    <Alert variant="crit" role="alert" className={className}>
      <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <AlertDescription>
        <p>{text}</p>
        {onRetry !== undefined && (
          <Button size="sm" variant="secondary" className="mt-2" onClick={onRetry}>
            {t("contacts.retry")}
          </Button>
        )}
      </AlertDescription>
    </Alert>
  );
}

export default function ContactsScreen() {
  const t = useT();
  const [state, setState] = React.useState<Loading>({ kind: "loading" });

  const load = React.useCallback(async () => {
    try {
      const me = await loadMe();
      if (me === null) {
        setState({ kind: "signIn" });
        return;
      }
      setState({ kind: "ready", contacts: (await getContacts()).contacts });
    } catch (error) {
      if (error instanceof ApiError && error.status === 404) setState({ kind: "missing" });
      else if (error instanceof ApiError && error.status === 403) setState({ kind: "signIn" });
      else setState({ kind: "failed", message: message(error) });
    }
  }, []);

  React.useEffect(() => {
    void load();
  }, [load]);

  if (state.kind === "ready") {
    return <Book contacts={state.contacts} onContacts={(contacts) => setState({ kind: "ready", contacts })} />;
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
          <HugeiconsIcon icon={ContactBookIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </EmptyMedia>
        <EmptyTitle>{t("contacts.title")}</EmptyTitle>
        <EmptyDescription>
          {state.kind === "signIn"
            ? t("contacts.signIn.text")
            : state.kind === "missing"
              ? t("contacts.missing")
              : t("contacts.failed", { detail: state.message })}
        </EmptyDescription>
      </EmptyHeader>
      {state.kind === "signIn" && (
        <Button onClick={() => window.location.assign(signInUrl())}>{t("contacts.signIn.button")}</Button>
      )}
      {state.kind === "failed" && (
        <Button variant="secondary" onClick={() => void load()}>
          {t("contacts.retry")}
        </Button>
      )}
    </Empty>
  );
}

function Book({ contacts, onContacts }: { contacts: Contact[]; onContacts: (contacts: Contact[]) => void }) {
  const t = useT();
  const navigate = useNavigate();
  const [view, setView] = React.useState<View>("all");
  const [query, setQuery] = React.useState("");
  const [box, setBox] = React.useState<ContactSuggestion[] | null>(null);
  const [openKey, setOpenKey] = React.useState<string | null>(null);
  const [column, setColumn] = React.useState<"list" | "person">("list");
  const [editing, setEditing] = React.useState<{ contact?: Contact; initial?: ContactFields } | null>(null);
  const [deleting, setDeleting] = React.useState<Contact | null>(null);
  const [importing, setImporting] = React.useState(false);
  const [exporting, setExporting] = React.useState(false);
  const [listError, setListError] = React.useState<string | null>(null);

  const refresh = React.useCallback(async () => {
    try {
      onContacts((await getContacts()).contacts);
      setListError(null);
    } catch (error) {
      setListError(message(error));
    }
  }, [onContacts]);

  // /contacts?id=<contact> or ?person=<sign-in name> opens that card (a
  // sender's name in Mail, a person in Chat). Read once, then dropped.
  React.useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const id = params.get("id");
    const person = params.get("person");
    if (id === null && person === null) return;
    window.history.replaceState(window.history.state, "", window.location.pathname);
    if (id !== null) {
      setView("all");
      setOpenKey(`contact:${id}`);
    } else if (person !== null) {
      setView("box");
      setOpenKey(`box:${person}`);
    }
    setColumn("person");
  }, []);

  // The people on this box: everyone the box's own suggestions name.
  React.useEffect(() => {
    const controller = new AbortController();
    suggestContacts("", "box", { signal: controller.signal, limit: 50 })
      .then(setBox)
      .catch(() => setBox([]));
    return () => controller.abort();
  }, []);

  const everyone: Someone[] =
    view === "all"
      ? contacts.map((contact) => ({ kind: "contact", contact }))
      : (box ?? []).map((person) => ({ kind: "box", person }));
  const q = folded(query.trim());
  const shown = everyone.filter((s) => matches(s, q));
  const open =
    [...contacts.map((contact): Someone => ({ kind: "contact", contact })), ...(box ?? []).map((person): Someone => ({ kind: "box", person }))].find(
      (s) => keyOf(s) === openKey,
    ) ?? null;

  const pick = (s: Someone) => {
    setOpenKey(keyOf(s));
    setColumn("person");
  };

  const chat = sharedApp("chat");
  const reach = (s: Someone) => {
    const email = emailsOf(s)[0];
    const matrix = matrixOf(s);
    return {
      mail: email !== undefined && sharedApp("mail") ? () => navigate(composeMailHref({ to: [email] })) : undefined,
      // A chat or a call is a real navigation: LosOS Chat is a document of
      // its own at /chat/ under its own policy.
      chat: matrix !== null && chat ? () => window.location.assign(chatWithHref(matrix)) : undefined,
      call: matrix !== null && chat ? () => window.location.assign(chatWithHref(matrix, { call: "voice" })) : undefined,
      video: matrix !== null && chat ? () => window.location.assign(chatWithHref(matrix, { call: "video" })) : undefined,
    };
  };

  const addFrom = (p: ContactSuggestion) =>
    setEditing({
      initial: {
        name: p.name,
        emails: p.emails.map((value) => ({ value, label: "" })),
        ...(p.matrixId !== undefined ? { matrix: p.matrixId } : {}),
      },
    });

  const rowMenu =
    (s: Someone): MenuBuilder =>
    () => {
      const r = reach(s);
      const items: MenuItem[][] = [
        [
          { id: "mail", label: t("contacts.act.mail"), icon: Mail01Icon, hidden: r.mail === undefined, onSelect: () => r.mail?.() },
          { id: "chat", label: t("contacts.act.chat"), icon: BubbleChatIcon, hidden: r.chat === undefined, onSelect: () => r.chat?.() },
          { id: "call", label: t("contacts.act.call"), icon: Call02Icon, hidden: r.call === undefined, onSelect: () => r.call?.() },
          { id: "video", label: t("contacts.act.video"), icon: Video01Icon, hidden: r.video === undefined, onSelect: () => r.video?.() },
        ],
      ];
      if (s.kind === "contact") {
        const c = s.contact;
        items.push([
          { id: "edit", label: t("contacts.act.edit"), icon: PencilEdit02Icon, onSelect: () => setEditing({ contact: c }) },
          { id: "export", label: t("contacts.act.exportOne"), icon: Download04Icon, onSelect: () => download(contactsExportUrl("3.0", [c.id])) },
        ]);
        items.push([{ id: "delete", label: t("contacts.act.delete"), icon: Delete02Icon, danger: true, onSelect: () => setDeleting(c) }]);
      } else {
        items.push([{ id: "add", label: t("contacts.act.addToBook"), icon: UserAdd01Icon, onSelect: () => addFrom(s.person) }]);
      }
      return { label: nameOf(s), sections: items };
    };

  // Letters as headings, for a list long enough to scan.
  const groups: { letter: string; people: Someone[] }[] = [];
  for (const s of shown) {
    const letter = letterOf(nameOf(s));
    const last = groups[groups.length - 1];
    if (last?.letter === letter) last.people.push(s);
    else groups.push({ letter, people: [s] });
  }

  const views: { id: View; label: string; icon: IconSvgElement; count: number | null }[] = [
    { id: "all", label: t("contacts.view.all"), icon: ContactBookIcon, count: contacts.length },
    { id: "box", label: t("contacts.view.box"), icon: UserGroupIcon, count: box?.length ?? null },
  ];

  return (
    <TooltipProvider>
      <div className="flex flex-col">
        <div className="flex flex-wrap items-start gap-3">
          <div className="min-w-0 flex-1">
            <PaneHeader title={t("contacts.title")} summary={t("contacts.summary")} />
          </div>
          <div className="flex shrink-0 gap-2">
            <Button variant="secondary" onClick={() => setImporting(true)}>
              <HugeiconsIcon icon={Upload04Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("contacts.import")}
            </Button>
            <Button variant="secondary" onClick={() => setExporting(true)} disabled={contacts.length === 0}>
              <HugeiconsIcon icon={Download04Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("contacts.export")}
            </Button>
            <Button onClick={() => setEditing({})}>
              <HugeiconsIcon icon={UserAdd01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("contacts.new")}
            </Button>
          </div>
        </div>

        <div className="mb-3 flex flex-wrap items-center gap-2">
          <nav aria-label={t("contacts.views")} className="-mx-1 min-w-0 max-w-full overflow-x-auto px-1">
            <ul className="flex gap-1">
              {views.map((v) => {
                const current = v.id === view;
                return (
                  <li key={v.id} className="shrink-0">
                    <Button
                      variant={current ? "secondary" : "ghost"}
                      size="sm"
                      aria-current={current ? "page" : undefined}
                      onClick={() => {
                        setView(v.id);
                        setColumn("list");
                      }}
                      className={cn("gap-1.5", current && "text-ink")}
                    >
                      <HugeiconsIcon icon={v.icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                      {v.label}
                      {v.count !== null && (
                        <Badge variant="secondary" className="tabular-nums">
                          {v.count}
                        </Badge>
                      )}
                    </Button>
                  </li>
                );
              })}
            </ul>
          </nav>
          <form className="w-full sm:ml-auto sm:w-64" role="search" onSubmit={(event) => event.preventDefault()}>
            <InputGroup>
              <InputGroupAddon>
                <HugeiconsIcon icon={Search01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </InputGroupAddon>
              <InputGroupInput
                type="search"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t("contacts.search")}
                aria-label={t("contacts.search")}
              />
            </InputGroup>
          </form>
        </div>

        <Card className="flex h-[min(46rem,calc(100dvh-15rem))] min-h-[26rem] overflow-hidden">
          <section
            aria-label={views.find((v) => v.id === view)?.label}
            className={cn(
              "min-w-0 flex-1 flex-col md:flex md:w-72 md:flex-none md:border-r md:border-line lg:w-80",
              column === "list" ? "flex" : "hidden",
            )}
          >
            <div className="min-h-0 flex-1 overflow-y-auto">
              {listError !== null && <Failed className="m-3" text={listError} onRetry={() => void refresh()} />}
              {view === "box" && box === null && (
                <div className="grid place-items-center p-8">
                  <Spinner />
                </div>
              )}
              {shown.length === 0 && (view === "all" || box !== null) && (
                <Empty className="m-3 border-0">
                  <EmptyHeader>
                    <EmptyMedia variant="icon">
                      <HugeiconsIcon
                        icon={view === "all" ? ContactBookIcon : UserGroupIcon}
                        strokeWidth={1.5}
                        color="currentColor"
                        aria-hidden="true"
                      />
                    </EmptyMedia>
                    <EmptyDescription>
                      {q !== "" ? t("contacts.noResults") : view === "all" ? t("contacts.empty") : t("contacts.boxEmpty")}
                    </EmptyDescription>
                  </EmptyHeader>
                </Empty>
              )}
              {groups.map((g) => (
                <section key={g.letter} aria-label={g.letter}>
                  <h2 className="sticky top-0 z-10 border-b border-hair bg-surface/95 px-4 py-1 text-[12px] font-semibold text-muted backdrop-blur-sm">
                    {g.letter}
                  </h2>
                  <ul>
                    {g.people.map((s) => (
                      <li key={keyOf(s)} ref={contextMenu(rowMenu(s))}>
                        <button
                          type="button"
                          onClick={() => pick(s)}
                          aria-current={keyOf(s) === openKey ? "true" : undefined}
                          className={cn(
                            "flex w-full items-center gap-3 border-b border-hair px-3 py-2 text-left transition-colors",
                            keyOf(s) === openKey ? "bg-accent-wash" : "hover:bg-sunk",
                          )}
                        >
                          <PersonAvatar person={{ name: keyOf(s), displayName: nameOf(s), picture: pictureOf(s) }} size="sm" />
                          <span className="min-w-0 flex-1">
                            <span className="block truncate text-sm font-medium">{nameOf(s)}</span>
                            <span className="block truncate text-[12.5px] text-muted">{detailOf(s)}</span>
                          </span>
                        </button>
                      </li>
                    ))}
                  </ul>
                </section>
              ))}
            </div>
          </section>

          <article
            className={cn("min-w-0 flex-1 flex-col md:flex", column === "person" ? "flex" : "hidden")}
            aria-label={open !== null ? nameOf(open) : undefined}
          >
            {open === null ? (
              <Empty className="m-4 border-0">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <HugeiconsIcon icon={ContactBookIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  </EmptyMedia>
                  <EmptyDescription>{t("contacts.pick")}</EmptyDescription>
                </EmptyHeader>
              </Empty>
            ) : (
              <Person
                someone={open}
                reach={reach(open)}
                onBack={() => setColumn("list")}
                onEdit={open.kind === "contact" ? () => setEditing({ contact: open.contact }) : undefined}
                onDelete={open.kind === "contact" ? () => setDeleting(open.contact) : undefined}
                onAdd={open.kind === "box" ? () => addFrom(open.person) : undefined}
              />
            )}
          </article>
        </Card>
      </div>

      {editing !== null && (
        <ContactEditor
          {...(editing.contact !== undefined ? { contact: editing.contact } : {})}
          {...(editing.initial !== undefined ? { initial: editing.initial } : {})}
          onClose={() => setEditing(null)}
          onSaved={(saved) => {
            setEditing(null);
            toast.done(t("contacts.done.saved"));
            const rest = contacts.filter((c) => c.id !== saved.id);
            onContacts([...rest, saved].sort((a, b) => folded(a.name).localeCompare(folded(b.name))));
            setView("all");
            setOpenKey(`contact:${saved.id}`);
            setColumn("person");
          }}
        />
      )}
      {deleting !== null && (
        <DeleteDialog
          contact={deleting}
          onClose={() => setDeleting(null)}
          onDeleted={() => {
            onContacts(contacts.filter((c) => c.id !== deleting.id));
            if (openKey === `contact:${deleting.id}`) {
              setOpenKey(null);
              setColumn("list");
            }
            setDeleting(null);
            toast.done(t("contacts.done.deleted"));
          }}
        />
      )}
      {importing && (
        <ImportDialog
          onClose={() => setImporting(false)}
          onImported={() => {
            setView("all");
            void refresh();
          }}
        />
      )}
      {exporting && <ExportDialog onClose={() => setExporting(false)} />}
    </TooltipProvider>
  );
}

/* One way to reach someone, as a row: what it is, and a button for it. */
function Line({
  icon,
  label,
  value,
  href,
  onOpen,
}: {
  icon: IconSvgElement;
  label: string;
  value: string;
  href?: string;
  onOpen?: () => void;
}) {
  const t = useT();
  return (
    <li className="group flex items-start gap-3 py-2">
      <HugeiconsIcon icon={icon} size={18} strokeWidth={1.5} className="mt-0.5 shrink-0 text-muted" aria-hidden="true" />
      <div className="min-w-0 flex-1">
        <p className="text-[12px] text-muted">{label}</p>
        {href !== undefined ? (
          <a href={href} className="text-sm break-words text-accent hover:underline" rel="noreferrer">
            {value}
          </a>
        ) : onOpen !== undefined ? (
          <button type="button" onClick={onOpen} className="text-left text-sm break-words text-accent hover:underline">
            {value}
          </button>
        ) : (
          <p className="text-sm break-words whitespace-pre-line">{value}</p>
        )}
      </div>
      <Tooltip>
        <TooltipTrigger
          render={
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={`${t("contacts.act.copy")}: ${label}`}
              className="opacity-60 group-hover:opacity-100 focus-visible:opacity-100"
              onClick={() =>
                void navigator.clipboard
                  .writeText(value)
                  .then(() => toast.done(t("contacts.copied")))
                  .catch(() => undefined)
              }
            />
          }
        >
          <HugeiconsIcon icon={Copy01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </TooltipTrigger>
        <TooltipContent side="left">{t("contacts.act.copy")}</TooltipContent>
      </Tooltip>
    </li>
  );
}

function birthdayText(b: string, locale: string): string {
  const [y, m, d] = b.startsWith("--") ? ["2000", b.slice(2, 4), b.slice(5, 7)] : b.split("-");
  const date = new Date(Date.UTC(Number(y), Number(m) - 1, Number(d)));
  if (Number.isNaN(date.getTime())) return b;
  return new Intl.DateTimeFormat(locale, {
    day: "numeric",
    month: "long",
    ...(b.startsWith("--") ? {} : { year: "numeric" }),
    timeZone: "UTC",
  }).format(date);
}

function Person({
  someone,
  reach,
  onBack,
  onEdit,
  onDelete,
  onAdd,
}: {
  someone: Someone;
  reach: { mail?: (() => void) | undefined; chat?: (() => void) | undefined; call?: (() => void) | undefined; video?: (() => void) | undefined };
  onBack: () => void;
  onEdit: (() => void) | undefined;
  onDelete: (() => void) | undefined;
  onAdd: (() => void) | undefined;
}) {
  const t = useT();
  const locale = useLocale();
  const c = someone.kind === "contact" ? someone.contact : null;
  const subtitle = c !== null ? [c.title, c.org].filter((x) => x !== undefined && x !== "").join(" · ") : t("contacts.onBox");

  const action = (icon: IconSvgElement, label: string, run: (() => void) | undefined) =>
    run !== undefined && (
      <Button
        variant="secondary"
        onClick={run}
        className="h-auto min-h-16 flex-1 flex-col gap-1.5 py-3 sm:min-w-24 sm:flex-none sm:px-6"
        key={label}
      >
        <HugeiconsIcon icon={icon} size={22} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        <span className="text-[13px]">{label}</span>
      </Button>
    );

  const tool = (icon: IconSvgElement, label: string, run: () => void) => (
    <Tooltip>
      <TooltipTrigger render={<Button variant="ghost" size="icon-sm" onClick={run} aria-label={label} />}>
        <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );

  return (
    <>
      <div className="flex items-center gap-1 border-b border-hair px-3 py-2">
        <Button variant="ghost" size="icon-sm" className="md:hidden" onClick={onBack} aria-label={t("contacts.back")}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        <span className="ml-auto" />
        {onAdd !== undefined && tool(UserAdd01Icon, t("contacts.act.addToBook"), onAdd)}
        {onEdit !== undefined && tool(PencilEdit02Icon, t("contacts.act.edit"), onEdit)}
        {c !== null && tool(Download04Icon, t("contacts.act.exportOne"), () => download(contactsExportUrl("3.0", [c.id])))}
        {onDelete !== undefined && tool(Delete02Icon, t("contacts.act.delete"), onDelete)}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="flex flex-col items-center gap-2 px-5 pt-6 pb-4 text-center">
          <PersonAvatar
            person={{ name: keyOf(someone), displayName: nameOf(someone), picture: pictureOf(someone) }}
            size="xl"
          />
          <h2 className="mt-1 text-[19px] leading-tight font-semibold tracking-tight break-words">{nameOf(someone)}</h2>
          {c?.nickname !== undefined && c.nickname !== "" && <p className="text-sm text-muted">“{c.nickname}”</p>}
          {subtitle !== "" && <p className="text-[13px] text-muted">{subtitle}</p>}
          <div className="mt-4 flex w-full max-w-lg gap-2.5 sm:w-auto">
            {action(Mail01Icon, t("contacts.act.mail"), reach.mail)}
            {action(BubbleChatIcon, t("contacts.act.chat"), reach.chat)}
            {action(Call02Icon, t("contacts.act.call"), reach.call)}
            {action(Video01Icon, t("contacts.act.video"), reach.video)}
          </div>
        </div>
        <ul className="mx-5 mb-5 divide-y divide-hair border-t border-hair">
          {someone.kind === "box" &&
            someone.person.emails.map((e) => (
              <Line key={e} icon={Mail01Icon} label={t("contacts.field.emails")} value={e} {...(reach.mail !== undefined ? { onOpen: reach.mail } : {})} />
            ))}
          {c?.emails?.map((e, i) => (
            <Line
              key={`e${i}`}
              icon={Mail01Icon}
              label={e.label !== "" ? `${t("contacts.field.emails")} · ${labelText(e.label, t)}` : t("contacts.field.emails")}
              value={e.value}
              href={`mailto:${e.value}`}
            />
          ))}
          {c?.phones?.map((p, i) => (
            <Line
              key={`p${i}`}
              icon={SmartPhone01Icon}
              label={p.label !== "" ? `${t("contacts.field.phones")} · ${labelText(p.label, t)}` : t("contacts.field.phones")}
              value={p.value}
              href={`tel:${p.value.replace(/[^\d+]/g, "")}`}
            />
          ))}
          {matrixOf(someone) !== null && (
            <Line
              icon={BubbleChatIcon}
              label={t("contacts.field.matrix")}
              value={matrixOf(someone) ?? ""}
              {...(reach.chat !== undefined ? { onOpen: reach.chat } : {})}
            />
          )}
          {c?.mesh !== undefined && c.mesh !== "" && <Line icon={Share08Icon} label={t("contacts.field.mesh")} value={c.mesh} />}
          {c?.addresses?.map((a, i) => (
            <Line
              key={`a${i}`}
              icon={Location01Icon}
              label={a.label !== "" ? `${t("contacts.field.addresses")} · ${labelText(a.label, t)}` : t("contacts.field.addresses")}
              value={[a.street, [a.code, a.locality].filter((x) => x !== "").join(" "), a.region, a.country]
                .filter((x) => x !== "")
                .join("\n")}
            />
          ))}
          {c?.org !== undefined && c.org !== "" && c.title !== undefined && c.title !== "" && (
            <Line icon={Building03Icon} label={t("contacts.field.org")} value={c.org} />
          )}
          {c?.birthday !== undefined && c.birthday !== "" && (
            <Line icon={Calendar03Icon} label={t("contacts.field.birthday")} value={birthdayText(c.birthday, locale)} />
          )}
          {c?.urls?.map((u, i) => (
            <Line key={`u${i}`} icon={Globe02Icon} label={t("contacts.field.urls")} value={u.value} href={u.value} />
          ))}
          {c?.note !== undefined && c.note !== "" && <Line icon={Note01Icon} label={t("contacts.field.note")} value={c.note} />}
        </ul>
      </div>
    </>
  );
}

function DeleteDialog({ contact, onClose, onDeleted }: { contact: Contact; onClose: () => void; onDeleted: () => void }) {
  const t = useT();
  const [busy, setBusy] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);
  return (
    <Dialog open onOpenChange={(next) => !next && !busy && onClose()} labelledBy="contact-delete-title">
      <DialogHeader>
        <DialogTitle id="contact-delete-title">{t("contacts.act.delete")}</DialogTitle>
        <DialogDescription>{t("contacts.confirmDelete", { name: contact.name })}</DialogDescription>
      </DialogHeader>
      {problem !== null && (
        <DialogBody>
          <Failed text={problem} />
        </DialogBody>
      )}
      <DialogFooter>
        <Button variant="ghost" onClick={onClose} disabled={busy}>
          {t("contacts.edit.cancel")}
        </Button>
        <Button
          variant="destructive"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            deleteContact(contact.id)
              .then(onDeleted)
              .catch((error: unknown) => {
                setProblem(message(error));
                setBusy(false);
              });
          }}
        >
          {busy && <Spinner className="size-4" />}
          {t("contacts.act.delete")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

function ImportDialog({ onClose, onImported }: { onClose: () => void; onImported: () => void }) {
  const t = useT();
  const input = React.useRef<HTMLInputElement>(null);
  const [busy, setBusy] = React.useState<string | null>(null);
  const [done, setDone] = React.useState<ContactsImport | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [over, setOver] = React.useState(false);

  const take = (file: File) => {
    setBusy(file.name);
    setProblem(null);
    setDone(null);
    importContacts(file)
      .then((report) => {
        setDone(report);
        onImported();
      })
      .catch((error: unknown) => setProblem(message(error)))
      .finally(() => setBusy(null));
  };

  return (
    <Dialog open onOpenChange={(next) => !next && busy === null && onClose()} labelledBy="contacts-import-title">
      <DialogHeader>
        <DialogTitle id="contacts-import-title">{t("contacts.importTitle")}</DialogTitle>
        <DialogDescription>{t("contacts.importLead")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        <input
          ref={input}
          type="file"
          accept=".vcf,.vcard,text/vcard,text/x-vcard"
          hidden
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (file !== undefined) take(file);
            event.target.value = "";
          }}
        />
        <div
          onDragOver={(event) => {
            event.preventDefault();
            setOver(true);
          }}
          onDragLeave={() => setOver(false)}
          onDrop={(event) => {
            event.preventDefault();
            setOver(false);
            const file = event.dataTransfer.files[0];
            if (file !== undefined && busy === null) take(file);
          }}
          className={cn(
            "flex flex-col items-center gap-2 rounded-card border border-dashed border-line px-4 py-8 text-center transition-colors",
            over && "border-accent bg-accent-wash",
          )}
        >
          {busy !== null ? (
            <p className="flex items-center gap-2 text-sm text-muted">
              <Spinner size={16} />
              {t("contacts.importing", { file: busy })}
            </p>
          ) : (
            <>
              <HugeiconsIcon icon={Upload04Icon} size={24} strokeWidth={1.5} className="text-muted" aria-hidden="true" />
              <p className="text-sm text-muted">{t("contacts.importDrop")}</p>
              <Button variant="secondary" size="sm" onClick={() => input.current?.click()}>
                {t("contacts.importChoose")}
              </Button>
            </>
          )}
        </div>
        {done !== null && (
          <Alert role="status">
            <AlertDescription>
              <p className="font-medium">{t("contacts.importDone", { added: done.added, updated: done.updated })}</p>
              {done.skipped > 0 && <p>{t("contacts.importSkipped", { count: done.skipped })}</p>}
              {done.photosDropped > 0 && <p>{t("contacts.importPhotos", { count: done.photosDropped })}</p>}
              {done.leftOut > 0 && <p>{t("contacts.importLeftOut", { count: done.leftOut })}</p>}
            </AlertDescription>
          </Alert>
        )}
        {problem !== null && <Failed text={problem} />}
      </DialogBody>
      <DialogFooter>
        <Button variant={done !== null ? "default" : "ghost"} onClick={onClose} disabled={busy !== null}>
          {t("contacts.importClose")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

function ExportDialog({ onClose }: { onClose: () => void }) {
  const t = useT();
  return (
    <Dialog open onOpenChange={(next) => !next && onClose()} labelledBy="contacts-export-title">
      <DialogHeader>
        <DialogTitle id="contacts-export-title">{t("contacts.exportTitle")}</DialogTitle>
        <DialogDescription>{t("contacts.exportLead")}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="secondary" onClick={() => download(contactsExportUrl("4.0"))}>
          {t("contacts.export4")}
        </Button>
        <Button
          onClick={() => {
            download(contactsExportUrl("3.0"));
            onClose();
          }}
        >
          <HugeiconsIcon icon={Download04Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("contacts.export3")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
