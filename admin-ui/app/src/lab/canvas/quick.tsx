/* The bar over the selection in LosOS Diagrams (components/quick-actions.tsx
 * draws it): pick cards or arrows and their colour, alignment, grouping, an
 * arrow between two cards, a copy, the bin and the box's AI are above them.
 * Each button is an operation the canvas already has (ops.ts), the same
 * one as its key or the inspector's button, so one undo takes it back.
 *
 * It hides while a drag, a rectangle or an arrow is being drawn and while a
 * card is typed into, and comes back when that ends. Other parts add their
 * actions to "diagram.selection". */

import * as React from "react";
import {
  AlignBottomIcon,
  AlignHorizontalCenterIcon,
  AlignLeftIcon,
  AlignRightIcon,
  AlignTopIcon,
  AlignVerticalCenterIcon,
  ArrowRight02Icon,
  Copy01Icon,
  Delete02Icon,
  DistributeHorizontalCenterIcon,
  DistributeVerticalCenterIcon,
  GroupItemsIcon,
  PaintBoardIcon,
  PencilEdit02Icon,
} from "@hugeicons/core-free-icons";
import { QuickActions } from "@/components/quick-actions";
import { useT } from "@/lib/i18n-react";
import { quickActionsFor, type QuickSection } from "@/lib/quick-actions";
import type { CanvasEdge, CanvasNode, Side } from "../core";
import { useLab } from "../store";
import { colorValue, PRESETS } from "./colors";
import { canvasOps } from "./ops-hook";

declare module "@/lib/quick-actions" {
  interface QuickSubjects {
    /** Cards and arrows picked in a diagram. */
    "diagram.selection": { nodes: readonly CanvasNode[]; edges: readonly CanvasEdge[] };
  }
}

const COLOR_NAMES = ["dg.color.red", "dg.color.orange", "dg.color.yellow", "dg.color.green", "dg.color.cyan", "dg.color.purple"] as const;

/** The sides two cards face each other on, for an arrow between them. */
export function facing(a: CanvasNode, b: CanvasNode): [Side, Side] {
  const dx = b.x + b.width / 2 - (a.x + a.width / 2);
  const dy = b.y + b.height / 2 - (a.y + a.height / 2);
  if (Math.abs(dx) >= Math.abs(dy)) return dx >= 0 ? ["right", "left"] : ["left", "right"];
  return dy >= 0 ? ["bottom", "top"] : ["top", "bottom"];
}

/** What the AI reads of a card. */
function cardText(n: CanvasNode, device: (id: string) => string | undefined): string {
  if (n.dev !== undefined) return device(n.dev) ?? "";
  return [n.label, n.text, n.url, n.file].filter((x): x is string => typeof x === "string" && x.trim() !== "").join("\n");
}

export function DiagramQuickActions({ svg, busy }: { svg: React.RefObject<SVGSVGElement | null>; busy: boolean }) {
  const s = useLab();
  const t = useT();
  const [dismissed, setDismissed] = React.useState<string | null>(null);
  const ops = canvasOps();
  const cam = s.cams.canvas;
  const csel = s.ui.csel;
  const key = `${csel.nodes.join(",")}|${csel.edges.join(",")}`;

  React.useEffect(() => {
    if (dismissed !== null && dismissed !== key) setDismissed(null);
  }, [dismissed, key]);

  const byId = new Map(s.canvas.nodes.map((n) => [n.id, n]));
  const nodes = csel.nodes.map((id) => byId.get(id)).filter((n): n is CanvasNode => n !== undefined);
  const edges = s.canvas.edges.filter((e) => csel.edges.includes(e.id));
  const cards = nodes.filter((n) => n.type !== "group");
  const idle = !busy && s.ui.editing === null && s.ui.cardTool === null && s.ui.tool === "select";

  // The selection's box on screen: the cards, and both ends of each arrow.
  let anchor: DOMRect | null = null;
  const el = svg.current;
  if (idle && el !== null && (nodes.length > 0 || edges.length > 0) && dismissed !== key) {
    let x1 = Infinity;
    let y1 = Infinity;
    let x2 = -Infinity;
    let y2 = -Infinity;
    const take = (n: CanvasNode): void => {
      x1 = Math.min(x1, n.x);
      y1 = Math.min(y1, n.y);
      x2 = Math.max(x2, n.x + n.width);
      y2 = Math.max(y2, n.y + n.height);
    };
    nodes.forEach(take);
    for (const e of edges) {
      const a = byId.get(e.fromNode);
      const b = byId.get(e.toNode);
      if (a !== undefined && b !== undefined && nodes.length === 0) {
        // An arrow alone: the bar sits over its middle, not over both cards.
        const mx = (a.x + a.width / 2 + b.x + b.width / 2) / 2;
        const my = (a.y + a.height / 2 + b.y + b.height / 2) / 2;
        x1 = Math.min(x1, mx);
        y1 = Math.min(y1, my);
        x2 = Math.max(x2, mx);
        y2 = Math.max(y2, my);
      } else {
        if (a !== undefined) take(a);
        if (b !== undefined) take(b);
      }
    }
    const r = el.getBoundingClientRect();
    const left = r.left + cam.x + x1 * cam.k;
    const top = r.top + cam.y + y1 * cam.k;
    const right = r.left + cam.x + x2 * cam.k;
    const bottom = r.top + cam.y + y2 * cam.k;
    // Off the canvas, the bar goes with it.
    if (Number.isFinite(left) && right > r.left && left < r.right && bottom > r.top && top < r.bottom) {
      anchor = DOMRect.fromRect({ x: Math.max(left, r.left), y: Math.max(top, r.top), width: Math.min(right, r.right) - Math.max(left, r.left), height: Math.min(bottom, r.bottom) - Math.max(top, r.top) });
    }
  }

  const ids = cards.map((n) => n.id).concat(nodes.filter((n) => n.type === "group").map((n) => n.id));
  const colors = new Set([...nodes.map((n) => n.color ?? ""), ...edges.map((e) => e.color ?? "")]);
  const color = colors.size === 1 ? [...colors][0]! : "mixed";
  const one = nodes.length === 1 && edges.length === 0 ? nodes[0] : undefined;
  const pair = cards.length === 2 && nodes.length === 2 && edges.length === 0 ? cards : null;

  const align = (how: string, label: string, icon: typeof AlignLeftIcon) => ({ id: `align-${how}`, label, icon, onSelect: () => ops.align(ids, how) });

  const sections: QuickSection[] = [
    [
      {
        kind: "choice",
        id: "color",
        label: t("quick.dg.color"),
        icon: PaintBoardIcon,
        value: color,
        options: [
          { value: "", label: t("dg.color.none"), swatch: undefined },
          ...PRESETS.map((p, i) => ({ value: p, label: t(COLOR_NAMES[i]!), swatch: colorValue(p) })),
        ],
        onChange: (value) => ops.color(csel, value === "" ? null : value),
      },
      {
        id: "edit",
        label: t("quick.dg.edit"),
        icon: PencilEdit02Icon,
        keys: ["Enter"],
        hidden: one === undefined || !(one.type === "text" || one.type === "group") || one.dev !== undefined,
        onSelect: () => one !== undefined && ops.edit("node", one.id),
      },
    ],
    [
      {
        kind: "menu",
        id: "align",
        label: t("quick.dg.align"),
        icon: AlignLeftIcon,
        hidden: ids.length < 2,
        items: [
          align("left", t("dg.align.lefts"), AlignLeftIcon),
          align("center", t("dg.align.centres"), AlignHorizontalCenterIcon),
          align("right", t("dg.align.rights"), AlignRightIcon),
          align("top", t("dg.align.tops"), AlignTopIcon),
          align("middle", t("dg.align.middles"), AlignVerticalCenterIcon),
          align("bottom", t("dg.align.bottoms"), AlignBottomIcon),
          ...(ids.length > 2
            ? [
                align("horizontal", t("dg.align.spreadH"), DistributeHorizontalCenterIcon),
                align("vertical", t("dg.align.spreadV"), DistributeVerticalCenterIcon),
              ]
            : []),
        ],
      },
      {
        id: "connect",
        label: t("quick.dg.connect"),
        icon: ArrowRight02Icon,
        hidden: pair === null,
        onSelect: () => {
          if (pair === null) return;
          const [from, to] = facing(pair[0]!, pair[1]!);
          ops.connect(pair[0]!.id, from, pair[1]!.id, to);
        },
      },
      {
        id: "group",
        label: t("quick.dg.group"),
        icon: GroupItemsIcon,
        keys: ["Mod", "G"],
        hidden: ids.length === 0 || (one !== undefined && one.type === "group"),
        onSelect: () => ops.group(ids),
      },
      {
        id: "duplicate",
        label: t("quick.dg.duplicate"),
        icon: Copy01Icon,
        keys: ["Mod", "D"],
        hidden: nodes.length === 0,
        onSelect: () => ops.duplicate(),
      },
    ],
    quickActionsFor("diagram.selection", { nodes, edges }),
    [
      {
        id: "delete",
        label: t("quick.dg.delete"),
        icon: Delete02Icon,
        keys: ["Delete"],
        danger: true,
        onSelect: () => ops.remove(csel),
      },
    ],
  ];

  const device = (id: string): string | undefined => {
    const d = s.scene.dev(id);
    return d === undefined ? undefined : `${d.name} (${d.type})`;
  };

  return (
    <QuickActions
      anchor={anchor}
      label={t("quick.diagram")}
      sections={sections}
      ask={
        nodes.some((n) => cardText(n, device) !== "")
          ? {
              context: "diagrams",
              sources: () => [
                {
                  title: s.cloudFile?.path.split("/").pop() ?? t("quick.diagram"),
                  text: nodes
                    .map((n) => cardText(n, device))
                    .filter((x) => x !== "")
                    .join("\n\n---\n\n"),
                },
              ],
            }
          : undefined
      }
      onReturn={() => svg.current?.focus({ preventScroll: true })}
      onDismiss={() => setDismissed(key)}
    />
  );
}
