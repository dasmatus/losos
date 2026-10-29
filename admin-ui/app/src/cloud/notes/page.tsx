import * as React from "react";
import { useHref, useLocation, useNavigate, useSearchParams } from "react-router-dom";
import { EditorView, ViewPlugin } from "@codemirror/view";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon, ArrowLeft01Icon, Search01Icon, SidebarLeftIcon, Tag01Icon, WorkflowSquare01Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge, StatusDot } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { toast } from "@/components/ui/toast";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { CanvasView } from "@/notes/canvas-view";
import { GraphView } from "@/notes/graph";
import { folderOf, isNote } from "@/notes/links";
import { anchorFor, flavourOf, renderNote, toNote, type Rendered } from "@/notes/markdown";
import { MindMap, outline } from "@/notes/mindmap";
import { NoteView, useNotesReady } from "@/notes/note-view";
import { NoteSide } from "@/notes/side";
import { VaultTree } from "@/notes/tree";
import { DEFAULT_GRAPH, displayName, Vault, type GraphOptions, type Hit, type VaultIndex } from "@/notes/vault";
import { cloud, CloudError, isAbort, joinPath, parentOf } from "../api";
import { collab } from "../collab/api";
import { CollabEditor, type Connection, type Presence } from "../collab/editor";
import type { PresenceUser } from "../collab/cursors";
import { Faces } from "../collab/faces";
import { accentFor } from "../collab/people";
import { explain } from "../dialogs";
import { cloudSource, notes, readText, writeText } from "./api";
import { onText, wikilinkCompletion } from "./editing";
import { EditorQuickActions, ReaderQuickActions } from "../quick-text";

/* /cloud/notes: a folder of Markdown as Obsidian opens it, a vault.
 *
 * The vault's notes on the left (folders, search, tags), the open note in
 * the middle, read, edited together or drawn as a mind map, and beside it
 * its local graph, backlinks and outline. `?view=graph` is the whole
 * vault's graph and `?tag=` the notes with a tag. The vault is the folder
 * named in `?vault=`, "Notes" in the home by default; a folder holding
 * `.obsidian` is found from any note inside it (/notes/open). Links,
 * embeds and tags resolve as Obsidian resolves them (src/notes/). */

const DEFAULT_VAULT = "Notes";
type View = "read" | "edit" | "map" | "graph";
const VIEWS: readonly View[] = ["read", "edit", "map", "graph"];

function signature(index: VaultIndex): string {
  const parts: string[] = [];
  for (const n of index.notes) parts.push(`${n.path}\t${n.etag}`);
  for (const n of index.canvases) parts.push(`${n.path}\t${n.etag}`);
  for (const f of index.files) parts.push(f.path);
  parts.push(...index.folders);
  return parts.join("\n");
}

/** The vault, listed again every few seconds while the page is shown, so
 * links and the graph follow what is typed here and on other devices. */
function useVault(root: string) {
  const [vault, setVault] = React.useState<Vault | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  // The listing last drawn; null until the first, so an empty vault (a new
  // box's Notes, whose signature is "") is drawn too.
  const seen = React.useRef<string | null>(null);

  const reload = React.useCallback(
    async (signal?: AbortSignal) => {
      const index = await notes.vault(root, signal);
      const sig = signature(index);
      if (sig !== seen.current) {
        seen.current = sig;
        setVault(new Vault(index));
      }
    },
    [root],
  );

  React.useEffect(() => {
    seen.current = null;
    setVault(null);
    setError(null);
    const controller = new AbortController();
    const first = async () => {
      try {
        await reload(controller.signal);
      } catch (err) {
        if (isAbort(err)) return;
        // The default vault is made on first use.
        if (root === DEFAULT_VAULT && err instanceof CloudError && err.status === 404) {
          try {
            await cloud.mkdir(root);
            await reload(controller.signal);
            return;
          } catch (again) {
            if (!isAbort(again)) setError(again);
            return;
          }
        }
        setError(err);
      }
    };
    void first();
    const timer = window.setInterval(() => {
      if (document.visibilityState === "visible") reload().catch(() => {});
    }, 5000);
    return () => {
      controller.abort();
      window.clearInterval(timer);
    };
  }, [root, reload]);

  return { vault, error, reload };
}

/** The open note's text, read again when the vault says it changed. */
function useText(root: string, path: string | null, etag: string | undefined, paused: boolean) {
  const [doc, setDoc] = React.useState<{ path: string; text: string } | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  const [nonce, setNonce] = React.useState(0);

  React.useEffect(() => {
    if (path === null || paused) return;
    const controller = new AbortController();
    readText(joinPath(root, path), controller.signal).then(
      (r) => {
        setDoc({ path, text: r.text });
        setError(null);
      },
      (err: unknown) => {
        if (!isAbort(err)) setError(err);
      },
    );
    return () => controller.abort();
  }, [root, path, etag, paused, nonce]);

  const text = doc !== null && doc.path === path ? doc.text : null;
  return { text, error, refresh: () => setNonce((n) => n + 1) };
}

/** Whether a task line's box is ticked, written into its line. */
function tick(line: string, done: boolean): string {
  return line.replace(/^(\s*(?:>\s*)*(?:[-*+]|\d+[.)])\s+\[)(.)(\])/, (_m, a: string, _c: string, b: string) => `${a}${done ? "x" : " "}${b}`);
}

export function NotesPage() {
  const t = useT();
  useLocale();
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const { hash } = useLocation();
  const root = params.get("vault") ?? DEFAULT_VAULT;
  const asked = params.get("view") as View | null;
  const tag = params.get("tag");
  const { vault, error, reload } = useVault(root);
  // LosOS Cloud's own root, for an address that works outside the app.
  const cloudRoot = useHref("/").replace(/\/$/, "");
  const source = React.useMemo(() => cloudSource(root), [root]);

  const path = params.get("note") ?? (tag === null && asked !== "graph" ? (vault?.home()?.path ?? null) : null);
  const note = path === null ? undefined : vault?.note(path);
  const canvas = path !== null && /\.canvas$/i.test(path);
  const view: View = tag !== null ? "read" : asked !== null && VIEWS.includes(asked) && !(canvas && asked !== "graph") ? asked : "read";
  const editing = view === "edit" && note !== undefined;
  const { text, error: textError, refresh } = useText(root, note === undefined ? null : path, note?.etag, editing);

  const [live, setLive] = React.useState<string | null>(null);
  React.useEffect(() => setLive(null), [path, editing]);
  const shown = React.useDeferredValue(editing ? (live ?? text) : text);

  const [tree, setTree] = React.useState(false);
  const bodyRef = React.useRef<HTMLDivElement>(null);
  const vaultRef = React.useRef(vault);
  vaultRef.current = vault;

  const href = React.useCallback((p: string, sub = "") => {
    const q = new URLSearchParams({ vault: root, note: p });
    return `/notes?${q.toString()}${sub === "" ? "" : `#${encodeURIComponent(sub)}`}`;
  }, [root]);

  const open = React.useCallback(
    (p: string, sub = "", as?: View) => {
      const q = new URLSearchParams({ vault: root, note: p });
      const next = as ?? (view === "edit" ? "edit" : "read");
      if (next !== "read") q.set("view", next);
      navigate(`/notes?${q.toString()}${sub === "" ? "" : `#${encodeURIComponent(sub)}`}`);
      setTree(false);
    },
    [navigate, root, view],
  );

  const setView = (next: View) => {
    const q = new URLSearchParams({ vault: root });
    if (next !== "graph" && path !== null) q.set("note", path);
    if (next !== "read") q.set("view", next);
    navigate(`/notes?${q.toString()}`);
  };

  const showTag = (name: string) => {
    navigate(`/notes?${new URLSearchParams({ vault: root, tag: name }).toString()}`);
    setTree(false);
  };

  const notesReady = useNotesReady();
  const rendered: Rendered | null = React.useMemo(() => {
    if (!notesReady || shown === null || vault === null || path === null || canvas) return null;
    return renderNote(shown, vault.resolver(path, source), flavourOf(path));
  }, [notesReady, shown, vault, path, source, canvas]);

  const resolverFor = React.useCallback((p: string) => (vault === null ? NO_RESOLVER : vault.resolver(p, source)), [vault, source]);
  const load = React.useCallback((p: string) => source.text(p), [source]);
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

  // A #part in the address: a heading's text, a ^block, or an element id.
  React.useEffect(() => {
    if (hash === "" || rendered === null || view !== "read") return;
    const part = decodeURIComponent(hash.slice(1));
    const root = bodyRef.current;
    const el = root?.querySelector(`[id="${CSS.escape(part)}"]`) ?? root?.querySelector(`[id="${CSS.escape(anchorFor(part))}"]`);
    el?.scrollIntoView({ block: "start" });
    el?.classList.add("is-flash");
    const timer = window.setTimeout(() => el?.classList.remove("is-flash"), 1600);
    return () => window.clearTimeout(timer);
  }, [hash, rendered?.html, view]);

  React.useEffect(() => {
    const name = path === null ? (vault?.index.name ?? root) : displayName({ path, title: null });
    document.title = `${name} · ${t("notes.title")} · LosOS Cloud`;
  }, [path, vault?.index.name, root, t]);

  const create = async (target?: string) => {
    if (vault === null) return;
    let rel: string;
    if (target !== undefined) {
      const name = (target.split("#")[0] ?? "").split("|")[0]?.trim() ?? "";
      if (name === "") return;
      const found = vault.find(name, path ?? "");
      if (found !== null) {
        open(found.path);
        return;
      }
      // New notes go to the vault's top, as Obsidian puts them by default.
      rel = name.includes("/") ? (isNote(name) ? name : `${name}.note`) : vault.freeName("", name);
    } else {
      rel = vault.freeName(path === null ? "" : folderOf(path), t("notes.untitled"));
    }
    try {
      const folder = folderOf(rel);
      if (folder !== "" && !vault.index.folders.includes(folder)) {
        await cloud.mkdir(joinPath(root, folder)).catch((err: unknown) => {
          if (!(err instanceof CloudError && err.code === "exists")) throw err;
        });
      }
      await writeText(joinPath(root, rel), "", { create: true });
      await reload();
      open(rel, "", "edit");
    } catch (err) {
      toast.error(t("notes.error.create"), explain(err), { help: "index" });
    }
  };

  // A .md note as a .note: the same text in MDX, written over the .md
  // (refused if it changed meanwhile), then moved to the new name, so the
  // file keeps its history and its shares.
  const [converting, setConverting] = React.useState(false);
  const convert = async () => {
    if (path === null || vault === null || !/\.md$/i.test(path)) return;
    const name = displayName({ path, title: null });
    const next = path.replace(/\.md$/i, ".note");
    if (vault.index.notes.some((n) => n.path === next)) {
      toast.error(t("notes.convert.refused", { name }), t("notes.convert.taken"), { help: "index" });
      return;
    }
    setConverting(true);
    try {
      const file = joinPath(root, path);
      const now = await readText(file);
      let text: string;
      try {
        text = await toNote(now.text);
      } catch (err) {
        toast.error(t("notes.convert.refused", { name }), err instanceof Error ? err.message : String(err), { help: "index" });
        return;
      }
      if (text !== now.text) await writeText(file, text, { etag: now.etag });
      await cloud.move(file, joinPath(root, next));
      await reload();
      open(next);
      toast.success(t("notes.convert.done", { name }));
    } catch (err) {
      toast.error(t("notes.error.save"), explain(err), { help: "index" });
    } finally {
      setConverting(false);
    }
  };

  const toggleTask = async (line: number, done: boolean) => {
    if (path === null) return;
    const file = joinPath(root, path);
    try {
      const now = await readText(file);
      const lines = now.text.split("\n");
      const was = lines[line];
      if (was === undefined) return;
      const next = tick(was, done);
      if (next === was) return;
      lines[line] = next;
      await writeText(file, lines.join("\n"), { etag: now.etag });
      void reload();
    } catch (err) {
      toast.error(t("notes.error.save"), explain(err), { help: "index" });
    } finally {
      refresh();
    }
  };

  if (error !== null) {
    return (
      <Empty className="mx-auto mt-6 min-h-72 max-w-xl">
        <EmptyHeader>
          <EmptyTitle>{t("notes.error.vault")}</EmptyTitle>
          <EmptyDescription>{explain(error)}</EmptyDescription>
        </EmptyHeader>
        <Button variant="outline" onClick={() => navigate("/files")}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("collab.back.files")}
        </Button>
      </Empty>
    );
  }

  if (vault === null) {
    return (
      <div className="flex min-h-72 items-center justify-center gap-2 text-[13px] text-muted" role="status">
        <Spinner size={16} />
        {t("notes.loading")}
      </div>
    );
  }

  const left = (
    <VaultPane
      vault={vault}
      root={root}
      current={path}
      tag={tag}
      href={(p) => href(p)}
      onOpen={(p) => open(p)}
      onTag={showTag}
      onNew={() => void create()}
      onGraph={() => setView("graph")}
      graph={view === "graph"}
    />
  );

  let middle: React.ReactNode;
  if (tag !== null) {
    middle = <TagList vault={vault} tag={tag} href={(p) => href(p)} onOpen={(p) => open(p)} />;
  } else if (view === "graph") {
    middle = <VaultGraph vault={vault} onOpen={(p) => open(p, "", "read")} />;
  } else if (path === null) {
    middle = (
      <Empty className="min-h-72" data-notes-empty>
        <EmptyHeader>
          <EmptyTitle>{t("notes.empty.title")}</EmptyTitle>
          <EmptyDescription>{t("notes.empty.text")}</EmptyDescription>
        </EmptyHeader>
        <Button onClick={() => void create()} data-new-note>
          <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("notes.new")}
        </Button>
      </Empty>
    );
  } else if (note === undefined) {
    middle = (
      <Empty className="min-h-72">
        <EmptyHeader>
          <EmptyTitle>{t("notes.gone.title")}</EmptyTitle>
          <EmptyDescription>{path}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  } else {
    const folder = folderOf(path);
    middle = (
      <div className="flex min-w-0 flex-col gap-4">
        <header className="flex flex-wrap items-end justify-between gap-x-6 gap-y-3">
          <div className="flex min-w-0 flex-col gap-1">
            <p className="truncate text-[12.5px] text-muted" data-note-folder>
              {[vault.index.name, ...(folder === "" ? [] : folder.split("/"))].join(" / ")}
            </p>
            <h1 className="truncate text-[19px] leading-tight font-semibold tracking-tight" data-note-title>
              {displayName(note)}
            </h1>
          </div>
          {!canvas && (
            <div className="flex flex-wrap items-center gap-2">
              {/\.md$/i.test(path) && view === "read" && (
                <Button variant="outline" size="sm" onClick={() => void convert()} disabled={converting} title={t("notes.convert.what")} data-convert-note>
                  {converting && <Spinner />}
                  {t("notes.convert.action")}
                </Button>
              )}
              <ToggleGroup variant="outline" value={view} onValueChange={(v) => v !== "" && setView(v as View)} aria-label={t("notes.view.label")}>
                <ToggleGroupItem value="read" data-view="read">
                  {t("notes.view.read")}
                </ToggleGroupItem>
                <ToggleGroupItem value="edit" data-view="edit">
                  {t("notes.view.edit")}
                </ToggleGroupItem>
                <ToggleGroupItem value="map" data-view="map">
                  {t("notes.view.map")}
                </ToggleGroupItem>
              </ToggleGroup>
            </div>
          )}
        </header>
        {textError !== null && !editing && (
          <Alert variant="crit">
            <AlertDescription>{explain(textError)}</AlertDescription>
          </Alert>
        )}
        {editing ? (
          <NoteEditor
            key={note.id}
            file={note.id}
            name={displayName(note)}
            path={joinPath(root, path)}
            vault={vaultRef}
            onLive={setLive}
            preview={
              rendered !== null && (
                <NoteView
                  path={path}
                  rendered={rendered}
                  resolverFor={resolverFor}
                  load={load}
                  onOpen={(p, sub) => open(p, sub)}
                  onTag={showTag}
                  onMissing={(target) => void create(target)}
                  labels={labels}
                />
              )
            }
          />
        ) : shown === null ? (
          <div className="flex min-h-40 items-center gap-2 text-[13px] text-muted" role="status">
            <Spinner size={16} />
            {t("notes.loading")}
          </div>
        ) : canvas ? (
          <CanvasView text={shown} resolver={vault.resolver(path, source)} onOpen={(p, sub) => open(p, sub)} labels={{ unreadable: t("notes.canvas.unreadable") }} />
        ) : view === "map" && rendered !== null ? (
          <MindMap
            root={outline(displayName(note), rendered.body)}
            label={t("notes.map.label", { name: displayName(note) })}
            onHeading={(id) => open(path, id, "read")}
          />
        ) : (
          rendered !== null && (
            <div ref={bodyRef} tabIndex={-1} className="outline-none">
              <ReaderQuickActions
                container={bodyRef}
                app="notes"
                path={joinPath(root, path)}
                title={displayName(note)}
                sectionHref={(heading) => cloudRoot + href(path, heading)}
              />
              <NoteView
                path={path}
                rendered={rendered}
                resolverFor={resolverFor}
                load={load}
                onOpen={(p, sub) => open(p, sub)}
                onTag={showTag}
                onMissing={(target) => void create(target)}
                onTask={(line, done) => void toggleTask(line, done)}
                labels={labels}
              />
            </div>
          )
        )}
      </div>
    );
  }

  // While editing, the note drawn beside its text takes the side's place.
  const side =
    tag === null && view !== "graph" && view !== "edit" && note !== undefined && path !== null ? (
      <NoteSide
        vault={vault}
        path={path}
        headings={rendered?.headings ?? []}
        href={(p) => href(p)}
        onOpen={(p) => open(p)}
        onHeading={(id) => open(path, id, "read")}
        onMissing={(target) => void create(target)}
        className="xl:sticky xl:top-20 xl:w-64 xl:shrink-0 xl:self-start"
      />
    ) : null;

  return (
    <div className="mx-auto flex w-full max-w-[96rem] flex-col gap-4 lg:flex-row lg:gap-6" data-notes-page data-vault={root}>
      <div className="lg:hidden">
        <Button variant="outline" size="sm" onClick={() => setTree((v) => !v)} aria-expanded={tree} data-tree-toggle>
          <HugeiconsIcon icon={SidebarLeftIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {vault.index.name || t("notes.title")}
        </Button>
      </div>
      <div className={cn("lg:sticky lg:top-20 lg:block lg:max-h-[calc(100dvh-6rem)] lg:w-64 lg:shrink-0 lg:self-start lg:overflow-y-auto", !tree && "max-lg:hidden")}>
        {left}
      </div>
      <main className="flex min-w-0 flex-1 flex-col gap-6 xl:flex-row">
        <div className="min-w-0 flex-1">{middle}</div>
        {side}
      </main>
    </div>
  );
}

/** One of the graph's switches, its name beside the track: the Switch is
 * the track alone, and an outside label reaches its input by `id`. */
function GraphFlag({ name, label, on, onChange }: { name: string; label: string; on: boolean; onChange: (on: boolean) => void }) {
  const id = React.useId();
  return (
    <span className="inline-flex items-center gap-2">
      <Switch id={id} size="sm" isSelected={on} onChange={onChange} data-graph-option={name} />
      <label htmlFor={id} className="cursor-pointer text-[13px] whitespace-nowrap text-ink">
        {label}
      </label>
    </span>
  );
}

const NO_RESOLVER = { note: () => null, file: () => null, href: () => "#" };

function VaultPane({
  vault,
  root,
  current,
  tag,
  href,
  onOpen,
  onTag,
  onNew,
  onGraph,
  graph,
}: {
  vault: Vault;
  root: string;
  current: string | null;
  tag: string | null;
  href: (path: string) => string;
  onOpen: (path: string) => void;
  onTag: (tag: string) => void;
  onNew: () => void;
  onGraph: () => void;
  graph: boolean;
}) {
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
      notes.search(root, text, controller.signal).then(
        (r) => setHits(r.hits),
        () => {},
      );
    }, 250);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [q, root]);

  const tags = vault.tagCounts().slice(0, 40);

  return (
    <div className="flex flex-col gap-4 rounded-card border border-hair bg-surface p-3" data-vault-pane>
      <div className="flex items-center gap-1">
        <h2 className="min-w-0 flex-1 truncate px-1 text-[14px] font-semibold tracking-tight" data-vault-name>
          {vault.index.name || t("notes.title")}
        </h2>
        <Button size="icon-sm" variant="ghost" onClick={onNew} aria-label={t("notes.new")} title={t("notes.new")} data-new-note>
          <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        <Button
          size="icon-sm"
          variant={graph ? "secondary" : "ghost"}
          onClick={onGraph}
          aria-label={t("notes.graph.open")}
          title={t("notes.graph.open")}
          aria-pressed={graph}
          data-graph-button
        >
          <HugeiconsIcon icon={WorkflowSquare01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
      </div>
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
          data-notes-search
        />
      </div>
      {hits !== null ? (
        <ul className="flex flex-col gap-1" aria-label={t("notes.search.results")} data-search-hits>
          {hits.length === 0 && <li className="px-1.5 text-[12.5px] text-faint">{t("notes.search.none")}</li>}
          {hits.map((h, i) => (
            <li key={`${h.path}:${h.line}:${i}`}>
              <a
                href={href(h.path)}
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
        <VaultTree vault={vault} current={current} href={href} onOpen={onOpen} label={t("notes.tree.label")} />
      )}
      {tags.length > 0 && (
        <section className="flex flex-col gap-2 border-t border-hair pt-3" data-tags>
          <h2 className="flex items-center gap-1.5 px-1 text-[12px] font-semibold tracking-wide text-muted uppercase">
            <HugeiconsIcon icon={Tag01Icon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("notes.tags")}
          </h2>
          <ul className="flex flex-wrap gap-1">
            {tags.map(({ tag: name, count }) => (
              <li key={name}>
                <button
                  type="button"
                  onClick={() => onTag(name)}
                  aria-pressed={tag?.toLowerCase() === name.toLowerCase()}
                  className={cn(
                    "rounded-full px-2 py-0.5 text-[12px] focus-visible:outline-2 focus-visible:outline-accent",
                    tag?.toLowerCase() === name.toLowerCase() ? "bg-accent text-surface" : "bg-accent-wash text-accent hover:bg-accent/15",
                  )}
                  data-tag-chip={name}
                >
                  #{name} <span className="tabular-nums opacity-70">{count}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

function TagList({ vault, tag, href, onOpen }: { vault: Vault; tag: string; href: (path: string) => string; onOpen: (path: string) => void }) {
  const t = useT();
  const tagged = vault.tagged(tag);
  return (
    <div className="flex flex-col gap-4" data-tag-view={tag}>
      <h1 className="text-[19px] leading-tight font-semibold tracking-tight">#{tag}</h1>
      <p className="text-[13px] text-muted">{t("notes.tag.count", { count: tagged.length })}</p>
      <ul className="flex flex-col divide-y divide-hair rounded-card border border-hair bg-surface">
        {tagged.map((n) => (
          <li key={n.path}>
            <a
              href={href(n.path)}
              onClick={(event) => {
                if (event.button !== 0 || event.metaKey || event.ctrlKey) return;
                event.preventDefault();
                onOpen(n.path);
              }}
              className="flex items-baseline justify-between gap-4 px-4 py-2.5 hover:bg-sunk focus-visible:outline-2 focus-visible:outline-accent"
            >
              <span className="truncate text-[14px] font-medium text-ink">{displayName(n)}</span>
              <span className="truncate text-[12px] text-faint">{folderOf(n.path)}</span>
            </a>
          </li>
        ))}
      </ul>
    </div>
  );
}

function VaultGraph({ vault, onOpen }: { vault: Vault; onOpen: (path: string) => void }) {
  const t = useT();
  const [options, setOptions] = React.useState<GraphOptions>(DEFAULT_GRAPH);
  const graph = React.useMemo(() => vault.graph(options), [vault, options]);
  const flag = (key: "tags" | "attachments" | "missing" | "orphans", label: string) => (
    <GraphFlag key={key} name={key} label={label} on={options[key]} onChange={(on) => setOptions((o) => ({ ...o, [key]: on }))} />
  );
  return (
    <div className="flex flex-col gap-4" data-vault-graph>
      <header className="flex flex-col gap-1">
        <h1 className="text-[19px] leading-tight font-semibold tracking-tight">{t("notes.graph.title")}</h1>
        <p className="text-[13px] text-muted">{t("notes.graph.count", { notes: graph.nodes.length, links: graph.edges.length })}</p>
      </header>
      <div className="flex flex-wrap items-center gap-x-5 gap-y-3">
        <Input
          type="search"
          value={options.filter}
          onChange={(event) => setOptions((o) => ({ ...o, filter: event.target.value }))}
          placeholder={t("notes.graph.filter")}
          aria-label={t("notes.graph.filter")}
          className="h-8 w-56 text-[13px]"
        />
        <label className="flex items-center gap-2">{flag("tags", t("notes.graph.tags"))}</label>
        <label className="flex items-center gap-2">{flag("attachments", t("notes.graph.attachments"))}</label>
        <label className="flex items-center gap-2">{flag("missing", t("notes.graph.missing"))}</label>
        <label className="flex items-center gap-2">{flag("orphans", t("notes.graph.orphans"))}</label>
      </div>
      <GraphView
        nodes={graph.nodes}
        edges={graph.edges}
        onOpen={(node) => {
          if (node.kind === "note" || node.kind === "canvas") onOpen(node.key);
        }}
        label={t("notes.graph.title")}
        className="h-[68dvh] min-h-96 rounded-card border border-hair bg-surface"
      />
    </div>
  );
}

function NoteEditor({
  file,
  name,
  path,
  vault,
  onLive,
  preview,
}: {
  file: number;
  name: string;
  /** The note, a path in LosOS Cloud. */
  path: string;
  vault: React.RefObject<Vault | null>;
  onLive: (text: string) => void;
  preview: React.ReactNode;
}) {
  const t = useT();
  const [me, setMe] = React.useState<PresenceUser | null>(null);
  const [connection, setConnection] = React.useState<Connection>("connecting");
  const [people, setPeople] = React.useState<Presence[]>([]);
  const [error, setError] = React.useState<unknown>(null);
  const liveRef = React.useRef(onLive);
  liveRef.current = onLive;
  const [view, setView] = React.useState<EditorView | null>(null);
  const extensions = React.useMemo(
    () => [
      wikilinkCompletion(() => vault.current),
      onText((text) => liveRef.current(text)),
      ViewPlugin.define((v) => {
        setView(v);
        return { destroy: () => setView(null) };
      }),
    ],
    [vault],
  );

  React.useEffect(() => {
    const controller = new AbortController();
    collab.doc(file, undefined, controller.signal).then(
      (meta) => setMe({ ...meta.me, accent: accentFor(meta.me.name) }),
      (err: unknown) => {
        if (!isAbort(err)) setError(err);
      },
    );
    return () => controller.abort();
  }, [file]);

  if (error !== null) {
    return (
      <Alert variant="crit">
        <AlertDescription>{explain(error)}</AlertDescription>
      </Alert>
    );
  }
  if (me === null) {
    return (
      <div className="flex min-h-40 items-center gap-2 text-[13px] text-muted" role="status">
        <Spinner size={16} />
        {t("notes.loading")}
      </div>
    );
  }
  const dot = connection === "live" ? "ok" : connection === "offline" ? "warn" : "pending";
  return (
    <div className="flex flex-col gap-3" data-note-editor>
      <div className="flex flex-wrap items-center gap-3">
        <Faces people={people} me={me.address} />
        <Badge variant={dot === "ok" ? "ok" : dot === "warn" ? "warn" : "neutral"} data-connection={connection}>
          <StatusDot state={dot} />
          <span role="status">{t(connection === "live" ? "collab.status.live" : connection === "offline" ? "collab.status.offline" : "collab.status.connecting")}</span>
        </Badge>
        <span className="text-[12.5px] text-muted">{t("notes.edit.hint")}</span>
      </div>
      <div className="grid gap-4 xl:grid-cols-2">
        <CollabEditor
          file={file}
          me={me}
          readOnly={false}
          label={t("collab.label", { name })}
          placeholderText={t("notes.edit.placeholder")}
          onConnection={setConnection}
          onPresence={setPeople}
          onEdited={() => {}}
          extensions={extensions}
          className="min-h-[55dvh]"
        />
        <div className="max-xl:hidden rounded-card border border-hair bg-surface px-7 py-5" aria-label={t("notes.edit.preview")} data-note-preview>
          {preview}
        </div>
      </div>
      <EditorQuickActions view={view} app="notes" path={path} title={name} vault={() => vault.current} />
    </div>
  );
}

/** /cloud/notes/open?file=<path>: a Markdown file from the file list, in
 * its vault (the folder holding `.obsidian` above it, else its folder). */
export function NotesOpen() {
  const t = useT();
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const file = params.get("file") ?? "";
  const [error, setError] = React.useState<unknown>(null);

  React.useEffect(() => {
    const controller = new AbortController();
    notes.root(file, controller.signal).then(
      ({ root }) => {
        const vault = root ?? parentOf(file);
        const rel = vault === "" ? file : file.slice(vault.length + 1);
        navigate(`/notes?${new URLSearchParams({ vault, note: rel }).toString()}`, { replace: true });
      },
      (err: unknown) => {
        if (!isAbort(err)) setError(err);
      },
    );
    return () => controller.abort();
  }, [file, navigate]);

  return error !== null ? (
    <Alert variant="crit" className="mx-auto max-w-xl">
      <AlertDescription>{explain(error)}</AlertDescription>
    </Alert>
  ) : (
    <div className="flex min-h-72 items-center justify-center gap-2 text-[13px] text-muted" role="status">
      <Spinner size={16} />
      {t("notes.loading")}
    </div>
  );
}
