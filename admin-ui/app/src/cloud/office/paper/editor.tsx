/* A document (`.paper`) or a presentation (`.slides`) in LosOS Office,
 * edited as it looks: the formatting bar above the page (ribbon.tsx), the
 * fields of the selected part under it (fields.tsx), and the page itself,
 * sized and margined by the document's page setup.
 *
 * The file is MDX, read and written by losos-mdx in the page
 * (src/mdx/). Everyone in the file edits the same shared text as LosOS
 * Cloud's text editor and Notes do (sync.ts), and the box writes the file
 * a moment after the typing stops, so there is nothing to save. Nothing in
 * a document runs: the page draws each component itself.
 *
 * A presentation is a run of `<Slide>`s: the strip on the left draws them,
 * the page shows each as a 16:9 card, and Present plays them full screen.
 * Print, or Save as PDF in the print dialog, prints the page with its
 * header, footer and page numbers in the margins. */

import * as React from "react";
import { baseKeymap, chainCommands, exitCode } from "prosemirror-commands";
import { dropCursor } from "prosemirror-dropcursor";
import { gapCursor } from "prosemirror-gapcursor";
import { keymap } from "prosemirror-keymap";
import { EditorState, TextSelection } from "prosemirror-state";
import { goToNextCell, tableEditing } from "prosemirror-tables";
import { EditorView } from "prosemirror-view";
import { WebsocketProvider } from "y-websocket";
import * as Y from "yjs";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, PlayIcon, PrinterIcon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { getProfile, type Profile } from "@/lib/api";
import { useLocale, useT } from "@/lib/i18n-react";
import { intlTag } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { schema } from "@/mdx/schema";
import { loadMdx, type Resolver } from "@/mdx/wasm";
import { cloud, CloudError, contentUrl, joinPath, parentOf, upload, type Entry } from "../../api";
import { collab, type DocMeta } from "../../collab/api";
import type { Connection, Presence } from "../../collab/editor";
import { Faces } from "../../collab/faces";
import { accentFor, nameOf as loginOf } from "../../collab/people";
import { NameDialog, explain } from "../../dialogs";
import { hardBreak, insertBlock, markdownRules, splitItem, toggle, indent, outdent, applyMark, marks as M } from "./commands";
import { Fields } from "./fields";
import { DEFAULT_PAGE, pageOf, printRule, properties, withPage, type PageSetup } from "./page";
import { PageSetupDialog } from "./page-setup";
import { Filmstrip, Presenter, slidesOf } from "./present";
import { kindOf } from "./new";
import { Ribbon } from "./ribbon";
import { paperSync, syncKey } from "./sync";
import { nodeViews, tocPlugin, type ViewLabel } from "./views";
import "prosemirror-view/style/prosemirror.css";
import "prosemirror-tables/style/tables.css";
import "prosemirror-gapcursor/style/gapcursor.css";
import "./paper.css";

/** The shared text's name; the box uses the same (room.rs, TEXT). */
const TEXT = "content";

interface Comment {
  from: number;
  text: string;
  author: string | null;
  date: string | null;
  quote: string;
}

function commentsOf(state: EditorState): Comment[] {
  const out: Comment[] = [];
  const type = schema.marks["comment"]!;
  let last: { mark: unknown; comment: Comment; end: number } | null = null;
  state.doc.descendants((node, pos) => {
    if (!node.isInline) return true;
    const mark = type.isInSet(node.marks);
    if (mark === undefined) return false;
    if (last !== null && last.mark === mark && last.end === pos) {
      last.comment.quote += node.textContent;
      last.end = pos + node.nodeSize;
      return false;
    }
    const comment = {
      from: pos,
      text: String(mark.attrs["text"]),
      author: (mark.attrs["author"] as string | null) ?? null,
      date: (mark.attrs["date"] as string | null) ?? null,
      quote: node.textContent,
    };
    out.push(comment);
    last = { mark, comment, end: pos + node.nodeSize };
    return false;
  });
  return out;
}

/** The page's header text, and its footer as text with null for each
 * page number, for the print's margin boxes. */
function edgesOf(state: EditorState): { header: string | null; footer: (string | null)[] | null } {
  let header: string | null = null;
  let footer: (string | null)[] | null = null;
  state.doc.forEach((node) => {
    if (node.type.name === "page_header") header = node.textContent;
    if (node.type.name === "page_footer") {
      const parts: (string | null)[] = [];
      node.forEach((child) => {
        if (child.type.name === "page_number") parts.push(null);
        else if (child.isText) parts.push(child.text ?? "");
        else if (child.type.name === "hard_break") parts.push(" ");
      });
      footer = parts;
    }
  });
  return { header, footer };
}

/** The address of a picture beside the document, by the name it is
 * written with. Other sites' pictures are not fetched from the box's
 * pages. */
function pictureUrl(folder: string, src: string): string | null {
  if (src === "" || /^[a-z][a-z0-9+.-]*:/i.test(src) || src.startsWith("//")) return null;
  const parts = joinPath(folder, src.replace(/^\.\//, "")).split("/");
  const out: string[] = [];
  for (const p of parts) {
    if (p === "..") out.pop();
    else if (p !== "." && p !== "") out.push(p);
  }
  return contentUrl(out.join("/"));
}

export function PaperOffice({ entry: first, onClose }: { entry: Entry; onClose: () => void }) {
  const t = useT();
  const locale = useLocale();
  const [entry, setEntry] = React.useState(first);
  const kind = kindOf(entry.name);
  const folder = parentOf(entry.path);
  const [meta, setMeta] = React.useState<DocMeta | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  const [ready, setReady] = React.useState(false);
  const [connection, setConnection] = React.useState<Connection>("connecting");
  const [people, setPeople] = React.useState<Presence[]>([]);
  const [view, setView] = React.useState<EditorView | null>(null);
  const [state, setState] = React.useState<EditorState | null>(null);
  const [setup, setSetup] = React.useState(false);
  const [renaming, setRenaming] = React.useState(false);
  const [presenting, setPresenting] = React.useState<number | null>(null);
  const [pictureError, setPictureError] = React.useState<string | null>(null);
  const mountRef = React.useRef<HTMLDivElement>(null);
  const fileInput = React.useRef<HTMLInputElement>(null);
  const history = React.useRef<{ undo: () => boolean; redo: () => boolean } | null>(null);

  React.useEffect(() => {
    loadMdx().then(
      () => setReady(true),
      (e: unknown) => setError(e),
    );
  }, []);

  React.useEffect(() => {
    const ac = new AbortController();
    collab.doc(entry.id, undefined, ac.signal).then(setMeta, (e: unknown) => {
      if (!ac.signal.aborted) setError(e);
    });
    return () => ac.abort();
  }, [entry.id]);

  React.useEffect(() => {
    document.title = `${entry.name} · LosOS`;
  }, [entry.name]);

  const login = meta === null ? null : loginOf(meta.me.address);
  const [profile, setProfile] = React.useState<Profile | null>(null);
  React.useEffect(() => {
    if (login === null) return;
    const ac = new AbortController();
    getProfile(login, { signal: ac.signal }).then(setProfile, () => {});
    return () => ac.abort();
  }, [login]);
  const me = React.useMemo(
    () =>
      meta === null
        ? null
        : { ...meta.me, accent: profile?.accent ?? accentFor(loginOf(meta.me.address)), picture: profile?.picture ?? null },
    [meta?.me.name, meta?.me.address, profile?.accent, profile?.picture],
  );
  const meRef = React.useRef(me);
  meRef.current = me;

  const readOnly = meta !== null && meta.perm !== "edit";
  const readOnlyRef = React.useRef(readOnly);
  readOnlyRef.current = readOnly;

  const resolver: Resolver = React.useMemo(
    () => ({ file: (target: string) => pictureUrl(folder, target), href: (path: string, sub: string) => (sub ? `${path}#${sub}` : path) }),
    [folder],
  );

  const labelsRef = React.useRef(t);
  labelsRef.current = t;

  // The page: ProseMirror over the shared text, for this file.
  React.useEffect(() => {
    const host = mountRef.current;
    if (!ready || meta === null || host === null) return;
    const doc = new Y.Doc();
    const ytext = doc.getText(TEXT);
    const provider = new WebsocketProvider(collab.server(), "ws", doc, {
      params: collab.params(entry.id),
      disableBc: true,
    });
    const awareness = provider.awareness;
    if (meRef.current !== null) awareness.setLocalStateField("user", meRef.current);
    provider.on("status", ({ status }: { status: string }) => {
      if (status !== "connected") setConnection(status === "connecting" ? "connecting" : "offline");
    });
    provider.on("sync", (synced: boolean) => {
      if (synced) setConnection("live");
    });
    provider.on("connection-close", () => setConnection("offline"));
    const presence = () => {
      const out: Presence[] = [];
      awareness.getStates().forEach((s, client) => {
        const user = (s as { user?: Presence["user"] }).user;
        if (user !== undefined && typeof user.name === "string") out.push({ client, user });
      });
      setPeople(out);
    };
    awareness.on("change", presence);
    presence();

    const sync = paperSync(ytext, awareness);
    history.current = sync;
    const label = (key: ViewLabel) => labelsRef.current(key.startsWith("callout.") ? (`office.md.${key}` as never) : key.startsWith("layout.") ? (`paper.${key}` as never) : (`paper.view.${key}` as never));
    const ctx = { picture: (src: string) => pictureUrl(folder, src), locale: intlTag(), label };
    const undoKey = () => sync.undo();
    const redoKey = () => sync.redo();
    const editorState = EditorState.create({
      schema,
      plugins: [
        ...sync.plugins,
        markdownRules(),
        keymap({
          "Mod-z": undoKey,
          "Mod-y": redoKey,
          "Mod-Shift-z": redoKey,
          "Mod-b": toggle("strong"),
          "Mod-i": toggle("em"),
          "Mod-u": toggle("underline"),
          "Mod-e": toggle("code"),
          "Mod-Shift-x": toggle("strike"),
          "Mod-Shift-h": toggle("highlight"),
          "Mod-k": applyMark(M["link"]!.create({ href: "https://" }), "https://"),
          "Mod-Enter": chainCommands(exitCode, insertBlock(schema.nodes["page_break"]!.create())),
          "Shift-Enter": hardBreak,
          Enter: splitItem,
          Tab: chainCommands(goToNextCell(1), indent),
          "Shift-Tab": chainCommands(goToNextCell(-1), outdent),
        }),
        keymap(baseKeymap),
        dropCursor({ class: "paper-drop" }),
        gapCursor(),
        tableEditing(),
        tocPlugin(ctx),
      ],
    });
    const pm = new EditorView(host, {
      state: editorState,
      nodeViews: nodeViews(ctx),
      editable: () => !readOnlyRef.current,
      attributes: { class: "paper-text mdx-document", "aria-label": t("collab.label", { name: entry.name }), spellcheck: "true" },
      // The page never follows a link or opens what it shows.
      handleClickOn: (_v, _pos, _node, _nodePos, event) => (event.target as HTMLElement).closest("a") !== null,
      // `this`, not `pm`: the sync plugin reads the room's text in while
      // the view is still being made.
      dispatchTransaction(this: EditorView, tr) {
        const next = this.state.apply(tr);
        this.updateState(next);
        setState(next);
      },
    });
    setView(pm);
    setState(pm.state);
    host.dataset["ready"] = "";
    return () => {
      awareness.off("change", presence);
      provider.destroy();
      pm.destroy();
      doc.destroy();
      history.current = null;
      setView(null);
      setState(null);
      delete host.dataset["ready"];
    };
    // One page per file; the page is keyed by the file.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready, meta === null, entry.id]);

  React.useEffect(() => {
    if (view !== null && me !== null) {
      // A profile read after the page opened: the others see it at once.
      view.dispatch(view.state.tr);
    }
  }, [me, view]);

  React.useEffect(() => {
    view?.setProps({ editable: () => !readOnly });
  }, [readOnly, view]);

  // The page setup and the print's page rule, from the front matter.
  const yaml = state?.doc.firstChild?.type.name === "front_matter" ? String(state.doc.firstChild.attrs["yaml"]) : null;
  const page = React.useMemo<PageSetup>(() => (ready ? pageOf(properties(yaml)) : DEFAULT_PAGE), [yaml, ready]);
  const edges = state === null ? { header: null, footer: null } : edgesOf(state);
  const rule = printRule(kind === "slides" ? { size: "A4", orientation: "landscape", margin: "narrow" } : page, edges.header, edges.footer);
  React.useEffect(() => {
    const sheet = new CSSStyleSheet();
    sheet.replaceSync(`@media print { ${rule} }`);
    document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
    return () => {
      document.adoptedStyleSheets = document.adoptedStyleSheets.filter((s) => s !== sheet);
    };
  }, [rule]);

  const applyPage = (next: PageSetup) => {
    if (view === null) return;
    const text = withPage(yaml, next);
    const first = view.state.doc.firstChild;
    const tr = view.state.tr;
    if (first?.type.name === "front_matter") {
      if (text === "") tr.delete(0, first.nodeSize);
      else tr.setNodeMarkup(0, undefined, { yaml: text });
    } else if (text !== "") {
      tr.insert(0, schema.nodes["front_matter"]!.create({ yaml: text }));
    }
    view.dispatch(tr);
  };

  const deferred = React.useDeferredValue(state);
  const slides = React.useMemo(() => (deferred !== null && kind === "slides" && ready ? slidesOf(deferred.doc, resolver) : []), [deferred?.doc, kind, ready, resolver]);
  const current = React.useMemo(() => {
    if (state === null || kind !== "slides") return 0;
    const head = state.selection.from;
    let index = 0;
    slides.forEach((s, i) => {
      if (s.pos <= head) index = i;
    });
    return index;
  }, [state?.selection, slides, kind]);
  const goToSlide = (index: number) => {
    const s = slides[index];
    if (view === null || s === undefined) return;
    view.dispatch(view.state.tr.setSelection(TextSelection.near(view.state.doc.resolve(s.pos + 1))).scrollIntoView());
    view.focus();
  };

  // Printing prints a copy of the page on its own: the app around it is
  // hidden, and the editor's own strips (a component's label, the caret)
  // are left out by paper.css.
  const printPage = () => {
    if (view === null) return;
    const copy = document.createElement("div");
    copy.className = `paper-print mdx-document paper-print-${kind}`;
    const text = view.dom.cloneNode(true) as HTMLElement;
    text.removeAttribute("contenteditable");
    copy.append(text);
    document.body.append(copy);
    const done = () => {
      copy.remove();
      window.removeEventListener("afterprint", done);
    };
    window.addEventListener("afterprint", done);
    window.print();
  };

  const comments = state === null ? [] : commentsOf(state);
  const sync = state === null ? null : (syncKey.getState(state) ?? null);

  const addPicture = async (file: File) => {
    setPictureError(null);
    const dot = file.name.lastIndexOf(".");
    const base = dot > 0 ? file.name.slice(0, dot) : file.name;
    const ext = dot > 0 ? file.name.slice(dot) : "";
    for (let n = 1; n < 100; n++) {
      const name = n === 1 ? file.name : `${base} ${n}${ext}`;
      try {
        await upload(joinPath(folder, name), file, { ifNew: true }).done;
        if (view !== null) {
          insertBlock(schema.nodes["figure"]!.create({ src: name, alt: base, caption: "" }))(view.state, view.dispatch, view);
          view.focus();
        }
        return;
      } catch (err) {
        if (err instanceof CloudError && err.code === "exists") continue;
        setPictureError(`${t("paper.pictureFailed")}: ${explain(err)}`);
        return;
      }
    }
  };

  if (error !== null) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-6 text-center">
        <p className="max-w-md text-sm text-muted">{explain(error)}</p>
        <Button variant="secondary" onClick={onClose}>
          {t("office.back")}
        </Button>
      </div>
    );
  }

  const loaded = view !== null && connection !== "connecting";
  const status = connection === "live" ? t("office.saved") : connection === "offline" ? t("office.md.offline") : t("collab.status.connecting");
  const ext = kind === "slides" ? ".slides" : ".paper";

  return (
    <div className="flex h-full min-h-0 flex-col bg-ground text-ink" data-paper-office={kind}>
      <header data-testid="office-bar" className="paper-chrome-bar flex h-12 shrink-0 items-center gap-2 border-b border-hair bg-surface px-2 sm:px-3">
        <Button variant="ghost" size="sm" onClick={onClose}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <span className="max-sm:sr-only">{t("office.back")}</span>
        </Button>
        <span aria-hidden="true" className="h-5 w-px shrink-0 bg-line" />
        <div className="flex min-w-0 flex-col">
          <button
            type="button"
            className="truncate rounded-[4px] text-left text-sm leading-tight font-medium hover:underline focus-visible:outline-2 focus-visible:outline-accent disabled:no-underline"
            onClick={() => setRenaming(true)}
            disabled={readOnly || meta === null}
            title={t("office.md.rename")}
            data-testid="paper-name"
          >
            <h1 className="truncate">{entry.name}</h1>
          </button>
          {meta !== null && !readOnly && (
            <p className="truncate text-[12px] leading-tight text-muted" data-testid="office-saved" data-connection={connection} aria-live="polite">
              {status}
            </p>
          )}
        </div>
        <Badge variant="secondary" className="max-sm:hidden">
          {t(kind === "slides" ? "paper.kind.slides" : "paper.kind.paper")}
        </Badge>
        {readOnly && <Badge variant="secondary">{t("office.readOnly")}</Badge>}
        <span className="ml-auto" />
        {meta !== null && <Faces people={people} me={meta.me.address} />}
        <Tooltip>
          <TooltipTrigger render={<Button variant="ghost" size="icon-sm" aria-label={t("paper.print")} onClick={printPage} data-action="print" />}>
            <HugeiconsIcon icon={PrinterIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          </TooltipTrigger>
          <TooltipContent side="bottom">{t("paper.print")}</TooltipContent>
        </Tooltip>
        {kind === "slides" && (
          <Button size="sm" onClick={() => setPresenting(current)} disabled={slides.length === 0} data-testid="paper-present">
            <HugeiconsIcon icon={PlayIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:sr-only">{t("office.tool.present")}</span>
          </Button>
        )}
      </header>
      {!readOnly && (
        <div className="paper-chrome-bar">
          <Ribbon
            view={view}
            state={state}
            kind={kind}
            disabled={!loaded || (sync?.broken ?? null) !== null}
            me={login ?? ""}
            undo={() => {
              history.current?.undo();
              view?.focus();
            }}
            redo={() => {
              history.current?.redo();
              view?.focus();
            }}
            onPicture={() => fileInput.current?.click()}
            onPageSetup={() => setSetup(true)}
          />
          <Fields view={view} state={state} readOnly={readOnly} />
        </div>
      )}
      {sync !== null && sync.broken !== null && (
        <div role="alert" className="paper-chrome-bar flex items-center gap-3 border-b border-hair bg-warn/10 px-3 py-2 text-[13px]" data-testid="paper-broken">
          <span className="flex-1">{t("paper.broken", { error: sync.broken })}</span>
          <a href={`/cloud/edit/${entry.id}`} className={buttonVariants({ variant: "secondary", size: "sm" })}>
            {t("paper.openText")}
          </a>
        </div>
      )}
      {pictureError !== null && (
        <div role="alert" className="paper-chrome-bar border-b border-hair bg-crit/10 px-3 py-2 text-[13px]">
          {pictureError}
        </div>
      )}
      <input
        ref={fileInput}
        type="file"
        accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml"
        className="hidden"
        onChange={(e) => {
          const file = e.target.files?.[0];
          e.target.value = "";
          if (file !== undefined) void addPicture(file);
        }}
      />

      <main className="relative flex min-h-0 flex-1" aria-busy={!loaded}>
        {kind === "slides" && (
          <aside className="paper-chrome-bar w-56 shrink-0 border-r border-hair bg-surface max-md:hidden">
            <Filmstrip slides={slides} current={current} onPick={goToSlide} />
          </aside>
        )}
        <div className="paper-desk min-h-0 min-w-0 flex-1 overflow-auto" data-testid="paper-desk">
          <div
            className={cn(
              "paper-page",
              kind === "slides" ? "paper-deck" : [`paper-size-${page.size.toLowerCase()}`, `paper-${page.orientation}`, `paper-margin-${page.margin}`],
            )}
            data-testid="paper-page"
            data-size={page.size}
            data-orientation={page.orientation}
          >
            <div ref={mountRef} className="paper-mount" data-testid="paper-editor" />
          </div>
          {sync !== null && sync.problems.length > 0 && (
            <p className="paper-chrome-bar mx-auto mb-6 max-w-[210mm] text-center text-[12px] text-muted" data-testid="paper-problems">
              {t("paper.problems", { count: sync.problems.length })}
            </p>
          )}
        </div>
        {comments.length > 0 && (
          <aside className="paper-chrome-bar w-64 shrink-0 overflow-y-auto border-l border-hair bg-surface p-3 max-lg:hidden" data-testid="paper-comments">
            <h2 className="mb-2 text-[13px] font-medium">{t("paper.comments")}</h2>
            <ul className="flex flex-col gap-2">
              {comments.map((c, i) => (
                <li key={`${c.from}-${i}`}>
                  <button
                    type="button"
                    className="w-full rounded-control border border-line p-2 text-left text-[13px] hover:bg-sunk"
                    onClick={() => {
                      if (view === null) return;
                      view.dispatch(view.state.tr.setSelection(TextSelection.create(view.state.doc, c.from + 1)).scrollIntoView());
                      view.focus();
                    }}
                  >
                    <span className="block text-[11px] text-muted">
                      {c.author ?? ""}
                      {c.date !== null ? ` · ${new Intl.DateTimeFormat(intlTag(), { dateStyle: "medium" }).format(new Date(c.date))}` : ""}
                    </span>
                    <span className="block">{c.text}</span>
                    <span className="mt-1 block truncate text-[12px] text-faint">“{c.quote}”</span>
                  </button>
                </li>
              ))}
            </ul>
          </aside>
        )}
        {!loaded && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-ground/80">
            <Spinner size={28} label={t("office.opening", { name: entry.name })} />
          </div>
        )}
      </main>

      {presenting !== null && (
        <Presenter
          slides={slides}
          start={Math.min(presenting, Math.max(0, slides.length - 1))}
          onClose={(at) => {
            setPresenting(null);
            goToSlide(at);
          }}
        />
      )}

      <PageSetupDialog open={setup} onOpenChange={setSetup} page={page} onApply={applyPage} />

      <NameDialog
        open={renaming}
        onOpenChange={setRenaming}
        title={t("office.md.renameTitle")}
        submit={t("office.md.rename")}
        initial={entry.name.slice(0, entry.name.length - ext.length)}
        onSubmit={async (name) => {
          const file = name.toLowerCase().endsWith(ext) ? name : `${name}${ext}`;
          const moved = await cloud.move(entry.path, joinPath(folder, file));
          setEntry(moved);
        }}
      />
      <span hidden data-locale={locale} />
    </div>
  );
}
