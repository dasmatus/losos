/* The tray's diagram half: cards and shapes, templates, and machines on the
 * mesh. Each one arms the canvas; the next click puts it down there (Esc
 * disarms). The network categories are the Lab's own (tray.tsx). */

import { useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { StatusDot } from "@/components/ui/badge";
import type { CardTool } from "../store";
import { useLab } from "../store";
import { replicaTone } from "./machines";
import { canvasOps } from "./ops-hook";
import { outline } from "./shapes";

export const trayItem = cn(
  "flex w-[104px] flex-none flex-col items-center gap-0.5 rounded-card border border-line bg-surface px-1.5 pt-1 pb-1.5",
  "text-center text-[11.5px] leading-tight text-ink transition-colors hover:border-accent",
  "aria-pressed:border-accent aria-pressed:bg-accent-wash aria-pressed:shadow-[inset_0_0_0_1px_var(--accent)]",
  "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
);

const CARDS: { key: string; tool: CardTool; label: MessageKey }[] = [
  { key: "text", tool: { kind: "text" }, label: "dg.tray.text" },
  { key: "note", tool: { kind: "text", shape: "note" }, label: "dg.shape.note" },
  { key: "group", tool: { kind: "group" }, label: "dg.tray.group" },
  { key: "link", tool: { kind: "link" }, label: "dg.tray.link" },
  { key: "pill", tool: { kind: "text", shape: "pill" }, label: "dg.shape.pill" },
  { key: "diamond", tool: { kind: "text", shape: "diamond" }, label: "dg.shape.diamond" },
  { key: "circle", tool: { kind: "text", shape: "circle" }, label: "dg.shape.circle" },
  { key: "parallelogram", tool: { kind: "text", shape: "parallelogram" }, label: "dg.shape.parallelogram" },
  { key: "hexagon", tool: { kind: "text", shape: "hexagon" }, label: "dg.shape.hexagon" },
  { key: "database", tool: { kind: "text", shape: "database" }, label: "dg.shape.database" },
  { key: "document", tool: { kind: "text", shape: "document" }, label: "dg.shape.document" },
  { key: "predefined-process", tool: { kind: "text", shape: "predefined-process" }, label: "dg.shape.process" },
  { key: "cloud", tool: { kind: "text", shape: "cloud" }, label: "dg.shape.cloud" },
];

const TEMPLATES: { key: string; label: MessageKey; hint: MessageKey }[] = [
  { key: "flowchart", label: "dg.template.flowchart", hint: "dg.template.flowchartHint" },
  { key: "architecture", label: "dg.template.architecture", hint: "dg.template.architectureHint" },
  { key: "mindmap", label: "dg.template.mindmap", hint: "dg.template.mindmapHint" },
  { key: "board", label: "dg.template.board", hint: "dg.template.boardHint" },
  { key: "network", label: "dg.template.network", hint: "dg.template.networkHint" },
];

function same(a: CardTool | null, b: CardTool): boolean {
  return !!a && JSON.stringify(a) === JSON.stringify(b);
}

function Preview({ k }: { k: string }) {
  const w = 52;
  const h = k === "text" || k === "link" ? 26 : k === "group" ? 34 : 32;
  const o = outline(k === "text" || k === "group" || k === "link" ? undefined : k, w, h);
  return (
    <svg viewBox={`-2 -2 ${w + 4} ${h + 4}`} className="h-[34px] w-[56px]" aria-hidden="true">
      <path d={o.d} className={cn("dg-tray-shape", k === "group" && "group", k === "note" && "note")} />
      {o.inner && <path d={o.inner} className="dg-tray-shape inner" />}
      {k === "text" && <path d="M10 10h28M10 17h20" className="dg-tray-ink" />}
      {k === "link" && <path d="M21 15a4 4 0 0 0 5 0l3-3a4 4 0 0 0-5-5l-1 1M27 11a4 4 0 0 0-5 0l-3 3a4 4 0 0 0 5 5l1-1" className="dg-tray-ink" />}
    </svg>
  );
}

export function CardItems() {
  const s = useLab();
  const t = useT();
  return (
    <>
      {CARDS.map((c) => (
        <button
          key={c.key}
          type="button"
          className={trayItem}
          aria-pressed={same(s.ui.cardTool, c.tool)}
          data-card={c.key}
          onClick={() => canvasOps().arm(same(s.ui.cardTool, c.tool) ? null : c.tool)}
        >
          <Preview k={c.key} />
          <span>{t(c.label)}</span>
        </button>
      ))}
    </>
  );
}

export function TemplateItems() {
  const s = useLab();
  const t = useT();
  return (
    <>
      {TEMPLATES.map((c) => {
        const tool: CardTool = { kind: "template", key: c.key };
        return (
          <button
            key={c.key}
            type="button"
            className={cn(trayItem, "w-[140px]")}
            aria-pressed={same(s.ui.cardTool, tool)}
            data-template={c.key}
            onClick={() => canvasOps().arm(same(s.ui.cardTool, tool) ? null : tool)}
          >
            <span className="mt-1 font-semibold">{t(c.label)}</span>
            <small className="text-[10.5px] text-muted [text-wrap:balance]">{t(c.hint)}</small>
          </button>
        );
      })}
    </>
  );
}

export function MachineItems() {
  const s = useLab();
  const t = useT();
  const m = s.machines;
  if (m.state !== "ready") {
    const why: MessageKey =
      m.state === "loading"
        ? "dg.machine.loading"
        : m.state === "unavailable"
          ? m.reason === "notSharing"
            ? "dg.machine.notSharing"
            : m.reason === "noOfficialEdge"
              ? "dg.machine.noOfficialEdge"
              : m.reason === "signedOut"
                ? "dg.machine.signedOut"
                : "dg.machine.notOffered"
          : m.state === "failed"
            ? "dg.machine.failed"
            : "dg.machine.hosted";
    return (
      <p className="flex max-w-[560px] items-center self-center px-1 text-[12.5px] text-muted" data-testid="machines-unavailable">
        {t(why)}
        {m.state === "unavailable" && (
          <a href="/machines" className="ml-1.5 text-accent hover:underline">
            {t("dg.machine.openPane")}
          </a>
        )}
      </p>
    );
  }
  return (
    <>
      {m.machines.map((x) => {
        const tool: CardTool = { kind: "machine", order: x.order, name: x.name, image: x.image, imageName: x.imageName };
        const tone = x.replicas[0] ? replicaTone(x.replicas[0].status) : "idle";
        return (
          <button
            key={x.order}
            type="button"
            className={cn(trayItem, "w-[140px]")}
            aria-pressed={same(s.ui.cardTool, tool)}
            data-machine={x.order}
            onClick={() => canvasOps().arm(same(s.ui.cardTool, tool) ? null : tool)}
          >
            <span className="mt-1 inline-flex items-center gap-1.5 font-semibold">
              <StatusDot state={tone} />
              {x.name}
            </span>
            <small className="text-[10.5px] text-muted">{x.imageName}</small>
          </button>
        );
      })}
      {m.images.map((i) => {
        const tool: CardTool = { kind: "machine", image: i.id, imageName: i.name, name: i.name };
        return (
          <button
            key={i.id}
            type="button"
            className={cn(trayItem, "w-[140px] border-dashed")}
            aria-pressed={same(s.ui.cardTool, tool)}
            data-machine-image={i.id}
            onClick={() => canvasOps().arm(same(s.ui.cardTool, tool) ? null : tool)}
          >
            <span className="mt-1 font-semibold">{i.name}</span>
            <small className="text-[10.5px] text-muted">{t("dg.machine.plan")}</small>
          </button>
        );
      })}
      {m.machines.length === 0 && m.images.length === 0 && <p className="self-center px-1 text-[12.5px] text-muted">{t("dg.machine.none")}</p>}
    </>
  );
}
