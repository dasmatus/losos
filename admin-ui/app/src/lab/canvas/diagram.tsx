/* The diagram: the canvas view of LosOS Diagrams. Cards (Markdown, shapes,
 * links, files, groups, devices, machines) and the arrows between them, as
 * the core lays them out (admin-ui/lab/core/src/canvas/), drawn as SVG with
 * each card's words in a foreignObject.
 *
 * It works like Obsidian's canvas: a drag on empty canvas selects with a
 * rectangle, the wheel pans and Ctrl or ⌘ with the wheel (or a pinch) zooms,
 * Space or the hand pans, a double click on empty canvas adds a card, a
 * card's side handles draw an arrow (dropped on nothing it makes a card
 * there), a double click on a card types into it. Between two devices'
 * cards an arrow is a cable, so the network simulator is in the same
 * drawing. Keys: Delete, Ctrl+Z and Ctrl+Shift+Z or Ctrl+Y, Ctrl+A, Ctrl+C,
 * Ctrl+X, Ctrl+V, Ctrl+D, Ctrl+G, the arrows to nudge, Enter to type,
 * Shift+1 to fit and Shift+2 to show the selection.
 *
 * Under style-src 'self' nothing here writes a style attribute: positions
 * are SVG attributes, colours are classes in lab.css or custom properties
 * set through React's style prop (CSSOM). */

import * as React from "react";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import type { CanvasEdge, CanvasNode, Side } from "../core";
import { useEngine } from "../engine-hook";
import { IconGlyph } from "../icons";
import { fitCanvas, Scene, zoomCam } from "../scene";
import { type CanvasSel, type LabStore, NO_CSEL, store, useLab } from "../store";
import { colorValue } from "./colors";
import { Markdown } from "./md-view";
import { firstLine } from "./markdown";
import { machineOf, replicaTone } from "./machines";
import { canvasOps } from "./ops-hook";
import { centred, dash, head, outline, textBox } from "./shapes";

const SIDES: Side[] = ["top", "right", "bottom", "left"];
const MIN_K = 0.1;
const MAX_K = 4;

function sidePoint(n: CanvasNode, side: Side): { x: number; y: number } {
  switch (side) {
    case "top":
      return { x: n.x + n.width / 2, y: n.y };
    case "bottom":
      return { x: n.x + n.width / 2, y: n.y + n.height };
    case "left":
      return { x: n.x, y: n.y + n.height / 2 };
    default:
      return { x: n.x + n.width, y: n.y + n.height / 2 };
  }
}

/** The side of `n` nearest a point. */
function nearestSide(n: CanvasNode, x: number, y: number): Side {
  let best: Side = "top";
  let d = Infinity;
  for (const s of SIDES) {
    const p = sidePoint(n, s);
    const e = Math.hypot(p.x - x, p.y - y);
    if (e < d) {
      d = e;
      best = s;
    }
  }
  return best;
}

function intersects(n: CanvasNode, r: { x1: number; y1: number; x2: number; y2: number }): boolean {
  return n.x < r.x2 && n.x + n.width > r.x1 && n.y < r.y2 && n.y + n.height > r.y1;
}

type Gesture =
  | { kind: "pan"; sx: number; sy: number; ox: number; oy: number; moved: boolean }
  | { kind: "marquee"; x0: number; y0: number; add: CanvasSel; moved: boolean }
  | { kind: "move"; ids: string[]; x0: number; y0: number; done: { x: number; y: number }; g: number; moved: boolean; pick: string; shift: boolean }
  | { kind: "resize"; id: string; corner: string; x0: number; y0: number; rect: { x: number; y: number; width: number; height: number }; g: number }
  | { kind: "connect"; from: string; side: Side; moved: boolean };

// ── the cards ────────────────────────────────────────────────────────────

function DeviceBody({ n, scene, running }: { n: CanvasNode; scene: Scene; running: boolean }) {
  const d = scene.dev(n.dev);
  if (!d) return null;
  const T = scene.catalog.types[d.type];
  const ip = scene.ipOf(d);
  const badge = scene.badge(d);
  return (
    <div className="dg-device" data-off={!d.power || undefined}>
      <svg viewBox="0 0 64 64" className="dg-device-icon" aria-hidden="true">
        <IconGlyph type={d.type} />
      </svg>
      <div className="dg-device-text">
        <b>{d.name}</b>
        <span>{T?.short ?? d.type}</span>
        <span className="dg-mono">{!d.power ? "off" : (ip ?? (T?.endpoint ? "no address" : ""))}</span>
      </div>
      {badge && <i className={cn("dg-badge", "tone-" + badge.tone)} title={badge.title} />}
      {running && <i className="dg-vm">VM</i>}
    </div>
  );
}

function MachineBody({ n, s }: { n: CanvasNode; s: LabStore }) {
  const t = useT();
  const m = machineOf(n.losos);
  if (!m) return null;
  const info = s.machines.state === "ready" && m.order ? s.machines.machines.find((x) => x.order === m.order) : undefined;
  const image =
    info?.imageName ?? (s.machines.state === "ready" && m.image ? (s.machines.images.find((i) => i.id === m.image)?.name ?? m.image) : m.image);
  const line = !m.order
    ? t("dg.machine.planned")
    : info
      ? info.state === "pending"
        ? t("dg.machine.pending")
        : info.state === "lapsed"
          ? t("dg.machine.lapsed")
          : info.replicas.length === 0
            ? t("dg.machine.preparing")
            : null
      : s.machines.state === "ready"
        ? t("dg.machine.gone")
        : s.machines.state === "loading"
          ? "…"
          : t("dg.machine.unknown");
  return (
    <div className="dg-machine">
      <div className="dg-machine-head">
        <svg viewBox="0 0 24 24" className="dg-machine-icon" aria-hidden="true">
          <rect x="3" y="4" width="18" height="12" rx="2" />
          <path d="M8 20h8M12 16v4" />
        </svg>
        <b>{info?.name ?? m.name ?? n.text}</b>
      </div>
      {image && <span className="dg-machine-image">{image}</span>}
      {line && <span className="dg-machine-line">{line}</span>}
      {info && info.replicas.length > 0 && (
        <ul className="dg-replicas" aria-label={t("dg.machine.replicas")}>
          {info.replicas.map((r) => (
            <li key={r.name} title={`${r.name}: ${r.status}`}>
              <i className={"tone-" + replicaTone(r.status)} />
              {r.name}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function LinkBody({ n }: { n: CanvasNode }) {
  let host = n.url ?? "";
  try {
    host = new URL(n.url ?? "").host || host;
  } catch {
    /* not an address yet */
  }
  const ok = /^https?:\/\//.test(n.url ?? "");
  return (
    <div className="dg-link">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1" />
      </svg>
      <div>
        <b>{host}</b>
        {ok ? (
          <a href={n.url} target="_blank" rel="noreferrer noopener" data-md-link>
            {n.url}
          </a>
        ) : (
          <span>{n.url}</span>
        )}
      </div>
    </div>
  );
}

const IMAGE = /\.(png|jpe?g|gif|webp|svg|avif|bmp)$/i;

function FileBody({ n, cloud }: { n: CanvasNode; cloud: boolean }) {
  const file = n.file ?? "";
  const name = file.slice(file.lastIndexOf("/") + 1);
  if (cloud && IMAGE.test(file)) {
    return <img className="dg-file-img" src={`/cloud/api/files/content?path=${encodeURIComponent(file)}`} alt={name} draggable={false} />;
  }
  return (
    <div className="dg-file">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M6 3h8l4 4v14H6zM14 3v4h4" />
      </svg>
      <div>
        <b>{name || file}</b>
        <span>{file}</span>
      </div>
    </div>
  );
}

function Editor({ n, onDone }: { n: CanvasNode; onDone: (text: string) => void }) {
  const [text, setText] = React.useState(n.type === "link" ? (n.url ?? "") : (n.text ?? ""));
  const done = React.useRef(false);
  const finish = () => {
    if (done.current) return;
    done.current = true;
    onDone(text);
  };
  const t = useT();
  return n.type === "link" ? (
    <input
      className="dg-editor dg-editor-line"
      autoFocus
      value={text}
      aria-label={t("dg.edit.url")}
      onChange={(e) => setText(e.target.value)}
      onBlur={finish}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter" || e.key === "Escape") finish();
      }}
    />
  ) : (
    <textarea
      className="dg-editor"
      autoFocus
      value={text}
      aria-label={t("dg.edit.text")}
      onFocus={(e) => e.currentTarget.setSelectionRange(text.length, text.length)}
      onChange={(e) => setText(e.target.value)}
      onBlur={finish}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape" || (e.key === "Enter" && (e.ctrlKey || e.metaKey))) finish();
      }}
    />
  );
}

function Card({
  n,
  s,
  scene,
  selected,
  editing,
  running,
  cloud,
}: {
  n: CanvasNode;
  s: LabStore;
  scene: Scene;
  selected: boolean;
  editing: boolean;
  running: boolean;
  cloud: boolean;
}) {
  const t = useT();
  const shape = n.type === "text" ? n.styleAttributes?.["shape"] : undefined;
  const o = outline(shape, n.width, n.height);
  const tb = textBox(shape, n.width, n.height);
  const color = colorValue(n.color);
  const border = n.styleAttributes?.["border"];
  const align = n.styleAttributes?.["textAlign"];
  const machine = machineOf(n.losos);
  const label =
    n.dev !== undefined
      ? `${scene.name(n.dev)}, ${scene.catalog.types[scene.dev(n.dev)?.type ?? ""]?.label ?? ""}`
      : n.type === "text"
        ? firstLine(n.text ?? "") || t("dg.card.empty")
        : n.type === "link"
          ? (n.url ?? "")
          : (n.file ?? "");
  const style = color ? ({ "--dg-card": color } as React.CSSProperties) : undefined;
  return (
    <g
      className={cn("dg-node", "kind-" + n.type, color && "colored", selected && "sel", border && "border-" + border, n.dev && "device", machine && "machine")}
      data-node={n.id}
      transform={`translate(${n.x} ${n.y})`}
      style={style}
      role="button"
      tabIndex={-1}
      aria-label={label}
      aria-pressed={selected}
    >
      <path d={o.d} className="dg-shape" />
      {o.inner && <path d={o.inner} className="dg-shape-inner" />}
      <foreignObject x={tb.x} y={tb.y} width={Math.max(1, tb.w)} height={Math.max(1, tb.h)}>
        <div
          className={cn("dg-body", centred(shape) && "centred", align && "align-" + align, n.dev && "device")}
          data-node-body={n.id}
        >
          {editing ? (
            <Editor
              n={n}
              onDone={(text) => {
                const ops = canvasOps();
                if (n.type === "link") ops.update(n.id, { url: text.trim() });
                else if (text !== (n.text ?? "")) ops.update(n.id, { text });
                ops.stopEditing();
              }}
            />
          ) : n.dev ? (
            <DeviceBody n={n} scene={scene} running={running} />
          ) : machine ? (
            <MachineBody n={n} s={s} />
          ) : n.type === "link" ? (
            <LinkBody n={n} />
          ) : n.type === "file" ? (
            <FileBody n={n} cloud={cloud} />
          ) : (
            <Markdown text={n.text ?? ""} />
          )}
        </div>
      </foreignObject>
    </g>
  );
}

function Group({ n, selected, editing }: { n: CanvasNode; selected: boolean; editing: boolean }) {
  const t = useT();
  const color = colorValue(n.color);
  const style = color ? ({ "--dg-card": color } as React.CSSProperties) : undefined;
  const [label, setLabel] = React.useState(n.label ?? "");
  React.useEffect(() => setLabel(n.label ?? ""), [n.label, editing]);
  return (
    <g
      className={cn("dg-group", color && "colored", selected && "sel")}
      data-node={n.id}
      transform={`translate(${n.x} ${n.y})`}
      style={style}
      role="group"
      aria-label={n.label || t("dg.card.group")}
    >
      <rect width={n.width} height={n.height} rx={12} className="dg-group-box" />
      {editing ? (
        <foreignObject x={0} y={-34} width={Math.max(160, n.width)} height={30}>
          <input
            className="dg-editor dg-editor-line"
            autoFocus
            aria-label={t("dg.edit.groupLabel")}
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            onBlur={() => {
              canvasOps().update(n.id, { label });
              canvasOps().stopEditing();
            }}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter" || e.key === "Escape") (e.target as HTMLInputElement).blur();
            }}
          />
        </foreignObject>
      ) : (
        n.label && (
          <text x={4} y={-10} className="dg-group-label" data-group-label={n.id}>
            {n.label}
          </text>
        )
      )}
    </g>
  );
}

// ── the arrows ───────────────────────────────────────────────────────────

function Arrow({ e, scene, selected, editing }: { e: CanvasEdge; scene: Scene; selected: boolean; editing: boolean }) {
  const t = useT();
  const color = colorValue(e.color);
  const style = color ? ({ "--dg-edge": color } as React.CSSProperties) : undefined;
  const link = e.link ? scene.link(e.link) : undefined;
  const state = link ? scene.linkState(link) : null;
  const arrowStyle = e.styleAttributes?.["arrow"];
  const [label, setLabel] = React.useState(e.label ?? "");
  React.useEffect(() => setLabel(e.label ?? ""), [e.label, editing]);
  const words = e.label || (link ? `${link.a.port} – ${link.b.port}` : "");
  return (
    <g
      className={cn("dg-edge", color && "colored", selected && "sel", e.cable && "cable cable-" + e.cable, state && "state-" + state)}
      data-edge={e.id}
      style={style}
    >
      <path d={e.path} className="dg-edge-hit" />
      <path d={e.path} className="dg-edge-line" strokeDasharray={dash(e.styleAttributes?.["path"]) ?? (e.cable === "wifi" ? "2 6" : undefined)} />
      {e.heads.map((h) => {
        const hd = head(arrowStyle);
        return (
          <path
            key={h.end}
            d={hd.d}
            className={hd.fill ? "dg-head" : "dg-head open"}
            transform={`translate(${h.x} ${h.y}) rotate(${h.angle})`}
          />
        );
      })}
      {editing ? (
        <foreignObject x={e.labelAt[0] - 90} y={e.labelAt[1] - 15} width={180} height={30}>
          <input
            className="dg-editor dg-editor-line centred"
            autoFocus
            aria-label={t("dg.edit.edgeLabel")}
            value={label}
            onChange={(ev) => setLabel(ev.target.value)}
            onBlur={() => {
              canvasOps().updateEdge(e.id, { label });
              canvasOps().stopEditing();
            }}
            onKeyDown={(ev) => {
              ev.stopPropagation();
              if (ev.key === "Enter" || ev.key === "Escape") (ev.target as HTMLInputElement).blur();
            }}
          />
        </foreignObject>
      ) : (
        words && (
          <text x={e.labelAt[0]} y={e.labelAt[1]} className={cn("dg-edge-label", !e.label && "ports")} textAnchor="middle" dominantBaseline="middle">
            {words}
          </text>
        )
      )}
    </g>
  );
}

// ── the canvas ───────────────────────────────────────────────────────────

export function DiagramCanvas() {
  const s = useLab();
  useLab("cam");
  const t = useT();
  const engine = useEngine();
  const wrap = React.useRef<HTMLDivElement>(null);
  const svgRef = React.useRef<SVGSVGElement>(null);
  const gesture = React.useRef<Gesture | null>(null);
  const space = React.useRef(false);
  const pointer = React.useRef({ x: 0, y: 0, inside: false });
  const fitted = React.useRef(false);
  const [marquee, setMarquee] = React.useState<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  const [wire, setWire] = React.useState<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  const [hover, setHover] = React.useState<string | null>(null);
  const cam = s.cams.canvas;
  const { nodes, edges } = s.canvas;
  const csel = s.ui.csel;
  const editing = s.ui.editing;
  const ops = canvasOps();
  const scene = s.scene;
  const cloud = s.cloud;
  const running = React.useMemo(
    () => new Set(s.snap.world.devices.filter((d) => engine.isRunning(d.id)).map((d) => d.id)),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- engine.version says when a guest came or went
    [s.snap, engine.version],
  );

  const toWorld = React.useCallback((ev: { clientX: number; clientY: number }) => {
    const r = svgRef.current!.getBoundingClientRect();
    const c = store().cams.canvas;
    return { x: (ev.clientX - r.left - c.x) / c.k, y: (ev.clientY - r.top - c.y) / c.k };
  }, []);

  /** The middle of what is on screen, in canvas units. */
  const centre = React.useCallback(() => {
    const st = store();
    const c = st.cams.canvas;
    const p = pointer.current;
    if (p.inside) return { x: p.x, y: p.y };
    return { x: (st.viewport.width / 2 - c.x) / c.k, y: (st.viewport.height / 2 - c.y) / c.k };
  }, []);

  // The size, reported so the host can fit the camera.
  React.useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const r = el.getBoundingClientRect();
      const st = store();
      st.viewport = { width: r.width, height: r.height };
      if (!fitted.current && r.width) {
        fitted.current = true;
        st.fit();
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // The wheel pans; with Ctrl or ⌘ (and a trackpad's pinch) it zooms.
  React.useEffect(() => {
    const el = svgRef.current;
    if (!el) return;
    const onWheel = (ev: WheelEvent) => {
      if ((ev.target as Element).closest?.(".dg-editor")) return;
      ev.preventDefault();
      const st = store();
      const c = st.cams.canvas;
      if (ev.ctrlKey || ev.metaKey) {
        const r = el.getBoundingClientRect();
        st.setCam(zoomCam(c, ev.clientX - r.left, ev.clientY - r.top, Math.exp(-ev.deltaY * (ev.deltaMode ? 0.05 : 0.01)), MIN_K, MAX_K));
      } else {
        const k = ev.deltaMode ? 16 : 1;
        st.setCam({ ...c, x: c.x - (ev.shiftKey ? ev.deltaY : ev.deltaX) * k, y: c.y - (ev.shiftKey ? 0 : ev.deltaY) * k });
      }
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  // Keys and the clipboard, while nothing else has the keyboard.
  React.useEffect(() => {
    const busy = (el: EventTarget | null) =>
      !!(el as Element | null)?.closest?.("input, textarea, select, [contenteditable], .lab-term, .xterm, dialog, [role=dialog], [role=menu]");
    const onKey = (e: KeyboardEvent) => {
      if (e.key === " " && !busy(e.target)) space.current = e.type === "keydown";
      if (e.type !== "keydown" || busy(e.target)) return;
      const st = store();
      const sel = st.ui.csel;
      const o = canvasOps();
      const mod = e.ctrlKey || e.metaKey;
      const k = e.key.toLowerCase();
      const ids = sel.nodes;
      if (e.key === " ") {
        e.preventDefault();
        return;
      }
      if (mod && k === "z") {
        if (e.shiftKey) o.redo();
        else o.undo();
      } else if (mod && k === "y") o.redo();
      else if (mod && k === "a") o.selectAll();
      else if (mod && k === "d") o.duplicate();
      else if (mod && k === "g") o.group(ids);
      else if (e.key === "Delete" || e.key === "Backspace") o.remove();
      else if (e.key === "Escape") {
        if (st.ui.cardTool || st.ui.tool !== "select") o.arm(null);
        else o.clear();
      } else if (e.key === "Enter" && ids.length === 1) {
        const n = st.canvas.nodes.find((x) => x.id === ids[0]);
        if (n) o.edit("node", n.id);
      } else if (e.key.startsWith("Arrow") && ids.length > 0) {
        const step = e.shiftKey ? 40 : 10;
        const dx = e.key === "ArrowLeft" ? -step : e.key === "ArrowRight" ? step : 0;
        const dy = e.key === "ArrowUp" ? -step : e.key === "ArrowDown" ? step : 0;
        o.move(ids, dx, dy, o.gesture());
        o.moved();
      } else if (e.shiftKey && (e.code === "Digit1" || e.key === "!")) st.fit();
      else if (e.shiftKey && (e.code === "Digit2" || e.key === "@")) {
        const ns = st.canvas.nodes.filter((n) => ids.includes(n.id));
        if (ns.length) st.setCam(fitCanvas(ns, st.viewport.width, st.viewport.height));
      } else return;
      e.preventDefault();
    };
    const onCopy = (e: ClipboardEvent) => {
      if (busy(document.activeElement) || window.getSelection()?.toString()) return;
      const st = store();
      const text = canvasOps().copy(st.ui.csel.nodes);
      if (!text) return;
      e.clipboardData?.setData("text/plain", text);
      e.preventDefault();
      if (e.type === "cut") canvasOps().remove({ nodes: st.ui.csel.nodes, edges: st.ui.csel.edges });
    };
    const onPaste = (e: ClipboardEvent) => {
      if (busy(document.activeElement)) return;
      const text = e.clipboardData?.getData("text/plain") ?? "";
      if (!text) return;
      e.preventDefault();
      const c = centre();
      canvasOps().paste(text, c.x, c.y);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKey);
    document.addEventListener("copy", onCopy);
    document.addEventListener("cut", onCopy);
    document.addEventListener("paste", onPaste);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", onKey);
      document.removeEventListener("copy", onCopy);
      document.removeEventListener("cut", onCopy);
      document.removeEventListener("paste", onPaste);
    };
  }, [centre]);

  const nodeById = React.useMemo(() => new Map(nodes.map((n) => [n.id, n])), [nodes]);
  const selN = React.useMemo(() => new Set(csel.nodes), [csel.nodes]);
  const selE = React.useMemo(() => new Set(csel.edges), [csel.edges]);

  const onDown = (ev: React.PointerEvent<SVGSVGElement>) => {
    const target = ev.target as Element;
    if (target.closest(".dg-editor")) return;
    if (ev.button === 2) return;
    svgRef.current?.focus({ preventScroll: true });
    const w = toWorld(ev);
    const u = s.ui;
    const pan = ev.button === 1 || space.current || u.hand;
    if (pan) {
      gesture.current = { kind: "pan", sx: ev.clientX, sy: ev.clientY, ox: cam.x, oy: cam.y, moved: false };
      svgRef.current?.setPointerCapture(ev.pointerId);
      return;
    }
    if (ev.button !== 0) return;
    // An armed tool puts its card, template or device down where the click is.
    if (u.cardTool) {
      if (u.cardTool.kind === "template") ops.template(u.cardTool.key, w.x, w.y);
      else ops.addCard(u.cardTool, w.x, w.y);
      return;
    }
    if (u.tool === "place" && u.placeType) {
      ops.addDevice(u.placeType, w.x, w.y, ev.shiftKey);
      return;
    }
    if (target.closest("a[data-md-link]")) return;
    const handle = target.closest("[data-handle]");
    if (handle) {
      const id = handle.getAttribute("data-handle-node") ?? "";
      const side = handle.getAttribute("data-handle") as Side;
      const n = nodeById.get(id);
      if (!n) return;
      const p = sidePoint(n, side);
      gesture.current = { kind: "connect", from: id, side, moved: false };
      setWire({ x1: p.x, y1: p.y, x2: w.x, y2: w.y });
      svgRef.current?.setPointerCapture(ev.pointerId);
      return;
    }
    const corner = target.closest("[data-resize]");
    if (corner) {
      const id = corner.getAttribute("data-resize-node") ?? "";
      const n = nodeById.get(id);
      if (!n) return;
      gesture.current = {
        kind: "resize",
        id,
        corner: corner.getAttribute("data-resize") ?? "se",
        x0: w.x,
        y0: w.y,
        rect: { x: n.x, y: n.y, width: n.width, height: n.height },
        g: ops.gesture(),
      };
      svgRef.current?.setPointerCapture(ev.pointerId);
      return;
    }
    const nodeEl = target.closest("[data-node]");
    const edgeEl = target.closest("[data-edge]");
    if (nodeEl) {
      const id = nodeEl.getAttribute("data-node") ?? "";
      // A group is picked by its label or its edge, not the empty inside,
      // so a drag in a group still draws a rectangle (as in Obsidian).
      const n = nodeById.get(id);
      if (n?.type === "group" && !target.closest("[data-group-label]") && !selN.has(id)) {
        const inner = 14 / cam.k;
        const inside = w.x > n.x + inner && w.x < n.x + n.width - inner && w.y > n.y + inner && w.y < n.y + n.height - inner;
        if (inside) {
          startMarquee(ev, w);
          return;
        }
      }
      let ids = csel.nodes;
      if (ev.shiftKey || ev.metaKey || ev.ctrlKey) {
        const next = selN.has(id) ? csel.nodes.filter((x) => x !== id) : [...csel.nodes, id];
        ops.select({ nodes: next, edges: csel.edges });
        ids = next;
        if (!next.includes(id)) return;
      } else if (!selN.has(id)) {
        ops.select({ nodes: [id], edges: [] });
        ids = [id];
      }
      gesture.current = { kind: "move", ids, x0: w.x, y0: w.y, done: { x: 0, y: 0 }, g: ops.gesture(), moved: false, pick: id, shift: ev.shiftKey };
      svgRef.current?.setPointerCapture(ev.pointerId);
      return;
    }
    if (edgeEl) {
      const id = edgeEl.getAttribute("data-edge") ?? "";
      if (ev.shiftKey || ev.metaKey || ev.ctrlKey) {
        ops.select({ nodes: csel.nodes, edges: selE.has(id) ? csel.edges.filter((x) => x !== id) : [...csel.edges, id] });
      } else ops.select({ nodes: [], edges: [id] });
      if (s.phone) s.setUi({ sheet: true });
      return;
    }
    if (ev.pointerType === "touch") {
      gesture.current = { kind: "pan", sx: ev.clientX, sy: ev.clientY, ox: cam.x, oy: cam.y, moved: false };
      svgRef.current?.setPointerCapture(ev.pointerId);
      return;
    }
    startMarquee(ev, w);
  };

  const startMarquee = (ev: React.PointerEvent<SVGSVGElement>, w: { x: number; y: number }) => {
    const add = ev.shiftKey || ev.metaKey || ev.ctrlKey ? csel : NO_CSEL;
    gesture.current = { kind: "marquee", x0: w.x, y0: w.y, add, moved: false };
    svgRef.current?.setPointerCapture(ev.pointerId);
  };

  const onMove = (ev: React.PointerEvent<SVGSVGElement>) => {
    const w = toWorld(ev);
    pointer.current = { x: w.x, y: w.y, inside: true };
    const gs = gesture.current;
    if (!gs) return;
    switch (gs.kind) {
      case "pan": {
        const dx = ev.clientX - gs.sx;
        const dy = ev.clientY - gs.sy;
        if (Math.abs(dx) + Math.abs(dy) > 2) gs.moved = true;
        s.setCam({ ...cam, x: gs.ox + dx, y: gs.oy + dy });
        return;
      }
      case "marquee": {
        if (Math.abs(w.x - gs.x0) + Math.abs(w.y - gs.y0) > 3 / cam.k) gs.moved = true;
        if (!gs.moved) return;
        const r = { x1: Math.min(gs.x0, w.x), y1: Math.min(gs.y0, w.y), x2: Math.max(gs.x0, w.x), y2: Math.max(gs.y0, w.y) };
        setMarquee(r);
        const hit = nodes.filter((n) => intersects(n, r) && !(n.type === "group" && n.x < r.x1 && n.y < r.y1 && n.x + n.width > r.x2 && n.y + n.height > r.y2));
        const ids = [...new Set([...gs.add.nodes, ...hit.map((n) => n.id)])];
        const es = edges.filter((e) => ids.includes(e.fromNode) && ids.includes(e.toNode)).map((e) => e.id);
        ops.select({ nodes: ids, edges: [...new Set([...gs.add.edges, ...es])] });
        return;
      }
      case "move": {
        const tx = Math.round(w.x - gs.x0);
        const ty = Math.round(w.y - gs.y0);
        if (!gs.moved && Math.abs(tx) + Math.abs(ty) < 3 / cam.k) return;
        gs.moved = true;
        ops.move(gs.ids, tx - gs.done.x, ty - gs.done.y, gs.g);
        gs.done = { x: tx, y: ty };
        return;
      }
      case "resize": {
        const dx = w.x - gs.x0;
        const dy = w.y - gs.y0;
        const r = { ...gs.rect };
        if (gs.corner.includes("e")) r.width = Math.max(40, gs.rect.width + dx);
        if (gs.corner.includes("s")) r.height = Math.max(30, gs.rect.height + dy);
        if (gs.corner.includes("w")) {
          r.width = Math.max(40, gs.rect.width - dx);
          r.x = gs.rect.x + gs.rect.width - r.width;
        }
        if (gs.corner.includes("n")) {
          r.height = Math.max(30, gs.rect.height - dy);
          r.y = gs.rect.y + gs.rect.height - r.height;
        }
        ops.resize(gs.id, { x: Math.round(r.x), y: Math.round(r.y), width: Math.round(r.width), height: Math.round(r.height) }, gs.g);
        return;
      }
      case "connect": {
        gs.moved = true;
        setWire((p) => (p ? { ...p, x2: w.x, y2: w.y } : p));
        return;
      }
    }
  };

  const onUp = (ev: React.PointerEvent<SVGSVGElement>) => {
    const gs = gesture.current;
    gesture.current = null;
    if (!gs) return;
    switch (gs.kind) {
      case "pan":
        if (!gs.moved && !s.ui.hand && !space.current && ev.button !== 1) ops.clear();
        return;
      case "marquee":
        setMarquee(null);
        if (!gs.moved && gs.add === NO_CSEL) ops.clear();
        return;
      case "move":
        if (gs.moved) ops.moved();
        else if (!gs.shift && !ev.metaKey && !ev.ctrlKey && csel.nodes.length > 1) ops.select({ nodes: [gs.pick], edges: [] });
        if (!gs.moved && s.phone) s.setUi({ sheet: true });
        return;
      case "resize":
        ops.moved();
        return;
      case "connect": {
        setWire(null);
        if (!gs.moved) return;
        const w = toWorld(ev);
        const el = document.elementFromPoint(ev.clientX, ev.clientY)?.closest("[data-node]");
        const to = el?.getAttribute("data-node");
        const tn = to ? nodeById.get(to) : undefined;
        if (tn && tn.id !== gs.from) ops.connect(gs.from, gs.side, tn.id, nearestSide(tn, w.x, w.y));
        else if (!tn) ops.connectToNew(gs.from, gs.side, w.x, w.y);
        return;
      }
    }
  };

  const onDouble = (ev: React.MouseEvent<SVGSVGElement>) => {
    const target = ev.target as Element;
    if (target.closest(".dg-editor, a[data-md-link]")) return;
    const w = toWorld(ev);
    const label = target.closest("[data-group-label]");
    if (label) {
      ops.edit("node", label.getAttribute("data-group-label") ?? "");
      return;
    }
    const nodeEl = target.closest("[data-node]");
    const edgeEl = target.closest("[data-edge]");
    if (nodeEl) {
      const n = nodeById.get(nodeEl.getAttribute("data-node") ?? "");
      if (!n) return;
      if (n.dev) {
        const d = s.dev(n.dev);
        if (d) s.setUi({ tab: d.type === "laptop" ? "desktop" : s.consoleable(d) ? "console" : "status", ...(s.phone ? { sheet: true } : {}) });
        return;
      }
      if (n.type === "group") {
        const inner = 14 / cam.k;
        if (w.x > n.x + inner && w.x < n.x + n.width - inner && w.y > n.y + inner && w.y < n.y + n.height - inner) {
          ops.addCard({ kind: "text" }, w.x, w.y);
          return;
        }
      }
      if (!machineOf(n.losos)) ops.edit("node", n.id);
      return;
    }
    if (edgeEl) {
      ops.edit("edge", edgeEl.getAttribute("data-edge") ?? "");
      return;
    }
    ops.addCard({ kind: "text" }, w.x, w.y);
  };

  const onOver = (ev: React.PointerEvent<SVGSVGElement>) => {
    if (gesture.current) return;
    const el = (ev.target as Element).closest("[data-node],[data-handle],[data-resize]");
    const id = el?.getAttribute("data-node") ?? el?.getAttribute("data-handle-node") ?? el?.getAttribute("data-resize-node") ?? null;
    if (id !== hover) setHover(id);
  };

  const groups = nodes.filter((n) => n.type === "group");
  const cards = nodes.filter((n) => n.type !== "group");
  const single = csel.nodes.length === 1 && csel.edges.length === 0 ? nodeById.get(csel.nodes[0]!) : undefined;
  const handleFor = [...new Set([hover, single?.id].filter((x): x is string => !!x))]
    .map((id) => nodeById.get(id))
    .filter((n): n is CanvasNode => !!n && !(editing?.id === n.id));
  const k = cam.k;
  const hs = 6 / k;
  const empty = nodes.length === 0;
  const tool = s.ui.cardTool ? "place" : s.ui.tool === "place" ? "place" : s.ui.hand ? "hand" : "select";

  return (
    <div ref={wrap} className="relative h-full min-h-0 overflow-hidden" data-testid="diagram-wrap">
      <svg
        ref={svgRef}
        className={cn("dg-canvas", "tool-" + tool)}
        aria-label={t("dg.canvas.label")}
        tabIndex={0}
        data-testid="diagram"
        onPointerDown={onDown}
        onPointerMove={onMove}
        onPointerUp={onUp}
        onPointerCancel={onUp}
        onPointerOver={onOver}
        onPointerLeave={() => {
          pointer.current.inside = false;
          if (!gesture.current) setHover(null);
        }}
        onDoubleClick={onDouble}
        onContextMenu={(e) => e.preventDefault()}
      >
        <defs>
          <pattern id="dg-dots" width={20} height={20} patternUnits="userSpaceOnUse">
            <circle cx={1} cy={1} r={1} className="dg-dot" />
          </pattern>
        </defs>
        <g transform={`translate(${cam.x} ${cam.y}) scale(${k})`}>
          {k > 0.35 && <rect x={-20000} y={-20000} width={40000} height={40000} fill="url(#dg-dots)" className="dg-bg" />}
          <g>
            {groups.map((n) => (
              <Group key={n.id} n={n} selected={selN.has(n.id)} editing={editing?.kind === "node" && editing.id === n.id} />
            ))}
          </g>
          <g>
            {edges.map((e) => (
              <Arrow key={e.id} e={e} scene={scene} selected={selE.has(e.id)} editing={editing?.kind === "edge" && editing.id === e.id} />
            ))}
          </g>
          <g>
            {cards.map((n) => (
              <Card
                key={n.id}
                n={n}
                s={s}
                scene={scene}
                selected={selN.has(n.id)}
                editing={editing?.kind === "node" && editing.id === n.id}
                running={!!n.dev && running.has(n.dev)}
                cloud={cloud}
              />
            ))}
          </g>
          <g>
            {nodes
              .filter((n) => selN.has(n.id))
              .map((n) => (
                <rect key={n.id} x={n.x - 3 / k} y={n.y - 3 / k} width={n.width + 6 / k} height={n.height + 6 / k} rx={12} className="dg-sel-box" strokeWidth={2 / k} />
              ))}
            {single && !editing && (
              <g>
                {(["nw", "ne", "sw", "se"] as const).map((c) => (
                  <rect
                    key={c}
                    data-resize={c}
                    data-resize-node={single.id}
                    className={"dg-resize " + c}
                    x={(c.includes("w") ? single.x : single.x + single.width) - hs}
                    y={(c.includes("n") ? single.y : single.y + single.height) - hs}
                    width={hs * 2}
                    height={hs * 2}
                    rx={2 / k}
                    strokeWidth={1.5 / k}
                  />
                ))}
              </g>
            )}
            {!gesture.current &&
              handleFor.map((n) =>
                SIDES.map((side) => {
                  const p = sidePoint(n, side);
                  return (
                    <circle
                      key={n.id + side}
                      data-handle={side}
                      data-handle-node={n.id}
                      cx={p.x}
                      cy={p.y}
                      r={hs}
                      className="dg-handle"
                      strokeWidth={1.5 / k}
                    />
                  );
                }),
              )}
            {wire && <path d={`M${wire.x1} ${wire.y1}L${wire.x2} ${wire.y2}`} className="dg-wire" strokeWidth={2 / k} />}
            {marquee && (
              <rect x={marquee.x1} y={marquee.y1} width={marquee.x2 - marquee.x1} height={marquee.y2 - marquee.y1} className="dg-marquee" strokeWidth={1 / k} />
            )}
          </g>
        </g>
      </svg>
      {empty && (
        <div className="pointer-events-none absolute inset-0 grid place-items-center p-6 text-center text-[13.5px] text-muted" data-testid="diagram-empty">
          <p className="max-w-[360px]">{t("dg.canvas.empty")}</p>
        </div>
      )}
    </div>
  );
}
