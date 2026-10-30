import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Search01Icon, SidebarLeftIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { DisplayMenu } from "@/components/preferences-menu";
import { Spinner } from "@/components/ui/spinner";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useLocale, useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { cn } from "@/lib/utils";
import { CanvasView } from "@/notes/canvas-view";
import { GraphView } from "@/notes/graph";
import { folderOf } from "@/notes/links";
import { anchorFor, flavourOf, renderNote } from "@/notes/markdown";
import { MindMap, outline } from "@/notes/mindmap";
import { NoteView, useNotesReady } from "@/notes/note-view";
import { NoteSide } from "@/notes/side";
import { VaultTree } from "@/notes/tree";
import { DEFAULT_GRAPH, displayName, Vault, type Hit, type Source, type VaultIndex } from "@/notes/vault";

/* A knowledge base at /kb/<name>/<note>: the published folder's notes to
 * read, with the same links, embeds, tags, graph and mind map as Notes in
 * LosOS Cloud, and nothing to change them with. The address names a note
 * by its path without the extension, as Obsidian Publish does. A site for
 * people on the box answers 401 until its reader signs in; the page sends
 * them to LosOS Login and back. */

interface Site {
  slug: string;
  title: string;
  audience: "anyone" | "box";
  vault: VaultIndex;
}

type Failure = "notFound" | "offline";

const slug = window.location.pathname.split("/")[2] ?? "";
const API = `/kb/api/${encodeURIComponent(slug)}`;
const BASE = `/kb/${encodeURIComponent(slug)}/`;

async function get(url: string, signal?: AbortSignal): Promise<Response> {
  const init: RequestInit = { credentials: "same-origin", cache: "no-store" };
  if (signal !== undefined) init.signal = signal;
  const res = await fetch(url, init);
  if (res.status === 401) {
    window.location.assign(`/auth/?rd=${encodeURIComponent(window.location.href)}`);
    throw new Error("signIn");
  }
  if (!res.ok) throw new Error(res.status === 404 ? "notFound" : "offline");
  return res;
}

/** A note's page address. */
function pageOf(path: string, sub = ""): string {
  const bare = path.replace(/\.(note|md|markdown)$/i, "");
  return `${BASE}${bare.split("/").map(encodeURIComponent).join("/")}${sub === "" ? "" : `#${encodeURIComponent(sub)}`}`;
}

/** The note an address names, or null for the site's home. */
function noteAt(vault: Vault, pathname: string): string | null {
  if (!pathname.startsWith(BASE)) return null;
  const rest = pathname
    .slice(BASE.length)
    .split("/")
    .filter((s) => s !== "")
    .map((s) => {
      try {
        return decodeURIComponent(s);
      } catch {
        return s;
      }
    })
    .join("/");
  if (rest === "") return null;
  for (const p of [rest, `${rest}.note`, `${rest}.md`, `${rest}.markdown`]) if (vault.note(p) !== undefined) return p;
  return rest;
}

const source: Source = {
  text: async (path, signal) => (await get(`${API}/note?path=${encodeURIComponent(path)}`, signal)).text(),
  fileUrl: (path) => `${API}/file?path=${encodeURIComponent(path)}`,
  href: (path, sub) => pageOf(path, sub),
  search: async (q, signal) => ((await (await get(`${API}/search?q=${encodeURIComponent(q)}`, signal)).json()) as { hits: Hit[] }).hits,
};

function useAddress(): [string, string, (url: string) => void] {
  const [where, setWhere] = React.useState(() => ({ path: window.location.pathname, hash: window.location.hash }));
  React.useEffect(() => {
    const pop = () => setWhere({ path: window.location.pathname, hash: window.location.hash });
    window.addEventListener("popstate", pop);
    return () => window.removeEventListener("popstate", pop);
  }, []);
  const go = React.useCallback((url: string) => {
    window.history.pushState(null, "", url);
    setWhere({ path: window.location.pathname, hash: window.location.hash });
    if (window.location.hash === "") window.scrollTo({ top: 0 });
  }, []);
  return [where.path, where.hash, go];
}

export function KbRoot() {
  const t = useT();
  useLocale();
  const [site, setSite] = React.useState<Site | null>(null);
  const [failure, setFailure] = React.useState<Failure | null>(null);
  const [pathname, hash, go] = useAddress();

  React.useEffect(() => {
    const controller = new AbortController();
    get(`${API}/site`, controller.signal)
      .then((r) => r.json() as Promise<Site>)
      .then(setSite, (err: unknown) => {
        if (controller.signal.aborted) return;
        const msg = err instanceof Error ? err.message : "";
        if (msg !== "signIn") setFailure(msg === "notFound" ? "notFound" : "offline");
      });
    return () => controller.abort();
  }, []);

  const vault = React.useMemo(() => (site === null ? null : new Vault(site.vault)), [site]);

  return (
    <div className="losos-shell min-h-dvh text-ink" data-kb={slug}>
      <Header title={site?.title ?? ""} go={go} />
      {failure !== null ? (
        <Empty className="mx-auto mt-10 min-h-72 max-w-xl" data-kb-missing>
          <EmptyHeader>
            <EmptyTitle>{t(failure === "notFound" ? "kb.notFound.title" : "kb.offline.title")}</EmptyTitle>
            <EmptyDescription>{t(failure === "notFound" ? "kb.notFound.text" : "kb.offline.text")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : site === null || vault === null ? (
        <div className="flex min-h-72 items-center justify-center gap-2 text-[13px] text-muted" role="status">
          <Spinner size={16} />
          {t("notes.loading")}
        </div>
      ) : (
        <Reader site={site} vault={vault} pathname={pathname} hash={hash} go={go} />
      )}
    </div>
  );
}

function Header({ title, go }: { title: string; go: (url: string) => void }) {
  const t = useT();
  const logo = logoFor();
  return (
    <header className="sticky top-0 z-40 border-b border-line bg-surface/85 backdrop-blur-sm">
      <div className="mx-auto flex w-full max-w-[96rem] items-center gap-3 px-4 py-3 sm:px-6">
        <a
          href={BASE}
          onClick={(event) => {
            if (event.button !== 0 || event.metaKey || event.ctrlKey) return;
            event.preventDefault();
            go(BASE);
          }}
          className="flex min-w-0 items-center gap-2.5 rounded-control focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
        >
          <img src={logo.url} alt="" width={32} height={32} className="size-8" />
          <span className="truncate text-[15px] font-semibold tracking-tight" data-kb-title>
            {title || t("notes.title")}
          </span>
        </a>
        <div className="flex-1" />
        <DisplayMenu />
      </div>
    </header>
  );
}

type View = "read" | "map" | "graph";

function Reader({ site, vault, pathname, hash, go }: { site: Site; vault: Vault; pathname: string; hash: string; go: (url: string) => void }) {
  const t = useT();
  const asked = noteAt(vault, pathname);
  const path = asked ?? vault.home()?.path ?? null;
  const note = path === null ? undefined : vault.note(path);
  const canvas = path !== null && /\.canvas$/i.test(path);
  const [view, setView] = React.useState<View>("read");
  const [text, setText] = React.useState<{ path: string; text: string } | null>(null);
  const [missing, setMissing] = React.useState(false);
  const [tree, setTree] = React.useState(false);
  const bodyRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    if (path === null || note === undefined) return;
    const controller = new AbortController();
    setMissing(false);
    source.text(path, controller.signal).then(
      (body) => setText({ path, text: body }),
      () => {
        if (!controller.signal.aborted) setMissing(true);
      },
    );
    return () => controller.abort();
  }, [path, note]);

  React.useEffect(() => setTree(false), [path]);

  React.useEffect(() => {
    document.title = `${note !== undefined ? `${displayName(note)} · ` : ""}${site.title}`;
  }, [note, site.title]);

  const body = text !== null && text.path === path ? text.text : null;
  const notesReady = useNotesReady();
  const rendered = React.useMemo(
    () => (!notesReady || body === null || path === null || canvas ? null : renderNote(body, vault.resolver(path, source), flavourOf(path))),
    [notesReady, body, path, canvas, vault],
  );
  const resolverFor = React.useCallback((p: string) => vault.resolver(p, source), [vault]);
  const load = React.useCallback((p: string) => source.text(p), []);
  const labels = React.useMemo(
    () => ({
      properties: t("notes.properties"),
      embedLoading: t("notes.embed.loading"),
      embedMissing: t("notes.embed.missing"),
      problems: t("notes.problems"),
      line: (line: number) => t("notes.problems.line", { line }),
    }),
    [t],
  );

  React.useEffect(() => {
    if (hash === "" || rendered === null || view !== "read") return;
    const part = decodeURIComponent(hash.slice(1));
    const root = bodyRef.current;
    const el = root?.querySelector(`[id="${CSS.escape(part)}"]`) ?? root?.querySelector(`[id="${CSS.escape(anchorFor(part))}"]`);
    el?.scrollIntoView({ block: "start" });
  }, [hash, rendered?.html, view]);

  const open = (p: string, sub = "") => {
    setView("read");
    go(pageOf(p, sub));
  };

  const graph = React.useMemo(() => (view === "graph" ? vault.graph(DEFAULT_GRAPH) : null), [vault, view]);

  let middle: React.ReactNode;
  if (graph !== null) {
    middle = (
      <div className="flex flex-col gap-4" data-vault-graph>
        <h1 className="text-[19px] leading-tight font-semibold tracking-tight">{t("notes.graph.title")}</h1>
        <GraphView
          nodes={graph.nodes}
          edges={graph.edges}
          {...(path !== null ? { current: `n:${path}` } : {})}
          onOpen={(node) => {
            if (node.kind === "note" || node.kind === "canvas") open(node.key);
          }}
          label={t("notes.graph.title")}
          className="h-[68dvh] min-h-96 rounded-card border border-hair bg-surface"
        />
      </div>
    );
  } else if (path === null || note === undefined || missing) {
    middle = (
      <Empty className="min-h-72" data-kb-note-missing>
        <EmptyHeader>
          <EmptyTitle>{t("kb.noNote.title")}</EmptyTitle>
          <EmptyDescription>{t("kb.noNote.text")}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  } else {
    const folder = folderOf(path);
    middle = (
      <div className="flex min-w-0 flex-col gap-4">
        <header className="flex flex-col gap-1">
          {folder !== "" && <p className="truncate text-[12.5px] text-muted">{folder.split("/").join(" / ")}</p>}
          <h1 className="text-[19px] leading-tight font-semibold tracking-tight" data-note-title>
            {displayName(note)}
          </h1>
        </header>
        {body === null ? (
          <div className="flex min-h-40 items-center gap-2 text-[13px] text-muted" role="status">
            <Spinner size={16} />
            {t("notes.loading")}
          </div>
        ) : canvas ? (
          <CanvasView text={body} resolver={vault.resolver(path, source)} onOpen={open} labels={{ unreadable: t("notes.canvas.unreadable") }} />
        ) : rendered === null ? null : view === "map" ? (
          <MindMap
            root={outline(displayName(note), rendered.body)}
            label={t("notes.map.label", { name: displayName(note) })}
            onHeading={(id) => open(path, id)}
          />
        ) : (
          <div ref={bodyRef}>
            <NoteView path={path} rendered={rendered} resolverFor={resolverFor} load={load} onOpen={open} labels={labels} />
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="mx-auto flex w-full max-w-[96rem] flex-col gap-4 px-4 pt-5 pb-12 sm:px-6 lg:flex-row lg:gap-6" data-kb-reader>
      <div className="flex items-center gap-2 lg:hidden">
        <Button variant="outline" size="sm" onClick={() => setTree((v) => !v)} aria-expanded={tree}>
          <HugeiconsIcon icon={SidebarLeftIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("kb.contents")}
        </Button>
      </div>
      <div className={cn("lg:sticky lg:top-20 lg:block lg:max-h-[calc(100dvh-6rem)] lg:w-64 lg:shrink-0 lg:self-start lg:overflow-y-auto", !tree && "max-lg:hidden")}>
        <Contents vault={vault} current={path} onOpen={open} />
      </div>
      <main className="flex min-w-0 flex-1 flex-col gap-4">
        {!canvas && (
          <ToggleGroup variant="outline" value={view} onValueChange={(v) => v !== "" && setView(v as View)} aria-label={t("notes.view.label")} className="w-fit">
            <ToggleGroupItem value="read" data-view="read">
              {t("notes.view.read")}
            </ToggleGroupItem>
            <ToggleGroupItem value="map" data-view="map">
              {t("notes.view.map")}
            </ToggleGroupItem>
            <ToggleGroupItem value="graph" data-view="graph">
              {t("notes.graph.title")}
            </ToggleGroupItem>
          </ToggleGroup>
        )}
        <div className="flex min-w-0 flex-col gap-6 xl:flex-row">
          <div className="min-w-0 flex-1">{middle}</div>
          {graph === null && note !== undefined && path !== null && (
            <NoteSide
              vault={vault}
              path={path}
              headings={rendered?.headings ?? []}
              href={(p) => pageOf(p)}
              onOpen={(p) => open(p)}
              onHeading={(id) => open(path, id)}
              className="xl:sticky xl:top-20 xl:w-64 xl:shrink-0 xl:self-start"
            />
          )}
        </div>
      </main>
    </div>
  );
}

function Contents({ vault, current, onOpen }: { vault: Vault; current: string | null; onOpen: (path: string) => void }) {
  const t = useT();
  const [q, setQ] = React.useState("");
  const [hits, setHits] = React.useState<Hit[] | null>(null);

  React.useEffect(() => {
    const text = q.trim();
    if (text === "") {
      setHits(null);
      return;
    }
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      source.search(text, controller.signal).then(setHits, () => {});
    }, 250);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [q]);

  return (
    <div className="flex flex-col gap-4 rounded-card border border-hair bg-surface p-3" data-kb-contents>
      <div className="relative">
        <HugeiconsIcon
          icon={Search01Icon}
          size={14}
          strokeWidth={1.5}
          color="currentColor"
          className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-faint"
          aria-hidden="true"
        />
        <Input
          type="search"
          value={q}
          onChange={(event) => setQ(event.target.value)}
          placeholder={t("notes.search.placeholder")}
          aria-label={t("notes.search.label")}
          className="h-8 pl-8 text-[13px]"
          data-kb-search
        />
      </div>
      {hits !== null ? (
        <ul className="flex flex-col gap-1" aria-label={t("notes.search.results")} data-search-hits>
          {hits.length === 0 && <li className="px-1.5 text-[12.5px] text-faint">{t("notes.search.none")}</li>}
          {hits.map((h, i) => (
            <li key={`${h.path}:${h.line}:${i}`}>
              <a
                href={pageOf(h.path)}
                onClick={(event) => {
                  if (event.button !== 0 || event.metaKey || event.ctrlKey) return;
                  event.preventDefault();
                  onOpen(h.path);
                }}
                className="block rounded-control px-1.5 py-1 hover:bg-sunk focus-visible:outline-2 focus-visible:outline-accent"
              >
                <span className="block truncate text-[13px] font-medium text-ink">{displayName({ path: h.path, title: null })}</span>
                {h.text !== "" && <span className="line-clamp-2 text-[12px] text-muted">{h.text}</span>}
              </a>
            </li>
          ))}
        </ul>
      ) : (
        <VaultTree vault={vault} current={current} href={(p) => pageOf(p)} onOpen={onOpen} label={t("kb.contents")} />
      )}
    </div>
  );
}
