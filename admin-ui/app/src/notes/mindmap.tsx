import * as React from "react";
import { anchorFor } from "./markdown";

/* A note as a mind map: its name in the middle of the left edge, its
 * headings branching to the right, and the list items under each heading
 * as the twigs, so an outline reads at a glance. Each top branch has one
 * colour of the palette. SVG with classes only (note.css); a click on a
 * heading's node goes to that heading in the note. */

export interface MindNode {
  text: string;
  /** The heading's anchor, for a heading; null for a list item. */
  anchor: string | null;
  children: MindNode[];
}

const HEADING = /^(#{1,6})\s+(.*?)\s*#*\s*$/;
const ITEM = /^(\s*)(?:[-*+]|\d+[.)])\s+(?:\[.\]\s+)?(.*)$/;

/** Plain text of a line of Markdown, for a node's label. */
function plain(text: string): string {
  return text
    .replace(/!?\[\[([^\]|#]*)(?:#[^\]|]*)?(?:\|([^\]]*))?\]\]/g, (_m, t: string, a?: string) => a ?? t)
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/[*_~=`]+/g, "")
    .replace(/\s\^[\w-]+$/, "")
    .trim();
}

/** The outline of a note's body (front matter already out). */
export function outline(title: string, body: string): MindNode {
  const root: MindNode = { text: title, anchor: null, children: [] };
  // The heading path: stack[i] is the node at heading level i+1's place.
  const stack: { level: number; node: MindNode }[] = [{ level: 0, node: root }];
  let items: { indent: number; node: MindNode }[] = [];
  let fence = false;
  const used = new Map<string, number>();
  for (const line of body.split("\n")) {
    if (/^\s{0,3}(```|~~~)/.test(line)) {
      fence = !fence;
      continue;
    }
    if (fence) continue;
    const h = HEADING.exec(line);
    if (h !== null) {
      const level = h[1]?.length ?? 1;
      const text = plain(h[2] ?? "");
      const node: MindNode = { text, anchor: anchorFor(text, used), children: [] };
      while (stack.length > 1 && (stack[stack.length - 1]?.level ?? 0) >= level) stack.pop();
      stack[stack.length - 1]?.node.children.push(node);
      stack.push({ level, node });
      items = [];
      continue;
    }
    const m = ITEM.exec(line);
    if (m === null) {
      if (line.trim() !== "" && !/^\s/.test(line)) items = [];
      continue;
    }
    const indent = (m[1] ?? "").replace(/\t/g, "    ").length;
    const node: MindNode = { text: plain(m[2] ?? ""), anchor: null, children: [] };
    if (node.text === "") continue;
    while (items.length > 0 && (items[items.length - 1]?.indent ?? 0) >= indent) items.pop();
    // Three levels of list under a heading at most: deeper is detail.
    if (items.length >= 3) continue;
    const parent = items[items.length - 1]?.node ?? stack[stack.length - 1]?.node ?? root;
    parent.children.push(node);
    items.push({ indent, node });
  }
  // A note that opens with its own name as a heading would show the name
  // twice in a row: the heading is the middle then.
  const only = root.children[0];
  if (root.children.length === 1 && only !== undefined && only.anchor !== null && only.text.toLowerCase() === title.trim().toLowerCase()) {
    return only;
  }
  return root;
}

interface Placed {
  node: MindNode;
  x: number;
  y: number;
  w: number;
  depth: number;
  branch: number;
  parent: Placed | null;
}

const ROW = 30;
const GAP = 46;
const MAX_LABEL = 42;
/** Twigs (list items) are cut shorter: they carry detail, not structure. */
const MAX_TWIG = 30;

function label(text: string, depth = 0): string {
  const max = depth >= 2 ? MAX_TWIG : MAX_LABEL;
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

/** Widths from a canvas, which measures with the page's own font. */
function measurer(): (text: string, depth: number) => number {
  const ctx = document.createElement("canvas").getContext("2d");
  const family = getComputedStyle(document.body).fontFamily;
  return (text, depth) => {
    if (ctx === null) return text.length * 7.5;
    ctx.font = `${depth === 0 ? 600 : depth === 1 ? 600 : 400} ${depth === 0 ? 15 : 13}px ${family}`;
    return ctx.measureText(text).width;
  };
}

function layout(root: MindNode): { placed: Placed[]; width: number; height: number } {
  const measure = measurer();
  const placed: Placed[] = [];
  const colWidth: number[] = [];
  const walkWidths = (n: MindNode, depth: number) => {
    colWidth[depth] = Math.max(colWidth[depth] ?? 0, measure(label(n.text, depth), depth) + (depth <= 1 ? 24 : 8));
    for (const c of n.children) walkWidths(c, depth + 1);
  };
  walkWidths(root, 0);
  const colX: number[] = [];
  let x = 8;
  for (let d = 0; d < colWidth.length; d++) {
    colX[d] = x;
    x += (colWidth[d] ?? 0) + GAP;
  }
  let row = 0;
  const place = (n: MindNode, depth: number, branch: number, parent: Placed | null): Placed => {
    const p: Placed = { node: n, x: colX[depth] ?? 0, y: 0, w: measure(label(n.text, depth), depth) + (depth <= 1 ? 24 : 8), depth, branch, parent };
    placed.push(p);
    if (n.children.length === 0) {
      p.y = row * ROW + ROW / 2;
      row++;
    } else {
      const kids = n.children.map((c, i) => place(c, depth + 1, depth === 0 ? i : branch, p));
      p.y = ((kids[0]?.y ?? 0) + (kids[kids.length - 1]?.y ?? 0)) / 2;
    }
    return p;
  };
  place(root, 0, 0, null);
  return { placed, width: x - GAP + 8, height: Math.max(1, row) * ROW + 8 };
}

export function MindMap({ root, onHeading, label: aria }: { root: MindNode; onHeading: (anchor: string) => void; label: string }) {
  const { placed, width, height } = React.useMemo(() => layout(root), [root]);
  return (
    <div className="overflow-auto rounded-card border border-hair bg-surface p-3" data-mindmap>
      <svg role="img" aria-label={aria} width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="mindmap">
        {placed.map((p, i) => {
          if (p.parent === null) return null;
          const a = p.parent;
          const x0 = a.x + a.w;
          const x1 = p.x;
          const mid = (x0 + x1) / 2;
          return <path key={`l${i}`} className={`mm-link mm-b${p.branch % 6}`} d={`M${x0},${a.y} C${mid},${a.y} ${mid},${p.y} ${x1},${p.y}`} />;
        })}
        {placed.map((p, i) => {
          const heading = p.node.anchor;
          const content = (
            <>
              {p.depth <= 1 && <rect className={p.depth === 0 ? "mm-root" : `mm-pill mm-b${p.branch % 6}`} x={p.x} y={p.y - 12} width={p.w} height={24} rx={12} />}
              {p.depth > 1 && <line className={`mm-under mm-b${p.branch % 6}`} x1={p.x} x2={p.x + p.w} y1={p.y + 9} y2={p.y + 9} />}
              <text className={p.depth === 0 ? "mm-text mm-text-root" : p.depth === 1 ? "mm-text mm-text-top" : "mm-text"} x={p.x + (p.depth <= 1 ? 12 : 2)} y={p.y + 4.5}>
                {label(p.node.text, p.depth)}
              </text>
              {label(p.node.text, p.depth) !== p.node.text && <title>{p.node.text}</title>}
            </>
          );
          return heading !== null ? (
            <g
              key={`n${i}`}
              role="link"
              tabIndex={0}
              className="mm-node mm-link-node"
              aria-label={p.node.text}
              onClick={() => onHeading(heading)}
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onHeading(heading);
                }
              }}
            >
              {content}
            </g>
          ) : (
            <g key={`n${i}`} className="mm-node">
              {content}
            </g>
          );
        })}
      </svg>
    </div>
  );
}
