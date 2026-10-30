/* A Markdown file in LosOS Office: a document or a presentation, written in
 * the Markdown Obsidian and GitHub read, under the same two bars as a Word
 * file (../editor.tsx). New documents start here (new.ts).
 *
 * The text is the shared editor of LosOS Cloud (../../collab/editor.tsx):
 * everyone in the file types together, and the box writes the file a
 * moment after the typing stops, so there is nothing to save. It is drawn
 * by the renderer Notes uses (src/notes/), with links, embeds and pictures
 * resolved in the file's vault: the folder holding `.obsidian` above it,
 * else its own folder.
 *
 * A document is written on the left and read on the right; either side can
 * take the whole width. A presentation (`slides: true` in its properties,
 * slides.ts) shows its slides in a strip on the left, the text in the
 * middle and the slide the cursor is in on the right, and plays full
 * screen. PDF prints the read side, or every slide a page. */

import * as React from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import { EditorSelection } from "@codemirror/state";
import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { ArrowLeft01Icon, Pdf01Icon, PlayIcon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { getProfile, type Profile } from "@/lib/api";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { readProperties, renderNote, splitFrontMatter, type Resolver } from "@/notes/markdown";
import { NoteView, useNotesReady } from "@/notes/note-view";
import { Vault } from "@/notes/vault";
import { cloud, contentUrl, joinPath, parentOf, type Entry } from "../../api";
import { collab, type DocMeta } from "../../collab/api";
import { CollabEditor, type Connection, type Presence } from "../../collab/editor";
import { Faces } from "../../collab/faces";
import { accentFor, nameOf as loginOf } from "../../collab/people";
import { NameDialog, explain } from "../../dialogs";
import { cloudSource, noteHref, notes } from "../../notes/api";
import { onText, wikilinkCompletion } from "../../notes/editing";
import { EditorQuickActions } from "../../quick-text";
import { activeAt, formatKeys, headingAt, type Action, type Words } from "./format";
import { Filmstrip, Presenter, SlideView } from "./slide";
import { isDeck, slideAt, splitSlides } from "./slides";
import { MarkdownToolbar } from "./toolbar";
import "@/notes/note.css";
import "./markdown.css";

type Layout = "write" | "split" | "read";

interface Cursor {
  active: ReadonlySet<Action>;
  heading: number;
  /** The cursor's line, from 0. */
  line: number;
}

const NO_CURSOR: Cursor = { active: new Set(), heading: 0, line: 0 };

/** The file's vault, listed again now and then so a picture added beside
 * the document shows up. */
function useVault(path: string) {
  const [state, setState] = React.useState<{ root: string; vault: Vault } | null>(null);
  React.useEffect(() => {
    let live = true;
    let timer = 0;
    const load = async () => {
      const found = await notes.root(path).catch(() => ({ root: null }));
      const root = found.root ?? parentOf(path);
      const list = async () => {
        const index = await notes.vault(root);
        if (live) setState({ root, vault: new Vault(index) });
      };
      await list().catch(() => {});
      timer = window.setInterval(() => {
        if (document.visibilityState === "visible") list().catch(() => {});
      }, 10_000);
    };
    void load();
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [path]);
  return state;
}

export function MarkdownOffice({ entry: first, onClose }: { entry: Entry; onClose: () => void }) {
  const t = useT();
  useLocale();
  const navigate = useNavigate();
  const [entry, setEntry] = React.useState(first);
  const [meta, setMeta] = React.useState<DocMeta | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  const [connection, setConnection] = React.useState<Connection>("connecting");
  const [people, setPeople] = React.useState<Presence[]>([]);
  const [text, setText] = React.useState("");
  const [view, setView] = React.useState<EditorView | null>(null);
  const [cursor, setCursor] = React.useState<Cursor>(NO_CURSOR);
  const [layout, setLayout] = React.useState<Layout>("split");
  const [presenting, setPresenting] = React.useState<number | null>(null);
  const [printing, setPrinting] = React.useState(false);
  const [renaming, setRenaming] = React.useState(false);

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

  // This person's colour and picture from their profile, as the shared
  // editor shows them to the others.
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

  const found = useVault(entry.path);
  const folder = parentOf(entry.path);
  const rel = found === null ? entry.name : found.root === "" ? entry.path : entry.path.slice(found.root.length + 1);
  const source = React.useMemo(() => (found === null ? null : cloudSource(found.root)), [found?.root]);
  const resolver: Resolver = React.useMemo(
    () =>
      found !== null && source !== null
        ? found.vault.resolver(rel, source)
        : // Until the vault is listed: pictures beside the file.
          { note: () => null, file: (target) => contentUrl(joinPath(folder, target)), href: () => "#" },
    [found, source, rel, folder],
  );
  const resolverFor = React.useCallback(
    (p: string) => (found !== null && source !== null ? found.vault.resolver(p, source) : resolver),
    [found, source, resolver],
  );
  const load = React.useCallback((p: string) => (source === null ? Promise.reject(new Error("no vault")) : source.text(p)), [source]);

  const words: Words = React.useMemo(
    () => ({
      text: t("office.md.word.text"),
      code: t("office.md.word.code"),
      formula: "E = mc^2",
      key: "Ctrl",
      url: "https://",
      picture: t("office.md.word.picture"),
      note: t("office.md.word.note"),
      tag: t("office.md.word.tag"),
      title: t("office.md.word.title"),
      summary: t("office.md.word.summary"),
      column: t("office.md.word.column"),
      footnote: t("office.md.word.footnote"),
      callout: t("office.md.word.callout"),
    }),
    [t],
  );
  const wordsRef = React.useRef(words);
  wordsRef.current = words;
  const vaultRef = React.useRef<Vault | null>(null);
  vaultRef.current = found?.vault ?? null;

  // Read once, when the editor starts.
  const extensions = React.useMemo(
    () => [
      onText(setText),
      formatKeys(() => wordsRef.current),
      wikilinkCompletion(() => vaultRef.current),
      ViewPlugin.define((v) => {
        setView(v);
        return { destroy: () => setView(null) };
      }),
      EditorView.updateListener.of((u: ViewUpdate) => {
        if (!u.selectionSet && !u.docChanged && !u.focusChanged) return;
        const state = u.state;
        setCursor({ active: activeAt(state), heading: headingAt(state), line: state.doc.lineAt(state.selection.main.head).number - 1 });
      }),
    ],
    [],
  );

  const shown = React.useDeferredValue(text);
  const properties = React.useMemo(() => readProperties(splitFrontMatter(shown).yaml).properties, [shown]);
  const deck = isDeck(properties);
  const slides = React.useMemo(() => (deck ? splitSlides(shown) : []), [deck, shown]);
  const current = deck ? slideAt(slides, cursor.line) : 0;
  const notesReady = useNotesReady();
  const rendered = React.useMemo(() => (deck || !notesReady ? null : renderNote(shown, resolver)), [deck, notesReady, shown, resolver]);
  const labels = React.useMemo(
    () => ({ properties: t("notes.properties"), embedLoading: t("notes.embed.loading"), embedMissing: t("notes.embed.missing") }),
    [t],
  );

  const goToSlide = (index: number) => {
    const slide = slides[index];
    if (view === null || slide === undefined) return;
    const doc = view.state.doc;
    let n = Math.min(doc.lines, slide.line + 1);
    // The slide's first line with something on it.
    while (n < doc.lines && doc.line(n).text.trim() === "") n++;
    view.dispatch({ selection: EditorSelection.cursor(doc.line(n).to), scrollIntoView: true });
    view.focus();
  };

  const tick = (line: number, done: boolean) => {
    if (view === null) return;
    const l = view.state.doc.line(line + 1);
    const m = /^(\s*(?:>\s*)*(?:[-*+]|\d+[.)])\s+\[)(.)(\])/.exec(l.text);
    if (m === null) return;
    const at = l.from + (m[1]?.length ?? 0);
    view.dispatch({ changes: { from: at, to: at + 1, insert: done ? "x" : " " } });
  };

  // Print once the copy for it is on the page; drop it afterwards.
  React.useEffect(() => {
    if (!printing) return;
    const done = () => setPrinting(false);
    window.addEventListener("afterprint", done);
    const frame = window.requestAnimationFrame(() => window.print());
    return () => {
      window.cancelAnimationFrame(frame);
      window.removeEventListener("afterprint", done);
    };
  }, [printing]);

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

  const readOnly = meta !== null && meta.perm !== "edit";
  // Offline is still a document: the editor keeps the typing and merges
  // it when the box is back, as LosOS Cloud's editor does.
  const loaded = meta !== null && me !== null && view !== null && connection !== "connecting";
  const status =
    connection === "live" ? t("office.saved") : connection === "offline" ? t("office.md.offline") : t("collab.status.connecting");
  const showEditor = deck || layout !== "read";
  const showRead = !deck && layout !== "write";

  return (
    <div className="flex h-full min-h-0 flex-col bg-ground text-ink" data-markdown-office data-deck={deck ? "" : undefined}>
      <header data-testid="office-bar" className="flex h-12 shrink-0 items-center gap-2 border-b border-hair bg-surface px-2 sm:px-3">
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
            data-testid="md-name"
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
          {deck ? t("office.md.kindDeck") : t("office.md.kindDoc")}
        </Badge>
        {readOnly && <Badge variant="secondary">{t("office.readOnly")}</Badge>}
        <span className="ml-auto" />
        {meta !== null && <Faces people={people} me={meta.me.address} />}
        {!deck && (
          <ToggleGroup
            variant="outline"
            value={layout}
            onValueChange={(v) => v !== "" && setLayout(v as Layout)}
            aria-label={t("office.md.layout")}
            className="max-md:hidden"
          >
            <ToggleGroupItem value="write" data-layout="write">
              {t("office.md.write")}
            </ToggleGroupItem>
            <ToggleGroupItem value="split" data-layout="split">
              {t("office.md.split")}
            </ToggleGroupItem>
            <ToggleGroupItem value="read" data-layout="read">
              {t("office.md.read")}
            </ToggleGroupItem>
          </ToggleGroup>
        )}
        <Tool icon={Pdf01Icon} label={t("office.pdf")} onClick={() => setPrinting(true)} />
        {deck && (
          <Button size="sm" onClick={() => setPresenting(current)} disabled={slides.length === 0} data-testid="md-present">
            <HugeiconsIcon icon={PlayIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:sr-only">{t("office.tool.present")}</span>
          </Button>
        )}
      </header>
      {!readOnly && <MarkdownToolbar view={view} active={cursor.active} heading={cursor.heading} deck={deck} words={words} disabled={!loaded} />}

      <div className="relative flex min-h-0 flex-1" aria-busy={!loaded}>
        {deck && (
          <aside className="w-52 shrink-0 border-r border-hair bg-surface max-md:hidden">
            <Filmstrip slides={slides} current={current} resolver={resolver} onPick={goToSlide} />
          </aside>
        )}
        <section className={cn("min-h-0 min-w-0 flex-1 bg-surface", !showEditor && "hidden", showRead && "border-r border-hair")}>
          {meta !== null && me !== null && (
            <CollabEditor
              key={entry.id}
              file={entry.id}
              me={me}
              readOnly={readOnly}
              label={t("collab.label", { name: entry.name })}
              // One text for both: the editor starts again when it changes.
              placeholderText={t("office.md.placeholder")}
              onConnection={setConnection}
              onPresence={setPeople}
              onEdited={() => {}}
              extensions={extensions}
              className="h-full min-h-0 rounded-none border-0 shadow-none"
            />
          )}
        </section>
        {showRead && rendered !== null && (
          <section className="min-h-0 min-w-0 flex-1 overflow-y-auto px-4 py-6 sm:px-8" data-testid="md-read">
            {/* The document as a page, the way it prints. */}
            <div className="mx-auto min-h-full max-w-3xl rounded-card bg-surface px-6 py-8 shadow-card ring-1 ring-line sm:px-12 sm:py-10">
              <NoteView
                path={rel}
                rendered={rendered}
                resolverFor={resolverFor}
                load={load}
                onOpen={(p, sub) => found !== null && navigate(noteHref(found.root, p, sub))}
                onTag={(tag) => found !== null && navigate(`/notes?${new URLSearchParams({ vault: found.root, tag }).toString()}`)}
                onTask={readOnly ? undefined : tick}
                labels={labels}
              />
            </div>
          </section>
        )}
        {deck && (
          <section className="flex min-h-0 min-w-0 flex-1 items-center justify-center overflow-auto bg-sunk p-6" data-testid="md-stage">
            {slides[current] !== undefined && (
              <SlideView slide={slides[current]} resolver={resolver} className="max-w-full rounded-card shadow-card ring-1 ring-line" current />
            )}
          </section>
        )}
        {!loaded && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-ground/80">
            <Spinner size={28} label={t("office.opening", { name: entry.name })} />
          </div>
        )}
      </div>

      {meta !== null && presenting === null && (
        <EditorQuickActions
          view={view}
          app="office"
          path={entry.path}
          title={entry.name}
          vault={() => vaultRef.current}
          readOnly={readOnly}
          deck={deck}
        />
      )}

      {presenting !== null && (
        <Presenter
          slides={slides}
          start={Math.min(presenting, Math.max(0, slides.length - 1))}
          resolver={resolver}
          onClose={(at) => {
            setPresenting(null);
            goToSlide(at);
          }}
        />
      )}

      {printing &&
        createPortal(
          <div className={cn("md-print", deck && "md-print-deck")} data-theme-light="">
            {deck ? (
              slides.map((slide, i) => <SlideView key={i} slide={slide} resolver={resolver} />)
            ) : rendered !== null ? (
              <NoteView path={rel} rendered={rendered} resolverFor={resolverFor} load={load} onOpen={() => {}} labels={labels} />
            ) : null}
          </div>,
          document.body,
        )}

      <NameDialog
        open={renaming}
        onOpenChange={setRenaming}
        title={t("office.md.renameTitle")}
        submit={t("office.md.rename")}
        initial={entry.name.replace(/\.(md|markdown)$/i, "")}
        onSubmit={async (name) => {
          const file = /\.(md|markdown)$/i.test(name) ? name : `${name}.md`;
          const moved = await cloud.move(entry.path, joinPath(folder, file));
          setEntry(moved);
        }}
      />
    </div>
  );
}

function Tool({ icon, label, onClick }: { icon: IconSvgElement; label: string; onClick: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger render={<Button variant="ghost" size="icon-sm" aria-label={label} onClick={onClick} />}>
        <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );
}
