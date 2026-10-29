import * as React from "react";
import { cn } from "@/lib/utils";
import "@/mdx/document.css";
import { flavourOf, loadNotes, notesReady, renderNote, section, type Rendered, type Resolver } from "./markdown";
import "./note.css";

/* A rendered note on a page: its properties, then its text. The HTML comes
 * from markdown.ts, already sanitised, with what the note asked for and
 * did not get listed above it; this fills in the ![[embeds]] (the
 * note embedded, or the part of it a heading or block names, three deep at
 * most and never itself), and turns clicks on links, tags and task boxes
 * into the page's own calls. */

export interface NoteViewProps {
  /** The note's path, so an embed of itself is refused. */
  path: string;
  rendered: Rendered;
  /** The resolver for a note, by its path: embeds resolve from their own. */
  resolverFor: (path: string) => Resolver;
  load: (path: string) => Promise<string>;
  onOpen: (path: string, sub: string) => void;
  onTag?: (tag: string) => void;
  /** A link to a note that does not exist yet. */
  onMissing?: (target: string) => void;
  /** A task box clicked: its line in the file and whether it is now done. */
  onTask?: (line: number, done: boolean) => void;
  className?: string;
  labels: { properties: string; embedLoading: string; embedMissing: string; problems?: string; line?: (line: number) => string };
}

/** Whether notes can be drawn yet: the renderer is WebAssembly, loaded once
 * per page. A component that renders notes re-renders when it is. */
export function useNotesReady(): boolean {
  const [ready, setReady] = React.useState(notesReady);
  React.useEffect(() => {
    if (ready) return;
    let live = true;
    void loadNotes().then(() => {
      if (live) setReady(true);
    });
    return () => {
      live = false;
    };
  }, [ready]);
  return ready;
}

const MAX_DEPTH = 3;

async function fillEmbeds(
  root: HTMLElement,
  props: Pick<NoteViewProps, "resolverFor" | "load">,
  chain: string[],
  labels: NoteViewProps["labels"],
  cancelled: () => boolean,
): Promise<void> {
  const spots = [...root.querySelectorAll<HTMLElement>("span.note-embed:not([data-filled])")];
  await Promise.all(
    spots.map(async (spot) => {
      spot.dataset["filled"] = "";
      const path = spot.dataset["embedPath"] ?? "";
      const sub = spot.dataset["sub"] ?? "";
      if (chain.includes(path) || chain.length > MAX_DEPTH) {
        spot.textContent = path;
        spot.classList.add("embed-missing");
        return;
      }
      spot.textContent = labels.embedLoading;
      let text: string;
      try {
        text = await props.load(path);
      } catch {
        if (!cancelled()) {
          spot.textContent = labels.embedMissing;
          spot.classList.add("embed-missing");
        }
        return;
      }
      if (cancelled()) return;
      const part = section(text, sub);
      const html = renderNote(part, props.resolverFor(path), flavourOf(path)).html;
      const box = document.createElement("div");
      box.className = "embed-body";
      // Sanitised by renderNote, like the page's own HTML.
      box.innerHTML = html;
      spot.replaceChildren(box);
      spot.dataset["embedOf"] = path;
      await fillEmbeds(box, props, [...chain, path], labels, cancelled);
    }),
  );
}

function Value({ value }: { value: unknown }) {
  if (Array.isArray(value)) {
    return (
      <span className="flex flex-wrap gap-1">
        {value.map((v, i) => (
          <span key={i} className="note-chip">
            {String(v)}
          </span>
        ))}
      </span>
    );
  }
  if (typeof value === "boolean") return <input type="checkbox" checked={value} readOnly disabled className="size-4 accent-accent" />;
  if (value === null || value === undefined) return <span className="text-faint">—</span>;
  if (value instanceof Date) return <span>{value.toLocaleDateString()}</span>;
  if (typeof value === "object") return <code>{JSON.stringify(value)}</code>;
  return <span>{String(value)}</span>;
}

export function Properties({ properties, label }: { properties: Record<string, unknown>; label: string }) {
  const entries = Object.entries(properties).filter(([k]) => k !== "position");
  if (entries.length === 0) return null;
  return (
    <dl className="note-properties" aria-label={label} data-properties>
      {entries.map(([k, v]) => (
        <div key={k} className="contents">
          <dt>{k}</dt>
          <dd>
            <Value value={v} />
          </dd>
        </div>
      ))}
    </dl>
  );
}

export function NoteView({ path, rendered, resolverFor, load, onOpen, onTag, onMissing, onTask, className, labels }: NoteViewProps) {
  const ref = React.useRef<HTMLDivElement>(null);
  const loadRef = React.useRef({ resolverFor, load });
  loadRef.current = { resolverFor, load };

  React.useEffect(() => {
    const root = ref.current;
    if (root === null) return;
    let stop = false;
    void fillEmbeds(root, loadRef.current, [path], labels, () => stop);
    return () => {
      stop = true;
    };
  }, [rendered.html, path, labels]);

  const click = (event: React.MouseEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement;
    const box = target.closest<HTMLInputElement>("input.task-box");
    if (box !== null) {
      const line = Number(box.dataset["line"]);
      // Only in the note's own text, not in an embed of another.
      if (onTask === undefined || box.closest("[data-embed-of]") !== null || !Number.isFinite(line) || line < 0) {
        event.preventDefault();
        return;
      }
      onTask(line, box.checked);
      return;
    }
    const a = target.closest<HTMLAnchorElement>("a");
    if (a === null || a.target === "_blank") return;
    if (a.dataset["notePath"] !== undefined) {
      event.preventDefault();
      onOpen(a.dataset["notePath"], a.dataset["sub"] ?? "");
    } else if (a.dataset["tag"] !== undefined) {
      event.preventDefault();
      onTag?.(a.dataset["tag"]);
    } else if (a.dataset["noteTarget"] !== undefined) {
      event.preventDefault();
      onMissing?.(a.dataset["noteTarget"]);
    } else if (a.getAttribute("href")?.startsWith("#") === true) {
      event.preventDefault();
      const id = decodeURIComponent(a.getAttribute("href")?.slice(1) ?? "");
      ref.current?.querySelector(`[id="${CSS.escape(id)}"]`)?.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  };

  return (
    <article className={cn("note", className)} data-note={path}>
      {rendered.properties !== null && <Properties properties={rendered.properties} label={labels.properties} />}
      {rendered.propertiesError !== null && <pre className="note-properties-raw">{rendered.propertiesError}</pre>}
      {rendered.problems.length > 0 && (
        <ul className="note-problems" aria-label={labels.problems} data-problems>
          {rendered.problems.map((p, i) => (
            <li key={i}>
              <span className="note-problem-line">{labels.line?.(p.line) ?? p.line}</span> {p.message}
            </li>
          ))}
        </ul>
      )}
      {/* The HTML is sanitised by markdown.ts (DOMPurify, no style). */}
      <div ref={ref} className="note-body" onClick={click} dangerouslySetInnerHTML={{ __html: rendered.html }} />
    </article>
  );
}

/** Scroll the note to a heading or block, as a link's #part asks. */
export function scrollToPart(root: HTMLElement | null, id: string): void {
  if (root === null || id === "") return;
  root.querySelector(`[id="${CSS.escape(id)}"]`)?.scrollIntoView({ block: "start" });
}
