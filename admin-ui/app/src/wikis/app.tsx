/* LosOS Wiki: a screen of the admin UI at /wikis, inside its top bar and
 * sidebar like Mail. It is the box's directory of wikis and where a space's
 * pages are written.
 *
 * On a wide screen one card holds three columns: every wiki this person may
 * read (the spaces of LosOS Wiki, the knowledge bases published from LosOS
 * Cloud's Notes, and the handbook), the open one's pages as a tree, and the
 * open page, read or edited in place. The search above the card looks
 * through all of them. Narrower, the card shows one column at a time with a
 * way back.
 *
 * A space is a LosOS Git repository's wiki (backend/src/forge/wiki.rs):
 * pages of Markdown in `<repo>.wiki.git`, every save a commit, readable and
 * writable by whoever may read and push to the repository, and cloned with
 * git like any repository. Each space is also a site of its own at
 * /wiki/<owner>/<space>/ (src/wiki/site.tsx), open to whoever the
 * repository is: anyone with the address, the people on this box, or its
 * members. Who sees what is decided by LosOS Git and LosOS Cloud, never
 * here: each list is what that app answers for this person. */

import * as React from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Add01Icon,
  ArrowLeft01Icon,
  BookOpen01Icon,
  Delete02Icon,
  Globe02Icon,
  LibraryIcon,
  LinkSquare02Icon,
  Notebook01Icon,
  PencilEdit02Icon,
  Search01Icon,
  SquareLock02Icon,
  UserGroupIcon,
} from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { PaneHeader } from "@/screens/settings/rows";
import { git, GitError, type Visibility, type Wiki, type WikiHit, type WikiPage, type WikiSpace } from "@/git/api";
import { VisibilityField } from "@/git/parts";
import { handbookHome, handbookHref, handbookSearchHref, type HandbookEntry } from "@/lib/handbook";
import type { MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { pageAt, pageHref, titleOf } from "@/wiki/links";
import { DeletePageDialog, PageTree, WikiArticle, WikiEditor } from "@/wiki/parts";

// ── What the other apps answer ─────────────────────────────────────────────

/** A knowledge base as LosOS Cloud lists it (backend/src/cloud/notes/kb.rs). */
interface KbSite {
  slug: string;
  title: string;
  audience: "anyone" | "box";
  notes: number;
  updated: number | null;
  mine: boolean;
}

interface KbHit {
  slug: string;
  site: string;
  path: string;
  line: number;
  text: string;
}

interface KbNote {
  path: string;
  title: string | null;
}

/** GET /kb/api/: the sites this person may read. Off the mesh there are
 *  none and the call answers 404. */
async function knowledgeBases(q: string, signal?: AbortSignal): Promise<{ sites: KbSite[]; hits: KbHit[] }> {
  const init: RequestInit = { credentials: "same-origin", cache: "no-store" };
  if (signal !== undefined) init.signal = signal;
  const res = await fetch(`/kb/api/${q === "" ? "" : `?q=${encodeURIComponent(q)}`}`, init);
  if (!res.ok) return { sites: [], hits: [] };
  return (await res.json()) as { sites: KbSite[]; hits: KbHit[] };
}

async function kbNotes(slug: string, signal?: AbortSignal): Promise<KbNote[]> {
  const init: RequestInit = { credentials: "same-origin", cache: "no-store" };
  if (signal !== undefined) init.signal = signal;
  const res = await fetch(`/kb/api/${encodeURIComponent(slug)}/site`, init);
  if (!res.ok) throw new Error(String(res.status));
  const site = (await res.json()) as { vault: { notes: KbNote[] } };
  return site.vault.notes;
}

const kbPage = (slug: string, path: string) =>
  `/kb/${encodeURIComponent(slug)}/${path.replace(/\.(note|md|markdown)$/iu, "").split("/").map(encodeURIComponent).join("/")}`;

/** The handbook's chapters, in its own order. */
const CHAPTERS: { entry: HandbookEntry; label: MessageKey }[] = [
  { entry: "chapter-start", label: "wiki.handbook.start" },
  { entry: "chapter-types", label: "wiki.handbook.types" },
  { entry: "chapter-manual", label: "wiki.handbook.manual" },
  { entry: "chapter-troubleshooting", label: "wiki.handbook.troubleshooting" },
  { entry: "chapter-reference", label: "wiki.handbook.reference" },
  { entry: "chapter-in-depth", label: "wiki.handbook.inDepth" },
];

/** Who may read a wiki, in the reader's words. */
const AUDIENCE: Record<Visibility | "anyone" | "box" | "lan", { icon: IconSvgElement; label: MessageKey }> = {
  public: { icon: Globe02Icon, label: "wiki.audience.anyone" },
  anyone: { icon: Globe02Icon, label: "wiki.audience.anyone" },
  internal: { icon: UserGroupIcon, label: "wiki.audience.box" },
  box: { icon: UserGroupIcon, label: "wiki.audience.box" },
  private: { icon: SquareLock02Icon, label: "wiki.audience.members" },
  lan: { icon: UserGroupIcon, label: "wiki.audience.lan" },
};

function Audience({ who }: { who: keyof typeof AUDIENCE }) {
  const t = useT();
  const a = AUDIENCE[who];
  return (
    <span className="inline-flex items-center gap-1 text-[12px] text-muted" data-audience={who}>
      <HugeiconsIcon icon={a.icon} size={12} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      {t(a.label)}
    </span>
  );
}

// ── Where the screen is ────────────────────────────────────────────────────

type Open =
  | { kind: "none" }
  | { kind: "handbook" }
  | { kind: "kb"; slug: string }
  | { kind: "space"; owner: string; repo: string; page: string };

function openAt(pathname: string, search: string): Open {
  const q = new URLSearchParams(search);
  if (q.has("handbook")) return { kind: "handbook" };
  const kb = q.get("kb");
  if (kb !== null && kb !== "") return { kind: "kb", slug: kb };
  const parts = pathname.replace(/^\/wikis\/?/u, "").split("/");
  const [owner, repo] = parts;
  if (owner === undefined || owner === "" || repo === undefined || repo === "") return { kind: "none" };
  return { kind: "space", owner: decodeURIComponent(owner), repo: decodeURIComponent(repo), page: pageAt(spaceBase(owner, repo), pathname) };
}

const spaceBase = (owner: string, repo: string) => `/wikis/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`;

function hrefOf(open: Open): string {
  switch (open.kind) {
    case "none":
      return "/wikis";
    case "handbook":
      return "/wikis?handbook";
    case "kb":
      return `/wikis?kb=${encodeURIComponent(open.slug)}`;
    case "space":
      return pageHref(spaceBase(open.owner, open.repo), open.page);
  }
}

function useLoaded<T>(load: (signal: AbortSignal) => Promise<T>, deps: React.DependencyList) {
  const [state, setState] = React.useState<{ data: T | null; error: unknown }>({ data: null, error: null });
  const [round, setRound] = React.useState(0);
  React.useEffect(() => {
    const controller = new AbortController();
    setState({ data: null, error: null });
    load(controller.signal).then(
      (data) => !controller.signal.aborted && setState({ data, error: null }),
      (error: unknown) => !controller.signal.aborted && setState({ data: null, error }),
    );
    return () => controller.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, round]);
  return { ...state, reload: () => setRound((r) => r + 1) };
}

function explain(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

// ── The screen ─────────────────────────────────────────────────────────────

export default function WikisScreen() {
  const t = useT();
  const locale = useLocale();
  const navigate = useNavigate();
  const location = useLocation();
  const open = openAt(location.pathname, location.search);
  const go = React.useCallback((next: Open) => navigate(hrefOf(next)), [navigate]);

  const spaces = useLoaded((signal) => git.wikis("", signal), []);
  const kbs = useLoaded((signal) => knowledgeBases("", signal), []);
  const [query, setQuery] = React.useState("");
  const [search, setSearch] = React.useState("");
  const [making, setMaking] = React.useState(false);
  const [column, setColumn] = React.useState<"list" | "pages" | "page">(open.kind === "none" ? "list" : "page");

  const gitDown = spaces.error !== null;
  const spaceList = spaces.data?.wikis ?? [];
  const kbList = kbs.data?.sites ?? [];
  const words = query.trim().toLowerCase();
  const matches = (s: string) => words === "" || s.toLowerCase().includes(words);

  const pick = (next: Open) => {
    setSearch("");
    go(next);
    setColumn("pages");
  };

  const item = (key: string, current: boolean, icon: IconSvgElement, title: string, sub: React.ReactNode, onClick: () => void) => (
    <li key={key}>
      <button
        type="button"
        aria-current={current ? "page" : undefined}
        onClick={onClick}
        data-wiki-entry={key}
        className={cn(
          "flex w-full items-start gap-2 rounded-control px-2 py-1.5 text-left outline-none transition-colors duration-150",
          "focus-visible:ring-2 focus-visible:ring-accent/40",
          current ? "bg-accent-wash text-ink" : "text-muted hover:bg-sunk hover:text-ink",
        )}
      >
        <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="mt-0.5 shrink-0" />
        <span className="flex min-w-0 flex-1 flex-col">
          <span className={cn("truncate text-[13px]", current && "font-medium")}>{title}</span>
          <span className="truncate">{sub}</span>
        </span>
      </button>
    </li>
  );

  const groupTitle = (label: string, count: number) => (
    <h3 className="flex items-center gap-2 px-2 pt-3 pb-1 text-[11.5px] font-semibold tracking-wide text-faint uppercase first:pt-1">
      <span className="flex-1">{label}</span>
      <span className="tabular-nums">{count}</span>
    </h3>
  );

  const shownSpaces = spaceList.filter((s) => matches(s.full_name) || matches(s.description));
  const shownKbs = kbList.filter((k) => matches(k.title));
  const handbookShown = matches(t("wiki.handbook.title"));

  return (
    <div className="flex flex-col" data-wikis>
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <PaneHeader title={t("wiki.title")} summary={t("wiki.summary")} />
        </div>
        {!gitDown && (
          <Button onClick={() => setMaking(true)} className="shrink-0">
            <HugeiconsIcon icon={Add01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("wiki.space.new")}
          </Button>
        )}
      </div>

      <form
        className="mb-3 flex w-full sm:ml-auto sm:w-80"
        role="search"
        onSubmit={(event) => {
          event.preventDefault();
          setSearch(query.trim());
          setColumn("pages");
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
            placeholder={t("wiki.search")}
            aria-label={t("wiki.searchLabel")}
            data-wiki-search
          />
        </InputGroup>
      </form>

      <Card className="flex h-[min(46rem,calc(100dvh-15rem))] min-h-[26rem] overflow-hidden">
        {/* Every wiki */}
        <nav
          aria-label={t("wiki.all")}
          className={cn("min-w-0 flex-1 flex-col border-r border-line lg:flex lg:w-60 lg:flex-none xl:w-64", column === "list" ? "flex" : "hidden")}
        >
          <div className="flex items-center gap-2 border-b border-hair px-4 py-2.5">
            <h2 className="min-w-0 flex-1 truncate text-[15px] font-semibold tracking-tight">{t("wiki.all")}</h2>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-1.5" data-wiki-directory>
            {spaces.data === null && spaces.error === null ? (
              <div className="grid place-items-center p-6">
                <Spinner />
              </div>
            ) : (
              <>
                {groupTitle(t("wiki.group.spaces"), shownSpaces.length)}
                {gitDown && <p className="px-2 py-1 text-[12.5px] text-muted">{t("wiki.gitDown")}</p>}
                {!gitDown && shownSpaces.length === 0 && (
                  <p className="px-2 py-1 text-[12.5px] text-muted">{words === "" ? t("wiki.noSpaces") : t("wiki.noMatch")}</p>
                )}
                <ul className="flex flex-col gap-px">
                  {shownSpaces.map((s) =>
                    item(
                      `space:${s.full_name}`,
                      open.kind === "space" && open.owner.toLowerCase() === s.owner.toLowerCase() && open.repo.toLowerCase() === s.repo.toLowerCase(),
                      LibraryIcon,
                      s.repo,
                      <span className="flex items-center gap-2">
                        <span className="text-[12px] text-muted">{t("wiki.pages", { count: s.pages })}</span>
                        <Audience who={s.visibility} />
                      </span>,
                      () => pick({ kind: "space", owner: s.owner, repo: s.repo, page: "" }),
                    ),
                  )}
                </ul>
                {kbList.length > 0 && (
                  <>
                    {groupTitle(t("wiki.group.kb"), shownKbs.length)}
                    <ul className="flex flex-col gap-px">
                      {shownKbs.map((k) =>
                        item(
                          `kb:${k.slug}`,
                          open.kind === "kb" && open.slug === k.slug,
                          Notebook01Icon,
                          k.title,
                          <span className="flex items-center gap-2">
                            <span className="text-[12px] text-muted">{t("wiki.notes", { count: k.notes })}</span>
                            <Audience who={k.audience} />
                          </span>,
                          () => pick({ kind: "kb", slug: k.slug }),
                        ),
                      )}
                    </ul>
                  </>
                )}
                {handbookShown && (
                  <>
                    {groupTitle(t("wiki.group.builtIn"), 1)}
                    <ul className="flex flex-col gap-px">
                      {item(
                        "handbook",
                        open.kind === "handbook",
                        BookOpen01Icon,
                        t("wiki.handbook.title"),
                        <Audience who="lan" />,
                        () => pick({ kind: "handbook" }),
                      )}
                    </ul>
                  </>
                )}
              </>
            )}
          </div>
        </nav>

        {search !== "" ? (
          <Results
            q={search}
            locale={locale}
            className={cn("md:flex md:w-80 md:flex-none md:border-r md:border-line", column === "pages" ? "flex" : "hidden")}
            onBack={() => setColumn("list")}
            onOpen={(hit) => {
              go({ kind: "space", owner: hit.owner, repo: hit.repo, page: hit.page });
              setColumn("page");
            }}
          />
        ) : null}

        {open.kind === "space" ? (
          <Space
            key={`${open.owner}/${open.repo}`}
            owner={open.owner}
            repo={open.repo}
            page={open.page}
            column={column}
            searching={search !== ""}
            setColumn={setColumn}
            go={(page) => go({ ...open, page })}
            onChanged={spaces.reload}
          />
        ) : open.kind === "kb" ? (
          <KnowledgeBase site={kbList.find((k) => k.slug === open.slug) ?? null} slug={open.slug} hidden={search !== "" || column === "list"} onBack={() => setColumn("list")} />
        ) : open.kind === "handbook" ? (
          <Handbook hidden={search !== "" || column === "list"} onBack={() => setColumn("list")} />
        ) : search === "" ? (
          <Empty className="m-4 hidden border-0 lg:flex">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={LibraryIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>{t("wiki.pick.title")}</EmptyTitle>
              <EmptyDescription>{t("wiki.pick.text")}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : null}
      </Card>
      {making && (
        <NewSpace
          onClose={() => setMaking(false)}
          onMade={(owner, repo) => {
            setMaking(false);
            spaces.reload();
            go({ kind: "space", owner, repo, page: "" });
            setColumn("page");
          }}
        />
      )}
    </div>
  );
}

// ── Search ─────────────────────────────────────────────────────────────────

function Results({
  q,
  locale,
  className,
  onBack,
  onOpen,
}: {
  q: string;
  locale: string;
  className: string;
  onBack: () => void;
  onOpen: (hit: WikiHit) => void;
}) {
  const t = useT();
  const found = useLoaded(
    async (signal) => {
      const [g, k] = await Promise.all([git.wikis(q, signal).catch(() => ({ wikis: [], hits: [] })), knowledgeBases(q, signal)]);
      return { git: g.hits, kb: k.hits };
    },
    [q],
  );
  const total = (found.data?.git.length ?? 0) + (found.data?.kb.length ?? 0);
  return (
    <section aria-label={t("wiki.results", { query: q })} className={cn("min-w-0 flex-1 flex-col", className)} data-wiki-results>
      <div className="flex items-center gap-2 border-b border-hair px-4 py-2.5">
        <Button variant="ghost" size="icon-sm" className="lg:hidden" onClick={onBack} aria-label={t("wiki.back")}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        <h2 className="min-w-0 flex-1 truncate text-[15px] font-semibold tracking-tight">{t("wiki.results", { query: q })}</h2>
        {found.data !== null && <span className="text-[13px] text-muted tabular-nums">{total}</span>}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-1.5">
        {found.data === null ? (
          <div className="grid place-items-center p-6">
            <Spinner />
          </div>
        ) : (
          <ul className="flex flex-col gap-px">
            {found.data.git.map((h, i) => (
              <li key={`g${i}`}>
                <button
                  type="button"
                  onClick={() => onOpen(h)}
                  className="block w-full rounded-control px-2 py-1.5 text-left hover:bg-sunk focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none"
                  data-hit={`${h.repo}/${h.page}`}
                >
                  <span className="block truncate text-[13px] font-medium text-ink">{h.title}</span>
                  <span className="block truncate text-[12px] text-muted">{h.repo}</span>
                  {h.text !== null && <span className="line-clamp-2 text-[12px] text-muted">{h.text}</span>}
                </button>
              </li>
            ))}
            {found.data.kb.map((h, i) => (
              <li key={`k${i}`}>
                <a
                  href={kbPage(h.slug, h.path)}
                  target="_blank"
                  rel="noopener"
                  className="block rounded-control px-2 py-1.5 hover:bg-sunk focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none"
                  data-hit={`${h.slug}/${h.path}`}
                >
                  <span className="block truncate text-[13px] font-medium text-ink">{titleOf(h.path.replace(/\.(note|md|markdown)$/iu, ""))}</span>
                  <span className="block truncate text-[12px] text-muted">{h.site}</span>
                  {h.text !== "" && <span className="line-clamp-2 text-[12px] text-muted">{h.text}</span>}
                </a>
              </li>
            ))}
            {total === 0 && <li className="px-2 py-2 text-[13px] text-muted">{t("wiki.noResults")}</li>}
            <li className="mt-2 border-t border-hair pt-2">
              <a
                href={handbookSearchHref(q, locale)}
                target="_blank"
                rel="noopener"
                className="flex items-center gap-2 rounded-control px-2 py-1.5 text-[13px] text-accent hover:bg-sunk"
                data-hit="handbook"
              >
                <HugeiconsIcon icon={BookOpen01Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("wiki.searchHandbook", { query: q })}
              </a>
            </li>
          </ul>
        )}
      </div>
    </section>
  );
}

// ── A space ────────────────────────────────────────────────────────────────

type Mode = { kind: "read" } | { kind: "edit" } | { kind: "new"; parent: string; title: string };

function Space({
  owner,
  repo,
  page,
  column,
  searching,
  setColumn,
  go,
  onChanged,
}: {
  owner: string;
  repo: string;
  page: string;
  column: "list" | "pages" | "page";
  searching: boolean;
  setColumn: (c: "list" | "pages" | "page") => void;
  go: (page: string) => void;
  onChanged: () => void;
}) {
  const t = useT();
  const base = spaceBase(owner, repo);
  const wiki = useLoaded((signal) => git.wiki(owner, repo, signal), [owner, repo]);
  const name = page !== "" ? page : (wiki.data?.home ?? "");
  const opened = useLoaded(
    (signal) => (name === "" ? Promise.resolve(null) : git.wikiPage(owner, repo, name, signal)),
    [owner, repo, name],
  );
  const [mode, setMode] = React.useState<Mode>({ kind: "read" });
  const [deleting, setDeleting] = React.useState(false);
  React.useEffect(() => setMode({ kind: "read" }), [name]);

  const w: Wiki | null = wiki.data;
  const names = React.useMemo(() => (w?.pages ?? []).map((p) => p.name), [w]);
  const site = `/wiki/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/`;
  const saved = (p: WikiPage) => {
    toast.done(t("wiki.edit.saved"));
    wiki.reload();
    onChanged();
    setMode({ kind: "read" });
    if (p.name === name) opened.reload();
    else go(p.name);
  };

  return (
    <>
      {!searching && (
        <section
          aria-label={repo}
          className={cn("min-w-0 flex-1 flex-col md:flex md:w-60 md:flex-none md:border-r md:border-line xl:w-64", column === "pages" ? "flex" : "hidden")}
          data-wiki-space={`${owner}/${repo}`}
        >
          <div className="flex items-center gap-1 border-b border-hair py-1.5 pr-1.5 pl-2 lg:pl-4">
            <Button variant="ghost" size="icon-sm" className="lg:hidden" onClick={() => setColumn("list")} aria-label={t("wiki.back")}>
              <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </Button>
            <h2 className="min-w-0 flex-1 truncate text-[15px] font-semibold tracking-tight">{repo}</h2>
            {w?.writable === true && (
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={t("wiki.page.new")}
                title={t("wiki.page.new")}
                onClick={() => {
                  setMode({ kind: "new", parent: "", title: w.exists ? "" : "Home" });
                  setColumn("page");
                }}
              >
                <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </Button>
            )}
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-1.5">
            {wiki.error !== null ? (
              <p className="p-2 text-[13px] text-crit">{explain(wiki.error)}</p>
            ) : w === null ? (
              <div className="grid place-items-center p-6">
                <Spinner />
              </div>
            ) : (
              <PageTree
                names={names}
                current={opened.data?.name ?? name}
                base={base}
                label={t("wiki.pagesOf", { space: repo })}
                onOpen={(n) => {
                  go(n);
                  setColumn("page");
                }}
              />
            )}
          </div>
          {w !== null && (
            <div className="flex flex-col gap-1.5 border-t border-hair px-4 py-3 text-[12.5px]">
              <span className="text-muted">{owner}</span>
              <a href={site} target="_blank" rel="noopener" className="inline-flex items-center gap-1.5 text-accent hover:underline" data-wiki-site>
                <HugeiconsIcon icon={LinkSquare02Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("wiki.space.site")}
              </a>
              <span className="truncate font-mono text-[11.5px] text-faint" title={w.clone_url}>
                git clone {w.clone_url}
              </span>
            </div>
          )}
        </section>
      )}
      <article className={cn("min-w-0 flex-1 flex-col md:flex", column === "page" ? "flex" : "hidden")} aria-label={opened.data?.title}>
        <div className="flex items-center gap-1 border-b border-hair px-2 py-1.5 md:hidden">
          <Button variant="ghost" size="icon-sm" onClick={() => setColumn("pages")} aria-label={t("wiki.back")}>
            <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          </Button>
        </div>
        {mode.kind === "new" ? (
          <WikiEditor
            owner={owner}
            repo={repo}
            page={null}
            parent={mode.parent}
            initialTitle={mode.title}
            onSaved={saved}
            onCancel={() => setMode({ kind: "read" })}
          />
        ) : mode.kind === "edit" && opened.data !== null ? (
          <WikiEditor owner={owner} repo={repo} page={opened.data} onSaved={saved} onCancel={() => setMode({ kind: "read" })} />
        ) : w !== null && !w.exists ? (
          <Empty className="m-4 border-0">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={LibraryIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>{t("wiki.empty.title")}</EmptyTitle>
              <EmptyDescription>{w.writable ? t("wiki.empty.writer") : t("wiki.empty.reader")}</EmptyDescription>
            </EmptyHeader>
            {w.writable && (
              <Button onClick={() => setMode({ kind: "new", parent: "", title: "Home" })}>{t("wiki.empty.first")}</Button>
            )}
          </Empty>
        ) : opened.error !== null ? (
          <Empty className="m-4 border-0">
            <EmptyHeader>
              <EmptyTitle>{t("wiki.page.missing")}</EmptyTitle>
              <EmptyDescription>
                {opened.error instanceof GitError && opened.error.status === 404 ? t("wiki.page.missingText") : explain(opened.error)}
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : opened.data === null ? (
          <div className="m-auto p-6">
            <Spinner />
          </div>
        ) : (
          <div className="min-h-0 flex-1 overflow-y-auto">
            <WikiArticle
              owner={owner}
              repo={repo}
              page={opened.data}
              base={base}
              go={(href) => go(pageAt(base, href))}
              history
              actions={
                w?.writable === true ? (
                  <div className="flex items-center gap-1">
                    <Button variant="secondary" size="sm" onClick={() => setMode({ kind: "edit" })} data-wiki-edit>
                      <HugeiconsIcon icon={PencilEdit02Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                      {t("wiki.page.edit")}
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t("wiki.page.child")}
                      title={t("wiki.page.child")}
                      onClick={() => setMode({ kind: "new", parent: opened.data?.name ?? "", title: "" })}
                    >
                      <HugeiconsIcon icon={Add01Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                    </Button>
                    <Button variant="ghost" size="icon-sm" aria-label={t("wiki.page.delete")} title={t("wiki.page.delete")} onClick={() => setDeleting(true)}>
                      <HugeiconsIcon icon={Delete02Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                    </Button>
                  </div>
                ) : undefined
              }
            />
          </div>
        )}
      </article>
      {deleting && opened.data !== null && (
        <DeletePageDialog
          title={opened.data.title}
          onClose={() => setDeleting(false)}
          onDelete={async () => {
            await git.deleteWikiPage(owner, repo, opened.data?.name ?? "", opened.data?.sha ?? "");
            toast.done(t("wiki.delete.done"));
            setDeleting(false);
            wiki.reload();
            onChanged();
            go("");
          }}
        />
      )}
    </>
  );
}

// ── The others: a knowledge base, the handbook ─────────────────────────────

function KnowledgeBase({ site, slug, hidden, onBack }: { site: KbSite | null; slug: string; hidden: boolean; onBack: () => void }) {
  const t = useT();
  const notes = useLoaded((signal) => kbNotes(slug, signal), [slug]);
  const href = `/kb/${encodeURIComponent(slug)}/`;
  return (
    <section className={cn("min-w-0 flex-1 flex-col md:flex", hidden ? "hidden" : "flex")} aria-label={site?.title ?? slug} data-wiki-kb={slug}>
      <Overview
        icon={Notebook01Icon}
        title={site?.title ?? slug}
        kind={t("wiki.kb.kind")}
        audience={site?.audience ?? "box"}
        text={t("wiki.kb.text")}
        href={href}
        open={t("wiki.kb.open")}
        onBack={onBack}
      >
        {notes.data === null && notes.error === null ? (
          <Spinner />
        ) : (
          <ul className="grid gap-px sm:grid-cols-2">
            {(notes.data ?? []).map((n) => (
              <li key={n.path}>
                <a
                  href={kbPage(slug, n.path)}
                  target="_blank"
                  rel="noopener"
                  className="block truncate rounded-control px-2 py-1.5 text-[13px] text-muted hover:bg-sunk hover:text-ink"
                >
                  {n.title ?? titleOf(n.path.replace(/\.(note|md|markdown)$/iu, ""))}
                </a>
              </li>
            ))}
          </ul>
        )}
      </Overview>
    </section>
  );
}

function Handbook({ hidden, onBack }: { hidden: boolean; onBack: () => void }) {
  const t = useT();
  return (
    <section className={cn("min-w-0 flex-1 flex-col md:flex", hidden ? "hidden" : "flex")} aria-label={t("wiki.handbook.title")} data-wiki-handbook>
      <Overview
        icon={BookOpen01Icon}
        title={t("wiki.handbook.title")}
        kind={t("wiki.handbook.kind")}
        audience="lan"
        text={t("wiki.handbook.text")}
        href={handbookHome()}
        open={t("wiki.handbook.open")}
        onBack={onBack}
      >
        <ul className="grid gap-px sm:grid-cols-2">
          {CHAPTERS.map((c) => (
            <li key={c.entry}>
              <a
                href={handbookHref(c.entry)}
                target="_blank"
                rel="noopener"
                className="block truncate rounded-control px-2 py-1.5 text-[13px] text-muted hover:bg-sunk hover:text-ink"
              >
                {t(c.label)}
              </a>
            </li>
          ))}
        </ul>
      </Overview>
    </section>
  );
}

function Overview({
  icon,
  title,
  kind,
  audience,
  text,
  href,
  open,
  onBack,
  children,
}: {
  icon: IconSvgElement;
  title: string;
  kind: string;
  audience: keyof typeof AUDIENCE;
  text: string;
  href: string;
  open: string;
  onBack: () => void;
  children: React.ReactNode;
}) {
  const t = useT();
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <div className="flex items-center gap-1 border-b border-hair px-2 py-1.5 lg:hidden">
        <Button variant="ghost" size="icon-sm" onClick={onBack} aria-label={t("wiki.back")}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
      </div>
      <div className="flex flex-col gap-4 px-5 py-4">
        <div className="flex items-start gap-3">
          <span className="grid size-10 shrink-0 place-items-center rounded-control bg-accent-wash text-accent">
            <HugeiconsIcon icon={icon} size={20} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          </span>
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <h2 className="text-[19px] leading-tight font-semibold tracking-tight">{title}</h2>
            <span className="flex flex-wrap items-center gap-2">
              <Badge variant="outline">{kind}</Badge>
              <Audience who={audience} />
            </span>
          </div>
          <a href={href} target="_blank" rel="noopener" className={buttonVariants({ variant: "secondary", size: "sm" })}>
            <HugeiconsIcon icon={LinkSquare02Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {open}
          </a>
        </div>
        <p className="max-w-prose text-[13px] text-muted">{text}</p>
        {children}
      </div>
    </div>
  );
}

// ── A new space ────────────────────────────────────────────────────────────

function NewSpace({ onClose, onMade }: { onClose: () => void; onMade: (owner: string, repo: string) => void }) {
  const t = useT();
  const id = React.useId();
  const [name, setName] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [visibility, setVisibility] = React.useState<Visibility>("internal");
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const repoName = name.trim().replace(/\s+/gu, "-").replace(/[^A-Za-z0-9._-]/gu, "");

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()} labelledBy={`${id}-t`} dialogClassName="w-[min(32rem,calc(100vw-2rem))]">
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (repoName === "") return;
          setBusy(true);
          setError(null);
          (async () => {
            const repo = await git.createRepo(repoName, description.trim(), visibility);
            await git.saveWikiPage(
              repo.owner.login,
              repo.name,
              "Home",
              `# ${name.trim()}\n\n${description.trim() === "" ? t("wiki.space.homeText") : description.trim()}\n`,
              null,
              "",
            );
            toast.done(t("wiki.space.made"));
            onMade(repo.owner.login, repo.name);
          })().catch((err: unknown) => {
            setBusy(false);
            setError(err instanceof GitError && err.status === 409 ? t("wiki.space.taken") : explain(err));
          });
        }}
      >
        <DialogHeader>
          <DialogTitle id={`${id}-t`}>{t("wiki.space.newTitle")}</DialogTitle>
          <DialogDescription>{t("wiki.space.newBody")}</DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          <Field>
            <FieldLabel htmlFor={`${id}-name`}>{t("wiki.space.name")}</FieldLabel>
            <Input id={`${id}-name`} value={name} onChange={(e) => setName(e.target.value)} required autoFocus aria-describedby={`${id}-repo`} />
            <FieldDescription id={`${id}-repo`}>
              {repoName === "" ? t("wiki.space.nameHint") : t("wiki.space.repo", { repo: repoName })}
            </FieldDescription>
          </Field>
          <Field>
            <FieldLabel htmlFor={`${id}-description`}>{t("wiki.space.description")}</FieldLabel>
            <Input id={`${id}-description`} value={description} onChange={(e) => setDescription(e.target.value)} />
          </Field>
          <VisibilityField value={visibility} onChange={setVisibility} />
          {error !== null && (
            <p role="alert" className="text-[13px] text-crit">
              {error}
            </p>
          )}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="secondary" onClick={onClose}>
            {t("wiki.cancel")}
          </Button>
          <Button type="submit" disabled={busy || repoName === ""}>
            {t("wiki.space.make")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

export type { WikiSpace };
