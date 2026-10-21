import * as React from "react";
import { forceCenter, forceCollide, forceLink, forceManyBody, forceSimulation, forceX, forceY, type SimulationLinkDatum, type SimulationNodeDatum } from "d3-force";
import { cn } from "@/lib/utils";
import type { GraphEdge, GraphNode } from "./vault";

/* The graph of a vault, as Obsidian draws it: every note a dot, every link
 * a line, laid out by forces (d3-force) and drawn on a canvas, so there is
 * no element per dot and no style written anywhere. Bigger dots are linked
 * more. Hover shows a note's neighbours and their names; a click opens it;
 * dragging moves a dot or, on the empty canvas, the view; the wheel and a
 * pinch zoom. A list of the same notes follows for keyboards and screen
 * readers. Colours are the palette's, read from the page, and follow a
 * theme change. */

interface Node extends SimulationNodeDatum, GraphNode {
  r: number;
}
type Link = SimulationLinkDatum<Node>;

export interface GraphViewProps {
  nodes: readonly GraphNode[];
  edges: readonly GraphEdge[];
  /** The open note's node id, drawn in the accent. */
  current?: string;
  onOpen: (node: GraphNode) => void;
  label: string;
  className?: string;
  /** Labels on every dot rather than only when close enough. */
  labels?: boolean;
}

interface Palette {
  note: string;
  canvas: string;
  tag: string;
  file: string;
  missing: string;
  current: string;
  line: string;
  lineHot: string;
  text: string;
  halo: string;
}

function readPalette(el: Element): Palette {
  const css = getComputedStyle(el);
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  return {
    note: v("--muted", "#5c6a77"),
    canvas: v("--person-teal", "#0e7c7b"),
    tag: v("--person-violet", "#6a4fb3"),
    file: v("--person-amber", "#9a6410"),
    missing: v("--faint", "#8c98a4"),
    current: v("--accent", "#0e6e7d"),
    line: v("--line", "#dce1e6"),
    lineHot: v("--accent", "#0e6e7d"),
    text: v("--ink", "#18212a"),
    halo: v("--surface", "#ffffff"),
  };
}

function usePalette(ref: React.RefObject<Element | null>): Palette | null {
  const [palette, setPalette] = React.useState<Palette | null>(null);
  React.useEffect(() => {
    const el = ref.current;
    if (el === null) return;
    const read = () => setPalette(readPalette(el));
    read();
    const observer = new MutationObserver(read);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "class"] });
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", read);
    return () => {
      observer.disconnect();
      media.removeEventListener("change", read);
    };
  }, [ref]);
  return palette;
}

const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

export function GraphView({ nodes, edges, current, onOpen, label, className, labels = false }: GraphViewProps) {
  const wrap = React.useRef<HTMLDivElement>(null);
  const canvasRef = React.useRef<HTMLCanvasElement>(null);
  const palette = usePalette(wrap);
  const [size, setSize] = React.useState({ w: 0, h: 0 });
  const state = React.useRef({
    nodes: [] as Node[],
    links: [] as Link[],
    view: { k: 1, x: 0, y: 0 },
    hover: null as Node | null,
    neighbours: new Set<string>(),
  });
  const draw = React.useRef<() => void>(() => {});
  const sim = React.useRef<ReturnType<typeof forceSimulation<Node>> | null>(null);
  const openRef = React.useRef(onOpen);
  openRef.current = onOpen;

  React.useEffect(() => {
    const el = wrap.current;
    if (el === null) return;
    const ro = new ResizeObserver(([entry]) => {
      if (entry === undefined) return;
      setSize({ w: Math.round(entry.contentRect.width), h: Math.round(entry.contentRect.height) });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // A new set of dots: lay them out, keeping where known ones were.
  React.useEffect(() => {
    const old = new Map(state.current.nodes.map((n) => [n.id, n]));
    const ns: Node[] = nodes.map((n, i) => {
      const was = old.get(n.id);
      // Phyllotaxis to start, so the same vault gives the same picture.
      const a = i * 2.39996;
      const d = 12 * Math.sqrt(i + 0.5);
      return {
        ...n,
        r: 3.5 + Math.min(9, Math.sqrt(n.degree) * 2),
        x: was?.x ?? d * Math.cos(a),
        y: was?.y ?? d * Math.sin(a),
      };
    });
    const ids = new Set(ns.map((n) => n.id));
    const ls: Link[] = edges.filter((e) => ids.has(e.source) && ids.has(e.target)).map((e) => ({ source: e.source, target: e.target }));
    state.current.nodes = ns;
    state.current.links = ls;
    state.current.hover = null;
    sim.current?.stop();
    const s = forceSimulation<Node>(ns)
      .force(
        "link",
        forceLink<Node, Link>(ls)
          .id((n) => n.id)
          .distance(48)
          .strength(0.6),
      )
      .force("charge", forceManyBody<Node>().strength(-90).distanceMax(400))
      .force("center", forceCenter(0, 0))
      .force("x", forceX<Node>(0).strength(0.05))
      .force("y", forceY<Node>(0).strength(0.05))
      .force(
        "collide",
        forceCollide<Node>((n) => n.r + 3),
      )
      .stop();
    // Settle before the first frame: the picture does not swim in.
    for (let i = 0; i < 300; i++) s.tick();
    s.on("tick", () => draw.current());
    sim.current = s;
    fit();
    draw.current();
    return () => {
      s.stop();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodes, edges]);

  const fit = () => {
    const { nodes: ns, view } = state.current;
    const el = wrap.current;
    if (el === null || ns.length === 0) return;
    const w = el.clientWidth;
    const h = el.clientHeight;
    let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
    for (const n of ns) {
      x0 = Math.min(x0, (n.x ?? 0) - n.r);
      y0 = Math.min(y0, (n.y ?? 0) - n.r);
      x1 = Math.max(x1, (n.x ?? 0) + n.r);
      y1 = Math.max(y1, (n.y ?? 0) + n.r);
    }
    const k = Math.min(2.2, Math.max(0.2, Math.min((w - 48) / Math.max(1, x1 - x0), (h - 48) / Math.max(1, y1 - y0))));
    view.k = k;
    view.x = w / 2 - ((x0 + x1) / 2) * k;
    view.y = h / 2 - ((y0 + y1) / 2) * k;
  };

  React.useEffect(() => {
    fit();
    draw.current();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [size.w, size.h]);

  draw.current = () => {
    const canvas = canvasRef.current;
    if (canvas === null || palette === null) return;
    const dpr = window.devicePixelRatio || 1;
    const w = size.w;
    const h = size.h;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    }
    const ctx = canvas.getContext("2d");
    if (ctx === null) return;
    const { nodes: ns, links: ls, view, hover, neighbours } = state.current;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    ctx.translate(view.x, view.y);
    ctx.scale(view.k, view.k);
    const lit = hover !== null;
    ctx.lineWidth = 1 / Math.max(0.6, view.k);
    for (const l of ls) {
      const a = l.source as Node;
      const b = l.target as Node;
      const hot = lit && (a === hover || b === hover);
      ctx.strokeStyle = hot ? palette.lineHot : palette.line;
      ctx.globalAlpha = lit && !hot ? 0.35 : 1;
      ctx.beginPath();
      ctx.moveTo(a.x ?? 0, a.y ?? 0);
      ctx.lineTo(b.x ?? 0, b.y ?? 0);
      ctx.stroke();
    }
    ctx.globalAlpha = 1;
    for (const n of ns) {
      const isCurrent = n.id === current;
      const near = !lit || n === hover || neighbours.has(n.id);
      ctx.globalAlpha = near ? 1 : 0.3;
      ctx.fillStyle = isCurrent || n === hover ? palette.current : palette[n.kind];
      ctx.beginPath();
      ctx.arc(n.x ?? 0, n.y ?? 0, n.r, 0, Math.PI * 2);
      ctx.fill();
      if (isCurrent) {
        ctx.strokeStyle = palette.halo;
        ctx.lineWidth = 2 / view.k;
        ctx.stroke();
      }
    }
    // Names: close up, on a small graph, or around the hovered dot.
    const all = labels || view.k > 1.15 || ns.length <= 24;
    ctx.font = `${12 / view.k}px ${getComputedStyle(canvas).fontFamily}`;
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    for (const n of ns) {
      const near = n === hover || neighbours.has(n.id) || n.id === current;
      if (!all && !near) continue;
      if (lit && !near) continue;
      ctx.globalAlpha = 1;
      ctx.lineWidth = 3 / view.k;
      ctx.strokeStyle = palette.halo;
      const y = (n.y ?? 0) + n.r + 3 / view.k;
      ctx.strokeText(n.label, n.x ?? 0, y);
      ctx.fillStyle = palette.text;
      ctx.fillText(n.label, n.x ?? 0, y);
    }
    ctx.globalAlpha = 1;
  };

  React.useEffect(() => draw.current(), [palette, current, size]);

  const at = (event: { clientX: number; clientY: number }): Node | null => {
    const canvas = canvasRef.current;
    if (canvas === null) return null;
    const rect = canvas.getBoundingClientRect();
    const { view, nodes: ns } = state.current;
    const x = (event.clientX - rect.left - view.x) / view.k;
    const y = (event.clientY - rect.top - view.y) / view.k;
    let best: Node | null = null;
    let bestD = Infinity;
    for (const n of ns) {
      const d = Math.hypot((n.x ?? 0) - x, (n.y ?? 0) - y);
      if (d < n.r + 6 / view.k && d < bestD) {
        best = n;
        bestD = d;
      }
    }
    return best;
  };

  const setHover = (n: Node | null) => {
    const st = state.current;
    if (st.hover === n) return;
    st.hover = n;
    st.neighbours = new Set();
    if (n !== null) {
      for (const l of st.links) {
        const a = l.source as Node;
        const b = l.target as Node;
        if (a === n) st.neighbours.add(b.id);
        if (b === n) st.neighbours.add(a.id);
      }
    }
    const canvas = canvasRef.current;
    if (canvas !== null) canvas.dataset["hover"] = n?.key ?? "";
    draw.current();
  };

  const drag = React.useRef<{ node: Node | null; x: number; y: number; moved: boolean; pointers: Map<number, { x: number; y: number }>; pinch: number } | null>(null);

  const onPointerDown = (event: React.PointerEvent<HTMLCanvasElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    const d = drag.current ?? { node: null, x: event.clientX, y: event.clientY, moved: false, pointers: new Map(), pinch: 0 };
    d.pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (d.pointers.size === 1) {
      d.node = at(event);
      d.x = event.clientX;
      d.y = event.clientY;
      d.moved = false;
    }
    if (d.pointers.size === 2) {
      const [p, q] = [...d.pointers.values()] as [{ x: number; y: number }, { x: number; y: number }];
      d.pinch = Math.hypot(p.x - q.x, p.y - q.y);
      d.node = null;
    }
    drag.current = d;
  };

  const onPointerMove = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    const { view } = state.current;
    if (d === null) {
      setHover(at(event));
      return;
    }
    d.pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (d.pointers.size === 2) {
      const [p, q] = [...d.pointers.values()] as [{ x: number; y: number }, { x: number; y: number }];
      const dist = Math.hypot(p.x - q.x, p.y - q.y);
      if (d.pinch > 0) zoomAt((p.x + q.x) / 2, (p.y + q.y) / 2, dist / d.pinch);
      d.pinch = dist;
      d.moved = true;
      return;
    }
    const dx = event.clientX - d.x;
    const dy = event.clientY - d.y;
    if (Math.abs(dx) + Math.abs(dy) > 3) d.moved = true;
    if (!d.moved) return;
    if (d.node !== null) {
      d.node.fx = (d.node.x ?? 0) + dx / view.k;
      d.node.fy = (d.node.y ?? 0) + dy / view.k;
      d.node.x = d.node.fx;
      d.node.y = d.node.fy;
      if (!reducedMotion()) sim.current?.alphaTarget(0.25).restart();
      else draw.current();
    } else {
      view.x += dx;
      view.y += dy;
      draw.current();
    }
    d.x = event.clientX;
    d.y = event.clientY;
  };

  const onPointerUp = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    if (d === null) return;
    d.pointers.delete(event.pointerId);
    if (d.pointers.size > 0) return;
    if (d.node !== null) {
      if (!d.moved) openRef.current(d.node);
      d.node.fx = null;
      d.node.fy = null;
      sim.current?.alphaTarget(0);
    }
    drag.current = null;
  };

  const zoomAt = (clientX: number, clientY: number, factor: number) => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    const rect = canvas.getBoundingClientRect();
    const { view } = state.current;
    const k = Math.min(6, Math.max(0.1, view.k * factor));
    const px = clientX - rect.left;
    const py = clientY - rect.top;
    view.x = px - ((px - view.x) / view.k) * k;
    view.y = py - ((py - view.y) / view.k) * k;
    view.k = k;
    draw.current();
  };

  React.useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    // Not passive: the page must not scroll while the graph zooms.
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      zoomAt(event.clientX, event.clientY, Math.exp(-event.deltaY * 0.0015));
    };
    canvas.addEventListener("wheel", wheel, { passive: false });
    return () => canvas.removeEventListener("wheel", wheel);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div ref={wrap} className={cn("relative h-full min-h-48 w-full overflow-hidden", className)} data-graph data-nodes={nodes.length} data-edges={edges.length}>
      <canvas
        ref={canvasRef}
        role="img"
        aria-label={label}
        className="absolute inset-0 size-full touch-none cursor-grab font-sans active:cursor-grabbing"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        onPointerLeave={() => drag.current === null && setHover(null)}
      />
      <ul className="sr-only">
        {nodes.map((n) => (
          <li key={n.id}>
            <button type="button" onClick={() => onOpen(n)}>
              {n.label}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
