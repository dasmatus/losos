/* What the diagram's controls do: each one a core call (the canvas half of
 * admin-ui/lab/core), then a re-read and the UI state that follows. Like
 * actions.ts, components call these and never the core.
 *
 * The canvas selection (ui.csel) is mirrored into the simulator's (ui.sel)
 * when it is one device's card or one cable, so the inspector's device and
 * cable panes work on the canvas as they do in the other two views. */

import { getLocale, t } from "@/lib/i18n";
import { type CanvasNode, type EdgePatch, type EdgeSpec, isErr, type NodePatch, type NodeSpec, type Side } from "../core";
import type { Engine } from "../engine";
import { type CanvasSel, type CardTool, type LabStore, NO_CSEL, type Tab } from "../store";

/** The size a new card of each kind starts at. */
const SIZE: Record<string, [number, number]> = {
  text: [250, 60],
  shape: [180, 100],
  group: [400, 300],
  link: [300, 90],
  file: [260, 80],
  machine: [220, 110],
};

export class CanvasOps {
  private gestures = 0;

  constructor(
    readonly s: LabStore,
    readonly engine: Engine,
  ) {}

  /** A number for one drag; every step of it is one undo step together. */
  gesture(): number {
    this.gestures += 1;
    return this.gestures;
  }

  // ── the selection ──────────────────────────────────────────────────────
  select(csel: CanvasSel): void {
    const s = this.s;
    let sel: LabStore["ui"]["sel"] = null;
    if (csel.nodes.length === 1 && csel.edges.length === 0) {
      const dev = s.canvas.nodes.find((n) => n.id === csel.nodes[0])?.dev;
      if (dev) sel = { kind: "dev", id: dev };
    } else if (csel.edges.length === 1 && csel.nodes.length === 0) {
      const link = s.canvas.edges.find((e) => e.id === csel.edges[0])?.link;
      if (link) sel = { kind: "link", id: link };
    }
    let tab: Tab = s.ui.tab;
    if (sel?.kind === "dev") {
      const d = s.dev(sel.id);
      if (d && (tab === "desktop" ? d.type !== "laptop" : tab === "console" ? !s.consoleable(d) : false)) tab = "status";
    }
    const editing = s.ui.editing && !csel.nodes.includes(s.ui.editing.id) && !csel.edges.includes(s.ui.editing.id) ? null : s.ui.editing;
    s.setUi({ csel, sel, tab, editing });
  }

  selectAll(): void {
    this.select({ nodes: this.s.canvas.nodes.map((n) => n.id), edges: [] });
  }

  clear(): void {
    this.s.setUi({ csel: NO_CSEL, sel: null, editing: null, cardTool: null });
  }

  arm(tool: CardTool | null): void {
    // One tool at a time: a card to put down, or a device or a cable kind.
    this.s.setUi({ cardTool: tool, tool: "select", placeType: null, linkKind: null, connectFrom: null });
  }

  edit(kind: "node" | "edge", id: string): void {
    const s = this.s;
    if (kind === "node") {
      const n = s.canvas.nodes.find((x) => x.id === id);
      if (!n || n.dev || n.type === "file") return;
    }
    s.setUi({ editing: { kind, id } });
  }

  stopEditing(): void {
    if (this.s.ui.editing) this.s.setUi({ editing: null });
  }

  // ── cards ──────────────────────────────────────────────────────────────
  /** Puts down what the tray armed (or a text card) centred on x, y. */
  addCard(tool: Exclude<CardTool, { kind: "template" }>, x: number, y: number): CanvasNode | null {
    const s = this.s;
    let spec: NodeSpec;
    const at = (kind: string): { x: number; y: number; width: number; height: number } => {
      const [w, h] = SIZE[kind] ?? SIZE["text"]!;
      return { x: Math.round(x - w / 2), y: Math.round(y - h / 2), width: w, height: h };
    };
    switch (tool.kind) {
      case "text":
        spec = tool.shape
          ? { type: "text", ...at("shape"), text: "", styleAttributes: { shape: tool.shape } }
          : { type: "text", ...at("text"), text: "" };
        if (tool.shape === "note") spec.color = "3";
        break;
      case "group":
        spec = { type: "group", ...at("group"), label: t("dg.card.groupLabel") };
        break;
      case "link":
        spec = { type: "link", ...at("link"), url: "https://" };
        break;
      case "file":
        spec = { type: "file", ...at("file"), file: tool.file ?? "" };
        break;
      case "machine": {
        const name = tool.name ?? t("dg.machine.newName");
        spec = {
          type: "text",
          ...at("machine"),
          color: "5",
          text: `**${name}**\n${tool.imageName ?? ""}`.trim(),
          losos: { machine: { name, ...(tool.image ? { image: tool.image } : {}), ...(tool.order ? { order: tool.order } : {}) } },
        };
        break;
      }
    }
    const r = s.ok(s.core.canvasAddNode(spec));
    if (!r) return null;
    s.refreshCanvas();
    s.ui.cardTool = null;
    this.select({ nodes: [r.node.id], edges: [] });
    if (r.node.type === "text" && tool.kind !== "machine") this.edit("node", r.node.id);
    if (r.node.type === "link") this.edit("node", r.node.id);
    return r.node;
  }

  addDevice(type: string, x: number, y: number, keep: boolean): void {
    const s = this.s;
    const r = s.ok(s.core.canvasAddDevice(type, Math.round(x), Math.round(y)));
    if (!r) return;
    s.refresh();
    s.placePhysical();
    this.select({ nodes: [r.node.id], edges: [] });
    s.setUi({ tab: r.device.type === "laptop" ? "desktop" : "config", ...(keep ? {} : { tool: "select", placeType: null }) });
  }

  update(id: string, patch: NodePatch): void {
    const s = this.s;
    if (!s.ok(s.core.canvasUpdateNode(id, patch))) {
      s.emit("model");
      return;
    }
    s.refreshCanvas();
  }

  updateEdge(id: string, patch: EdgePatch): void {
    const s = this.s;
    if (!s.ok(s.core.canvasUpdateEdge(id, patch))) {
      s.emit("model");
      return;
    }
    s.refreshCanvas();
  }

  /** The style keys of each card in `ids`, merged with `patch` (null drops one). */
  style(ids: string[], patch: Record<string, string | null>): void {
    const s = this.s;
    for (const id of ids) {
      const n = s.canvas.nodes.find((x) => x.id === id);
      if (!n) continue;
      s.core.canvasUpdateNode(id, { styleAttributes: { ...(n.styleAttributes ?? {}), ...patch } });
    }
    s.refreshCanvas();
  }

  edgeStyle(ids: string[], patch: Record<string, string | null>): void {
    const s = this.s;
    for (const id of ids) {
      const e = s.canvas.edges.find((x) => x.id === id);
      if (!e) continue;
      s.core.canvasUpdateEdge(id, { styleAttributes: { ...(e.styleAttributes ?? {}), ...patch } });
    }
    s.refreshCanvas();
  }

  color(sel: CanvasSel, color: string | null): void {
    const s = this.s;
    for (const id of sel.nodes) s.core.canvasUpdateNode(id, { color });
    for (const id of sel.edges) s.core.canvasUpdateEdge(id, { color });
    s.refreshCanvas();
  }

  // ── moving ─────────────────────────────────────────────────────────────
  /** One step of a drag: the cards and what lies inside the groups among them. */
  move(ids: string[], dx: number, dy: number, gesture: number): void {
    const s = this.s;
    if (!dx && !dy) return;
    const r = s.core.canvasMove(ids, dx, dy, gesture);
    if (isErr(r)) {
      s.toast(r.error);
      return;
    }
    s.refreshCanvas();
  }

  /** The drag is over: the world moved with the device cards, so re-read it all. */
  moved(): void {
    this.s.refresh();
  }

  resize(id: string, rect: { x: number; y: number; width: number; height: number }, gesture: number): void {
    const s = this.s;
    if (!s.ok(s.core.canvasResize(id, rect, gesture))) return;
    s.refreshCanvas();
  }

  // ── arrows ─────────────────────────────────────────────────────────────
  /** An arrow from one card to another. Between two devices' cards it is a
   *  cable (of the armed kind, or auto), unless "arrow" is armed. */
  connect(from: string, fromSide: Side, to: string, toSide: Side | undefined): void {
    const s = this.s;
    if (from === to) return;
    const a = s.canvas.nodes.find((n) => n.id === from);
    const b = s.canvas.nodes.find((n) => n.id === to);
    if (!a || !b) return;
    const spec: EdgeSpec = { fromNode: from, toNode: to, fromSide, ...(toSide ? { toSide } : {}) };
    const kind = s.ui.tool === "connect" ? s.ui.linkKind : null;
    if (a.dev && b.dev && kind !== "arrow") spec.cable = kind ?? "auto";
    const r = s.core.canvasConnect(spec);
    if (isErr(r)) {
      s.toast(r.error);
      return;
    }
    if (r.edge.link) {
      s.linkBorn.set(r.edge.link, performance.now());
      window.setTimeout(() => s.emit("model"), 1500);
    }
    s.refresh();
    this.select({ nodes: [], edges: [r.edge.id] });
  }

  /** An arrow dropped on empty canvas: a new text card there, joined to it. */
  connectToNew(from: string, fromSide: Side, x: number, y: number): void {
    const s = this.s;
    const [w, h] = SIZE["text"]!;
    const r = s.ok(s.core.canvasAddNode({ type: "text", x: Math.round(x - w / 2), y: Math.round(y - h / 2), width: w, height: h, text: "" }));
    if (!r) return;
    const e = s.core.canvasConnect({ fromNode: from, fromSide, toNode: r.node.id });
    if (isErr(e)) s.toast(e.error);
    s.refreshCanvas();
    this.select({ nodes: [r.node.id], edges: [] });
    this.edit("node", r.node.id);
  }

  // ── the selection as a whole ───────────────────────────────────────────
  remove(sel: CanvasSel = this.s.ui.csel): void {
    const s = this.s;
    if (sel.nodes.length === 0 && sel.edges.length === 0) return;
    for (const id of sel.nodes) {
      const dev = s.canvas.nodes.find((n) => n.id === id)?.dev;
      if (dev) this.engine.stop(dev);
    }
    if (!s.ok(s.core.canvasDelete(sel))) return;
    s.refresh();
    this.clear();
  }

  group(ids: string[]): void {
    const s = this.s;
    if (ids.length === 0) return;
    const r = s.ok(s.core.canvasGroup(ids, t("dg.card.groupLabel")));
    if (!r) return;
    s.refreshCanvas();
    this.select({ nodes: [r.node.id], edges: [] });
  }

  order(ids: string[], how: "front" | "back"): void {
    const s = this.s;
    if (s.ok(s.core.canvasOrder(ids, how))) s.refreshCanvas();
  }

  align(ids: string[], how: string): void {
    const s = this.s;
    if (s.ok(s.core.canvasAlign(ids, how))) s.refresh();
  }

  copy(ids: string[]): string | null {
    if (ids.length === 0) return null;
    const r = this.s.core.canvasCopy(ids);
    return isErr(r) ? null : r.text;
  }

  /** A canvas fragment, centred on x, y; anything else becomes a card. */
  paste(text: string, x: number, y: number): void {
    const s = this.s;
    const trimmed = text.trim();
    if (!trimmed) return;
    if (trimmed.startsWith("{")) {
      const r = s.core.canvasPaste(trimmed, Math.round(x), Math.round(y));
      if (!isErr(r)) {
        s.refresh();
        s.placePhysical();
        this.select({ nodes: r.nodes, edges: r.edges });
        if (r.skipped) s.toast(t("dg.toast.pasteSkipped", { count: r.skipped }));
        return;
      }
    }
    const url = /^https?:\/\/\S+$/.test(trimmed);
    const [w, h] = url ? SIZE["link"]! : [Math.min(400, Math.max(200, trimmed.length * 7)), Math.min(400, 40 + 22 * trimmed.split("\n").length)];
    const spec: NodeSpec = url
      ? { type: "link", url: trimmed, x: Math.round(x - w / 2), y: Math.round(y - h / 2), width: w, height: h }
      : { type: "text", text: trimmed.slice(0, 20000), x: Math.round(x - w / 2), y: Math.round(y - h / 2), width: w, height: h };
    const r = s.ok(s.core.canvasAddNode(spec));
    if (!r) return;
    s.refreshCanvas();
    this.select({ nodes: [r.node.id], edges: [] });
  }

  duplicate(sel: CanvasSel = this.s.ui.csel): void {
    const text = this.copy(sel.nodes);
    if (!text) return;
    const ns = this.s.canvas.nodes.filter((n) => sel.nodes.includes(n.id));
    const cx = (Math.min(...ns.map((n) => n.x)) + Math.max(...ns.map((n) => n.x + n.width))) / 2;
    const cy = (Math.min(...ns.map((n) => n.y)) + Math.max(...ns.map((n) => n.y + n.height))) / 2;
    this.paste(text, cx + 40, cy + 40);
  }

  template(key: string, x: number, y: number): void {
    const s = this.s;
    const r = s.ok(s.core.canvasInsertTemplate(key, getLocale(), Math.round(x), Math.round(y)));
    if (!r) return;
    s.refresh();
    s.placePhysical();
    s.ui.cardTool = null;
    this.select({ nodes: r.nodes, edges: r.edges });
  }

  undo(): void {
    const s = this.s;
    const r = s.core.canvasUndo();
    if (isErr(r)) return;
    s.refresh();
    s.placePhysical();
    this.stopEditing();
  }

  redo(): void {
    const s = this.s;
    const r = s.core.canvasRedo();
    if (isErr(r)) return;
    s.refresh();
    s.placePhysical();
    this.stopEditing();
  }

  // ── the document ───────────────────────────────────────────────────────
  newDiagram(): void {
    const s = this.s;
    this.engine.stopAll();
    s.core.canvasNew();
    s.openedText = null;
    s.cloudFile = null;
    s.startedWorld(null);
    s.setUi({ view: "canvas" });
    s.fit();
  }

  rename(name: string): boolean {
    const s = this.s;
    const r = s.ok(s.core.renameDoc(name));
    s.emit("model");
    return r !== null;
  }
}
