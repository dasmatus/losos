import * as React from "react";
import { cn } from "@/lib/utils";
import { renderNote, type Resolver } from "./markdown";
import { isNote, noteName } from "./links";
import { useNotesReady } from "./note-view";
import "./note.css";

/* An Obsidian canvas (.canvas, JSON Canvas 1.0) read as a page: its cards
 * in reading order, top to bottom and left to right, with each group's
 * label above the cards inside it. Text cards are Markdown, file cards
 * link the note or show the picture, link cards link out. Laying the cards
 * out where the canvas puts them would need a style per card, which the
 * page's policy refuses; the order keeps what the canvas says. */

interface CanvasNode {
  id: string;
  type: "text" | "file" | "link" | "group";
  x: number;
  y: number;
  width: number;
  height: number;
  text?: string;
  file?: string;
  url?: string;
  label?: string;
  color?: string;
}

interface Card {
  node: CanvasNode;
  group: string | null;
}

const IMAGE = /\.(png|jpe?g|gif|webp|svg|avif|bmp)$/i;

function parse(text: string): CanvasNode[] | null {
  try {
    const data = JSON.parse(text) as { nodes?: unknown };
    if (!Array.isArray(data.nodes)) return null;
    return data.nodes.filter(
      (n): n is CanvasNode =>
        typeof n === "object" && n !== null && typeof (n as CanvasNode).id === "string" && typeof (n as CanvasNode).type === "string",
    );
  } catch {
    return null;
  }
}

function inside(n: CanvasNode, g: CanvasNode): boolean {
  return n.x >= g.x && n.y >= g.y && n.x + n.width <= g.x + g.width && n.y + n.height <= g.y + g.height;
}

function order(nodes: CanvasNode[]): Card[] {
  const groups = nodes.filter((n) => n.type === "group");
  const cards = nodes
    .filter((n) => n.type !== "group")
    .map((node) => {
      // The smallest group around a card is the one it sits in.
      const around = groups.filter((g) => inside(node, g)).sort((a, b) => a.width * a.height - b.width * b.height)[0];
      return { node, group: around?.id ?? null };
    });
  const rank = (c: Card) => {
    const g = groups.find((x) => x.id === c.group);
    return [g?.y ?? c.node.y, g?.x ?? c.node.x, c.node.y, c.node.x];
  };
  return cards.sort((a, b) => {
    const ra = rank(a);
    const rb = rank(b);
    for (let i = 0; i < ra.length; i++) {
      const d = (ra[i] ?? 0) - (rb[i] ?? 0);
      if (Math.abs(d) > 20 || (i === ra.length - 1 && d !== 0)) return d;
    }
    return 0;
  });
}

export interface CanvasViewProps {
  text: string;
  resolver: Resolver;
  onOpen: (path: string, sub: string) => void;
  className?: string;
  labels: { unreadable: string };
}

export function CanvasView({ text, resolver, onOpen, className, labels }: CanvasViewProps) {
  const nodes = React.useMemo(() => parse(text), [text]);
  // The cards render again once the renderer has loaded.
  useNotesReady();
  if (nodes === null) return <p className="text-[13px] text-muted">{labels.unreadable}</p>;
  const cards = order(nodes);
  const groups = new Map(nodes.filter((n) => n.type === "group").map((g) => [g.id, g.label ?? ""]));
  let lastGroup: string | null = null;
  const out: React.ReactNode[] = [];
  for (const { node, group } of cards) {
    if (group !== lastGroup && group !== null && (groups.get(group) ?? "") !== "") {
      out.push(
        <h2 key={`g:${group}`} className="col-span-full mt-2 text-[13px] font-semibold tracking-wide text-muted uppercase">
          {groups.get(group)}
        </h2>,
      );
    }
    lastGroup = group;
    out.push(<CanvasCard key={node.id} node={node} resolver={resolver} onOpen={onOpen} />);
  }
  return (
    <div className={cn("note grid gap-3 sm:grid-cols-2", className)} data-canvas>
      {out}
    </div>
  );
}

function CanvasCard({ node, resolver, onOpen }: { node: CanvasNode; resolver: Resolver; onOpen: (path: string, sub: string) => void }) {
  const box = "rounded-card border border-hair bg-surface p-4";
  if (node.type === "text") {
    // A card's text is Markdown, as Obsidian writes it.
    const html = renderNote(node.text ?? "", resolver, "md").html;
    return (
      <div
        className={cn(box, "note-body text-[14px]")}
        data-canvas-card="text"
        onClick={(event) => {
          const a = (event.target as HTMLElement).closest<HTMLAnchorElement>("a[data-note-path]");
          if (a === null) return;
          event.preventDefault();
          onOpen(a.dataset["notePath"] ?? "", a.dataset["sub"] ?? "");
        }}
        // Sanitised by renderNote (DOMPurify, no style).
        dangerouslySetInnerHTML={{ __html: html }}
      />
    );
  }
  if (node.type === "file") {
    const file = node.file ?? "";
    const note = isNote(file) || /\.canvas$/i.test(file) ? resolver.note(file) : null;
    if (note !== null) {
      return (
        <a
          href={resolver.href(note.path, "")}
          onClick={(event) => {
            if (event.button !== 0 || event.metaKey || event.ctrlKey) return;
            event.preventDefault();
            onOpen(note.path, "");
          }}
          className={cn(box, "block font-medium text-accent hover:bg-sunk")}
          data-canvas-card="note"
        >
          {noteName(note.path).replace(/\.canvas$/i, "")}
        </a>
      );
    }
    const url = resolver.file(file);
    if (url !== null && IMAGE.test(file)) {
      return (
        <figure className={box} data-canvas-card="image">
          <img src={url} alt={file.slice(file.lastIndexOf("/") + 1)} className="max-h-80 w-full rounded-[6px] object-contain" />
        </figure>
      );
    }
    return (
      <div className={cn(box, "text-[13px] text-muted")} data-canvas-card="file">
        {url !== null ? (
          <a href={url} target="_blank" rel="noopener" className="text-accent underline">
            {file}
          </a>
        ) : (
          file
        )}
      </div>
    );
  }
  const url = node.url ?? "";
  const safe = /^https?:\/\//i.test(url);
  return (
    <div className={cn(box, "text-[13px]")} data-canvas-card="link">
      {safe ? (
        <a href={url} target="_blank" rel="noopener noreferrer" className="break-all text-accent underline">
          {url}
        </a>
      ) : (
        <span className="break-all text-muted">{url}</span>
      )}
    </div>
  );
}
