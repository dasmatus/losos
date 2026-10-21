/* The inspector on the diagram: the document with nothing selected, the
 * look of the selected cards and arrows (colour, shape, border, words'
 * alignment; an arrow's route, line, ends and label), arranging them, a
 * machine's state with the way to the Machines pane, and for one device's
 * card or one cable the Lab's own panes (inspector.tsx) under the colour. */

import * as React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { StatusDot } from "@/components/ui/badge";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import type { CanvasEdge, CanvasNode } from "../core";
import { Inspector } from "../inspector";
import { type LabStore, useLab } from "../store";
import { PRESETS, colorValue } from "./colors";
import { machineOf, replicaTone } from "./machines";
import { canvasOps } from "./ops-hook";
import { SHAPES } from "./shapes";

function Sect({ children }: { children: React.ReactNode }) {
  return <div className="mt-4 mb-1.5 text-[11px] font-semibold tracking-[0.07em] text-muted uppercase first:mt-0">{children}</div>;
}

function Choice({ label, value, options, onChange, testid }: { label: string; value: string; options: [string, string][]; onChange: (v: string) => void; testid?: string }) {
  const id = React.useId();
  return (
    <div className="mb-2 grid grid-cols-[96px_minmax(0,1fr)] items-center gap-2 text-[13px]">
      <label htmlFor={id} className="text-muted">
        {label}
      </label>
      <NativeSelect id={id} value={value} onChange={(e) => onChange(e.target.value)} data-testid={testid}>
        {options.map(([v, l]) => (
          <NativeSelectOption key={v} value={v}>
            {l}
          </NativeSelectOption>
        ))}
      </NativeSelect>
    </div>
  );
}

/** A text field that commits when it loses focus or on Enter. */
function Field({ label, value, onCommit, testid, placeholder }: { label: string; value: string; onCommit: (v: string) => void; testid?: string; placeholder?: string }) {
  const id = React.useId();
  const [v, setV] = React.useState(value);
  React.useEffect(() => setV(value), [value]);
  return (
    <div className="mb-2 grid grid-cols-[96px_minmax(0,1fr)] items-center gap-2 text-[13px]">
      <label htmlFor={id} className="text-muted">
        {label}
      </label>
      <Input
        id={id}
        value={v}
        placeholder={placeholder}
        data-testid={testid}
        onChange={(e) => setV(e.target.value)}
        onBlur={() => v !== value && onCommit(v)}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
      />
    </div>
  );
}

const COLOR_NAME: Record<string, MessageKey> = {
  "1": "dg.color.red",
  "2": "dg.color.orange",
  "3": "dg.color.yellow",
  "4": "dg.color.green",
  "5": "dg.color.cyan",
  "6": "dg.color.purple",
};

function Colors({ value, onPick }: { value: string | undefined; onPick: (c: string | null) => void }) {
  const t = useT();
  const custom = value && !(PRESETS as readonly string[]).includes(value) ? value : "";
  return (
    <div className="mb-2 flex flex-wrap items-center gap-1.5" role="radiogroup" aria-label={t("dg.style.color")}>
      <button
        type="button"
        role="radio"
        aria-checked={!value}
        aria-label={t("dg.color.none")}
        title={t("dg.color.none")}
        className="dg-swatch none"
        onClick={() => onPick(null)}
      />
      {PRESETS.map((p) => (
        <button
          key={p}
          type="button"
          role="radio"
          aria-checked={value === p}
          aria-label={t(COLOR_NAME[p]!)}
          title={t(COLOR_NAME[p]!)}
          className="dg-swatch"
          style={{ "--dg-card": colorValue(p) } as React.CSSProperties}
          onClick={() => onPick(p)}
          data-color={p}
        />
      ))}
      <label className="relative inline-flex" title={t("dg.color.custom")}>
        <span className="sr-only">{t("dg.color.custom")}</span>
        <input
          type="color"
          className="dg-swatch custom"
          value={custom || "#888888"}
          aria-checked={!!custom}
          onChange={(e) => onPick(e.target.value)}
        />
      </label>
    </div>
  );
}

function DocumentPanel({ s }: { s: LabStore }) {
  const t = useT();
  const doc = s.core.exportSetup();
  const cards = s.canvas.nodes.filter((n) => n.type !== "group").length;
  return (
    <div className="overflow-auto px-3.5 py-4 text-[13px]" data-testid="inspector-diagram">
      <Sect>{t("dg.doc.title")}</Sect>
      <Field label={t("dg.doc.name")} value={doc.name} onCommit={(v) => canvasOps().rename(v)} testid="diagram-name" />
      <p className="mb-2 text-muted">
        {t("dg.doc.counts", { cards, arrows: s.canvas.edges.length, devices: s.snap.world.devices.length })}
      </p>
      {s.cloudFile && (
        <p className="mb-2 text-muted">
          {t("dg.doc.inCloud")} <code className="font-mono text-[12px] text-ink">{s.cloudFile.path}</code>
        </p>
      )}
      <Sect>{t("dg.doc.how")}</Sect>
      <ul className="m-0 list-disc space-y-1 pl-[18px] text-muted">
        <li>{t("dg.help.add")}</li>
        <li>{t("dg.help.connect")}</li>
        <li>{t("dg.help.select")}</li>
        <li>{t("dg.help.move")}</li>
        <li>{t("dg.help.network")}</li>
        <li>{t("dg.help.keys")}</li>
        <li>{t("dg.help.obsidian")}</li>
      </ul>
    </div>
  );
}

function MachinePanel({ s, n }: { s: LabStore; n: CanvasNode }) {
  const t = useT();
  const m = machineOf(n.losos)!;
  const ready = s.machines.state === "ready" ? s.machines : null;
  const info = ready && m.order ? ready.machines.find((x) => x.order === m.order) : undefined;
  const bind = (order: string) => {
    const x = ready?.machines.find((y) => y.order === order);
    if (!x) return;
    canvasOps().update(n.id, { losos: { ...(n.losos ?? {}), machine: { order: x.order, name: x.name, image: x.image } }, text: `**${x.name}**\n${x.imageName}` });
  };
  const orderHref = `/machines?${new URLSearchParams({ ...(m.image ? { image: m.image } : {}), ...(m.name ? { name: m.name } : {}) }).toString()}`;
  return (
    <>
      <Sect>{t("dg.machine.title")}</Sect>
      {info ? (
        <>
          <p className="mb-1.5 text-[14px] font-semibold">{info.name}</p>
          <p className="mb-2 text-muted">{info.imageName}</p>
          {info.replicas.length > 0 ? (
            <ul className="mb-2 flex flex-col gap-1" aria-label={t("dg.machine.replicas")} data-testid="machine-replicas">
              {info.replicas.map((r) => (
                <li key={r.name} className="flex items-center gap-2">
                  <StatusDot state={replicaTone(r.status)} />
                  <span className="font-mono text-[12px]">{r.name}</span>
                  <span className="text-muted">{r.status}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="mb-2 text-muted">{info.state === "pending" ? t("dg.machine.pending") : info.state === "lapsed" ? t("dg.machine.lapsed") : t("dg.machine.preparing")}</p>
          )}
          {info.address && (
            <p className="mb-2">
              <a href={info.address} target="_blank" rel="noreferrer" className="text-accent hover:underline">
                {info.address}
              </a>
            </p>
          )}
          <a href="/machines" className="text-accent hover:underline" data-testid="machine-open">
            {t("dg.machine.openPane")}
          </a>
        </>
      ) : (
        <>
          <p className="mb-2 text-muted">{m.order ? t("dg.machine.gone") : t("dg.machine.plannedLong")}</p>
          {ready && ready.machines.length > 0 && (
            <Choice
              label={t("dg.machine.bind")}
              value=""
              options={[["", t("dg.machine.bindPick")], ...ready.machines.map((x): [string, string] => [x.order, x.name])]}
              onChange={(v) => v && bind(v)}
            />
          )}
          {ready && (
            <a href={orderHref} className="text-accent hover:underline" data-testid="machine-order">
              {t("dg.machine.order")}
            </a>
          )}
          {!ready && <p className="text-muted">{t("dg.machine.cannotCheck")}</p>}
        </>
      )}
    </>
  );
}

function NodeStyle({ s, ns }: { s: LabStore; ns: CanvasNode[] }) {
  const t = useT();
  const one = ns.length === 1 ? ns[0]! : undefined;
  const texts = ns.filter((n) => n.type === "text" && !n.dev && !machineOf(n.losos));
  const plainCards = ns.filter((n) => !n.dev);
  const shapeOf = (n: CanvasNode) => n.styleAttributes?.["shape"] ?? "";
  const common = (f: (n: CanvasNode) => string, list: CanvasNode[]) => (list.length && list.every((n) => f(n) === f(list[0]!)) ? f(list[0]!) : "");
  const ids = ns.map((n) => n.id);
  const ops = canvasOps();
  const shapeName: Record<string, MessageKey> = {
    "": "dg.shape.card",
    pill: "dg.shape.pill",
    diamond: "dg.shape.diamond",
    parallelogram: "dg.shape.parallelogram",
    circle: "dg.shape.circle",
    "predefined-process": "dg.shape.process",
    document: "dg.shape.document",
    database: "dg.shape.database",
    hexagon: "dg.shape.hexagon",
    cloud: "dg.shape.cloud",
    note: "dg.shape.note",
  };
  return (
    <>
      <Sect>{t("dg.style.color")}</Sect>
      <Colors value={common((n) => n.color ?? "", ns) || undefined} onPick={(c) => ops.color({ nodes: ids, edges: [] }, c)} />
      {texts.length > 0 && (
        <Choice
          label={t("dg.style.shape")}
          value={common(shapeOf, texts)}
          options={(["", ...SHAPES] as string[]).map((k): [string, string] => [k, t(shapeName[k]!)])}
          onChange={(v) => ops.style(texts.map((n) => n.id), { shape: v || null })}
          testid="style-shape"
        />
      )}
      {plainCards.length > 0 && (
        <Choice
          label={t("dg.style.border")}
          value={common((n) => n.styleAttributes?.["border"] ?? "", plainCards)}
          options={[
            ["", t("dg.border.solid")],
            ["dashed", t("dg.border.dashed")],
            ["dotted", t("dg.border.dotted")],
            ["invisible", t("dg.border.invisible")],
          ]}
          onChange={(v) => ops.style(plainCards.map((n) => n.id), { border: v || null })}
        />
      )}
      {texts.length > 0 && (
        <Choice
          label={t("dg.style.align")}
          value={common((n) => n.styleAttributes?.["textAlign"] ?? "", texts)}
          options={[
            ["", t("dg.align.left")],
            ["center", t("dg.align.center")],
            ["right", t("dg.align.right")],
          ]}
          onChange={(v) => ops.style(texts.map((n) => n.id), { textAlign: v || null })}
        />
      )}
      {one?.type === "group" && <Field label={t("dg.style.label")} value={one.label ?? ""} onCommit={(v) => ops.update(one.id, { label: v })} />}
      {one?.type === "link" && <Field label={t("dg.style.url")} value={one.url ?? ""} onCommit={(v) => ops.update(one.id, { url: v.trim() })} />}
      {one?.type === "file" && <Field label={t("dg.style.file")} value={one.file ?? ""} onCommit={(v) => ops.update(one.id, { file: v.trim() })} />}
      {one?.type === "text" && !one.dev && !machineOf(one.losos) && (
        <Button variant="secondary" size="sm" className="mt-1" onClick={() => ops.edit("node", one.id)}>
          {t("dg.style.editText")}
        </Button>
      )}
      {one && machineOf(one.losos) && <MachinePanel s={s} n={one} />}
    </>
  );
}

function EdgeStyle({ es }: { es: CanvasEdge[] }) {
  const t = useT();
  const one = es.length === 1 ? es[0]! : undefined;
  const ids = es.map((e) => e.id);
  const ops = canvasOps();
  const common = (f: (e: CanvasEdge) => string) => (es.every((e) => f(e) === f(es[0]!)) ? f(es[0]!) : "");
  const ends = common((e) => `${e.fromEnd ?? "none"}-${e.toEnd ?? "arrow"}`);
  return (
    <>
      <Sect>{t("dg.style.arrow")}</Sect>
      <Colors value={common((e) => e.color ?? "") || undefined} onPick={(c) => ops.color({ nodes: [], edges: ids }, c)} />
      <Choice
        label={t("dg.style.route")}
        value={common((e) => e.styleAttributes?.["pathfindingMethod"] ?? "")}
        options={[
          ["", t("dg.route.curve")],
          ["direct", t("dg.route.straight")],
          ["square", t("dg.route.square")],
        ]}
        onChange={(v) => ops.edgeStyle(ids, { pathfindingMethod: v || null })}
        testid="edge-route"
      />
      <Choice
        label={t("dg.style.line")}
        value={common((e) => e.styleAttributes?.["path"] ?? "")}
        options={[
          ["", t("dg.line.solid")],
          ["dotted", t("dg.line.dotted")],
          ["short-dashed", t("dg.line.shortDashed")],
          ["long-dashed", t("dg.line.longDashed")],
        ]}
        onChange={(v) => ops.edgeStyle(ids, { path: v || null })}
      />
      <Choice
        label={t("dg.style.ends")}
        value={ends}
        options={[
          ["none-arrow", t("dg.ends.to")],
          ["arrow-none", t("dg.ends.from")],
          ["arrow-arrow", t("dg.ends.both")],
          ["none-none", t("dg.ends.none")],
        ]}
        onChange={(v) => {
          const [fromEnd, toEnd] = v.split("-");
          for (const id of ids) ops.updateEdge(id, { fromEnd, toEnd });
        }}
        testid="edge-ends"
      />
      <Choice
        label={t("dg.style.head")}
        value={common((e) => e.styleAttributes?.["arrow"] ?? "")}
        options={[
          ["", t("dg.head.triangle")],
          ["triangle-outline", t("dg.head.triangleOutline")],
          ["thin-triangle", t("dg.head.thin")],
          ["halved-triangle", t("dg.head.halved")],
          ["diamond", t("dg.head.diamond")],
          ["diamond-outline", t("dg.head.diamondOutline")],
          ["circle", t("dg.head.circle")],
          ["circle-outline", t("dg.head.circleOutline")],
          ["blunt", t("dg.head.blunt")],
        ]}
        onChange={(v) => ops.edgeStyle(ids, { arrow: v || null })}
      />
      {one && <Field label={t("dg.style.label")} value={one.label ?? ""} onCommit={(v) => ops.updateEdge(one.id, { label: v })} testid="edge-label" />}
    </>
  );
}

function Arrange({ ns, count }: { ns: CanvasNode[]; count: number }) {
  const t = useT();
  const ops = canvasOps();
  const ids = ns.map((n) => n.id);
  const s = useLab();
  return (
    <>
      <Sect>{t("dg.arrange.title")}</Sect>
      <div className="flex flex-wrap gap-1.5">
        {ids.length > 0 && (
          <>
            <Button variant="secondary" size="xs" onClick={() => ops.order(ids, "front")}>
              {t("dg.arrange.front")}
            </Button>
            <Button variant="secondary" size="xs" onClick={() => ops.order(ids, "back")}>
              {t("dg.arrange.back")}
            </Button>
            <Button variant="secondary" size="xs" onClick={() => ops.group(ids)} data-testid="arrange-group">
              {t("dg.arrange.group")}
            </Button>
            <Button variant="secondary" size="xs" onClick={() => ops.duplicate()}>
              {t("dg.arrange.duplicate")}
            </Button>
          </>
        )}
        <Button variant="secondary" size="xs" className="text-crit" onClick={() => ops.remove(s.ui.csel)} data-testid="arrange-delete">
          {count > 1 ? t("dg.arrange.deleteMany", { count }) : t("dg.arrange.delete")}
        </Button>
      </div>
      {ids.length > 1 && (
        <div className="mt-1.5 flex flex-wrap gap-1.5" aria-label={t("dg.arrange.align")}>
          {(
            [
              ["left", "dg.align.lefts"],
              ["center", "dg.align.centres"],
              ["right", "dg.align.rights"],
              ["top", "dg.align.tops"],
              ["middle", "dg.align.middles"],
              ["bottom", "dg.align.bottoms"],
              ["horizontal", "dg.align.spreadH"],
              ["vertical", "dg.align.spreadV"],
            ] as [string, MessageKey][]
          ).map(([how, k]) => (
            <Button key={how} variant="ghost" size="xs" onClick={() => ops.align(ids, how)}>
              {t(k)}
            </Button>
          ))}
        </div>
      )}
    </>
  );
}

export function DiagramInspector() {
  const s = useLab();
  const t = useT();
  const { csel } = s.ui;
  const ns = s.canvas.nodes.filter((n) => csel.nodes.includes(n.id));
  const es = s.canvas.edges.filter((e) => csel.edges.includes(e.id));
  if (ns.length === 0 && es.length === 0) return <DocumentPanel s={s} />;
  // One device's card or one cable: the Lab's panes, with the card's colour above.
  if (s.ui.sel) {
    const card = ns[0] ?? es[0];
    return (
      <div className="flex min-h-0 flex-1 flex-col">
        <div className="border-b border-hair px-3.5 pt-3 pb-1 text-[13px]">
          <Colors
            value={card?.color}
            onPick={(c) => canvasOps().color({ nodes: ns.map((n) => n.id), edges: es.map((e) => e.id) }, c)}
          />
        </div>
        <div className="flex min-h-0 flex-1 flex-col overflow-auto">
          <Inspector />
        </div>
      </div>
    );
  }
  const count = ns.length + es.length;
  return (
    <div className={cn("overflow-auto px-3.5 py-4 text-[13px]")} data-testid="inspector-selection">
      <h2 className="m-0 mb-2 text-[15px] font-semibold">
        {count === 1 ? t(ns[0] ? (ns[0].type === "group" ? "dg.sel.group" : "dg.sel.card") : "dg.sel.arrow") : t("dg.sel.many", { count })}
      </h2>
      {ns.length > 0 && <NodeStyle s={s} ns={ns} />}
      {es.length > 0 && <EdgeStyle es={es} />}
      <Arrange ns={ns} count={count} />
    </div>
  );
}
